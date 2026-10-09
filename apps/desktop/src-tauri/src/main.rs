//! Fuselane desktop app: a Tauri 2 shell around `service::Service`.
//!
//! The window talks to the backend only through the commands below plus one
//! event channel (`subscribe`). Errors reach the window as `UiError` (code,
//! message, hint), never as raw strings (ERRORS.md).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api_bridge;
mod automation;
mod native;
mod opens;
mod power;
mod selftest;
mod sends;
mod service;
mod torrents;

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
type Tor<'a> = tauri::State<'a, Arc<torrents::Torrents>>;
type Snd<'a> = tauri::State<'a, Arc<sends::Sends>>;

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
async fn add_download(
    svc: State<'_>,
    url: String,
    dir: Option<String>,
    options: Option<service::AddRequest>,
) -> Result<i64, UiError> {
    svc.add_with(&url, dir.as_deref(), &options.unwrap_or_default())
}

#[tauri::command]
async fn add_batch(
    svc: State<'_>,
    text: String,
    dir: Option<String>,
    later: Option<bool>,
) -> Result<service::BatchResult, UiError> {
    if text.len() > 1024 * 1024 {
        return Err(UiError::new_public(
            "too-many",
            "That's too much text to read links from (over 1 MB).",
            Some("Paste the links in smaller groups."),
        ));
    }
    svc.add_batch(&text, dir.as_deref(), later.unwrap_or(false))
}

#[tauri::command]
fn automation(svc: State<'_>) -> service::AutomationView {
    svc.automation_view()
}

#[tauri::command]
async fn set_automation(
    svc: State<'_>,
    settings: automation::Automation,
) -> Result<service::AutomationView, UiError> {
    svc.set_automation(settings)
}

#[tauri::command]
fn cancel_when_done(svc: State<'_>) {
    svc.cancel_when_done();
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowPrefs {
    start_at_login: bool,
    close_to_tray: bool,
    watch_clipboard: bool,
}

#[tauri::command]
fn window_prefs(app: tauri::AppHandle, svc: State<'_>) -> WindowPrefs {
    use tauri_plugin_autostart::ManagerExt;
    WindowPrefs {
        start_at_login: app.autolaunch().is_enabled().unwrap_or(false),
        close_to_tray: svc.close_to_tray(),
        watch_clipboard: svc.watch_clipboard(),
    }
}

#[tauri::command]
fn set_watch_clipboard(app: tauri::AppHandle, svc: State<'_>, on: bool) -> Result<bool, UiError> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    if on {
        // A link copied before turning this on isn't offered.
        svc.clipboard_baseline(&app.clipboard().read_text().unwrap_or_default());
    }
    svc.set_watch_clipboard(on)
}

/// Copied download links (opt-in in Settings): the clipboard is read once a second
/// while watching is on, only on this computer, and only a single download link
/// opens the New download dialog. Nothing else copied is kept or acted on.
fn watch_clipboard(app: tauri::AppHandle, svc: Arc<Service>) {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    use tauri_plugin_notification::NotificationExt;
    let mut watching = false;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if !svc.watch_clipboard() {
            watching = false;
            continue;
        }
        let Ok(text) = app.clipboard().read_text() else {
            continue;
        };
        if !watching {
            // Launched with watching on: what was already copied isn't news.
            watching = true;
            svc.clipboard_baseline(&text);
            continue;
        }
        let Some(link) = svc.clipboard_seen(&text) else {
            continue;
        };
        svc.open_request(link);
        let focused = app
            .get_webview_window("main")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if !focused {
            let _ = app
                .notification()
                .builder()
                .title("Download link copied")
                .body("Open Fuselane to download it. Turn this off in Settings.")
                .show();
        }
    }
}

#[tauri::command]
fn set_start_at_login(app: tauri::AppHandle, on: bool) -> Result<bool, UiError> {
    use tauri_plugin_autostart::ManagerExt;
    let a = app.autolaunch();
    let r = if on { a.enable() } else { a.disable() };
    r.map_err(|e| {
        UiError::new_public(
            "autostart",
            format!("Your system didn't accept the change ({e})."),
            Some("Try again, or add Fuselane to your login items yourself."),
        )
    })?;
    Ok(a.is_enabled().unwrap_or(on))
}

#[tauri::command]
fn set_close_to_tray(svc: State<'_>, on: bool) -> Result<bool, UiError> {
    svc.set_close_to_tray(on)
}

#[tauri::command]
fn max_running(svc: State<'_>) -> usize {
    svc.max_running()
}

#[tauri::command]
async fn set_max_running(svc: State<'_>, n: usize) -> Result<usize, UiError> {
    svc.set_max_running(n)
}

#[tauri::command]
async fn reorder(svc: State<'_>, ids: Vec<i64>) -> Result<(), UiError> {
    svc.reorder(&ids)
}

#[tauri::command]
async fn trash_file(svc: State<'_>, id: i64) -> Result<(), UiError> {
    svc.trash_file(id)
}

#[tauri::command]
async fn set_job_limit(svc: State<'_>, id: i64, rate: u64) -> Result<(), UiError> {
    svc.set_job_limit(id, rate)
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

#[tauri::command]
async fn torrent_inspect_magnet(
    tor: Tor<'_>,
    magnet: String,
    dir: Option<String>,
) -> Result<torrents::ListingView, UiError> {
    tor.inspect_magnet(&magnet, dir.as_deref()).await
}

#[tauri::command]
async fn torrent_inspect_file(
    tor: Tor<'_>,
    path: String,
    dir: Option<String>,
) -> Result<torrents::ListingView, UiError> {
    tor.inspect_file(std::path::Path::new(&path), dir.as_deref())
        .await
}

#[tauri::command]
async fn torrent_inspect_bytes(
    tor: Tor<'_>,
    bytes: Vec<u8>,
    dir: Option<String>,
) -> Result<torrents::ListingView, UiError> {
    tor.inspect_bytes(bytes, dir.as_deref()).await
}

#[tauri::command]
async fn torrent_add(tor: Tor<'_>, token: String, files: Vec<usize>) -> Result<String, UiError> {
    tor.add(&token, files).await
}

#[tauri::command]
async fn torrent_list(tor: Tor<'_>) -> Result<Vec<torrents::TorrentView>, UiError> {
    Ok(tor.list().await)
}

#[tauri::command]
async fn torrent_files(
    tor: Tor<'_>,
    id: String,
) -> Result<Vec<torrents::TorrentFileView>, UiError> {
    tor.files(&id).await
}

#[tauri::command]
async fn torrent_pause(tor: Tor<'_>, id: String) -> Result<(), UiError> {
    tor.pause(&id).await
}

#[tauri::command]
async fn torrent_resume(tor: Tor<'_>, id: String) -> Result<(), UiError> {
    tor.resume(&id).await
}

#[tauri::command]
async fn torrent_select(tor: Tor<'_>, id: String, files: Vec<usize>) -> Result<(), UiError> {
    tor.select(&id, files).await
}

#[tauri::command]
async fn torrent_remove(tor: Tor<'_>, id: String, delete_files: bool) -> Result<(), UiError> {
    tor.remove(&id, delete_files).await
}

#[tauri::command]
fn torrent_seed_settings(tor: Tor<'_>) -> torrents::SeedSettings {
    tor.seed_settings()
}

#[tauri::command]
async fn set_torrent_seed_settings(
    tor: Tor<'_>,
    settings: torrents::SeedSettings,
) -> Result<torrents::SeedSettings, UiError> {
    tor.set_seed_settings(settings).await
}

#[tauri::command]
async fn torrent_peers(tor: Tor<'_>, id: String) -> Result<Vec<torrents::PeerView>, UiError> {
    tor.peers(&id).await
}

#[tauri::command]
async fn torrent_pieces(tor: Tor<'_>, id: String, cells: usize) -> Result<Vec<u8>, UiError> {
    tor.pieces(&id, cells).await
}

#[tauri::command]
async fn torrent_stop_sharing(tor: Tor<'_>, id: String) -> Result<(), UiError> {
    tor.stop_sharing(&id).await
}

#[tauri::command]
async fn torrent_reveal(app: tauri::AppHandle, tor: Tor<'_>, id: String) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let path = tor.folder_of(&id).await?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| ui_error("open-failed", format!("Couldn't show the files: {e}")))
}

/// Opens a page in the browser so a hotel or café network shows its sign-in page.
#[tauri::command]
fn open_sign_in(app: tauri::AppHandle) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    // Fuselane's own site over plain HTTP: a sign-in page intercepts it.
    let url = format!(
        "http://{}{}",
        fuselane_transport::probe::HOST,
        fuselane_transport::probe::PATH
    );
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| ui_error("open-failed", format!("Couldn't open the browser: {e}")))
}

/// Asks for a .torrent file; `None` if they cancel.
#[tauri::command]
async fn pick_torrent(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .set_title("Open a torrent")
        .add_filter("Torrent", &["torrent"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Saves every download's link to a text file the person picks. How many links
/// were saved, or `None` if they cancelled.
#[tauri::command]
async fn export_links(app: tauri::AppHandle, svc: State<'_>) -> Result<Option<usize>, UiError> {
    use tauri_plugin_dialog::DialogExt;
    let (text, n) = svc.export_text()?;
    let day = chrono::Local::now().format("%Y-%m-%d");
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Export download links")
        .set_file_name(format!("Fuselane downloads {day}.txt"))
        .add_filter("Text", &["txt"])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    std::fs::write(&path, text).map_err(|e| {
        UiError::new_public(
            "write-failed",
            format!("Couldn't save \"{}\": {e}", path.display()),
            Some("Pick a folder you can write to."),
        )
    })?;
    Ok(Some(n))
}

/// Adds the links in a text file the person picks (an export, or any list of
/// links), waiting to be started. `None` if they cancelled.
#[tauri::command]
async fn import_links(
    app: tauri::AppHandle,
    svc: State<'_>,
) -> Result<Option<service::BatchResult>, UiError> {
    use tauri_plugin_dialog::DialogExt;
    const MAX: u64 = 1024 * 1024;
    let Some(path) = app
        .dialog()
        .file()
        .set_title("Import download links")
        .add_filter("Text", &["txt", "csv", "list"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    if size > MAX {
        return Err(UiError::new_public(
            "too-big",
            format!(
                "That file is {} KB; a list of links is at most 1 MB.",
                size.div_ceil(1024)
            ),
            Some("Pick the text file with the links, or split it into smaller files."),
        ));
    }
    let bytes = std::fs::read(&path).map_err(|e| {
        UiError::new_public(
            "read-failed",
            format!("Couldn't read \"{}\": {e}", path.display()),
            None,
        )
    })?;
    let text = String::from_utf8_lossy(&bytes);
    svc.add_batch(&text, None, true).map(Some)
}

/// Asks for a file to send with Fuse Send; `None` if they cancel.
#[tauri::command]
async fn send_pick(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .set_title("Send a file")
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn send_file(snd: Snd<'_>, path: String) -> Result<String, UiError> {
    snd.inner().send(&path).await
}

#[tauri::command]
async fn sends_state(
    snd: Snd<'_>,
) -> Result<(Vec<sends::ShareView>, Vec<sends::ReceiveView>), UiError> {
    Ok(snd.views().await)
}

#[tauri::command]
async fn stop_send(snd: Snd<'_>, id: String) -> Result<(), UiError> {
    snd.stop(&id).await
}

#[tauri::command]
async fn send_once(snd: Snd<'_>, id: String, on: bool) -> Result<(), UiError> {
    snd.set_once(&id, on).await
}

#[tauri::command]
async fn receive_link(snd: Snd<'_>, link: String, dir: Option<String>) -> Result<String, UiError> {
    snd.inner().receive(&link, dir.as_deref()).await
}

#[tauri::command]
async fn reveal_received(app: tauri::AppHandle, snd: Snd<'_>, id: String) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let path = snd.received_path(&id).await?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| ui_error("open-failed", format!("Couldn't show the file: {e}")))
}

#[tauri::command]
async fn dismiss_receive(snd: Snd<'_>, id: String) -> Result<(), UiError> {
    snd.dismiss(&id).await;
    Ok(())
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
            UiEvent::WhenDone { action, seconds } => {
                // The window may be hidden: say what is about to happen and how to stop it.
                let what = match action {
                    automation::WhenDone::Sleep => "Your computer goes to sleep",
                    automation::WhenDone::ShutDown => "Your computer shuts down",
                    automation::WhenDone::Quit => "Fuselane quits",
                    automation::WhenDone::Nothing => return,
                };
                let _ = app
                    .notification()
                    .builder()
                    .title("Downloads finished")
                    .body(format!(
                        "{what} in {seconds} seconds. Open Fuselane to cancel."
                    ))
                    .show();
            }
            UiEvent::Torrents { .. }
            | UiEvent::Sends { .. }
            | UiEvent::Open { .. }
            | UiEvent::Networks { .. }
            | UiEvent::WhenDoneCancelled
            | UiEvent::Automation { .. } => {}
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

fn open_sends(svc: &Arc<Service>) -> Arc<sends::Sends> {
    let dir = fuselane_core::home::home()
        .map(|h| h.join("shares"))
        .unwrap_or_else(|_| std::env::temp_dir().join("fuselane-shares"));
    let weak = Arc::downgrade(svc);
    let nets = Arc::downgrade(svc);
    sends::Sends::new(
        svc.store(),
        dir,
        svc.default_dir().to_path_buf(),
        Arc::new(move || match nets.upgrade() {
            Some(svc) => svc.download_networks(),
            None => fuselane_core::runner::pick_networks(&[]),
        }),
        Some(svc.limiter()),
        true,
        Arc::new(move |e| {
            if let Some(svc) = weak.upgrade() {
                svc.send(e);
            }
        }),
    )
}

fn open_torrents(svc: &Arc<Service>) -> Arc<torrents::Torrents> {
    let state = fuselane_core::home::home()
        .map(|h| h.join("torrents"))
        .unwrap_or_else(|_| std::env::temp_dir().join("fuselane-torrents"));
    let weak = Arc::downgrade(svc);
    torrents::Torrents::new(
        svc.store(),
        state,
        svc.default_dir().to_path_buf(),
        {
            // Torrents skip networks behind a sign-in page too.
            let weak = Arc::downgrade(svc);
            Arc::new(move || match weak.upgrade() {
                Some(svc) => svc.download_networks(),
                None => fuselane_core::runner::pick_networks(&[]),
            })
        },
        true,
        Some(svc.limiter()),
        Arc::new(move |e| {
            if let Some(svc) = weak.upgrade() {
                svc.send(e);
            }
        }),
    )
}

/// Points every installed browser at this copy of the app (STEPS 7.4). Runs on
/// each launch, off the main thread: cheap when nothing changed, and a moved or
/// updated app fixes its own manifests.
fn register_browser_host() {
    use fuselane_api::hosts;
    let (Ok(current), Ok(home)) = (std::env::current_exe(), fuselane_core::home::home()) else {
        return;
    };
    let exe = hosts::host_exe(current, std::env::var_os("APPIMAGE").map(Into::into));
    match hosts::register(&exe, &home) {
        Err(problem) => eprintln!("fuselane: browser extension: {problem}"),
        Ok(outcomes) => {
            for (browser, outcome) in outcomes {
                if let hosts::Outcome::Failed(e) = outcome {
                    eprintln!("fuselane: browser extension: couldn't register with {browser}: {e}");
                }
            }
        }
    }
}

fn main() {
    // Started by a browser for the extension: relay its messages, never open a window.
    let args: Vec<String> = std::env::args().collect();
    if fuselane_api::native::is_host_invocation(&args) {
        let code = match fuselane_core::home::home() {
            Ok(home) => fuselane_api::native::run_host(&home),
            Err(_) => 1,
        };
        std::process::exit(code);
    }
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
    std::thread::spawn(register_browser_host);
    // Launched to open something (Windows and Linux pass it as an argument).
    for t in opens::from_args(std::env::args()) {
        svc.open_request(t.as_draft());
    }
    let for_open = svc.clone();
    // Data allowances: count usage and apply allowances every few seconds.
    {
        let weak = Arc::downgrade(&svc);
        std::thread::spawn(move || {
            while let Some(svc) = weak.upgrade() {
                svc.tick_usage(service::local_today());
                svc.tick_schedule();
                svc.tick_retries();
                svc.update_awake();
                drop(svc);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
    }
    let tor = open_torrents(&svc);
    let snd = open_sends(&svc);
    {
        // Torrents and Fuse Send count as work: no sleep or shut-down meanwhile.
        let tor = Arc::downgrade(&tor);
        let snd = Arc::downgrade(&snd);
        svc.set_busy_elsewhere(Arc::new(move || {
            tor.upgrade().is_some_and(|t| t.busy()) || snd.upgrade().is_some_and(|s| s.busy())
        }));
    }
    // Debug builds only (like FUSELANE_DEV_ADD): add a .torrent file with all its
    // files at launch, for checking the real window without clicking.
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("FUSELANE_DEV_TORRENT") {
        let tor = tor.clone();
        tauri::async_runtime::spawn(async move {
            let path = std::path::PathBuf::from(path);
            match tor.inspect_file(&path, None).await {
                Ok(l) => {
                    let all = l.files.iter().map(|f| f.index).collect();
                    if let Err(e) = tor.add(&l.token, all).await {
                        eprintln!("fuselane: FUSELANE_DEV_TORRENT refused: {e}");
                    }
                }
                Err(e) => eprintln!("fuselane: FUSELANE_DEV_TORRENT refused: {e}"),
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
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            show_main(app);
            // A second launch (double-clicked .torrent, a browser's magnet link on
            // Windows or Linux) hands its arguments to this instance.
            let svc = app.state::<Arc<Service>>();
            for t in opens::from_args_in(argv, Some(std::path::Path::new(&cwd))) {
                svc.open_request(t.as_draft());
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Remembers the window's size and position between launches.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // Start at login (off until chosen in Settings); then it opens in the tray.
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .on_window_event(|window, event| {
            // With "keep running" on, closing hides the window; downloads carry on.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window.label() == "main"
                && window.app_handle().state::<Arc<Service>>().close_to_tray()
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .manage(svc)
        .manage(tor.clone())
        .manage(snd.clone())
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
            // Started at login: stay in the tray until someone opens the window.
            if std::env::args().any(|a| a == "--minimized")
                && let Some(w) = app.get_webview_window("main")
            {
                let _ = w.hide();
            }
            {
                // "Quit Fuselane" goes through the normal exit (downloads pause and save).
                let handle = app.handle().clone();
                for_shell.set_power_action(Arc::new(move |action| match action {
                    automation::WhenDone::Quit => handle.exit(0),
                    other => service::run_power_action(other),
                }));
            }
            // The local API (2.43) for the CLI and the browser extension's host.
            {
                let svc = for_shell.clone();
                tauri::async_runtime::spawn(async move {
                    let Ok(home) = fuselane_core::home::home() else {
                        return;
                    };
                    let endpoint = fuselane_api::client::endpoint(&home);
                    let bridge = Arc::new(api_bridge::ApiBridge { svc });
                    if let Err(e) = fuselane_api::server::start(&endpoint, bridge).await {
                        eprintln!("fuselane: the local API isn't available: {e}");
                    }
                });
            }
            // Sign-in page checks (2.15): at launch, every minute, and on network changes.
            tauri::async_runtime::spawn(for_shell.clone().watch_reach());
            {
                let handle = app.handle().clone();
                let svc = for_shell.clone();
                std::thread::spawn(move || watch_clipboard(handle, svc));
            }
            // Saved torrents come back (rechecked from disk), then a tick every second.
            let tor = tor.clone();
            tauri::async_runtime::spawn(async move {
                tor.restore().await;
                loop {
                    tor.tick().await;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
            // Shares from before a restart start seeding again, then a tick every second.
            let snd = snd.clone();
            tauri::async_runtime::spawn(async move {
                snd.restore().await;
                loop {
                    snd.tick().await;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_jobs,
            list_networks,
            add_download,
            add_batch,
            export_links,
            import_links,
            window_prefs,
            set_start_at_login,
            set_close_to_tray,
            set_watch_clipboard,
            automation,
            set_automation,
            cancel_when_done,
            max_running,
            set_max_running,
            reorder,
            set_job_limit,
            trash_file,
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
            pick_torrent,
            open_sign_in,
            torrent_inspect_magnet,
            torrent_inspect_file,
            torrent_inspect_bytes,
            torrent_add,
            torrent_list,
            torrent_files,
            torrent_pause,
            torrent_resume,
            torrent_select,
            torrent_remove,
            torrent_reveal,
            torrent_seed_settings,
            set_torrent_seed_settings,
            torrent_stop_sharing,
            torrent_peers,
            torrent_pieces,
            send_pick,
            send_file,
            sends_state,
            stop_send,
            send_once,
            receive_link,
            dismiss_receive,
            reveal_received,
            subscribe
        ])
        .build(tauri::generate_context!());
    match result {
        Ok(app) => app.run(move |app, event| {
            // macOS delivers "Open with" and magnet links as URLs.
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            if let tauri::RunEvent::Opened { urls } = &event {
                show_main(app);
                for u in urls {
                    if let Some(t) = opens::parse(u.as_str()) {
                        for_open.open_request(t.as_draft());
                    }
                }
            }
            // Clicking the Dock icon brings a hidden window back.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = &event {
                show_main(app);
            }
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            let _ = (app, &for_open);
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
