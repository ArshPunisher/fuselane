//! Proxy passwords in the system's credential store (SECURITY.md T16): the
//! macOS Keychain, Windows Credential Manager or the Secret Service on Linux.
//!
//! The settings on disk keep only a marker saying the password is in the
//! keychain. When there's no keychain to use (a Linux machine without a Secret
//! Service, a locked or refused keychain, any error), the password stays in
//! Fuselane's settings as before and is marked so the window and diagnostics
//! can say where it is; every later save, and every launch, tries the keychain
//! again. A password is never dropped because the keychain failed, and never
//! logged: errors here name only the network and what went wrong.

use serde::{Deserialize, Serialize};

use super::NetPref;

/// The keychain item's service name: the app's identity, so the entries are
/// easy to find (and remove) in Keychain Access, Credential Manager or Seahorse.
pub const SERVICE: &str = "app.fuselane.Fuselane";

/// Where a saved proxy password is kept.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PasswordHome {
    /// In the system's credential store; the settings hold only this marker.
    Keychain,
    /// In Fuselane's own settings, because no system keychain was available.
    Settings,
}

/// Why the credential store couldn't be used. Carries a short reason only,
/// never the secret.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SecretError(pub String);

/// A place for secrets, one per account name. Behind a trait so tests use an
/// in-memory store and never touch the real keychain (CI has none).
pub trait Secrets: Send + Sync {
    /// The secret, or `None` when there's no entry.
    fn get(&self, account: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError>;
    /// Removes the entry; a missing one is fine.
    fn delete(&self, account: &str) -> Result<(), SecretError>;
}

/// The account a network's proxy password is saved under. Networks are keyed
/// by device name, which a rename (a label) never changes.
pub fn account(network: &str) -> String {
    format!("proxy:{network}")
}

/// The operating system's credential store, through the `keyring` crate.
pub struct OsKeychain;

impl OsKeychain {
    fn entry(account: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, account).map_err(reason)
    }
}

/// A short, secret-free description of a keyring error. (Its `Debug` form can
/// hold the stored bytes, so it's never used.)
fn reason(e: keyring::Error) -> SecretError {
    SecretError(
        match e {
            keyring::Error::NoStorageAccess(_) => "the keychain is locked or refused access",
            keyring::Error::PlatformFailure(_) => "no system keychain is available",
            keyring::Error::BadEncoding(_) => "the keychain entry isn't readable text",
            keyring::Error::TooLong(..) | keyring::Error::Invalid(..) => {
                "the keychain didn't accept the entry"
            }
            keyring::Error::Ambiguous(_) => "the keychain has more than one entry for it",
            _ => "the keychain failed",
        }
        .to_string(),
    )
}

impl Secrets for OsKeychain {
    fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        match Self::entry(account)?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(reason(e)),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        Self::entry(account)?.set_password(secret).map_err(reason)
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(reason(e)),
        }
    }
}

/// Writes a secret and reads it back: only a confirmed write lets the
/// settings forget their copy.
fn put(secrets: &dyn Secrets, account: &str, secret: &str) -> Result<(), SecretError> {
    secrets.set(account, secret)?;
    match secrets.get(account)? {
        Some(back) if back == secret => Ok(()),
        _ => Err(SecretError("the keychain didn't keep the password".into())),
    }
}

fn warn(network: &str, what: &str, e: &SecretError) {
    eprintln!("fuselane: proxy password for {network}: {what}: {e}");
}

/// Fills in passwords kept in the keychain, after loading or when an earlier
/// read failed. Returns whether the prefs changed in a way worth saving (an
/// entry someone deleted from the keychain, or passwords still in the settings
/// that should move there).
pub(super) fn fill(secrets: &dyn Secrets, prefs: &mut [NetPref]) -> bool {
    let mut changed = false;
    for pref in prefs.iter_mut() {
        let Some(p) = pref.proxy.as_mut() else {
            continue;
        };
        match (&p.password, p.password_in) {
            (None, Some(PasswordHome::Keychain)) => match secrets.get(&account(&pref.name)) {
                Ok(Some(found)) => p.password = Some(found),
                Ok(None) => {
                    // Removed from the keychain by hand: there's nothing to keep.
                    p.password_in = None;
                    changed = true;
                }
                // Locked or unavailable: keep the marker and try again later.
                Err(e) => warn(&pref.name, "couldn't read it from the keychain", &e),
            },
            (None, _) => p.password_in = None,
            // Saved in the settings (or before keychains were used): move it.
            (Some(_), home) => changed |= home != Some(PasswordHome::Keychain),
        }
        p.has_password = p.password.is_some() || p.password_in == Some(PasswordHome::Keychain);
    }
    changed
}

/// What a save did to the keychain, so a failed settings write can undo it.
#[derive(Debug, Default)]
pub(super) struct Stashed {
    written: Vec<String>,
}

/// Puts every password the keychain doesn't hold yet into it, and returns the
/// prefs as they go on disk: without the passwords the keychain confirmed.
/// `all` gets each password's home. `before` is what was saved last time, so
/// unchanged passwords aren't written again.
pub(super) fn stash(
    secrets: &dyn Secrets,
    before: &[NetPref],
    all: &mut [NetPref],
) -> (Vec<NetPref>, Stashed) {
    let mut done = Stashed::default();
    for pref in all.iter_mut() {
        let Some(p) = pref.proxy.as_mut() else {
            continue;
        };
        let Some(password) = p.password.clone() else {
            // Kept only when it's in the keychain but couldn't be read yet.
            if p.password_in != Some(PasswordHome::Keychain) {
                p.password_in = None;
            }
            p.has_password = p.password_in.is_some();
            continue;
        };
        let already = before
            .iter()
            .find(|b| b.name == pref.name)
            .and_then(|b| b.proxy.as_ref())
            .is_some_and(|b| {
                b.password_in == Some(PasswordHome::Keychain)
                    && b.password.as_deref() == Some(&password)
            });
        let key = account(&pref.name);
        p.password_in = Some(if already {
            PasswordHome::Keychain
        } else {
            match put(secrets, &key, &password) {
                Ok(()) => {
                    done.written.push(pref.name.clone());
                    PasswordHome::Keychain
                }
                Err(e) => {
                    warn(&pref.name, "kept in Fuselane's settings", &e);
                    PasswordHome::Settings
                }
            }
        });
        p.has_password = true;
    }
    let disk = all
        .iter()
        .cloned()
        .map(|mut pref| {
            if let Some(p) = pref.proxy.as_mut()
                && p.password_in == Some(PasswordHome::Keychain)
            {
                p.password = None;
            }
            pref
        })
        .collect();
    (disk, done)
}

/// Undoes `stash` after the settings couldn't be written: each entry goes back
/// to the password saved before, or away if there was none in the keychain.
/// (One that was there but unreadable is left with the new password rather than
/// deleted: the settings still point at the keychain.)
pub(super) fn rollback(secrets: &dyn Secrets, before: &[NetPref], done: Stashed) {
    for name in done.written {
        let old = before
            .iter()
            .find(|b| b.name == name)
            .and_then(|b| b.proxy.as_ref())
            .filter(|b| b.password_in == Some(PasswordHome::Keychain));
        let key = account(&name);
        let undone = match old.map(|b| b.password.as_deref()) {
            Some(Some(pw)) => secrets.set(&key, pw),
            // It was there but couldn't be read: the new one is better than none.
            Some(None) => Ok(()),
            None => secrets.delete(&key),
        };
        if let Err(e) = undone {
            warn(&name, "couldn't undo the keychain change", &e);
        }
    }
}

/// After a save: removes keychain entries no network uses any more (a proxy or
/// its password removed, a network forgotten, or the password now in the
/// settings).
pub(super) fn forget_stale(secrets: &dyn Secrets, before: &[NetPref], all: &[NetPref]) {
    let in_keychain = |prefs: &[NetPref], name: &str| {
        prefs
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.proxy.as_ref())
            .is_some_and(|p| p.password_in == Some(PasswordHome::Keychain))
    };
    for b in before {
        if in_keychain(before, &b.name)
            && !in_keychain(all, &b.name)
            && let Err(e) = secrets.delete(&account(&b.name))
        {
            warn(&b.name, "couldn't remove it from the keychain", &e);
        }
    }
}

/// An in-memory credential store for tests, which can be made to fail.
#[cfg(test)]
#[derive(Default)]
pub struct Memory {
    pub entries: std::sync::Mutex<std::collections::HashMap<String, String>>,
    /// Every call fails, like a machine without a keychain.
    pub broken: std::sync::atomic::AtomicBool,
    /// Writes calls made, to check unchanged passwords aren't rewritten.
    pub writes: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl Memory {
    fn check(&self) -> Result<(), SecretError> {
        if self.broken.load(std::sync::atomic::Ordering::SeqCst) {
            Err(SecretError("no system keychain is available".into()))
        } else {
            Ok(())
        }
    }

    pub fn break_it(&self, broken: bool) {
        self.broken
            .store(broken, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn entry(&self, account: &str) -> Option<String> {
        super::lock(&self.entries).get(account).cloned()
    }
}

#[cfg(test)]
impl Secrets for Memory {
    fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        self.check()?;
        Ok(super::lock(&self.entries).get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        self.check()?;
        self.writes
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        super::lock(&self.entries).insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        self.check()?;
        super::lock(&self.entries).remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::proxy::{ProxyPref, ProxyType};
    use super::super::{NetPref, NetUse};
    use super::*;

    fn pref(name: &str, password: Option<&str>, home: Option<PasswordHome>) -> NetPref {
        NetPref {
            name: name.into(),
            label: None,
            lane: None,
            use_for: NetUse::Always,
            hours: None,
            proxy: Some(ProxyPref {
                kind: ProxyType::Socks5,
                host: "proxy.lan".into(),
                port: 1080,
                username: Some("ann".into()),
                has_password: password.is_some() || home.is_some(),
                password: password.map(Into::into),
                password_in: home,
            }),
        }
    }

    fn home(p: &NetPref) -> Option<PasswordHome> {
        p.proxy.as_ref().and_then(|x| x.password_in)
    }

    #[test]
    fn a_password_goes_to_the_keychain_and_off_the_disk() {
        let keys = Memory::default();
        let mut all = vec![pref("en0", Some("hunter2"), None)];
        let (disk, _) = stash(&keys, &[], &mut all);
        assert_eq!(keys.entry("proxy:en0").as_deref(), Some("hunter2"));
        assert_eq!(home(&all[0]), Some(PasswordHome::Keychain));
        // In memory it stays usable; on disk only the marker is left.
        assert_eq!(
            all[0].proxy.as_ref().unwrap().password.as_deref(),
            Some("hunter2")
        );
        let json = serde_json::to_string(&disk).unwrap();
        assert!(!json.contains("hunter2"), "{json}");
        assert!(json.contains("\"passwordIn\":\"keychain\""), "{json}");
        // Read back after a restart.
        let mut loaded: Vec<NetPref> = serde_json::from_str(&json).unwrap();
        assert!(!fill(&keys, &mut loaded), "nothing to save again");
        assert_eq!(loaded, all);
    }

    #[test]
    fn an_unchanged_password_isnt_written_again() {
        let keys = Memory::default();
        let mut all = vec![pref("en0", Some("pw"), None)];
        stash(&keys, &[], &mut all);
        let before = all.clone();
        stash(&keys, &before, &mut all);
        assert_eq!(keys.writes.load(std::sync::atomic::Ordering::SeqCst), 1);
        // A new one is.
        let mut changed = vec![pref("en0", Some("new"), None)];
        stash(&keys, &before, &mut changed);
        assert_eq!(keys.entry("proxy:en0").as_deref(), Some("new"));
    }

    #[test]
    fn without_a_keychain_the_password_stays_in_the_settings() {
        let keys = Memory::default();
        keys.break_it(true);
        let mut all = vec![pref("en0", Some("hunter2"), None)];
        let (disk, _) = stash(&keys, &[], &mut all);
        assert_eq!(home(&all[0]), Some(PasswordHome::Settings));
        let json = serde_json::to_string(&disk).unwrap();
        assert!(json.contains("hunter2"), "never lost");
        assert!(json.contains("\"passwordIn\":\"settings\""), "{json}");
        // The next launch moves it once a keychain is there.
        let mut loaded: Vec<NetPref> = serde_json::from_str(&json).unwrap();
        keys.break_it(false);
        assert!(fill(&keys, &mut loaded), "worth saving: it can move");
        let before = loaded.clone();
        let (disk, _) = stash(&keys, &before, &mut loaded);
        assert!(!serde_json::to_string(&disk).unwrap().contains("hunter2"));
        assert_eq!(keys.entry("proxy:en0").as_deref(), Some("hunter2"));
    }

    #[test]
    fn a_password_from_an_older_version_is_migrated() {
        // Saved by beta.10 and earlier: in the settings, with no marker.
        let old = r#"[{"name":"en0","label":null,"lane":null,"useFor":"always","hours":null,
            "proxy":{"kind":"socks5","host":"proxy.lan","port":1080,"username":"ann",
            "password":"hunter2","hasPassword":true}}]"#;
        let keys = Memory::default();
        let mut loaded: Vec<NetPref> = serde_json::from_str(old).unwrap();
        assert!(fill(&keys, &mut loaded));
        let before = loaded.clone();
        let (disk, _) = stash(&keys, &before, &mut loaded);
        assert_eq!(keys.entry("proxy:en0").as_deref(), Some("hunter2"));
        assert!(!serde_json::to_string(&disk).unwrap().contains("hunter2"));
    }

    #[test]
    fn a_write_the_keychain_doesnt_keep_isnt_trusted() {
        struct Forgetful;
        impl Secrets for Forgetful {
            fn get(&self, _: &str) -> Result<Option<String>, SecretError> {
                Ok(None)
            }
            fn set(&self, _: &str, _: &str) -> Result<(), SecretError> {
                Ok(())
            }
            fn delete(&self, _: &str) -> Result<(), SecretError> {
                Ok(())
            }
        }
        let mut all = vec![pref("en0", Some("hunter2"), None)];
        let (disk, _) = stash(&Forgetful, &[], &mut all);
        assert_eq!(home(&all[0]), Some(PasswordHome::Settings));
        assert!(serde_json::to_string(&disk).unwrap().contains("hunter2"));
    }

    #[test]
    fn a_locked_keychain_at_launch_keeps_the_marker() {
        let keys = Memory::default();
        keys.break_it(true);
        let mut loaded = vec![pref("en0", None, Some(PasswordHome::Keychain))];
        assert!(!fill(&keys, &mut loaded));
        let p = loaded[0].proxy.clone().unwrap();
        assert_eq!(
            (p.password, p.password_in),
            (None, Some(PasswordHome::Keychain))
        );
        assert!(p.has_password, "still shown as saved");
        // Saving other changes meanwhile keeps the marker and the entry.
        let before = loaded.clone();
        let (disk, _) = stash(&keys, &before, &mut loaded);
        assert_eq!(home(&disk[0]), Some(PasswordHome::Keychain));
        forget_stale(&keys, &before, &loaded);
        // Unlocked later: it's read.
        keys.break_it(false);
        lock_in(&keys, "proxy:en0", "hunter2");
        fill(&keys, &mut loaded);
        assert_eq!(
            loaded[0].proxy.as_ref().unwrap().password.as_deref(),
            Some("hunter2")
        );
    }

    fn lock_in(keys: &Memory, account: &str, secret: &str) {
        super::super::lock(&keys.entries).insert(account.into(), secret.into());
    }

    #[test]
    fn an_entry_deleted_by_hand_means_no_saved_password() {
        let keys = Memory::default();
        let mut loaded = vec![pref("en0", None, Some(PasswordHome::Keychain))];
        assert!(fill(&keys, &mut loaded), "saved without the marker");
        let p = loaded[0].proxy.clone().unwrap();
        assert_eq!((p.password_in, p.has_password), (None, false));
    }

    #[test]
    fn removing_a_proxy_or_its_password_removes_the_entry() {
        let keys = Memory::default();
        let mut all = vec![pref("en0", Some("a"), None), pref("en1", Some("b"), None)];
        stash(&keys, &[], &mut all);
        let before = all.clone();
        let mut after = all.clone();
        after[0].proxy = None; // proxy removed
        after.remove(1); // network forgotten
        stash(&keys, &before, &mut after);
        forget_stale(&keys, &before, &after);
        assert_eq!(keys.entry("proxy:en0"), None);
        assert_eq!(keys.entry("proxy:en1"), None);
        // A password cleared on a kept proxy too.
        let mut all = vec![pref("en2", Some("c"), None)];
        stash(&keys, &[], &mut all);
        let before = all.clone();
        let mut after = vec![pref("en2", None, None)];
        stash(&keys, &before, &mut after);
        forget_stale(&keys, &before, &after);
        assert_eq!(keys.entry("proxy:en2"), None);
        assert_eq!(home(&after[0]), None);
    }

    #[test]
    fn a_failed_settings_write_undoes_the_keychain() {
        let keys = Memory::default();
        let mut all = vec![pref("en0", Some("old"), None)];
        stash(&keys, &[], &mut all);
        let before = all.clone();
        let mut next = vec![pref("en0", Some("new"), None), pref("en1", Some("x"), None)];
        let (_, done) = stash(&keys, &before, &mut next);
        rollback(&keys, &before, done);
        assert_eq!(keys.entry("proxy:en0").as_deref(), Some("old"));
        assert_eq!(keys.entry("proxy:en1"), None);
    }

    /// The real store on this machine. Run by hand (`--ignored`); CI has no
    /// keychain. Leaves nothing behind.
    #[test]
    #[ignore = "uses the system keychain"]
    fn the_system_keychain_round_trips() {
        let key = account(&format!("fuselane-test-{}", std::process::id()));
        let os = OsKeychain;
        put(&os, &key, "fuselane-test-secret").unwrap();
        assert_eq!(
            os.get(&key).unwrap().as_deref(),
            Some("fuselane-test-secret")
        );
        os.delete(&key).unwrap();
        assert_eq!(os.get(&key).unwrap(), None);
        os.delete(&key).unwrap();
    }

    #[test]
    fn errors_never_carry_the_password() {
        let e = reason(keyring::Error::BadEncoding(b"hunter2".to_vec()));
        assert!(!format!("{e} {e:?}").contains("hunter2"));
    }
}
