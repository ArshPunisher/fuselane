//! Fuselane desktop app: a Tauri 2 shell around `service::Service`.
//!
//! The window talks to the backend only through the commands below plus one
//! event channel (`subscribe`). Errors reach the window as `UiError` (code,
//! message, hint), never as raw strings (ERRORS.md).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod service;

use std::sync::Arc;

use service::{JobView, NetView, Service, UiError, UiEvent};
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
}

#[tauri::command]
fn app_info(svc: State<'_>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        default_dir: svc.default_dir().to_string_lossy().into_owned(),
    }
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

fn open_service() -> Result<Arc<Service>, String> {
    let home = match std::env::var_os("FUSELANE_HOME") {
        Some(h) => std::path::PathBuf::from(h),
        None => dirs::data_dir()
            .ok_or("no data folder on this system")?
            .join(if cfg!(target_os = "macos") {
                "app.fuselane"
            } else {
                "fuselane"
            }),
    };
    std::fs::create_dir_all(&home)
        .map_err(|e| format!("couldn't create {}: {e}", home.display()))?;
    let store = fuselane_core::Store::open(&home.join("fuselane.db")).map_err(|e| e.to_string())?;
    let downloads = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| home.clone());
    Service::new(store, downloads).map_err(|e| e.message)
}

fn main() {
    let svc = match open_service() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("fuselane: {e}");
            std::process::exit(1);
        }
    };
    let on_exit = svc.clone();
    let result = tauri::Builder::default()
        // Must be registered first: a second launch focuses the existing window.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main(app)
        }))
        .manage(svc)
        .setup(|app| {
            let show = MenuItem::with_id(app, "show", "Show Fuselane", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new().menu(&menu).tooltip("Fuselane");
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "show" => show_main(app),
                "quit" => app.exit(0),
                _ => {}
            })
            .build(app)?;
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
