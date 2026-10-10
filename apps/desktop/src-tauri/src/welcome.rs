//! The welcome (guided first run): shown once on a fresh install. Someone who
//! already has downloads (updating from an earlier version) never sees it.

use crate::service::{Service, UiError};

const SETTING: &str = "welcome_seen";

/// Whether the welcome has been seen (or isn't needed).
pub fn seen(svc: &Service) -> bool {
    if svc.store().setting(SETTING).ok().flatten().as_deref() == Some("true") {
        return true;
    }
    let existing = svc.jobs().map(|j| !j.is_empty()).unwrap_or(false);
    if existing {
        let _ = svc.store().set_setting(SETTING, "true");
    }
    existing
}

pub fn set_seen(svc: &Service, seen: bool) -> Result<(), UiError> {
    svc.store()
        .set_setting(SETTING, if seen { "true" } else { "false" })
        .map_err(|e| {
            UiError::new_public(
                "store",
                format!("Fuselane couldn't save that: {e}"),
                Some("Check that your disk has free space, then try again."),
            )
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn shown_once_on_a_fresh_install_and_never_to_people_updating() {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("db")).unwrap();
        let svc = Service::new(store, dir.path().to_path_buf()).unwrap();
        assert!(!seen(&svc), "a fresh install shows it");
        set_seen(&svc, true).unwrap();
        assert!(seen(&svc));
        set_seen(&svc, false).unwrap();
        assert!(!seen(&svc), "Settings can show it again");

        // Someone updating already has downloads: not shown, and remembered.
        let dir2 = tempfile::tempdir().unwrap();
        let store2 = fuselane_core::Store::open(&dir2.path().join("db")).unwrap();
        let svc2 = Service::new(store2, dir2.path().to_path_buf()).unwrap();
        svc2.add_with(
            "https://example.com/a.iso",
            None,
            &crate::service::AddRequest {
                later: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(seen(&svc2));
        assert_eq!(
            svc2.store().setting(SETTING).unwrap().as_deref(),
            Some("true")
        );
    }
}
