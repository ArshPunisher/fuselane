//! Which browsers this computer has, and whether each one has Fuselane's
//! extension installed and turned on (BROWSER-EXTENSION.md §7). The app's
//! Browser extension page shows this so people can add it where it's missing.
//!
//! Read-only: Chromium browsers keep each profile's extensions in its
//! `Preferences` / `Secure Preferences` file (`extensions.settings.<id>`);
//! Firefox keeps them in each profile's `extensions.json`. Nothing here writes to
//! a browser's folder, and every path comes in as an argument so tests never touch
//! the real ones.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::hosts::{CHROME_EXTENSION_IDS, FIREFOX_EXTENSION_ID};

/// Where a browser installs extensions from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    Chrome,
    Edge,
    Firefox,
}

impl Store {
    /// Fuselane's page in that store.
    pub fn url(self) -> &'static str {
        match self {
            Store::Chrome => {
                "https://chromewebstore.google.com/detail/fuselane/nggljghjikdkigiekdciocigdnnhponl"
            }
            Store::Edge => {
                "https://microsoftedge.microsoft.com/addons/detail/fuselane/aeacngcllkfecpfoclekcfhpokjhijgc"
            }
            Store::Firefox => "https://addons.mozilla.org/firefox/addon/fuselane/",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Store::Chrome => "chrome",
            Store::Edge => "edge",
            Store::Firefox => "firefox",
        }
    }
}

/// The extension's state in one browser, over all of its profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtState {
    /// No profile has it.
    Missing,
    /// Installed, but turned off in every profile that has it.
    Off,
    /// Installed and turned on in at least one profile.
    On,
}

impl ExtState {
    pub fn word(self) -> &'static str {
        match self {
            ExtState::Missing => "missing",
            ExtState::Off => "off",
            ExtState::On => "on",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Chromium,
    Firefox,
}

/// A browser Fuselane knows how to look at and open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Known {
    /// Stable id the UI and the extension use ("chrome", "edge", …).
    pub id: &'static str,
    pub name: &'static str,
    family: Family,
    pub store: Store,
    /// The browser's own extensions page, for turning it back on.
    pub extensions_page: &'static str,
}

pub const KNOWN: &[Known] = &[
    Known {
        id: "chrome",
        name: "Chrome",
        family: Family::Chromium,
        store: Store::Chrome,
        extensions_page: "chrome://extensions/",
    },
    Known {
        id: "edge",
        name: "Microsoft Edge",
        family: Family::Chromium,
        store: Store::Edge,
        extensions_page: "edge://extensions/",
    },
    Known {
        id: "brave",
        name: "Brave",
        family: Family::Chromium,
        store: Store::Chrome,
        extensions_page: "brave://extensions/",
    },
    Known {
        id: "vivaldi",
        name: "Vivaldi",
        family: Family::Chromium,
        store: Store::Chrome,
        extensions_page: "vivaldi://extensions/",
    },
    Known {
        id: "chromium",
        name: "Chromium",
        family: Family::Chromium,
        store: Store::Chrome,
        extensions_page: "chrome://extensions/",
    },
    Known {
        id: "firefox",
        name: "Firefox",
        family: Family::Firefox,
        store: Store::Firefox,
        extensions_page: "about:addons",
    },
];

pub fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

/// Each browser's user-data folder, relative to a base the caller passes in:
/// `~/Library/Application Support` (macOS), `~/.config` and `~` (Linux),
/// `%LOCALAPPDATA%` and `%APPDATA%` (Windows).
#[derive(Debug, Clone, Copy)]
pub enum Base {
    /// macOS Application Support, Linux `~/.config`, Windows `%LOCALAPPDATA%`.
    Local,
    /// Linux home (Firefox), Windows `%APPDATA%` (Firefox).
    Roaming,
}

#[cfg(target_os = "macos")]
const ROOTS: &[(&str, Base, &str)] = &[
    ("chrome", Base::Local, "Google/Chrome"),
    ("edge", Base::Local, "Microsoft Edge"),
    ("brave", Base::Local, "BraveSoftware/Brave-Browser"),
    ("vivaldi", Base::Local, "Vivaldi"),
    ("chromium", Base::Local, "Chromium"),
    ("firefox", Base::Local, "Firefox/Profiles"),
];

#[cfg(all(unix, not(target_os = "macos")))]
const ROOTS: &[(&str, Base, &str)] = &[
    ("chrome", Base::Local, "google-chrome"),
    ("edge", Base::Local, "microsoft-edge"),
    ("brave", Base::Local, "BraveSoftware/Brave-Browser"),
    ("vivaldi", Base::Local, "vivaldi"),
    ("chromium", Base::Local, "chromium"),
    ("firefox", Base::Roaming, ".mozilla/firefox"),
];

#[cfg(windows)]
const ROOTS: &[(&str, Base, &str)] = &[
    ("chrome", Base::Local, r"Google\Chrome\User Data"),
    ("edge", Base::Local, r"Microsoft\Edge\User Data"),
    (
        "brave",
        Base::Local,
        r"BraveSoftware\Brave-Browser\User Data",
    ),
    ("vivaldi", Base::Local, r"Vivaldi\User Data"),
    ("chromium", Base::Local, r"Chromium\User Data"),
    ("firefox", Base::Roaming, r"Mozilla\Firefox\Profiles"),
];

/// One browser found on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub known: &'static Known,
    pub state: ExtState,
}

/// The browsers that have run on this computer (their data folder exists), with
/// the extension's state in each. `local` and `roaming` are the two bases above.
pub fn scan(local: &Path, roaming: &Path) -> Vec<Found> {
    ROOTS
        .iter()
        .filter_map(|&(id, base, rel)| {
            let root = match base {
                Base::Local => local,
                Base::Roaming => roaming,
            }
            .join(rel);
            if !root.is_dir() {
                return None;
            }
            let known = known(id)?;
            let state = match known.family {
                Family::Chromium => chromium_state(&root),
                Family::Firefox => firefox_state(&root),
            };
            Some(Found { known, state })
        })
        .collect()
}

/// The base folders for this computer, or None if the OS doesn't say.
pub fn bases() -> Option<(PathBuf, PathBuf)> {
    #[cfg(target_os = "macos")]
    {
        let support = dirs::config_dir()?;
        Some((support.clone(), support))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Some((dirs::config_dir()?, dirs::home_dir()?))
    }
    #[cfg(windows)]
    {
        Some((dirs::data_local_dir()?, dirs::data_dir()?))
    }
}

/// Folders right under `root` (profiles), skipping anything unreadable.
fn subdirs(root: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(root)
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

/// Reads a JSON file no bigger than 32 MiB (a real Preferences file is far smaller).
fn read_json(path: &Path) -> Option<Value> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > 32 * 1024 * 1024 {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// One Chromium extension entry: on, off, or not there. Newer Chrome marks a
/// turned-off extension with a non-empty `disable_reasons` (a list, or a bit mask
/// in older versions); older ones with `"state": 0`.
pub fn chromium_entry(entry: Option<&Value>) -> ExtState {
    let Some(e) = entry.filter(|e| e.is_object()) else {
        return ExtState::Missing;
    };
    let reasons = match e.get("disable_reasons") {
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Number(n)) => n.as_u64().is_some_and(|n| n != 0),
        _ => false,
    };
    let state_off = e.get("state").and_then(Value::as_u64) == Some(0);
    if reasons || state_off {
        ExtState::Off
    } else {
        ExtState::On
    }
}

fn merge(a: ExtState, b: ExtState) -> ExtState {
    use ExtState::*;
    match (a, b) {
        (On, _) | (_, On) => On,
        (Off, _) | (_, Off) => Off,
        _ => Missing,
    }
}

/// The extension's state over every profile of a Chromium browser.
pub fn chromium_state(root: &Path) -> ExtState {
    let mut state = ExtState::Missing;
    for profile in subdirs(root) {
        for file in ["Secure Preferences", "Preferences"] {
            let Some(prefs) = read_json(&profile.join(file)) else {
                continue;
            };
            let settings = prefs.pointer("/extensions/settings");
            for id in CHROME_EXTENSION_IDS {
                state = merge(state, chromium_entry(settings.and_then(|s| s.get(*id))));
            }
        }
    }
    state
}

/// The extension's state in one Firefox `extensions.json`.
pub fn firefox_entry(json: &Value) -> ExtState {
    let Some(addon) = json.get("addons").and_then(Value::as_array).and_then(|a| {
        a.iter()
            .find(|x| x.get("id").and_then(Value::as_str) == Some(FIREFOX_EXTENSION_ID))
    }) else {
        return ExtState::Missing;
    };
    let flag = |k: &str| addon.get(k).and_then(Value::as_bool);
    if flag("active") == Some(true) && flag("userDisabled") != Some(true) {
        ExtState::On
    } else {
        ExtState::Off
    }
}

/// The extension's state over every Firefox profile.
pub fn firefox_state(root: &Path) -> ExtState {
    subdirs(root)
        .iter()
        .filter_map(|p| read_json(&p.join("extensions.json")))
        .map(|j| firefox_entry(&j))
        .fold(ExtState::Missing, merge)
}

/// Which browser and store a native-messaging caller is, from how the browser
/// started the host: `chrome-extension://<id>/` or Firefox's extension id.
pub fn caller_store(args: &[String]) -> Option<Store> {
    args.iter().skip(1).find_map(|a| {
        if a == FIREFOX_EXTENSION_ID {
            return Some(Store::Firefox);
        }
        let id = a.strip_prefix("chrome-extension://")?.trim_end_matches('/');
        if id == crate::hosts::EDGE_STORE_ID {
            Some(Store::Edge)
        } else if CHROME_EXTENSION_IDS.contains(&id) {
            Some(Store::Chrome)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CHROME_ID: &str = crate::hosts::CHROME_STORE_ID;
    const EDGE_ID: &str = crate::hosts::EDGE_STORE_ID;

    fn write(path: &Path, v: &Value) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(v).unwrap()).unwrap();
    }

    #[test]
    fn chromium_entries_read_both_old_and_new_ways_of_turning_off() {
        assert_eq!(chromium_entry(None), ExtState::Missing);
        assert_eq!(chromium_entry(Some(&json!(null))), ExtState::Missing);
        assert_eq!(chromium_entry(Some(&json!({"path": "x"}))), ExtState::On);
        assert_eq!(
            chromium_entry(Some(&json!({"disable_reasons": []}))),
            ExtState::On
        );
        assert_eq!(
            chromium_entry(Some(&json!({"disable_reasons": [1]}))),
            ExtState::Off
        );
        assert_eq!(
            chromium_entry(Some(&json!({"disable_reasons": 1}))),
            ExtState::Off
        );
        assert_eq!(chromium_entry(Some(&json!({"state": 0}))), ExtState::Off);
        assert_eq!(chromium_entry(Some(&json!({"state": 1}))), ExtState::On);
    }

    #[test]
    fn any_profile_with_it_on_counts_as_on() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Google/Chrome");
        write(
            &root.join("Default/Secure Preferences"),
            &json!({"extensions": {"settings": {CHROME_ID: {"disable_reasons": [1]}}}}),
        );
        assert_eq!(chromium_state(&root), ExtState::Off);
        write(
            &root.join("Profile 2/Preferences"),
            &json!({"extensions": {"settings": {CHROME_ID: {"state": 1}}}}),
        );
        assert_eq!(chromium_state(&root), ExtState::On);
    }

    #[test]
    fn edge_counts_the_edge_store_copy_and_ignores_other_extensions() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Microsoft Edge");
        write(
            &root.join("Default/Preferences"),
            &json!({"extensions": {"settings": {"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa": {"state": 1}}}}),
        );
        assert_eq!(chromium_state(&root), ExtState::Missing);
        write(
            &root.join("Default/Secure Preferences"),
            &json!({"extensions": {"settings": {EDGE_ID: {"disable_reasons": []}}}}),
        );
        assert_eq!(chromium_state(&root), ExtState::On);
    }

    #[test]
    fn broken_or_huge_files_are_skipped_not_fatal() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("chrome");
        std::fs::create_dir_all(root.join("Default")).unwrap();
        std::fs::write(root.join("Default/Preferences"), b"{not json").unwrap();
        std::fs::write(root.join("Default/Secure Preferences"), b"").unwrap();
        assert_eq!(chromium_state(&root), ExtState::Missing);
        assert_eq!(chromium_state(&d.path().join("nowhere")), ExtState::Missing);
    }

    #[test]
    fn firefox_needs_it_active_and_not_turned_off() {
        let on = json!({"addons": [{"id": "fuselane@fuselane.app", "active": true, "userDisabled": false}]});
        let off = json!({"addons": [{"id": "fuselane@fuselane.app", "active": false, "userDisabled": true}]});
        let other = json!({"addons": [{"id": "other@x", "active": true}]});
        assert_eq!(firefox_entry(&on), ExtState::On);
        assert_eq!(firefox_entry(&off), ExtState::Off);
        assert_eq!(firefox_entry(&other), ExtState::Missing);
        assert_eq!(firefox_entry(&json!({})), ExtState::Missing);

        let d = tempfile::tempdir().unwrap();
        write(&d.path().join("abc.default/extensions.json"), &off);
        assert_eq!(firefox_state(d.path()), ExtState::Off);
        write(&d.path().join("xyz.default-release/extensions.json"), &on);
        assert_eq!(firefox_state(d.path()), ExtState::On);
    }

    #[test]
    fn only_browsers_that_have_run_are_listed() {
        let d = tempfile::tempdir().unwrap();
        let (local, roaming) = (d.path().join("local"), d.path().join("roaming"));
        std::fs::create_dir_all(&local).unwrap();
        std::fs::create_dir_all(&roaming).unwrap();
        assert!(scan(&local, &roaming).is_empty());
        let (_, base, rel) = ROOTS.iter().find(|r| r.0 == "edge").unwrap();
        let root = match base {
            Base::Local => &local,
            Base::Roaming => &roaming,
        }
        .join(rel);
        write(
            &root.join("Default/Preferences"),
            &json!({"extensions": {"settings": {EDGE_ID: {}}}}),
        );
        let found = scan(&local, &roaming);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].known.id, "edge");
        assert_eq!(found[0].state, ExtState::On);
    }

    #[test]
    fn the_host_knows_which_store_started_it() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            caller_store(&a(&["f", &format!("chrome-extension://{CHROME_ID}/")])),
            Some(Store::Chrome)
        );
        assert_eq!(
            caller_store(&a(&[
                "f",
                &format!("chrome-extension://{EDGE_ID}/"),
                "--parent-window=1"
            ])),
            Some(Store::Edge)
        );
        assert_eq!(
            caller_store(&a(&[
                "f",
                "/x/app.fuselane.host.json",
                "fuselane@fuselane.app"
            ])),
            Some(Store::Firefox)
        );
        assert_eq!(
            caller_store(&a(&[
                "f",
                "chrome-extension://abcdefghijklmnopabcdefghijklmnop/"
            ])),
            None
        );
        assert_eq!(caller_store(&a(&["f"])), None);
    }

    #[test]
    fn every_known_browser_has_a_root_and_a_store_page() {
        for k in KNOWN {
            assert!(ROOTS.iter().any(|r| r.0 == k.id), "{} has no root", k.id);
            assert!(k.store.url().starts_with("https://"));
        }
    }

    /// Read-only look at this computer's browsers: `cargo test -p fuselane-api
    /// this_computer -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn this_computer() {
        let (local, roaming) = bases().unwrap();
        for f in scan(&local, &roaming) {
            println!("{:<16} {}", f.known.name, f.state.word());
        }
    }
}
