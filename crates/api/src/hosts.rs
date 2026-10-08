//! Telling browsers where Fuselane's native-messaging host is (BROWSER-EXTENSION.md
//! §2, STEPS 7.4). The app writes these on every launch: macOS and AppImage users
//! have no installer, and a moved or updated app fixes its own manifests.
//!
//! Per user only, and only for browsers that are installed. Flatpak and Snap
//! browsers can't start hosts outside their sandbox; they need the localhost
//! fallback (7.5).

use std::path::{Path, PathBuf};

use serde_json::json;

pub const HOST_NAME: &str = "app.fuselane.host";
/// Chrome Web Store id. Edge and Brave install from the Chrome Web Store too.
pub const CHROME_EXTENSION_IDS: &[&str] = &["nggljghjikdkigiekdciocigdnnhponl"];
/// Fixed in the Firefox build's manifest (`apps/extension/wxt.config.ts`).
pub const FIREFOX_EXTENSION_ID: &str = "fuselane@fuselane.app";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Chromium,
    Firefox,
}

// `Home` is only used on Linux; Windows uses the registry instead.
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
#[derive(Debug, Clone, Copy)]
enum Base {
    /// `~/Library/Application Support` on macOS, `$XDG_CONFIG_HOME` on Linux.
    Config,
    Home,
}

/// A browser that reads host manifests from a folder (macOS and Linux).
#[derive(Debug, Clone, Copy)]
struct Browser {
    name: &'static str,
    kind: Kind,
    base: Base,
    /// Exists once the browser has run, so its absence means "not installed".
    profile: &'static str,
    hosts: &'static str,
}

#[cfg_attr(windows, allow(dead_code))]
const fn b(
    name: &'static str,
    kind: Kind,
    base: Base,
    profile: &'static str,
    hosts: &'static str,
) -> Browser {
    Browser {
        name,
        kind,
        base,
        profile,
        hosts,
    }
}

#[cfg(target_os = "macos")]
const BROWSERS: &[Browser] = {
    use Base::Config as C;
    use Kind::{Chromium as Cr, Firefox as Ff};
    &[
        b(
            "Chrome",
            Cr,
            C,
            "Google/Chrome",
            "Google/Chrome/NativeMessagingHosts",
        ),
        b(
            "Chrome Beta",
            Cr,
            C,
            "Google/Chrome Beta",
            "Google/Chrome Beta/NativeMessagingHosts",
        ),
        b(
            "Chrome Dev",
            Cr,
            C,
            "Google/Chrome Dev",
            "Google/Chrome Dev/NativeMessagingHosts",
        ),
        b(
            "Chrome Canary",
            Cr,
            C,
            "Google/Chrome Canary",
            "Google/Chrome Canary/NativeMessagingHosts",
        ),
        b(
            "Chromium",
            Cr,
            C,
            "Chromium",
            "Chromium/NativeMessagingHosts",
        ),
        b(
            "Edge",
            Cr,
            C,
            "Microsoft Edge",
            "Microsoft Edge/NativeMessagingHosts",
        ),
        b(
            "Brave",
            Cr,
            C,
            "BraveSoftware/Brave-Browser",
            "BraveSoftware/Brave-Browser/NativeMessagingHosts",
        ),
        b("Vivaldi", Cr, C, "Vivaldi", "Vivaldi/NativeMessagingHosts"),
        b("Firefox", Ff, C, "Firefox", "Mozilla/NativeMessagingHosts"),
    ]
};

#[cfg(all(unix, not(target_os = "macos")))]
const BROWSERS: &[Browser] = {
    use Base::{Config as C, Home as H};
    use Kind::{Chromium as Cr, Firefox as Ff};
    &[
        b(
            "Chrome",
            Cr,
            C,
            "google-chrome",
            "google-chrome/NativeMessagingHosts",
        ),
        b(
            "Chrome Beta",
            Cr,
            C,
            "google-chrome-beta",
            "google-chrome-beta/NativeMessagingHosts",
        ),
        b(
            "Chrome Dev",
            Cr,
            C,
            "google-chrome-unstable",
            "google-chrome-unstable/NativeMessagingHosts",
        ),
        b(
            "Chromium",
            Cr,
            C,
            "chromium",
            "chromium/NativeMessagingHosts",
        ),
        b(
            "Edge",
            Cr,
            C,
            "microsoft-edge",
            "microsoft-edge/NativeMessagingHosts",
        ),
        b(
            "Brave",
            Cr,
            C,
            "BraveSoftware/Brave-Browser",
            "BraveSoftware/Brave-Browser/NativeMessagingHosts",
        ),
        b("Vivaldi", Cr, C, "vivaldi", "vivaldi/NativeMessagingHosts"),
        b(
            "Firefox",
            Ff,
            H,
            ".mozilla",
            ".mozilla/native-messaging-hosts",
        ),
    ]
};

#[cfg(windows)]
const BROWSERS: &[Browser] = &[];

/// Windows: browsers find the manifest through a registry key (under HKCU).
pub const REGISTRY: &[(&str, Kind, &str)] = &[
    (
        "Chrome",
        Kind::Chromium,
        r"Software\Google\Chrome\NativeMessagingHosts\app.fuselane.host",
    ),
    (
        "Chromium",
        Kind::Chromium,
        r"Software\Chromium\NativeMessagingHosts\app.fuselane.host",
    ),
    (
        "Edge",
        Kind::Chromium,
        r"Software\Microsoft\Edge\NativeMessagingHosts\app.fuselane.host",
    ),
    (
        "Brave",
        Kind::Chromium,
        r"Software\BraveSoftware\Brave-Browser\NativeMessagingHosts\app.fuselane.host",
    ),
    (
        "Firefox",
        Kind::Firefox,
        r"Software\Mozilla\NativeMessagingHosts\app.fuselane.host",
    ),
];

/// The manifest for one browser family. `extra_ids`: unpacked development builds.
pub fn manifest(kind: Kind, exe: &Path, extra_ids: &[String]) -> String {
    let path = exe.to_string_lossy();
    let v = match kind {
        Kind::Chromium => {
            let origins: Vec<String> = CHROME_EXTENSION_IDS
                .iter()
                .map(|s| s.to_string())
                .chain(extra_ids.iter().cloned())
                .map(|id| format!("chrome-extension://{id}/"))
                .collect();
            json!({
                "name": HOST_NAME,
                "description": "Fuselane: hands big downloads from the browser to the app",
                "path": path,
                "type": "stdio",
                "allowed_origins": origins,
            })
        }
        Kind::Firefox => json!({
            "name": HOST_NAME,
            "description": "Fuselane: hands big downloads from the browser to the app",
            "path": path,
            "type": "stdio",
            "allowed_extensions": [FIREFOX_EXTENSION_ID],
        }),
    };
    let mut s = serde_json::to_string_pretty(&v).unwrap_or_default();
    s.push('\n');
    s
}

/// Chrome extension ids are 32 letters a–p. Anything else is ignored.
pub fn valid_chrome_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|c| (b'a'..=b'p').contains(&c))
}

/// Ids from `FUSELANE_EXTRA_EXTENSION_IDS` (comma separated), for testing an
/// unpacked build. Invalid ones are dropped.
pub fn extra_ids(var: Option<&str>) -> Vec<String> {
    var.unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|id| valid_chrome_id(id))
        .map(String::from)
        .collect()
}

/// The path browsers should start. An AppImage runs from a mount that changes on
/// every launch, so it's the `.AppImage` file itself.
pub fn host_exe(current: PathBuf, appimage: Option<PathBuf>) -> PathBuf {
    appimage.filter(|p| p.is_absolute()).unwrap_or(current)
}

/// Why the manifests can't point at this copy of the app, as a message for the user.
pub fn exe_problem(exe: &Path) -> Option<&'static str> {
    let s = exe.to_string_lossy();
    if !exe.is_absolute() {
        Some("Fuselane couldn't tell where it is installed, so browsers can't find it.")
    } else if s.contains("/AppTranslocation/") || s.starts_with("/Volumes/") {
        Some(
            "Fuselane is running from the disk image or a temporary copy. Drag it into \
             Applications and open it from there so browsers can find it.",
        )
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Written(PathBuf),
    Unchanged,
    NotInstalled,
    Failed(String),
}

/// Writes `contents` unless it's already there, creating the folder if needed.
fn write_if_changed(path: &Path, contents: &str) -> Outcome {
    if std::fs::read_to_string(path).is_ok_and(|old| old == contents) {
        return Outcome::Unchanged;
    }
    let result = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| {
            // Replace atomically: a browser reading mid-write must not see half a file.
            let tmp = path.with_extension("json.tmp");
            std::fs::write(&tmp, contents)?;
            std::fs::rename(&tmp, path)
        });
    match result {
        Ok(()) => Outcome::Written(path.to_path_buf()),
        Err(e) => Outcome::Failed(format!("{}: {e}", path.display())),
    }
}

/// macOS and Linux: one manifest per installed browser. `config` and `home` are
/// passed in so tests never touch the real ones.
pub fn install_files(
    config: &Path,
    home: &Path,
    exe: &Path,
    extra_ids: &[String],
) -> Vec<(&'static str, Outcome)> {
    BROWSERS
        .iter()
        .map(|br| {
            let base = match br.base {
                Base::Config => config,
                Base::Home => home,
            };
            if !base.join(br.profile).is_dir() {
                return (br.name, Outcome::NotInstalled);
            }
            let file = base.join(br.hosts).join(format!("{HOST_NAME}.json"));
            (
                br.name,
                write_if_changed(&file, &manifest(br.kind, exe, extra_ids)),
            )
        })
        .collect()
}

/// Sets a registry key's default value (HKCU). A trait so the Windows logic is
/// tested on every OS.
pub trait Registry {
    fn set_default(&mut self, key: &str, value: &str) -> std::io::Result<()>;
}

/// Windows: the two manifests live in `dir`; each browser's key points at one.
pub fn install_registry(
    dir: &Path,
    exe: &Path,
    extra_ids: &[String],
    reg: &mut dyn Registry,
) -> Vec<(&'static str, Outcome)> {
    let files = [Kind::Chromium, Kind::Firefox].map(|kind| {
        let name = match kind {
            Kind::Chromium => "chrome.json",
            Kind::Firefox => "firefox.json",
        };
        let path = dir.join(name);
        let wrote = write_if_changed(&path, &manifest(kind, exe, extra_ids));
        (kind, path, wrote)
    });
    REGISTRY
        .iter()
        .map(|&(name, kind, key)| {
            let Some((_, path, wrote)) = files.iter().find(|(k, _, _)| *k == kind) else {
                return (name, Outcome::Failed("no manifest".into()));
            };
            if let Outcome::Failed(e) = wrote {
                return (name, Outcome::Failed(e.clone()));
            }
            match reg.set_default(key, &path.to_string_lossy()) {
                Ok(()) => (name, wrote.clone()),
                Err(e) => (name, Outcome::Failed(format!(r"HKCU\{key}: {e}"))),
            }
        })
        .collect()
}

/// Registers `exe` with every installed browser for this user, the way this OS
/// expects. `data` is Fuselane's data folder (Windows keeps the manifests there).
/// An `Err` is a message for the user; per-browser failures are in the list.
pub fn register(exe: &Path, data: &Path) -> Result<Vec<(&'static str, Outcome)>, String> {
    if let Some(problem) = exe_problem(exe) {
        return Err(problem.to_string());
    }
    let extra = extra_ids(
        std::env::var("FUSELANE_EXTRA_EXTENSION_IDS")
            .ok()
            .as_deref(),
    );
    #[cfg(windows)]
    {
        Ok(install_registry(
            &data.join("NativeMessagingHosts"),
            exe,
            &extra,
            &mut WindowsRegistry,
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = data;
        match (dirs::config_dir(), dirs::home_dir()) {
            (Some(config), Some(home)) => Ok(install_files(&config, &home, exe, &extra)),
            _ => Err(
                "couldn't find your home folder, so browsers can't be told where Fuselane is."
                    .into(),
            ),
        }
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub struct WindowsRegistry;

#[cfg(windows)]
impl Registry for WindowsRegistry {
    fn set_default(&mut self, key: &str, value: &str) -> std::io::Result<()> {
        use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, REG_SZ, RegSetKeyValueW};
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (k, v) = (wide(key), wide(value));
        // SAFETY: both buffers are NUL-terminated UTF-16 that outlive the call, and
        // the byte length includes the terminator, as REG_SZ requires.
        let rc = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                k.as_ptr(),
                std::ptr::null(),
                REG_SZ,
                v.as_ptr().cast(),
                u32::try_from(v.len() * 2).unwrap_or(u32::MAX),
            )
        };
        if rc == 0 {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(rc as i32))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const EXE: &str = "/Applications/Fuselane.app/Contents/MacOS/fuselane-desktop";

    #[test]
    fn chrome_manifests_name_the_store_id_and_the_exact_path() {
        let m: Value =
            serde_json::from_str(&manifest(Kind::Chromium, Path::new(EXE), &[])).unwrap();
        assert_eq!(m["name"], HOST_NAME);
        assert_eq!(m["path"], EXE);
        assert_eq!(m["type"], "stdio");
        assert_eq!(
            m["allowed_origins"],
            json!(["chrome-extension://nggljghjikdkigiekdciocigdnnhponl/"])
        );
        assert!(m.get("allowed_extensions").is_none());
    }

    #[test]
    fn firefox_manifests_name_the_gecko_id_only() {
        let m: Value = serde_json::from_str(&manifest(Kind::Firefox, Path::new(EXE), &[])).unwrap();
        assert_eq!(m["allowed_extensions"], json!(["fuselane@fuselane.app"]));
        assert!(m.get("allowed_origins").is_none());
    }

    #[test]
    fn a_development_id_is_added_only_when_it_is_a_real_chrome_id() {
        let ids = extra_ids(Some(
            " abcdefghijklmnopabcdefghijklmnop , short, ABCDEFGHIJKLMNOPABCDEFGHIJKLMNOP, zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz, ../../evil,",
        ));
        assert_eq!(ids, ["abcdefghijklmnopabcdefghijklmnop"]);
        assert!(extra_ids(None).is_empty());
        let m: Value =
            serde_json::from_str(&manifest(Kind::Chromium, Path::new(EXE), &ids)).unwrap();
        assert_eq!(m["allowed_origins"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn an_appimage_points_at_its_file_not_its_temporary_mount() {
        let mount = PathBuf::from("/tmp/.mount_FuselaXyZ/usr/bin/fuselane-desktop");
        let file = PathBuf::from("/home/u/Apps/Fuselane.AppImage");
        assert_eq!(host_exe(mount.clone(), Some(file.clone())), file);
        assert_eq!(host_exe(mount.clone(), None), mount);
        assert_eq!(
            host_exe(mount.clone(), Some("rel.AppImage".into())),
            mount,
            "relative ignored"
        );
    }

    #[test]
    fn copies_that_will_move_or_vanish_are_refused_with_a_reason() {
        assert!(exe_problem(Path::new(EXE)).is_none());
        for p in [
            "/Volumes/Fuselane/Fuselane.app/Contents/MacOS/fuselane-desktop",
            "/private/var/folders/x/AppTranslocation/1A2B/d/Fuselane.app/Contents/MacOS/fuselane-desktop",
            "fuselane-desktop",
        ] {
            assert!(exe_problem(Path::new(p)).is_some(), "{p}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn manifests_go_only_to_installed_browsers_and_are_rewritten_only_when_changed() {
        let d = tempfile::tempdir().unwrap();
        let (config, home) = (d.path().join("config"), d.path().join("home"));
        let installed = &BROWSERS[0];
        let base = |br: &Browser| match br.base {
            Base::Config => config.clone(),
            Base::Home => home.clone(),
        };
        std::fs::create_dir_all(base(installed).join(installed.profile)).unwrap();
        let firefox = BROWSERS.iter().find(|b| b.kind == Kind::Firefox).unwrap();
        std::fs::create_dir_all(base(firefox).join(firefox.profile)).unwrap();

        let first = install_files(&config, &home, Path::new(EXE), &[]);
        for (name, outcome) in &first {
            let want_written = *name == installed.name || *name == "Firefox";
            assert_eq!(
                matches!(outcome, Outcome::Written(_)),
                want_written,
                "{name}: {outcome:?}"
            );
            if !want_written {
                assert_eq!(*outcome, Outcome::NotInstalled, "{name}");
            }
        }
        let file = base(installed)
            .join(installed.hosts)
            .join("app.fuselane.host.json");
        let m: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(m["path"], EXE);
        let ff = base(firefox)
            .join(firefox.hosts)
            .join("app.fuselane.host.json");
        let m: Value = serde_json::from_str(&std::fs::read_to_string(&ff).unwrap()).unwrap();
        assert_eq!(m["allowed_extensions"], json!(["fuselane@fuselane.app"]));

        let again = install_files(&config, &home, Path::new(EXE), &[]);
        assert!(
            again
                .iter()
                .all(|(_, o)| matches!(o, Outcome::Unchanged | Outcome::NotInstalled))
        );

        // The app moved: the manifest follows it.
        let moved = install_files(
            &config,
            &home,
            Path::new("/Users/u/Applications/Fuselane.app/Contents/MacOS/fuselane-desktop"),
            &[],
        );
        assert!(matches!(moved[0].1, Outcome::Written(_)));
        assert!(
            !file.with_extension("json.tmp").exists(),
            "no temp file left"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_cannot_be_written_is_a_failure_not_a_panic() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let br = &BROWSERS[0];
        let root = match br.base {
            Base::Config => d.path().join("config"),
            Base::Home => d.path().join("home"),
        };
        let profile = root.join(br.profile);
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::set_permissions(&profile, std::fs::Permissions::from_mode(0o500)).unwrap();
        let out = install_files(
            &d.path().join("config"),
            &d.path().join("home"),
            Path::new(EXE),
            &[],
        );
        std::fs::set_permissions(&profile, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Root ignores permissions (some CI containers); then it simply succeeds.
        assert!(
            matches!(out[0].1, Outcome::Failed(_) | Outcome::Written(_)),
            "{:?}",
            out[0]
        );
    }

    #[derive(Default)]
    struct FakeRegistry {
        keys: Vec<(String, String)>,
        refuse: Option<&'static str>,
    }

    impl Registry for FakeRegistry {
        fn set_default(&mut self, key: &str, value: &str) -> std::io::Result<()> {
            if self.refuse.is_some_and(|r| key.contains(r)) {
                return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
            }
            self.keys.push((key.into(), value.into()));
            Ok(())
        }
    }

    #[test]
    fn windows_keys_point_each_browser_at_its_family_manifest() {
        let d = tempfile::tempdir().unwrap();
        let exe = Path::new(r"C:\Users\u\AppData\Local\Fuselane\fuselane-desktop.exe");
        let mut reg = FakeRegistry::default();
        let out = install_registry(d.path(), exe, &[], &mut reg);
        assert_eq!(out.len(), REGISTRY.len());
        assert!(
            out.iter().all(|(_, o)| matches!(o, Outcome::Written(_))),
            "{out:?}"
        );
        let chrome = d.path().join("chrome.json").to_string_lossy().into_owned();
        let firefox = d.path().join("firefox.json").to_string_lossy().into_owned();
        for (key, value) in &reg.keys {
            let want = if key.contains("Mozilla") {
                &firefox
            } else {
                &chrome
            };
            assert_eq!(value, want, "{key}");
            assert!(
                key.ends_with(r"\NativeMessagingHosts\app.fuselane.host"),
                "{key}"
            );
        }
        let m: Value = serde_json::from_str(&std::fs::read_to_string(&chrome).unwrap()).unwrap();
        assert_eq!(
            m["path"],
            exe.to_string_lossy().as_ref(),
            "backslashes survive JSON"
        );
    }

    #[test]
    fn a_refused_registry_key_fails_that_browser_only() {
        let d = tempfile::tempdir().unwrap();
        let mut reg = FakeRegistry {
            refuse: Some("Edge"),
            ..Default::default()
        };
        let out = install_registry(d.path(), Path::new(r"C:\f.exe"), &[], &mut reg);
        for (name, o) in &out {
            assert_eq!(
                matches!(o, Outcome::Failed(_)),
                *name == "Edge",
                "{name}: {o:?}"
            );
        }
    }
}
