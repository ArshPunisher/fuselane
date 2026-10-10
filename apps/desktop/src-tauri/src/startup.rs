//! When the download list can't be opened at launch, the app still opens a
//! window instead of exiting silently: launched from Finder or the Start menu,
//! nobody sees stderr. The window explains the problem and, when the list was
//! written by a newer Fuselane, offers the update through the normal updater,
//! so nobody has to reinstall by hand.
//!
//! This "problem mode" starts nothing else: no service, torrents, Nearby,
//! feeds, remote control, tray or browser registration, and it never opens the
//! database again.

use std::path::PathBuf;
use std::sync::Arc;

use fuselane_core::OpenError;
use fuselane_core::StoreError;
use serde::Serialize;
use tauri::Manager;
use tauri::ipc::Channel;

use crate::service::UiError;
use crate::update::UpdateProgress;

/// Why the normal app couldn't start.
#[derive(Debug)]
pub enum Failure {
    /// The shared list (or its folder) couldn't be opened.
    Open(OpenError),
    /// The list opened, but the service refused it.
    Service(UiError),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Open(e) => e.fmt(f),
            Failure::Service(e) => e.message.fmt(f),
        }
    }
}

/// What the window shows. `message` and `hint` are sentences from this
/// module, so the window can translate them; `detail` is the error as the
/// system gave it, for "Copy details".
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// newer | other
    pub kind: &'static str,
    pub message: String,
    pub hint: Option<String>,
    pub detail: String,
    /// This copy's version.
    pub version: &'static str,
    /// Fuselane's data folder, when there is one to show.
    pub home: Option<String>,
}

const FULL: (&str, &str) = (
    "The drive with Fuselane's folder is full.",
    "Free some space on it, then open Fuselane again.",
);
const DENIED: (&str, &str) = (
    "Fuselane isn't allowed to change its folder.",
    "Check that your account can write to the folder, then open Fuselane again.",
);
const BUSY: (&str, &str) = (
    "Another program is using the download list.",
    "Quit any other copy of Fuselane or the fuselane command, then open Fuselane again.",
);
const DAMAGED: (&str, &str) = (
    "The download list is damaged and couldn't be moved aside.",
    "Open the folder and move jobs.db somewhere else. Fuselane starts a new list next time.",
);
const UNKNOWN: (&str, &str) = (
    "Fuselane couldn't open your download list.",
    "Open the folder to check it, then open Fuselane again. Copy the details if you ask for help.",
);

fn io_words(e: &std::io::Error) -> (&'static str, &'static str) {
    use std::io::ErrorKind::{PermissionDenied, ReadOnlyFilesystem, StorageFull};
    match e.kind() {
        StorageFull => FULL,
        PermissionDenied | ReadOnlyFilesystem => DENIED,
        _ => UNKNOWN,
    }
}

fn store_words(e: &StoreError) -> (&'static str, &'static str) {
    use rusqlite::ErrorCode::{
        CannotOpen, DatabaseBusy, DatabaseCorrupt, DatabaseLocked, DiskFull, NotADatabase,
        PermissionDenied, ReadOnly,
    };
    match e {
        StoreError::Io(e) => io_words(e),
        StoreError::Sql(e) => match e.sqlite_error_code() {
            Some(DiskFull) => FULL,
            Some(ReadOnly | PermissionDenied | CannotOpen) => DENIED,
            Some(DatabaseBusy | DatabaseLocked) => BUSY,
            Some(DatabaseCorrupt | NotADatabase) => DAMAGED,
            _ => UNKNOWN,
        },
        _ => UNKNOWN,
    }
}

/// Sorts a startup failure into what the window says and offers. Pure, so
/// every case is tested without a window.
pub fn classify(failure: &Failure, home: Option<PathBuf>) -> Problem {
    let detail = failure.to_string();
    let (kind, message, hint) = match failure {
        Failure::Open(e) if e.newer().is_some() => (
            "newer",
            "This download list was made by a newer Fuselane.".to_string(),
            Some("Update Fuselane to keep going. Your downloads are kept.".to_string()),
        ),
        Failure::Open(OpenError::NoHome(_)) => (
            "other",
            "Fuselane couldn't find this computer's app data folder.".to_string(),
            Some(
                "Set FUSELANE_HOME to a folder you can write to, then open Fuselane again."
                    .to_string(),
            ),
        ),
        Failure::Open(e) => {
            let (m, h) = match e {
                OpenError::Folder { source, .. } => io_words(source),
                OpenError::Store(s) => store_words(s),
                OpenError::NoHome(_) => UNKNOWN,
            };
            ("other", m.to_string(), Some(h.to_string()))
        }
        Failure::Service(e) => ("other", e.message.clone(), e.hint.clone()),
    };
    Problem {
        kind,
        message,
        hint,
        detail,
        version: env!("CARGO_PKG_VERSION"),
        home: home.map(|h| h.to_string_lossy().into_owned()),
    }
}

/// What "Copy details" puts on the clipboard: enough for a bug report, with
/// no download links or names in it.
fn details(p: &Problem) -> String {
    format!(
        "Fuselane {} ({} {})\nProblem at launch: {}\nFolder: {}\n",
        p.version,
        std::env::consts::OS,
        std::env::consts::ARCH,
        p.detail,
        p.home.as_deref().unwrap_or("unknown"),
    )
}

/// The problem the window should show instead of the app, if any. Registered
/// in both modes, so the window asks once and needs no other hint.
#[tauri::command]
pub fn startup_problem(app: tauri::AppHandle) -> Option<Problem> {
    app.try_state::<Problem>().map(|p| p.inner().clone())
}

/// Shows Fuselane's data folder in the file manager.
#[tauri::command]
fn startup_reveal_home(
    app: tauri::AppHandle,
    problem: tauri::State<'_, Problem>,
) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let Some(home) = &problem.home else {
        return Err(UiError::new_public(
            "no-folder",
            "Fuselane has no folder to show.",
            None,
        ));
    };
    let folder = nearest_existing(std::path::Path::new(home));
    app.opener()
        .open_path(folder.to_string_lossy(), None::<&str>)
        .map_err(|e| {
            UiError::new_public(
                "open-failed",
                format!("Couldn't open the folder: {e}"),
                Some("It may not exist yet. Copy the details to see where it should be."),
            )
        })
}

/// The folder itself, or the closest folder above it that exists: when
/// Fuselane couldn't create its folder, the person lands where it should be.
fn nearest_existing(path: &std::path::Path) -> &std::path::Path {
    path.ancestors().find(|p| p.is_dir()).unwrap_or(path)
}

/// Opens the download page, for when the in-app update can't help.
#[tauri::command]
fn startup_get_fuselane(app: tauri::AppHandle) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(DOWNLOAD_PAGE, None::<&str>)
        .map_err(|e| {
            UiError::new_public(
                "open-failed",
                format!("Couldn't open the browser: {e}"),
                Some("Go to fuselane.app/download in your browser."),
            )
        })
}

/// The site's download page (apps/site/download).
pub const DOWNLOAD_PAGE: &str = "https://fuselane.app/download/";

#[tauri::command]
fn startup_copy_details(
    app: tauri::AppHandle,
    problem: tauri::State<'_, Problem>,
) -> Result<(), UiError> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard()
        .write_text(details(&problem))
        .map_err(|e| UiError::new_public("copy-failed", format!("Couldn't copy: {e}"), None))
}

/// Downloads, checks and installs the update, then restarts into it. Progress
/// goes to `progress`, as the banner's does through the event channel.
#[tauri::command]
async fn startup_install_update(
    app: tauri::AppHandle,
    progress: Channel<UpdateProgress>,
) -> Result<(), UiError> {
    let send: crate::ProgressSink = Arc::new(move |p| {
        let _ = progress.send(p);
    });
    crate::install_with(&app, send, || async {}).await
}

#[tauri::command]
fn startup_quit(app: tauri::AppHandle) {
    app.exit(0);
}

/// Runs the problem window until it is closed (or an update restarts it).
pub fn run(problem: Problem, context: tauri::Context<tauri::Wry>) {
    let mut single_instance =
        tauri_plugin_single_instance::Builder::new().callback(|app, _argv, _cwd| {
            crate::show_main(app);
        });
    if let Some(id) = crate::flatpak::id() {
        single_instance = single_instance.dbus_id(id);
    }
    let result = tauri::Builder::default()
        // First, as in the normal app: a second launch focuses this window.
        .plugin(single_instance.build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(problem)
        .setup(|app| {
            // Started at login (--minimized) or not, the person has to see this.
            crate::show_main(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            startup_problem,
            startup_reveal_home,
            startup_get_fuselane,
            startup_copy_details,
            startup_install_update,
            startup_quit,
            crate::check_update,
            crate::cancel_update,
        ])
        .build(context);
    match result {
        Ok(app) => app.run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = &event {
                crate::show_main(app);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, &event);
        }),
        Err(e) => {
            eprintln!("fuselane: the window couldn't start: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(e: OpenError) -> Problem {
        classify(&Failure::Open(e), Some(PathBuf::from("/home/fuselane")))
    }

    #[test]
    fn a_newer_list_offers_the_update() {
        let p = open(OpenError::Store(StoreError::TooNew {
            found: 15,
            known: 13,
        }));
        assert_eq!(p.kind, "newer");
        assert_eq!(p.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(p.home.as_deref(), Some("/home/fuselane"));
        assert!(p.detail.contains("schema v15"), "{}", p.detail);
    }

    #[test]
    fn folder_and_disk_problems_say_what_to_do() {
        let io = |k| {
            open(OpenError::Folder {
                dir: PathBuf::from("/x"),
                source: std::io::Error::from(k),
            })
        };
        let p = io(std::io::ErrorKind::PermissionDenied);
        assert_eq!((p.kind, p.message.as_str()), ("other", DENIED.0));
        assert!(p.detail.starts_with("couldn't create /x"), "{}", p.detail);
        assert_eq!(io(std::io::ErrorKind::ReadOnlyFilesystem).message, DENIED.0);
        assert_eq!(io(std::io::ErrorKind::StorageFull).message, FULL.0);
        assert_eq!(io(std::io::ErrorKind::Other).message, UNKNOWN.0);

        let sql = |code, ext| {
            open(OpenError::Store(StoreError::Sql(
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code,
                        extended_code: ext,
                    },
                    None,
                ),
            )))
        };
        use rusqlite::ErrorCode as C;
        use rusqlite::ffi;
        assert_eq!(sql(C::DiskFull, ffi::SQLITE_FULL).message, FULL.0);
        assert_eq!(sql(C::DatabaseBusy, ffi::SQLITE_BUSY).message, BUSY.0);
        assert_eq!(sql(C::ReadOnly, ffi::SQLITE_READONLY).message, DENIED.0);
        assert_eq!(sql(C::NotADatabase, ffi::SQLITE_NOTADB).message, DAMAGED.0);
        let p = open(OpenError::NoHome(
            "couldn't find the app-data folder".into(),
        ));
        assert_eq!(p.kind, "other");
        assert!(p.hint.unwrap().contains("FUSELANE_HOME"));
    }

    #[test]
    fn a_service_refusal_keeps_its_own_words() {
        let p = classify(
            &Failure::Service(UiError::new_public(
                "disk",
                "Couldn't read the list.",
                Some("Try again."),
            )),
            None,
        );
        assert_eq!(p.kind, "other");
        assert_eq!(p.message, "Couldn't read the list.");
        assert_eq!(p.hint.as_deref(), Some("Try again."));
        assert_eq!(p.home, None);
    }

    #[test]
    fn a_missing_folder_opens_the_closest_one_that_exists() {
        let d = tempfile::tempdir().unwrap();
        let missing = d.path().join("a").join("b");
        assert_eq!(nearest_existing(&missing), d.path());
        assert_eq!(nearest_existing(d.path()), d.path());
    }

    #[test]
    fn the_details_name_the_version_problem_and_folder() {
        let p = open(OpenError::Store(StoreError::TooNew {
            found: 15,
            known: 13,
        }));
        let d = details(&p);
        assert!(d.starts_with(&format!("Fuselane {}", env!("CARGO_PKG_VERSION"))));
        assert!(d.contains("schema v15"));
        assert!(d.contains("Folder: /home/fuselane"));
    }
}
