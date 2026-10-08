//! Where Fuselane keeps its download list. The CLI and the desktop app share it, so a
//! download started in one can be resumed in the other.

use std::path::PathBuf;

use crate::Store;

/// The list's file name inside the home folder.
pub const DB_FILE: &str = "jobs.db";

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
pub fn open_default() -> Result<Store, String> {
    let dir = home()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("couldn't create {}: {e}", dir.display()))?;
    Store::open(&dir.join(DB_FILE)).map_err(|e| format!("couldn't open the download list: {e}"))
}
