//! Where Fuselane keeps its download list. The CLI and the desktop app share it, so a
//! download started in one can be resumed in the other.

use std::path::PathBuf;

use crate::{Store, StoreError};

/// The list's file name inside the home folder.
pub const DB_FILE: &str = "jobs.db";

/// Why the shared list couldn't be opened. Typed, so the desktop app can tell a
/// list from a newer Fuselane (offer the update) from a folder or disk problem
/// without matching on the words. The messages are the ones the CLI always printed.
#[derive(Debug, thiserror::Error)]
pub enum OpenError {
    /// The OS has no app-data folder (no home folder, a broken environment).
    #[error("{0}")]
    NoHome(String),
    /// The home folder couldn't be created.
    #[error("couldn't create {}: {source}", dir.display())]
    Folder {
        dir: PathBuf,
        source: std::io::Error,
    },
    /// The list itself: newer, busy, unreadable, a full disk…
    #[error("couldn't open the download list: {0}")]
    Store(#[from] StoreError),
}

impl OpenError {
    /// `(found, known)` schema versions when the list was written by a newer
    /// Fuselane: the person needs the update, and nothing is wrong with the list.
    pub fn newer(&self) -> Option<(i64, i64)> {
        match self {
            OpenError::Store(StoreError::TooNew { found, known }) => Some((*found, *known)),
            _ => None,
        }
    }
}

/// `FUSELANE_HOME` if set, else the OS app-data folder.
pub fn home() -> Result<PathBuf, String> {
    if let Some(h) = std::env::var_os("FUSELANE_HOME") {
        return Ok(PathBuf::from(h));
    }
    let base = dirs::data_dir().ok_or("couldn't find the app-data folder")?;
    Ok(base.join(if cfg!(target_os = "macos") {
        "app.fuselane"
    } else if cfg!(windows) {
        "Fuselane"
    } else {
        "fuselane"
    }))
}

/// Opens the shared list, creating its folder. A damaged list is moved aside and a
/// fresh one started; `Store::recovered_from` says where the old file went.
pub fn open_default() -> Result<Store, OpenError> {
    let dir = home().map_err(OpenError::NoHome)?;
    std::fs::create_dir_all(&dir).map_err(|source| OpenError::Folder {
        dir: dir.clone(),
        source,
    })?;
    Ok(Store::open(&dir.join(DB_FILE))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_list_is_told_apart_from_other_problems() {
        let newer = OpenError::Store(StoreError::TooNew {
            found: 15,
            known: 13,
        });
        assert_eq!(newer.newer(), Some((15, 13)));
        // The CLI's words are unchanged.
        assert_eq!(
            newer.to_string(),
            "couldn't open the download list: the database was made by a newer Fuselane (schema v15, this build knows v13)"
        );
        let folder = OpenError::Folder {
            dir: PathBuf::from("/x"),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
        assert_eq!(folder.newer(), None);
        assert!(folder.to_string().starts_with("couldn't create /x: "));
        assert_eq!(OpenError::NoHome("no".into()).newer(), None);
        assert_eq!(OpenError::Store(StoreError::NotFound(1)).newer(), None);
    }

    #[test]
    fn a_newer_list_on_disk_comes_back_typed_and_untouched() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(DB_FILE);
        {
            let c = rusqlite::Connection::open(&p).unwrap();
            c.pragma_update(None, "user_version", 99).unwrap();
        }
        let before = std::fs::read(&p).unwrap();
        let e = Store::open(&p)
            .map(drop)
            .map_err(OpenError::from)
            .unwrap_err();
        assert_eq!(e.newer().map(|(found, _)| found), Some(99));
        assert_eq!(
            std::fs::read(&p).unwrap(),
            before,
            "the list is never changed"
        );
    }
}
