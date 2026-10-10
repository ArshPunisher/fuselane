//! A proxy per network (STEPS 8.4): saved with the network's other settings,
//! checked on request, and handed to the core, whose connection planner every
//! download, preview and page read goes through.
//!
//! The password goes to the system's keychain where there is one (`secrets`,
//! SECURITY.md T16), else into the network settings on this computer. It never
//! goes back to the window: the window learns only that one is saved, and where.
//! Torrents don't use these proxies; they connect to peers directly (ADR 0006).

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use fuselane_core::proxy::{Login, NetProxy, Proxy, ProxyError, ProxyKind};
use serde::{Deserialize, Serialize};

use super::secrets::{self, PasswordHome};
use super::{NetPref, Service, UiError, lock, valid_device};

/// Which protocol the proxy speaks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    /// An HTTP proxy, asked with CONNECT for a tunnel.
    Http,
    Socks5,
}

/// A network's proxy as saved. What the window gets has no password, only
/// `has_password`.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPref {
    pub kind: ProxyType,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    /// Held in memory; on disk only while there's no keychain. Never sent
    /// to the window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// A password is saved (for the window, which never sees it).
    #[serde(default)]
    pub has_password: bool,
    /// Where the password is kept, so the window and diagnostics can say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_in: Option<PasswordHome>,
}

impl std::fmt::Debug for ProxyPref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyPref")
            .field("kind", &self.kind)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("login", &self.username.as_ref().map(|_| "hidden"))
            .finish()
    }
}

/// What the window sends to set a proxy. A missing password keeps the saved
/// one; an empty one removes it.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRequest {
    pub kind: ProxyType,
    pub host: String,
    /// Wider than a port so a typo gets a clear message, not a parse error.
    pub port: u32,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

impl std::fmt::Debug for ProxyRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyRequest")
            .field("kind", &self.kind)
            .field("host", &self.host)
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

fn bad(message: &str, hint: &str) -> UiError {
    UiError::new("bad-proxy", message, Some(hint))
}

/// A proxy's name or address, cleaned: lowercase name, or an IP without brackets.
fn clean_host(raw: &str) -> Result<String, UiError> {
    let h = raw.trim();
    if h.is_empty() {
        return Err(bad(
            "Enter the proxy's name or address.",
            "For example proxy.example.com or 192.168.1.10.",
        ));
    }
    if h.contains("://") {
        return Err(bad(
            "Enter just the proxy's name or address.",
            "Leave out http:// or socks5://; choose the type above instead.",
        ));
    }
    let bare = h
        .strip_prefix('[')
        .and_then(|x| x.strip_suffix(']'))
        .unwrap_or(h);
    if let Ok(ip) = bare.parse::<IpAddr>() {
        if ip.is_unspecified() || ip.is_multicast() {
            return Err(bad(
                "That address can't be a proxy.",
                "Use the address of the computer that runs the proxy.",
            ));
        }
        return Ok(ip.to_string());
    }
    if h.contains(':') {
        return Err(bad(
            "Put the port in the Port box, not after the name.",
            "For example proxy.example.com, and 8080 as the port.",
        ));
    }
    let label_ok = |l: &str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    if h.len() > 253 || !h.trim_end_matches('.').split('.').all(label_ok) {
        return Err(bad(
            "That isn't a valid proxy name.",
            "Use letters, numbers, dots and dashes, like proxy.example.com.",
        ));
    }
    Ok(h.trim_end_matches('.').to_ascii_lowercase())
}

/// Checks a proxy from the window (L-97), keeping `saved`'s password when the
/// window sent none.
pub(super) fn validated(
    req: ProxyRequest,
    saved: Option<&ProxyPref>,
) -> Result<ProxyPref, UiError> {
    let host = clean_host(&req.host)?;
    let port = u16::try_from(req.port)
        .ok()
        .filter(|p| *p > 0)
        .ok_or_else(|| {
            bad(
                "That port isn't valid.",
                "Ports go from 1 to 65535; proxies often use 8080, 3128 or 1080.",
            )
        })?;
    let username = req
        .username
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    if let Some(u) = &username {
        if u.len() > 255 || u.chars().any(char::is_control) {
            return Err(bad(
                "That username isn't valid.",
                "Use up to 255 characters, without control characters.",
            ));
        }
        if req.kind == ProxyType::Http && u.contains(':') {
            return Err(bad(
                "An HTTP proxy's username can't contain a colon.",
                "Check the username your proxy gave you.",
            ));
        }
    }
    let (password, password_in) = match req.password {
        // Nothing typed: keep the saved password, while there's a login for it
        // (also one in a keychain that couldn't be read yet).
        None => saved
            .filter(|_| username.is_some())
            .map_or((None, None), |s| (s.password.clone(), s.password_in)),
        Some(p) if p.is_empty() => (None, None),
        Some(p) => (Some(p), None),
    };
    if let Some(p) = &password {
        if username.is_none() {
            return Err(bad(
                "Add the username that goes with the password.",
                "Or clear the password if the proxy doesn't need one.",
            ));
        }
        if p.len() > 255 || p.chars().any(char::is_control) {
            return Err(bad(
                "That password isn't valid.",
                "Use up to 255 characters, without control characters.",
            ));
        }
    }
    Ok(ProxyPref {
        kind: req.kind,
        host,
        port,
        has_password: password.is_some() || password_in == Some(PasswordHome::Keychain),
        password,
        password_in,
        username,
    })
}

/// Whether a proxy read back from disk is still one the window could have set.
/// (Its password may be in the keychain, which needs a username too.)
pub(super) fn saved_ok(p: &ProxyPref) -> bool {
    validated(
        ProxyRequest {
            kind: p.kind,
            host: p.host.clone(),
            port: u32::from(p.port),
            username: p.username.clone(),
            password: Some(p.password.clone().unwrap_or_default()),
        },
        None,
    )
    .is_ok_and(|v| {
        (&v.kind, &v.host, v.port, &v.username, &v.password)
            == (&p.kind, &p.host, p.port, &p.username, &p.password)
    }) && (p.password_in != Some(PasswordHome::Keychain) || p.username.is_some())
}

/// The prefs as the window sees them: passwords replaced by `has_password`
/// (and where it's kept).
pub(super) fn masked(mut prefs: Vec<NetPref>) -> Vec<NetPref> {
    for p in &mut prefs {
        if let Some(x) = &mut p.proxy {
            x.has_password = x.password.is_some() || x.password_in == Some(PasswordHome::Keychain);
            x.password = None;
            if !x.has_password {
                x.password_in = None;
            }
        }
    }
    prefs
}

fn to_core(p: &ProxyPref) -> Proxy {
    Proxy {
        kind: match p.kind {
            ProxyType::Http => ProxyKind::Http,
            ProxyType::Socks5 => ProxyKind::Socks5,
        },
        host: p.host.clone(),
        port: p.port,
        login: p.username.as_ref().map(|u| Login {
            user: u.clone(),
            password: p.password.clone().unwrap_or_default(),
        }),
    }
}

/// A short code per failure, so the window and tests can tell them apart.
fn code(e: &ProxyError) -> &'static str {
    match e {
        ProxyError::Lookup => "proxy-not-found",
        ProxyError::Unreachable(_) => "proxy-unreachable",
        ProxyError::LoginNeeded => "proxy-login-needed",
        ProxyError::LoginRefused => "proxy-login-refused",
        ProxyError::Refused { .. } => "proxy-refused",
        ProxyError::TargetUnreachable { .. } => "proxy-cant-reach",
        ProxyError::NotAProxy { .. } => "proxy-wrong-type",
        ProxyError::Unsupported => "proxy-unsupported",
        ProxyError::Timeout => "proxy-timeout",
    }
}

/// How long a check of a proxy may take.
const CHECK_WAIT: Duration = Duration::from_secs(8);

impl Service {
    /// What to call a network in messages: the person's name for it, else the
    /// system's ("Wi-Fi"), else the device name.
    fn net_label(&self, name: &str, ifaces: &[fuselane_netif::Interface]) -> String {
        lock(&self.net_prefs)
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.label.clone())
            .or_else(|| {
                ifaces
                    .iter()
                    .find(|i| i.name == name)
                    .map(|i| i.display_name.clone())
                    .filter(|d| !d.is_empty())
            })
            .unwrap_or_else(|| name.to_string())
    }

    /// Hands the saved proxies to the core, so every connection uses them.
    /// A password the keychain couldn't give before (locked) is asked for again.
    pub(super) fn apply_proxies(&self) {
        let unread = |p: &NetPref| {
            p.proxy.as_ref().is_some_and(|x| {
                x.password.is_none() && x.password_in == Some(PasswordHome::Keychain)
            })
        };
        // Read outside the lock: a keychain can stop to ask the user.
        let mut waiting: Vec<NetPref> = lock(&self.net_prefs)
            .iter()
            .filter(|p| unread(p))
            .cloned()
            .collect();
        if !waiting.is_empty() {
            secrets::fill(&*self.secrets, &mut waiting);
            for pref in lock(&self.net_prefs).iter_mut() {
                if let Some(read) = waiting.iter().find(|w| w.name == pref.name)
                    && unread(pref)
                    && let (Some(now), Some(got)) = (pref.proxy.as_mut(), read.proxy.as_ref())
                {
                    now.password.clone_from(&got.password);
                    now.password_in = got.password_in;
                    now.has_password = got.has_password;
                }
            }
        }
        let ifaces = fuselane_netif::list().unwrap_or_default();
        let saved: Vec<(String, ProxyPref)> = lock(&self.net_prefs)
            .iter()
            .filter_map(|p| p.proxy.clone().map(|x| (p.name.clone(), x)))
            .collect();
        let table: HashMap<String, NetProxy> = saved
            .into_iter()
            .map(|(name, p)| {
                let label = self.net_label(&name, &ifaces);
                (
                    name,
                    NetProxy {
                        proxy: to_core(&p),
                        label,
                    },
                )
            })
            .collect();
        fuselane_core::proxy::set(table);
    }

    /// Device names of networks with a proxy.
    pub(super) fn proxied(&self) -> Vec<String> {
        lock(&self.net_prefs)
            .iter()
            .filter(|p| p.proxy.is_some())
            .map(|p| p.name.clone())
            .collect()
    }

    /// Sets (or with `None` removes) a network's proxy. Returns every
    /// network's prefs as the window sees them.
    pub fn set_network_proxy(
        &self,
        name: &str,
        proxy: Option<ProxyRequest>,
    ) -> Result<Vec<NetPref>, UiError> {
        if !valid_device(name) {
            return Err(UiError::new(
                "bad-network-name",
                "That isn't a network on this computer.",
                None,
            ));
        }
        let current = lock(&self.net_prefs)
            .iter()
            .find(|p| p.name == name)
            .cloned();
        let mut pref = current.unwrap_or_else(|| NetPref {
            name: name.to_string(),
            label: None,
            lane: None,
            use_for: super::NetUse::default(),
            hours: None,
            proxy: None,
        });
        pref.proxy = match proxy {
            None => None,
            Some(req) => Some(validated(req, pref.proxy.as_ref())?),
        };
        let all = self.save_pref(pref)?;
        Ok(masked(all))
    }

    /// Checks a network's saved proxy by opening a tunnel through it, over that
    /// network, to Fuselane's own site (the one the sign-in check uses).
    pub async fn check_network_proxy(&self, name: &str) -> Result<String, UiError> {
        let Some(saved) = lock(&self.net_prefs)
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.proxy.clone())
        else {
            return Err(UiError::new(
                "no-proxy",
                "There's no proxy set for that network.",
                Some("Set one first, then check it."),
            ));
        };
        let ifaces = fuselane_netif::list().unwrap_or_default();
        let label = self.net_label(name, &ifaces);
        let Some(iface) = ifaces.into_iter().find(|i| i.name == name && i.usable()) else {
            return Err(UiError {
                code: "not-connected",
                message: format!(
                    "{label} isn't connected right now, so its proxy can't be checked."
                ),
                hint: Some("Connect it, then check again.".into()),
            });
        };
        let proxy = to_core(&saved);
        match fuselane_transport::proxy::connect(
            &proxy,
            &iface,
            fuselane_transport::probe::HOST,
            443,
            CHECK_WAIT,
        )
        .await
        {
            Ok(_) => Ok(format!(
                "Works: {label} reaches the internet through this proxy."
            )),
            Err(e) => {
                let (message, hint) = fuselane_core::proxy::problem(&e, &label, &proxy);
                Err(UiError {
                    code: code(&e),
                    message,
                    hint: Some(hint),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::secrets;
    use super::*;

    fn req(kind: ProxyType, host: &str, port: u32) -> ProxyRequest {
        ProxyRequest {
            kind,
            host: host.into(),
            port,
            username: None,
            password: None,
        }
    }

    #[test]
    fn hosts_are_checked_and_cleaned() {
        for (raw, want) in [
            ("Proxy.Example.com", "proxy.example.com"),
            (" 192.168.1.10 ", "192.168.1.10"),
            ("[2001:db8::1]", "2001:db8::1"),
            ("2001:db8::1", "2001:db8::1"),
            ("corp_proxy.lan.", "corp_proxy.lan"),
            ("localhost", "localhost"),
        ] {
            assert_eq!(clean_host(raw).unwrap(), want, "{raw}");
        }
        for (raw, says) in [
            ("", "Enter the proxy's name"),
            ("http://proxy:8080", "just the proxy's name"),
            ("proxy.example.com:8080", "Port box"),
            ("bad host", "valid proxy name"),
            ("-x.example", "valid proxy name"),
            ("0.0.0.0", "can't be a proxy"),
            ("224.0.0.1", "can't be a proxy"),
        ] {
            let e = clean_host(raw).unwrap_err();
            assert_eq!(e.code, "bad-proxy");
            assert!(e.message.contains(says), "{raw}: {}", e.message);
            assert!(e.hint.is_some(), "{raw}: says what to do");
        }
    }

    #[test]
    fn ports_and_logins_are_checked() {
        for port in [0, 65_536, 1_000_000] {
            let e = validated(req(ProxyType::Http, "proxy.lan", port), None).unwrap_err();
            assert!(e.message.contains("port"), "{port}");
        }
        let mut r = req(ProxyType::Http, "proxy.lan", 3128);
        r.username = Some("ann:x".into());
        assert!(
            validated(r.clone(), None)
                .unwrap_err()
                .message
                .contains("colon")
        );
        r.kind = ProxyType::Socks5; // SOCKS5 sends them apart: a colon is fine
        assert!(validated(r, None).is_ok());
        let mut r = req(ProxyType::Socks5, "proxy.lan", 1080);
        r.password = Some("secret".into());
        assert!(
            validated(r.clone(), None)
                .unwrap_err()
                .message
                .contains("username")
        );
        r.username = Some("ann".into());
        r.password = Some("x".repeat(256));
        assert!(validated(r, None).unwrap_err().message.contains("password"));
    }

    #[test]
    fn a_missing_password_keeps_the_saved_one_and_an_empty_one_removes_it() {
        let mut r = req(ProxyType::Socks5, "proxy.lan", 1080);
        r.username = Some("ann".into());
        r.password = Some("first".into());
        let saved = validated(r.clone(), None).unwrap();
        assert_eq!(saved.password.as_deref(), Some("first"));
        assert!(saved.has_password);
        r.password = None;
        let kept = validated(r.clone(), Some(&saved)).unwrap();
        assert_eq!(kept.password.as_deref(), Some("first"));
        r.password = Some(String::new());
        let gone = validated(r.clone(), Some(&saved)).unwrap();
        assert_eq!(gone.password, None);
        assert!(!gone.has_password);
        // Without a username, no password is kept either.
        r.username = None;
        r.password = None;
        assert_eq!(validated(r, Some(&saved)).unwrap().password, None);
    }

    #[test]
    fn the_window_never_gets_the_password_and_logs_never_show_the_login() {
        let mut r = req(ProxyType::Http, "proxy.lan", 3128);
        r.username = Some("ann".into());
        r.password = Some("hunter2".into());
        let p = validated(r.clone(), None).unwrap();
        let pref = NetPref {
            name: "en0".into(),
            label: None,
            lane: None,
            use_for: super::super::NetUse::Always,
            hours: None,
            proxy: Some(p.clone()),
        };
        let shown = serde_json::to_string(&masked(vec![pref.clone()])).unwrap();
        assert!(!shown.contains("hunter2"), "{shown}");
        assert!(shown.contains("\"hasPassword\":true"), "{shown}");
        for debug in [format!("{p:?}"), format!("{r:?}"), format!("{pref:?}")] {
            assert!(
                !debug.contains("hunter2") && !debug.contains("ann"),
                "{debug}"
            );
        }
    }

    #[test]
    fn a_damaged_saved_proxy_is_dropped() {
        let mut r = req(ProxyType::Http, "proxy.lan", 3128);
        r.username = Some("ann".into());
        r.password = Some("pw".into());
        let good = validated(r, None).unwrap();
        assert!(saved_ok(&good));
        let mut bad_host = good.clone();
        bad_host.host = "not a host".into();
        assert!(!saved_ok(&bad_host));
        let mut bad_port = good;
        bad_port.port = 0;
        assert!(!saved_ok(&bad_port));
    }

    fn service(dir: &std::path::Path) -> std::sync::Arc<Service> {
        let store = fuselane_core::Store::open(&dir.join("fuselane.db")).unwrap();
        Service::new(store, dir.to_path_buf()).unwrap()
    }

    /// A service whose keychain outlives it, like the real one across restarts.
    fn service_with(
        dir: &std::path::Path,
        keys: &std::sync::Arc<secrets::Memory>,
    ) -> std::sync::Arc<Service> {
        let store = fuselane_core::Store::open(&dir.join("fuselane.db")).unwrap();
        Service::with_secrets(store, dir.to_path_buf(), keys.clone()).unwrap()
    }

    fn login(kind: ProxyType, host: &str, port: u32, pass: Option<&str>) -> ProxyRequest {
        ProxyRequest {
            kind,
            host: host.into(),
            port,
            username: Some("ann".into()),
            password: pass.map(Into::into),
        }
    }

    #[test]
    fn a_proxy_is_saved_masked_survives_a_restart_and_a_rename() {
        let dir = tempfile::tempdir().unwrap();
        let keys = std::sync::Arc::new(secrets::Memory::default());
        let svc = service_with(dir.path(), &keys);
        let shown = svc
            .set_network_proxy(
                "en9",
                Some(login(ProxyType::Socks5, "Proxy.LAN", 1080, Some("hunter2"))),
            )
            .unwrap();
        let p = shown[0].proxy.clone().unwrap();
        assert_eq!((p.host.as_str(), p.port), ("proxy.lan", 1080));
        assert_eq!((p.password, p.has_password), (None, true), "masked");
        assert_eq!(p.password_in, Some(PasswordHome::Keychain), "says where");
        assert_eq!(svc.network_prefs(), shown);
        assert!(svc.proxied().contains(&"en9".to_string()));
        // In the keychain, not in the settings; a restart reads it back.
        let raw = svc.store.setting("network_prefs").unwrap().unwrap();
        assert!(!raw.contains("hunter2"), "{raw}");
        assert_eq!(keys.entry("proxy:en9").as_deref(), Some("hunter2"));
        assert!(
            svc.diagnostics(&[])
                .contains("passwords: 1 in the system keychain")
        );
        drop(svc);
        let svc = service_with(dir.path(), &keys);
        let saved = lock(&svc.net_prefs)[0].proxy.clone().unwrap();
        assert_eq!(saved.password.as_deref(), Some("hunter2"));
        // Renaming the network keeps its proxy, whatever the window sends.
        let mut renamed = svc.network_prefs()[0].clone();
        renamed.label = Some("Office".into());
        renamed.proxy = None;
        svc.set_network_pref(renamed).unwrap();
        let kept = lock(&svc.net_prefs)[0].clone();
        assert_eq!(kept.label.as_deref(), Some("Office"));
        assert_eq!(
            kept.proxy.and_then(|p| p.password).as_deref(),
            Some("hunter2")
        );
        // Changing the port without retyping the password keeps the password.
        svc.set_network_proxy(
            "en9",
            Some(login(ProxyType::Socks5, "proxy.lan", 1081, None)),
        )
        .unwrap();
        let p = lock(&svc.net_prefs)[0].proxy.clone().unwrap();
        assert_eq!((p.port, p.password.as_deref()), (1081, Some("hunter2")));
        assert_eq!(keys.entry("proxy:en9").as_deref(), Some("hunter2"));
        // Removing it leaves the rest of the network's settings, and the
        // keychain entry goes with it.
        let after = svc.set_network_proxy("en9", None).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].proxy, None);
        assert!(svc.proxied().is_empty());
        assert_eq!(keys.entry("proxy:en9"), None);
    }

    #[test]
    fn without_a_keychain_the_password_is_kept_in_the_settings_and_moves_later() {
        let dir = tempfile::tempdir().unwrap();
        let keys = std::sync::Arc::new(secrets::Memory::default());
        keys.break_it(true);
        let svc = service_with(dir.path(), &keys);
        let shown = svc
            .set_network_proxy(
                "en9",
                Some(login(ProxyType::Http, "proxy.lan", 3128, Some("hunter2"))),
            )
            .unwrap();
        let p = shown[0].proxy.clone().unwrap();
        assert_eq!(
            (p.password, p.has_password, p.password_in),
            (None, true, Some(PasswordHome::Settings)),
            "saved, masked, and the window can say where"
        );
        let raw = svc.store.setting("network_prefs").unwrap().unwrap();
        assert!(raw.contains("hunter2"), "never lost");
        assert!(
            svc.diagnostics(&[])
                .contains("1 in Fuselane's settings (no system keychain available)")
        );
        drop(svc);
        // Next launch, with a keychain: it moves there and leaves the settings.
        keys.break_it(false);
        let svc = service_with(dir.path(), &keys);
        assert_eq!(keys.entry("proxy:en9").as_deref(), Some("hunter2"));
        let raw = svc.store.setting("network_prefs").unwrap().unwrap();
        assert!(!raw.contains("hunter2"), "{raw}");
        let p = lock(&svc.net_prefs)[0].proxy.clone().unwrap();
        assert_eq!(p.password.as_deref(), Some("hunter2"), "still used");
    }

    #[test]
    fn a_locked_keychain_at_launch_loses_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let keys = std::sync::Arc::new(secrets::Memory::default());
        let svc = service_with(dir.path(), &keys);
        svc.set_network_proxy(
            "en9",
            Some(login(ProxyType::Socks5, "proxy.lan", 1080, Some("hunter2"))),
        )
        .unwrap();
        drop(svc);
        keys.break_it(true);
        let svc = service_with(dir.path(), &keys);
        let p = svc.network_prefs()[0].proxy.clone().unwrap();
        assert!(p.has_password, "still shown as saved");
        // Renaming and changing the port meanwhile keep the keychain entry.
        let mut renamed = svc.network_prefs()[0].clone();
        renamed.label = Some("Office".into());
        svc.set_network_pref(renamed).unwrap();
        svc.set_network_proxy(
            "en9",
            Some(login(ProxyType::Socks5, "proxy.lan", 1081, None)),
        )
        .unwrap();
        assert_eq!(keys.entry("proxy:en9").as_deref(), Some("hunter2"));
        // Once it opens, the next save reads the password.
        keys.break_it(false);
        svc.apply_proxies();
        let p = lock(&svc.net_prefs)[0].proxy.clone().unwrap();
        assert_eq!((p.port, p.password.as_deref()), (1081, Some("hunter2")));
    }

    #[test]
    fn a_password_saved_by_an_older_version_moves_to_the_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("fuselane.db")).unwrap();
        store
            .set_setting(
                "network_prefs",
                r#"[{"name":"en9","label":"Office","lane":null,"useFor":"always","hours":null,
                "proxy":{"kind":"socks5","host":"proxy.lan","port":1080,"username":"ann",
                "password":"hunter2","hasPassword":true}}]"#,
            )
            .unwrap();
        drop(store);
        let keys = std::sync::Arc::new(secrets::Memory::default());
        let svc = service_with(dir.path(), &keys);
        assert_eq!(keys.entry("proxy:en9").as_deref(), Some("hunter2"));
        let raw = svc.store.setting("network_prefs").unwrap().unwrap();
        assert!(!raw.contains("hunter2"), "{raw}");
        assert!(raw.contains("Office"), "the rest kept");
        // Forgetting the network removes the entry too.
        let mut plain = svc.network_prefs()[0].clone();
        plain.label = None;
        svc.set_network_pref(plain).unwrap();
        svc.set_network_proxy("en9", None).unwrap();
        assert!(svc.network_prefs().is_empty());
        assert_eq!(keys.entry("proxy:en9"), None);
    }

    #[test]
    fn a_proxy_with_nothing_else_set_is_forgotten_when_removed() {
        let dir = tempfile::tempdir().unwrap();
        let svc = service(dir.path());
        svc.set_network_proxy(
            "en9",
            Some(ProxyRequest {
                kind: ProxyType::Http,
                host: "10.0.0.2".into(),
                port: 3128,
                username: None,
                password: None,
            }),
        )
        .unwrap();
        assert!(svc.set_network_proxy("en9", None).unwrap().is_empty());
        let e = svc.set_network_proxy("bad\nname", None).unwrap_err();
        assert_eq!(e.code, "bad-network-name");
        let e = svc
            .set_network_proxy(
                "en9",
                Some(login(ProxyType::Http, "proxy:8080", 8080, None)),
            )
            .unwrap_err();
        assert_eq!(e.code, "bad-proxy");
        assert!(
            svc.network_prefs().is_empty(),
            "nothing saved on a bad request"
        );
    }

    #[tokio::test]
    async fn checking_a_proxy_says_what_is_wrong() {
        let dir = tempfile::tempdir().unwrap();
        let svc = service(dir.path());
        let e = svc.check_network_proxy("en9").await.unwrap_err();
        assert_eq!(e.code, "no-proxy");
        svc.set_network_proxy(
            "nowhere9",
            Some(login(ProxyType::Http, "127.0.0.1", 9, Some("pw"))),
        )
        .unwrap();
        let e = svc.check_network_proxy("nowhere9").await.unwrap_err();
        assert_eq!(e.code, "not-connected");
        assert!(e.hint.is_some());
        // A real network, a proxy on this computer that wants another password:
        // the check fails before anything leaves for the internet.
        let Some(net) = fuselane_netif::usable()
            .ok()
            .and_then(|v| v.into_iter().next())
        else {
            return; // no network on this machine
        };
        let tp = fuselane_testkit::TestProxy::start(
            fuselane_testkit::ProxyKind::Socks5,
            Some(("ann", "right")),
        )
        .await
        .unwrap();
        let at = tp.addr();
        svc.set_network_proxy(
            &net.name,
            Some(login(
                ProxyType::Socks5,
                &at.ip().to_string(),
                u32::from(at.port()),
                Some("wrong"),
            )),
        )
        .unwrap();
        let e = svc.check_network_proxy(&net.name).await.unwrap_err();
        assert_eq!(e.code, "proxy-login-refused", "{}", e.message);
        assert!(e.message.contains("turned down the username and password"));
        assert!(!e.message.contains("wrong"));
        assert!(tp.targets().is_empty(), "never asked to go anywhere");
        svc.set_network_proxy(&net.name, None).unwrap();
    }
}
