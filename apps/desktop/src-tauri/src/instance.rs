//! One Fuselane per data folder.
//!
//! Opening the download list runs crash recovery, which marks every running and
//! queued download as paused. A second launch (a magnet, a .torrent or a
//! fuselane:// link opened on Windows or Linux, or the app started again) used
//! to open the list before handing itself over to the running Fuselane, so it
//! paused all of that Fuselane's downloads. Now the first launch holds a lock in
//! the data folder for its whole life, and a launch that can't get it never
//! opens the list: it only hands its arguments over and exits.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;

/// Whether this launch may run the download list.
#[derive(Debug)]
pub enum Instance {
    /// This launch holds the lock; keep it alive until the process ends.
    Mine(File),
    /// Another Fuselane on this data folder is running.
    Elsewhere,
}

/// Claims the data folder `home` for this process.
pub fn claim(home: &Path) -> std::io::Result<Instance> {
    let dir = home.join("locks");
    std::fs::create_dir_all(&dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("app.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Instance::Mine(file)),
        Err(TryLockError::WouldBlock) => Ok(Instance::Elsewhere),
        Err(TryLockError::Error(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn only_the_first_launch_runs_the_list_until_it_ends() {
        let home = tempfile::tempdir().unwrap();
        let first = claim(home.path()).unwrap();
        assert!(matches!(first, Instance::Mine(_)));
        assert!(matches!(claim(home.path()).unwrap(), Instance::Elsewhere));
        drop(first);
        assert!(matches!(claim(home.path()).unwrap(), Instance::Mine(_)));
    }

    #[test]
    fn other_data_folders_are_independent() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let _a = claim(a.path()).unwrap();
        assert!(matches!(claim(b.path()).unwrap(), Instance::Mine(_)));
    }
}
