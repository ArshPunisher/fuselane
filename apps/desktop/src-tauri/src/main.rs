//! Fuselane desktop app: a Tauri 2 shell around `service::Service`.
//!
//! The window talks to the backend only through the commands below plus one
//! event channel (`subscribe`). Errors reach the window as `UiError` (code,
//! message, hint), never as raw strings (ERRORS.md).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod native;
mod selftest;
mod service;

use std::sync::Arc;

use service::{
    AllowanceRequest, AllowanceView, JobView, LimitsView, NetPref, NetView, PreviewView, Service,
    UiError, UiEvent,
};
use tauri::Manager;
use tauri::ipc::Channel;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

type State<'a> = tauri::State<'a, Arc<Service>>;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    default_dir: String,
    /// Set on the first launch after an update, for a one-time "Updated" message.
    updated_from: Option<String>,
}

#[tauri::command]
fn app_info(svc: State<'_>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        default_dir: svc.default_dir().to_string_lossy().into_owned(),
        updated_from: svc.updated_from().map(str::to_string),
    }
}

/// Opens this version's release notes in the browser. The URL is built here from
/// the app's own version, so the window can't open arbitrary links.
#[tauri::command]
fn open_release_notes(app: tauri::AppHandle) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let url = format!(
        "https://github.com/ArshPunisher/fuselane/releases/tag/v{}",
        env!("CARGO_PKG_VERSION")
    );
    app.opener().open_url(url, None::<&str>).map_err(|e| {
        ui_error(
            "open-failed",
            format!("Couldn't open the release notes: {e}"),
        )
    })
}

#[tauri::command]
fn list_jobs(svc: State<'_>) -> Result<Vec<JobView>, UiError> {
    svc.jobs()
}

#[tauri::command]
fn list_networks(svc: State<'_>) -> Result<Vec<NetView>, UiError> {
    svc.networks()
}

#[tauri::command]
async fn add_download(svc: State<'_>, url: String, dir: Option<String>) -> Result<i64, UiError> {
    svc.add(&url, dir.as_deref())
}

#[tauri::command]
async fn pause(svc: State<'_>, id: i64) -> Result<(), UiError> {
    svc.pause(id)
}

#[tauri::command]
async fn resume(svc: State<'_>, id: i64) -> Result<(), UiError> {
    svc.resume(id)
}

#[tauri::command]
async fn remove(svc: State<'_>, id: i64) -> Result<(), UiError> {
    svc.remove(id)
}

/// Continues a stopped download from a new link to the same file.
#[tauri::command]
async fn fix_link(svc: State<'_>, id: i64, url: String) -> Result<(), UiError> {
    svc.fix_link(id, &url)
}

/// Discards a download that can't continue and starts it fresh.
#[tauri::command]
async fn start_over(svc: State<'_>, id: i64) -> Result<i64, UiError> {
    svc.start_over(id)
}

/// A privacy-safe report for bug reports (the window copies it to the clipboard).
#[tauri::command]
async fn diagnostics(svc: State<'_>) -> Result<String, UiError> {
    let svc = svc.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let checks: Vec<(String, bool, String)> = selftest::quick()
            .into_iter()
            .map(|c| (c.name.to_string(), c.ok, c.detail))
            .collect();
        svc.diagnostics(&checks)
    })
    .await
    .map_err(|e| ui_error("diagnostics", format!("Couldn't build the report: {e}")))
}

/// Slow mode on or off; other limits stay as saved.
#[tauri::command]
fn set_slow(svc: State<'_>, on: bool) -> Result<LimitsView, UiError> {
    svc.set_slow(on)
}

#[tauri::command]
fn network_prefs(svc: State<'_>) -> Vec<NetPref> {
    svc.network_prefs()
}

/// Renames or recolours a network (clearing both forgets it).
#[tauri::command]
fn set_network_pref(svc: State<'_>, pref: NetPref) -> Result<Vec<NetPref>, UiError> {
    svc.set_network_pref(pref)
}

/// Each network's monthly allowance and how much of it is used.
#[tauri::command]
fn allowances(svc: State<'_>) -> Vec<AllowanceView> {
    svc.allowances(service::local_today())
}

/// Sets (bytes > 0) or removes (bytes = 0) a network's monthly allowance.
#[tauri::command]
fn set_allowance(svc: State<'_>, req: AllowanceRequest) -> Result<Vec<AllowanceView>, UiError> {
    svc.set_allowance(req, service::local_today())
}

#[tauri::command]
fn per_network_dns(svc: State<'_>) -> bool {
    svc.per_network_dns()
}

/// Per-network lookups on or off (they use public resolvers, so off by default).
#[tauri::command]
fn set_per_network_dns(svc: State<'_>, on: bool) -> Result<bool, UiError> {
    svc.set_per_network_dns(on)
}

#[tauri::command]
fn get_limits(svc: State<'_>) -> LimitsView {
    svc.limits()
}

/// Saves speed limits; running downloads follow them at once.
#[tauri::command]
fn set_limits(svc: State<'_>, limits: LimitsView) -> Result<LimitsView, UiError> {
    svc.set_limits(limits)
}

/// A newer version on our signed feed, if any (L-77: our own feed, not GitHub's
/// /releases/latest, which skips betas). Errors are quiet: offline is normal.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    version: String,
    notes: Option<String>,
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, UiError> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app
        .updater()
        .map_err(|e| ui_error("update-check", format!("Couldn't check for updates: {e}")))?;
    match updater.check().await {
        Ok(Some(u)) => Ok(Some(UpdateInfo {
            version: u.version.clone(),
            notes: u.body.clone(),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(ui_error(
            "update-check",
            format!("Couldn't check for updates: {e}"),
        )),
    }
}

/// Downloads and installs the update (its signature is verified against the
/// built-in key), after pausing downloads so their progress is saved, then restarts.
#[tauri::command]
async fn install_update(app: tauri::AppHandle, svc: State<'_>) -> Result<(), UiError> {
    use tauri_plugin_updater::UpdaterExt;
    let fail = |e: &dyn std::fmt::Display| {
        ui_error(
            "update-failed",
            format!("The update couldn't be installed: {e}"),
        )
    };
    let update = app
        .updater()
        .map_err(|e| fail(&e))?
        .check()
        .await
        .map_err(|e| fail(&e))?
        .ok_or_else(|| ui_error("no-update", "There's no update to install.".into()))?;
    svc.pause_all();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while svc.running() > 0 && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| fail(&e))?;
    app.restart();
}

/// Name, size and splittability of a link, before downloading it.
#[tauri::command]
async fn preview(url: String) -> Result<PreviewView, UiError> {
    service::preview(&url).await
}

/// Shows a finished file in Finder / Explorer / the file manager.
#[tauri::command]
fn reveal(app: tauri::AppHandle, svc: State<'_>, id: i64) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let path = svc.finished_file(id)?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| ui_error("open-failed", format!("Couldn't show the file: {e}")))
}

/// Opens a finished file with its default app.
#[tauri::command]
fn open_file(app: tauri::AppHandle, svc: State<'_>, id: i64) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let path = svc.finished_file(id)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| ui_error("open-failed", format!("Couldn't open the file: {e}")))
}

/// Asks the user for a folder to save into; `None` if they cancel.
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .set_title("Save downloads to")
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

fn ui_error(code: &'static str, message: String) -> UiError {
    UiError {
        code,
        message,
        hint: None,
    }
}

/// The window subscribes once; job lists and live updates then arrive on `channel`.
#[tauri::command]
fn subscribe(svc: State<'_>, channel: Channel<UiEvent>) {
    svc.subscribe(Arc::new(move |e| {
        let _ = channel.send(e);
    }));
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Feeds download events to the OS shell: notifications, icon progress, tray tooltip.
fn watch_for_shell(app: tauri::AppHandle, svc: &Arc<Service>) {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    use tauri::window::{ProgressBarState, ProgressBarStatus};
    use tauri_plugin_notification::NotificationExt;

    let watcher = Arc::new(Mutex::new(native::Watcher::default()));
    let last_paint = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(5)));
    let paint = {
        let app = app.clone();
        move |w: &native::Watcher| {
            if let Some(win) = app.get_webview_window("main") {
                let state = match w.progress() {
                    Some(p) => ProgressBarState {
                        status: Some(ProgressBarStatus::Normal),
                        progress: Some(p),
                    },
                    None => ProgressBarState {
                        status: Some(ProgressBarStatus::None),
                        progress: None,
                    },
                };
                let _ = win.set_progress_bar(state);
            }
            if let Some(tray) = app.tray_by_id("main") {
                let _ = tray.set_tooltip(Some(w.tooltip()));
            }
        }
    };
    svc.listen(Arc::new(move |e| {
        let mut w = watcher
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &e {
            UiEvent::Jobs { jobs } => {
                for n in w.jobs(jobs) {
                    let _ = app
                        .notification()
                        .builder()
                        .title(&n.title)
                        .body(&n.body)
                        .show();
                }
                paint(&w);
            }
            UiEvent::Live(l) => {
                w.live(l);
                // The icon and tooltip don't need 5 updates a second.
                let mut last = last_paint
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if last.elapsed() >= Duration::from_secs(1) {
                    *last = Instant::now();
                    paint(&w);
                }
            }
        }
    }));
}

fn open_service() -> Result<Arc<Service>, String> {
    let store = fuselane_core::open_default()?;
    if let Some(aside) = &store.recovered_from {
        eprintln!(
            "fuselane: the download list was damaged, so a fresh one was started. The old file is kept at {}.",
            aside.display()
        );
    }
    let home = fuselane_core::home::home()?;
    let downloads = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| home.clone());
    Service::new(store, downloads).map_err(|e| e.message)
}

fn main() {
    // Headless checks for the release workflow (L-75); never opens a window.
    if std::env::args().any(|a| a == "--self-test") {
        let checks = selftest::run();
        let report: String = checks
            .iter()
            .map(|c| {
                format!(
                    "{} {:<9} {}\n",
                    if c.ok { "ok  " } else { "FAIL" },
                    c.name,
                    c.detail
                )
            })
            .collect();
        print!("{report}");
        // Windows release builds have no console, so CI can ask for a report file.
        if let Some(path) = std::env::var_os("FUSELANE_SELFTEST_OUT") {
            let _ = std::fs::write(path, &report);
        }
        std::process::exit(if checks.iter().all(|c| c.ok) { 0 } else { 1 });
    }
    if std::env::args().any(|a| a == "--version") {
        println!("fuselane-desktop {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let svc = match open_service() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("fuselane: {e}");
            std::process::exit(1);
        }
    };
    let on_exit = svc.clone();
    // Data allowances: count usage and apply allowances every few seconds.
    {
        let weak = Arc::downgrade(&svc);
        std::thread::spawn(move || {
            while let Some(svc) = weak.upgrade() {
                svc.tick_usage(service::local_today());
                drop(svc);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
    }
    let for_shell = svc.clone();
    // Debug builds only (L-100): start a download at launch for smoke tests and
    // screenshots of the real window. Compiled out of release builds.
    #[cfg(debug_assertions)]
    if let Some(url) = std::env::var_os("FUSELANE_DEV_ADD") {
        let svc = svc.clone();
        let url = url.to_string_lossy().into_owned();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = svc.add(&url, None) {
                eprintln!("fuselane: FUSELANE_DEV_ADD refused: {e}");
            }
        });
    }
    let result = tauri::Builder::default()
        // Must be registered first: a second launch focuses the existing window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Remembers the window's size and position between launches.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(svc)
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "Show Fuselane", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .tooltip("Fuselane");
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "show" => show_main(app),
                "quit" => app.exit(0),
                _ => {}
            })
            .build(app)?;
            watch_for_shell(app.handle().clone(), &for_shell);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_jobs,
            list_networks,
            add_download,
            pause,
            resume,
            remove,
            reveal,
            preview,
            get_limits,
            per_network_dns,
            set_per_network_dns,
            allowances,
            set_allowance,
            set_slow,
            network_prefs,
            set_network_pref,
            diagnostics,
            check_update,
            open_release_notes,
            install_update,
            set_limits,
            fix_link,
            start_over,
            open_file,
            pick_folder,
            subscribe
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(move |_, event| {
            if let tauri::RunEvent::Exit = event {
                // Pausing saves a checkpoint, so the next launch resumes cleanly.
                on_exit.pause_all();
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                while on_exit.running() > 0 && std::time::Instant::now() < deadline {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            }
        }),
        Err(e) => {
            eprintln!("fuselane: the window couldn't start: {e}");
            std::process::exit(1);
        }
    }
}
