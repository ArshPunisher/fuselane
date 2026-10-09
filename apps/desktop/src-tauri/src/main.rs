//! Fuselane desktop app: a Tauri 2 shell around `service::Service`.
//!
//! The window talks to the backend only through the commands below plus one
//! event channel (`subscribe`). Errors reach the window as `UiError` (code,
//! message, hint), never as raw strings (ERRORS.md).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api_bridge;
mod automation;
mod battery;
mod native;
mod nearby;
mod opens;
mod power;
mod selftest;
mod sends;
mod service;
mod stream;
mod torrents;
mod unpack;
mod update;

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
type Near<'a> = tauri::State<'a, Arc<nearby::Nearby>>;

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
    group: Option<String>,
) -> Result<service::BatchResult, UiError> {
    if text.len() > 1024 * 1024 {
        return Err(UiError::new_public(
            "too-many",
            "That's too much text to read links from (over 1 MB).",
            Some("Paste the links in smaller groups."),
        ));
    }
    svc.add_batch(
        &text,
        dir.as_deref(),
        later.unwrap_or(false),
        group.as_deref(),
    )
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
async fn focus(svc: State<'_>, id: i64) -> Result<(), UiError> {
    svc.focus(id)
}

#[tauri::command]
async fn set_ready_by(svc: State<'_>, id: i64, at: Option<i64>) -> Result<(), UiError> {
    svc.set_ready_by(id, at)
}

/// The files a web page links to, to pick from (B9.3).
#[tauri::command]
async fn files_on_page(svc: State<'_>, url: String) -> Result<service::PageFiles, UiError> {
    if url.len() > 8192 {
        return Err(UiError::new_public(
            "bad-link",
            "That link is too long.",
            None,
        ));
    }
    svc.files_on_page(&url).await
}

#[tauri::command]
fn rename_group(svc: State<'_>, id: i64, name: String) -> Result<(), UiError> {
    svc.rename_group(id, &name)
}

#[tauri::command]
fn ungroup(svc: State<'_>, id: i64) -> Result<(), UiError> {
    svc.ungroup(id)
}

#[tauri::command]
async fn pause_group(svc: State<'_>, id: i64) -> Result<(), UiError> {
    svc.pause_group(id)
}

#[tauri::command]
async fn resume_group(svc: State<'_>, id: i64) -> Result<(), UiError> {
    svc.resume_group(id)
}

#[tauri::command]
async fn unfocus(svc: State<'_>) -> Result<(), UiError> {
    svc.unfocus();
    Ok(())
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
fn long_minutes(svc: State<'_>) -> u32 {
    svc.long_minutes()
}

#[tauri::command]
fn set_long_minutes(svc: State<'_>, minutes: u32) -> Result<u32, UiError> {
    svc.set_long_minutes(minutes)
}

#[tauri::command]
fn find_checksums(svc: State<'_>) -> bool {
    svc.find_checksums()
}

#[tauri::command]
fn set_find_checksums(svc: State<'_>, on: bool) -> Result<bool, UiError> {
    svc.set_find_checksums(on)
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
    /// The download's size, so the banner can say it before anything starts.
    size: Option<u64>,
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, UiError> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app
        .updater()
        .map_err(|e| ui_error("update-check", format!("Couldn't check for updates: {e}")))?;
    match updater.check().await {
        Ok(Some(u)) => {
            // A quick look at the package for its size; the banner works without it.
            let size = tokio::time::timeout(
                std::time::Duration::from_secs(8),
                fuselane_core::runner::preview(u.download_url.as_str()),
            )
            .await
            .ok()
            .and_then(Result::ok)
            .and_then(|p| p.total);
            Ok(Some(UpdateInfo {
                version: u.version.clone(),
                notes: u.body.clone(),
                size,
            }))
        }
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
    let bytes = download_update(&app, &svc, &update).await?;
    svc.send(UiEvent::Update {
        progress: update::UpdateProgress {
            phase: "installing",
            done: bytes.len() as u64,
            total: Some(bytes.len() as u64),
            rate: 0,
            networks: 0,
        },
    });
    update.install(bytes).map_err(|e| fail(&e))?;
    app.restart();
}

/// Stops the update download in progress (the banner's Cancel).
static UPDATE_CANCEL: std::sync::Mutex<Option<fuselane_engine_http::download::Cancel>> =
    std::sync::Mutex::new(None);

#[tauri::command]
fn cancel_update() {
    if let Some(c) = UPDATE_CANCEL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        c.cancel();
    }
}

/// Fetches the update package with Fuselane's engine over every network and
/// verifies it against the release key (B8.4). If the engine can't get it, the
/// updater's own single-connection download (which verifies too) is the fallback.
async fn download_update(
    app: &tauri::AppHandle,
    svc: &Arc<Service>,
    update: &tauri_plugin_updater::Update,
) -> Result<Vec<u8>, UiError> {
    use fuselane_core::runner::{RunOptions, fetch};
    use fuselane_engine_http::download::ProgressFn;
    let failed = |why: String| UiError {
        code: "update-failed",
        message: format!("Couldn't download the update: {why}."),
        hint: Some(
            "Try again. It picks up the check from the start, and your downloads are safe.".into(),
        ),
    };
    let pubkey = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .map(str::to_string)
        .ok_or_else(|| failed("this build has no update key".into()))?;
    let send_progress = {
        let svc = svc.clone();
        move |done: u64, total: Option<u64>, rate: u64, networks: u32| {
            svc.send(UiEvent::Update {
                progress: update::UpdateProgress {
                    phase: "downloading",
                    done,
                    total,
                    rate,
                    networks,
                },
            });
        }
    };
    let networks = u32::try_from(
        fuselane_core::runner::pick_networks(&[])
            .map(|n| n.len())
            .unwrap_or(1),
    )
    .unwrap_or(1)
    .max(1);
    send_progress(0, None, 0, networks);
    let dir = std::env::temp_dir().join(format!("fuselane-update-{}", update.version));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| failed(format!("no room for it ({e})")))?;
    let cancel = fuselane_engine_http::download::Cancel::new();
    *UPDATE_CANCEL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cancel.clone());
    let meter = Arc::new(std::sync::Mutex::new(update::Meter::new()));
    let progress = {
        let meter = meter.clone();
        let send = send_progress.clone();
        ProgressFn(Arc::new(move |done, total| {
            let finished = total.is_some_and(|t| done >= t);
            let rate = meter
                .lock()
                .ok()
                .and_then(|mut m| m.tick(std::time::Instant::now(), done, finished));
            if let Some(rate) = rate {
                send(done, total, rate, networks);
            }
        }))
    };
    let fetched = fetch(
        update.download_url.as_str(),
        dir.clone(),
        RunOptions {
            progress: Some(progress),
            cancel: Some(cancel.clone()),
            ..RunOptions::default()
        },
    )
    .await;
    UPDATE_CANCEL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if cancel.is_cancelled() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(UiError {
            code: "update-cancelled",
            message: "Update cancelled.".into(),
            hint: None,
        });
    }
    let bytes = match fetched {
        Ok(report) => {
            let bytes = std::fs::read(&report.path).map_err(|e| failed(e.to_string()))?;
            update::verify(&bytes, &update.signature, &pubkey, &update.version).map_err(failed)?;
            bytes
        }
        Err(why) => {
            // The plugin's download is a single connection, but it works where the
            // engine couldn't (for example a proxy only the system knows about).
            let _ = why;
            let mut meter = update::Meter::new();
            let mut done = 0u64;
            let send = send_progress.clone();
            update
                .download(
                    move |chunk, total| {
                        done += chunk as u64;
                        let finished = total.is_some_and(|t| done >= t);
                        if let Some(rate) = meter.tick(std::time::Instant::now(), done, finished) {
                            send(done, total, rate, 1);
                        }
                    },
                    || {},
                )
                .await
                .map_err(|e| failed(e.to_string()))?
        }
    };
    let _ = std::fs::remove_dir_all(&dir);
    Ok(bytes)
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

/// Whether `name` is already taken in the folder a new download would save to,
/// so the New download dialog can ask (B8.6).
#[tauri::command]
fn name_taken(svc: State<'_>, dir: Option<String>, name: String) -> bool {
    svc.name_taken(dir.as_deref(), &name)
}

/// A finished download with the same name and size, still on disk (B9.8).
#[tauri::command]
fn already_have(
    svc: State<'_>,
    name: String,
    size: Option<u64>,
) -> Result<Option<service::HaveView>, UiError> {
    if name.len() > 1024 {
        return Ok(None);
    }
    svc.already_have(&name, size)
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
async fn torrent_set_priority(
    tor: Tor<'_>,
    id: String,
    file: usize,
    priority: torrents::Priority,
) -> Result<(), UiError> {
    tor.set_priority(&id, file, priority).await
}

/// The local stream server for Play, started the first time it's needed.
static STREAMS: tokio::sync::OnceCell<stream::StreamServer> = tokio::sync::OnceCell::const_new();

/// Plays a torrent file while it downloads (B8.10): it goes to High priority,
/// and the system opens a local stream link (read in order as it arrives).
/// Returns the link, so it can also be pasted into a player such as VLC.
#[tauri::command]
async fn torrent_play(
    app: tauri::AppHandle,
    tor: Tor<'_>,
    id: String,
    file: usize,
) -> Result<String, UiError> {
    use tauri_plugin_opener::OpenerExt;
    let name = tor.file_name(&id, file).await.ok_or_else(|| {
        ui_error(
            "not-playable",
            "This torrent isn't downloading now, so it can't be played from here. Open the finished file instead.".into(),
        )
    })?;
    if !stream::playable(&name) {
        return Err(ui_error(
            "not-playable",
            format!("{name} isn't audio or video, so there's nothing to play."),
        ));
    }
    tor.set_priority(&id, file, torrents::Priority::High)
        .await?;
    let torrents = tor.inner().clone();
    let server = STREAMS
        .get_or_try_init(|| async move {
            let open: stream::OpenFn = Arc::new(move |id, file| {
                let t = torrents.clone();
                Box::pin(async move { t.open_stream(&id, file).await })
            });
            stream::StreamServer::start(open).await
        })
        .await
        .map_err(|e| ui_error("play-failed", format!("Couldn't start playing: {e}")))?;
    let url = server.url(&id, file, &name);
    let _ = app.opener().open_url(&url, None::<&str>);
    Ok(url)
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
    svc.add_batch(&text, None, true, None).map(Some)
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

/// Asks for files to send to a nearby device; empty if they cancel.
#[tauri::command]
async fn nearby_pick(app: tauri::AppHandle) -> Vec<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .set_title("Send files")
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

#[tauri::command]
async fn nearby_start(near: Near<'_>) -> Result<nearby::NearbyView, UiError> {
    near.inner().start().await
}

#[tauri::command]
fn nearby_state(near: Near<'_>) -> nearby::NearbyView {
    near.view()
}

#[tauri::command]
async fn nearby_set_everyone(near: Near<'_>, on: bool) -> Result<nearby::NearbyView, UiError> {
    Ok(near.inner().set_everyone(on).await)
}

/// Hands a paused download to another Fuselane over Nearby (B9.9): its partial
/// file and a manifest go there, and it carries on on that computer.
#[tauri::command]
async fn nearby_handoff(
    svc: State<'_>,
    near: Near<'_>,
    id: i64,
    fingerprint: String,
) -> Result<nearby::NearbyView, UiError> {
    let out = fuselane_core::home::home()
        .unwrap_or_else(|_| std::env::temp_dir().join("fuselane"))
        .join("handoff")
        .join(id.to_string());
    let files = svc.handoff_files(id, &out)?;
    near.inner()
        .send(
            &fingerprint,
            files
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        )
        .await
}

#[tauri::command]
async fn nearby_send(
    near: Near<'_>,
    fingerprint: String,
    paths: Vec<String>,
) -> Result<nearby::NearbyView, UiError> {
    near.inner().send(&fingerprint, paths).await
}

#[tauri::command]
fn nearby_answer(
    near: Near<'_>,
    id: u64,
    accept: bool,
    trust: bool,
) -> Result<nearby::NearbyView, UiError> {
    near.answer(id, accept, trust)
}

#[tauri::command]
async fn nearby_phone(near: Near<'_>, on: bool) -> Result<nearby::NearbyView, UiError> {
    near.inner().set_phone(on).await
}

#[tauri::command]
async fn nearby_phone_offer(
    near: Near<'_>,
    add: Vec<String>,
    remove: Option<String>,
) -> Result<nearby::NearbyView, UiError> {
    near.phone_offer(add, remove).await
}

#[tauri::command]
fn nearby_forget(near: Near<'_>, fingerprint: String) -> nearby::NearbyView {
    near.forget(&fingerprint)
}

#[tauri::command]
async fn nearby_cancel(near: Near<'_>, id: String) -> Result<(), UiError> {
    near.cancel(&id).await;
    Ok(())
}

#[tauri::command]
fn nearby_clear(near: Near<'_>, id: String) -> nearby::NearbyView {
    near.clear(&id)
}

#[tauri::command]
fn nearby_reveal(app: tauri::AppHandle, near: Near<'_>, id: String) -> Result<(), UiError> {
    use tauri_plugin_opener::OpenerExt;
    let path = near
        .received_path(&id)
        .ok_or_else(|| ui_error("not-found", "That file isn't here anymore.".into()))?;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| ui_error("open-failed", format!("Couldn't show the file: {e}")))
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
            | UiEvent::Update { .. }
            | UiEvent::Nearby { .. }
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

fn open_nearby(svc: &Arc<Service>) -> Arc<nearby::Nearby> {
    let state =
        fuselane_core::home::home().unwrap_or_else(|_| std::env::temp_dir().join("fuselane"));
    let weak = Arc::downgrade(svc);
    nearby::Nearby::new(
        svc.store(),
        state,
        svc.default_dir().to_path_buf(),
        Arc::new(nearby::lan_addrs),
        Arc::new(move |e| {
            if let Some(svc) = weak.upgrade() {
                svc.send(e);
            }
        }),
        nearby::computer_name(),
        fuselane_nearby::proto::PORT,
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
    let for_opener = svc.clone();
    // Data allowances: count usage and apply allowances every few seconds.
    {
        let weak = Arc::downgrade(&svc);
        std::thread::spawn(move || {
            let mut round = 0u32;
            while let Some(svc) = weak.upgrade() {
                svc.tick_usage(service::local_today());
                svc.tick_schedule();
                // The battery changes slowly: read it every 30 s (a process on macOS).
                if round.is_multiple_of(6) {
                    svc.tick_battery(battery::read());
                }
                round = round.wrapping_add(1);
                svc.tick_retries();
                svc.tick_starts(chrono::Utc::now().timestamp());
                svc.update_awake();
                drop(svc);
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
    }
    let tor = open_torrents(&svc);
    let snd = open_sends(&svc);
    let near = open_nearby(&svc);
    {
        // A download handed over from another Fuselane continues here (B9.9).
        let weak = Arc::downgrade(&svc);
        near.set_on_received(Arc::new(move |files| {
            if let Some(svc) = weak.upgrade()
                && let Some(Err(why)) = svc.import_handoff(&files)
            {
                // The two files stay in the downloads folder as they arrived.
                eprintln!("fuselane: a hand-off from another computer wasn't usable: {why}");
            }
        }));
    }
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
        .manage(near.clone())
        .setup(move |app| {
            // "Open it" when a download finishes uses the system's default app.
            {
                use tauri_plugin_opener::OpenerExt;
                let handle = app.handle().clone();
                for_opener.set_opener(Arc::new(move |p: &std::path::Path| {
                    let _ = handle.opener().open_path(p.to_string_lossy(), None::<&str>);
                }));
            }
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
            // Nearby: re-announce while visible, drop stale devices, refresh progress.
            let near = near.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    near.tick().await;
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
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
            focus,
            unfocus,
            already_have,
            nearby_handoff,
            set_ready_by,
            rename_group,
            files_on_page,
            ungroup,
            pause_group,
            resume_group,
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
            find_checksums,
            set_find_checksums,
            long_minutes,
            set_long_minutes,
            allowances,
            set_allowance,
            set_slow,
            network_prefs,
            set_network_pref,
            diagnostics,
            check_update,
            open_release_notes,
            install_update,
            cancel_update,
            nearby_pick,
            nearby_start,
            nearby_state,
            nearby_set_everyone,
            nearby_send,
            nearby_answer,
            nearby_forget,
            nearby_phone,
            nearby_phone_offer,
            nearby_cancel,
            nearby_clear,
            nearby_reveal,
            name_taken,
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
            torrent_set_priority,
            torrent_play,
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
