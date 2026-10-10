//! Running as a Flatpak (packaging/flatpak). The sandbox changes what the app can
//! do for itself: Flathub delivers updates (the app's files are read-only), start
//! at login and keeping awake go through XDG portals, and the browsers' and the
//! CLI's folders are out of reach. Everything here is off outside a Flatpak, and
//! always off on macOS and Windows, so other builds behave exactly as before.
//!
//! The decisions are pure functions of the sandbox's app id, so tests can inject
//! one; [`id`] reads the real one once.

use std::sync::OnceLock;

use crate::automation::WhenDone;
use crate::service::UiError;

/// The manifest's id, used only if `/.flatpak-info` exists but names no app.
const MANIFEST_ID: &str = "app.fuselane.Fuselane";

/// The Flatpak app id when this process runs inside a Flatpak sandbox, read once.
/// Always `None` off Linux.
pub fn id() -> Option<&'static str> {
    static ID: OnceLock<Option<String>> = OnceLock::new();
    ID.get_or_init(|| {
        if !cfg!(target_os = "linux") {
            return None;
        }
        let env = std::env::var("FLATPAK_ID").ok();
        // Flatpak sets FLATPAK_ID; /.flatpak-info covers a cleared environment.
        let info = std::fs::read_to_string("/.flatpak-info").ok();
        detect(env.as_deref(), info.as_deref())
    })
    .as_deref()
}

/// True inside a Flatpak.
pub fn active() -> bool {
    id().is_some()
}

/// The sandbox's app id from `FLATPAK_ID` (`env_id`) or the contents of
/// `/.flatpak-info` (`flatpak_info`, None when the file is missing).
///
/// The id becomes a D-Bus name (the single-instance name), and the single-instance
/// plugin panics on an invalid one, so only valid names are returned.
pub fn detect(env_id: Option<&str>, flatpak_info: Option<&str>) -> Option<String> {
    if let Some(id) = env_id.map(str::trim).filter(|id| valid_id(id)) {
        return Some(id.to_string());
    }
    let info = flatpak_info?;
    // The file only exists inside a sandbox, so it's a Flatpak even without a name.
    Some(
        info_name(info)
            .filter(|id| valid_id(id))
            .unwrap_or(MANIFEST_ID)
            .to_string(),
    )
}

/// `name=` in the `[Application]` group of a `/.flatpak-info` (a keyfile).
fn info_name(info: &str) -> Option<&str> {
    let mut in_app = false;
    for line in info.lines().map(str::trim) {
        if line.starts_with('[') {
            in_app = line == "[Application]";
        } else if in_app && let Some(v) = line.strip_prefix("name=") {
            return Some(v.trim());
        }
    }
    None
}

/// A Flatpak app id that is also a valid D-Bus well-known name: three or more
/// dot-separated parts of `[A-Za-z0-9_-]`, none empty or starting with a digit,
/// with room for the `.SingleInstance` the plugin adds (255 bytes in all).
fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    id.len() + ".SingleInstance".len() <= 255
        && parts.len() >= 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && !p.starts_with(|c: char| c.is_ascii_digit())
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

/// Why the app doesn't update itself in a Flatpak: Flathub (or the remote it was
/// installed from) does, and the app's own files are read-only.
pub fn updates_error(id: Option<&str>) -> Option<UiError> {
    id.map(|_| {
        UiError::new_public(
            "update-flatpak",
            "Updates come through your software centre (Flatpak).",
            Some("Update Fuselane there, or run flatpak update."),
        )
    })
}

/// Sleep and Shut down when everything finishes need `systemctl`, which the sandbox
/// doesn't have, and no portal offers them, so they're refused rather than failing
/// silently an hour later.
pub fn when_done_error(id: Option<&str>, action: WhenDone) -> Option<UiError> {
    match (id, action) {
        (Some(_), WhenDone::Sleep | WhenDone::ShutDown) => Some(UiError::new_public(
            "flatpak-power",
            "The Flatpak version can't put the computer to sleep or shut it down.",
            Some("Pick Quit or Nothing, or use the .deb or AppImage for this."),
        )),
        _ => None,
    }
}

/// Browsers look for the extension's helper in their own folders on the host,
/// which the sandbox can't write, and couldn't start a sandboxed helper anyway.
/// So a Flatpak doesn't register (the window says why instead).
pub fn registers_browsers(id: Option<&str>) -> bool {
    id.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO: &str = "[Application]\nname=app.fuselane.Fuselane\nruntime=runtime/org.gnome.Platform/x86_64/51\n\n[Instance]\ninstance-id=123\n";

    #[test]
    fn outside_a_flatpak_nothing_is_detected() {
        assert_eq!(detect(None, None), None);
        // An empty or invalid FLATPAK_ID without the info file isn't a sandbox.
        assert_eq!(detect(Some(""), None), None);
        assert_eq!(detect(Some("fuselane"), None), None);
    }

    #[test]
    fn the_env_var_names_the_app() {
        assert_eq!(
            detect(Some("app.fuselane.Fuselane"), None).as_deref(),
            Some("app.fuselane.Fuselane")
        );
        // It wins over the file.
        assert_eq!(
            detect(Some("io.github.someone.Fuselane"), Some(INFO)).as_deref(),
            Some("io.github.someone.Fuselane")
        );
    }

    #[test]
    fn the_info_file_is_enough() {
        assert_eq!(
            detect(None, Some(INFO)).as_deref(),
            Some("app.fuselane.Fuselane")
        );
        // name= in another group doesn't count; the file still means a sandbox.
        let other = "[Instance]\nname=not.this.one\n";
        assert_eq!(
            detect(None, Some(other)).as_deref(),
            Some("app.fuselane.Fuselane")
        );
        assert_eq!(
            detect(Some("bad id"), Some("")).as_deref(),
            Some("app.fuselane.Fuselane")
        );
    }

    #[test]
    fn only_valid_dbus_names_pass() {
        assert!(valid_id("app.fuselane.Fuselane"));
        assert!(valid_id("io.github.some-one.Fuse_lane"));
        assert!(!valid_id("app.fuselane"), "Flatpak needs three parts");
        assert!(!valid_id("app..Fuselane"));
        assert!(!valid_id("app.9fuselane.Fuselane"));
        assert!(!valid_id("app.fuse lane.Fuselane"));
        assert!(!valid_id("app.fuselane.Fuselane/x"));
        assert!(!valid_id(&format!("a.b.{}", "c".repeat(250))));
    }

    #[test]
    fn updates_are_refused_only_in_a_flatpak() {
        assert!(updates_error(None).is_none());
        let e = updates_error(Some("app.fuselane.Fuselane")).expect("refused");
        assert_eq!(e.code, "update-flatpak");
        assert!(e.message.contains("software centre"));
        assert!(e.hint.is_some());
    }

    #[test]
    fn sleep_and_shut_down_are_refused_only_in_a_flatpak() {
        let id = Some("app.fuselane.Fuselane");
        for a in [
            WhenDone::Nothing,
            WhenDone::Sleep,
            WhenDone::ShutDown,
            WhenDone::Quit,
        ] {
            assert!(when_done_error(None, a).is_none());
        }
        assert!(when_done_error(id, WhenDone::Sleep).is_some());
        assert!(when_done_error(id, WhenDone::ShutDown).is_some());
        assert!(when_done_error(id, WhenDone::Quit).is_none());
        assert!(when_done_error(id, WhenDone::Nothing).is_none());
    }

    #[test]
    fn browsers_are_registered_only_outside_a_flatpak() {
        assert!(registers_browsers(None));
        assert!(!registers_browsers(Some("app.fuselane.Fuselane")));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn never_a_flatpak_off_linux() {
        assert_eq!(id(), None);
        assert!(!active());
    }
}
