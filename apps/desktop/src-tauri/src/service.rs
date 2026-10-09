//! The desktop app's backend: owns the store, runs a small queue of downloads and
//! pushes live events to the window. It has no Tauri types so it can be tested
//! directly against the hostile test server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fuselane_core::runner::{self, parse_link, pick_networks};
use fuselane_core::{Event, Job, Outcome, RunOptions, StartError, Status, Store};
use fuselane_engine_http::download::{Cancel, Snapshot, SnapshotFn};
use fuselane_engine_http::headers::filename_from_path;
use fuselane_limits::{Date, LimitSettings, Limiter, Period, Usage, next_reset};
use serde::{Deserialize, Serialize};
use std::time::Duration;

mod already;
mod checksum;
mod deadline;
mod focus;
mod groups;
mod power_aware;
pub mod savings;
mod worth;

pub use worth::NetUse;

pub use already::HaveView;

/// Downloads that run at once; the rest wait their turn (L-53).
pub const MAX_RUNNING: usize = 3;
/// The most downloads at once a person can choose (each already uses many streams).
pub const MAX_RUNNING_LIMIT: usize = 8;

/// One row in the transfers list.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: i64,
    pub url: String,
    pub name: String,
    pub dir: String,
    pub status: &'static str,
    pub resumable: bool,
    pub written: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    /// The fix to offer: fix-link, retry, start-over or free-space.
    pub error_action: Option<String>,
    pub final_path: Option<String>,
    pub created_at: i64,
    /// Queue order: lower starts first.
    pub position: i64,
    /// A SHA-256 will be checked when it finishes.
    pub verify: bool,
    /// This download's own speed limit in bytes per second; 0 = none.
    pub speed_limit: u64,
    /// Seconds until Fuselane tries a failed download again by itself.
    pub retry_in: Option<u64>,
    /// When it starts by itself (unix seconds), for "Starts at 02:00".
    pub start_at: Option<i64>,
    /// Hosts of its mirrors (B8.9), and why any of them wasn't used.
    pub mirrors: Vec<String>,
    pub mirror_notes: Vec<String>,
    /// It has every network to itself ("Do this one now").
    pub focused: bool,
    /// Where its checksum was found by itself ("SHA256SUMS"), if it was (B9.7).
    pub checksum_from: Option<String>,
    /// Finished and matched its SHA-256.
    pub verified: bool,
    /// What each network carried and saved in the finishing run (B9.6).
    pub report: Option<savings::ReportView>,
    /// When it should be finished (unix seconds), and how that looks:
    /// on-track, at-risk or missed (B9.4).
    pub ready_by: Option<i64>,
    pub ready_state: Option<&'static str>,
    /// The group it was added in, and the group's name (B9.2).
    pub group_id: Option<i64>,
    pub group_name: Option<String>,
}

/// One network the app can see.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetView {
    pub name: String,
    pub label: String,
    pub kind: String,
    pub usable: bool,
    pub addrs: Vec<String>,
    /// online | portal | offline, once checked (2.15); None before the first check.
    pub reach: Option<&'static str>,
}

/// One network's part of a live update.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveNet {
    pub name: String,
    pub label: String,
    pub kind: String,
    pub bytes: u64,
    pub rate: f64,
    pub streams: u32,
    pub dead: bool,
}

/// What the Fuse Core draws for one running job, a few times a second.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Live {
    pub id: i64,
    pub written: u64,
    pub total: Option<u64>,
    pub rate: f64,
    pub networks: Vec<LiveNet>,
    /// Triples per tick: fill 0..=100, owner (index into `networks` + 1, 0 = none),
    /// in-flight network (same encoding). Flat so the IPC payload stays small.
    pub ticks: Vec<u16>,
    pub retries: u64,
    pub hedges: u64,
}

/// Speed limits as the window shows and edits them (bytes per second; 0 = none).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LimitsView {
    pub global: u64,
    pub networks: Vec<NetLimit>,
    /// Slow mode: a temporary overall cap that leaves `global` untouched.
    #[serde(default)]
    pub slow: bool,
    /// The slow-mode cap (0 = the default, 1 MiB/s).
    #[serde(default)]
    pub slow_rate: u64,
}

/// Slow mode's cap when none is chosen.
pub const DEFAULT_SLOW: u64 = 1024 * 1024;

/// A network's name and colour as the user chose them (by device name).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetPref {
    pub name: String,
    pub label: Option<String>,
    pub lane: Option<String>,
    /// When it helps: always, only for long downloads, or never (B9.5).
    #[serde(default)]
    pub use_for: NetUse,
}

const LANES: [&str; 8] = [
    "tide", "volt", "iris", "rose", "mint", "sky", "lilac", "steel",
];

/// Checks a device name from the window: exact, short, no control characters.
fn valid_device(name: &str) -> bool {
    !name.is_empty()
        && name == name.trim()
        && name.len() <= 64
        && !name.chars().any(char::is_control)
}

impl NetPref {
    fn validated(mut self) -> Result<NetPref, UiError> {
        let bad = |m: &str| UiError::new("bad-network-name", m, Some("Use up to 40 characters."));
        if !valid_device(&self.name) {
            return Err(bad("That isn't a network on this computer."));
        }
        self.label = self
            .label
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty());
        if let Some(l) = &self.label {
            if l.chars().count() > 40 {
                return Err(bad("That name is too long."));
            }
            if l.chars().any(char::is_control) {
                return Err(bad("Names can't contain control characters."));
            }
        }
        if let Some(lane) = &self.lane
            && !LANES.contains(&lane.as_str())
        {
            return Err(UiError::new(
                "bad-network-color",
                "That colour isn't one of Fuselane's network colours.",
                None,
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetLimit {
    pub name: String,
    pub rate: u64,
}

/// Highest limit accepted: anything above is a typo, not a speed (100 GB/s).
const MAX_LIMIT: u64 = 100 * 1024 * 1024 * 1024;

impl LimitsView {
    /// What the limiter enforces: slow mode caps the overall limit without changing it.
    fn to_settings(&self) -> LimitSettings {
        let slow = if self.slow_rate == 0 {
            DEFAULT_SLOW
        } else {
            self.slow_rate
        };
        let global = match (self.slow, self.global) {
            (false, g) => g,
            (true, 0) => slow,
            (true, g) => g.min(slow),
        };
        LimitSettings {
            global,
            networks: self
                .networks
                .iter()
                .map(|n| (n.name.clone(), n.rate))
                .collect(),
        }
    }

    /// Checks a request from the window (L-97: validate every IPC payload).
    fn validated(mut self) -> Result<LimitsView, UiError> {
        let bad = |m: &str| {
            UiError::new(
                "bad-limit",
                m,
                Some("Use a speed in KB/s or MB/s, or leave it empty for no limit."),
            )
        };
        if self.global > MAX_LIMIT {
            return Err(bad("That overall limit is too high to be a real speed."));
        }
        if self.slow_rate > MAX_LIMIT {
            return Err(bad("That slow-mode speed is too high to be a real speed."));
        }
        if self.slow_rate == 0 {
            self.slow_rate = DEFAULT_SLOW;
        }
        if self.networks.len() > 64 {
            return Err(bad("Too many network limits."));
        }
        let mut seen = std::collections::HashSet::new();
        for n in &self.networks {
            let name = n.name.trim();
            // Device names are exact: no control characters or stray spaces.
            if name.is_empty()
                || name != n.name
                || name.len() > 64
                || n.name.chars().any(char::is_control)
            {
                return Err(bad("A network limit has no valid network name."));
            }
            if n.rate > MAX_LIMIT {
                return Err(bad("That network limit is too high to be a real speed."));
            }
            if !seen.insert(name.to_string()) {
                return Err(bad("A network is listed twice."));
            }
        }
        self.networks.retain(|n| n.rate > 0);
        Ok(self)
    }
}

/// A network's monthly data allowance as saved (by device name).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Allowance {
    pub name: String,
    /// Bytes per period.
    pub bytes: u64,
    /// The day each month the count starts again (1 to 28).
    pub reset_day: u8,
}

/// A network's allowance and how much of it is used, for the window.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AllowanceView {
    pub name: String,
    pub allowance: Option<u64>,
    pub reset_day: u8,
    pub used: u64,
    /// The next reset, as YYYY-MM-DD.
    pub resets_on: String,
    pub reached: bool,
}

/// What the window sends to set (bytes > 0) or remove (bytes = 0) an allowance.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowanceRequest {
    pub name: String,
    pub bytes: u64,
    pub reset_day: u8,
}

/// Largest allowance accepted (100 TB): anything bigger is a typo.
const MAX_ALLOWANCE: u64 = 100 * 1024_u64.pow(4);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedUsage {
    name: String,
    start: String,
    bytes: u64,
}

fn date_text(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

fn parse_date(s: &str) -> Option<Date> {
    let mut it = s.split('-');
    let d = Date {
        year: it.next()?.parse().ok()?,
        month: it.next()?.parse().ok()?,
        day: it.next()?.parse().ok()?,
    };
    d.is_valid().then_some(d)
}

/// Today in the computer's own time zone (allowances reset at local midnight).
pub fn local_today() -> Date {
    use chrono::Datelike;
    let now = chrono::Local::now().date_naive();
    Date {
        year: now.year(),
        month: now.month() as u8,
        day: now.day() as u8,
    }
}

/// What a link points at, shown in the New download dialog before starting.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewView {
    pub filename: String,
    pub total: Option<u64>,
    pub splittable: bool,
}

/// Looks a link up without downloading it. Bad links fail fast with `bad-link`.
pub async fn preview(url: &str) -> Result<PreviewView, UiError> {
    preview_with(url, fuselane_engine_http::download::Headers::default())
        .await
        .map(|(v, _)| v)
}

/// `preview` with a browser session's headers; also says whether the answer was a
/// web page (often a sign-in page) rather than a file.
pub async fn preview_with(
    url: &str,
    headers: fuselane_engine_http::download::Headers,
) -> Result<(PreviewView, bool), UiError> {
    let url = url.trim();
    parse_link(url)
        .map_err(|m| UiError::new("bad-link", m, Some("Links start with http:// or https://.")))?;
    let p = runner::preview_with(url, headers)
        .await
        .map_err(|m| UiError::new("preview-failed", m, None))?;
    Ok((
        PreviewView {
            filename: p.filename,
            total: p.total,
            splittable: p.splittable,
        },
        p.web_page,
    ))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiEvent {
    Jobs {
        jobs: Vec<JobView>,
    },
    Live(Live),
    Torrents {
        torrents: Vec<crate::torrents::TorrentView>,
    },
    /// The OS asked Fuselane to open a magnet link or a .torrent file.
    Open {
        target: String,
    },
    /// The network list or a network's check changed.
    Networks {
        networks: Vec<NetView>,
    },
    /// Everything finished and an action is coming (sleep, shut down, quit) unless
    /// cancelled within `seconds`.
    WhenDone {
        action: crate::automation::WhenDone,
        seconds: u32,
    },
    /// The countdown ended without running its action (cancelled or new work).
    WhenDoneCancelled,
    /// The schedule started or paused downloads.
    Automation {
        view: AutomationView,
    },
    /// Fuse Send: files being shared from here and received from links.
    Sends {
        shares: Vec<crate::sends::ShareView>,
        receives: Vec<crate::sends::ReceiveView>,
    },
    /// The app's own update downloading or installing (B8.4).
    Update {
        progress: crate::update::UpdateProgress,
    },
    /// Nearby: devices, who can see this computer, requests, transfers (B8.11).
    Nearby {
        view: Box<crate::nearby::NearbyView>,
    },
}

/// Automation settings plus where the schedule stands right now.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationView {
    pub settings: crate::automation::Automation,
    /// Downloads may run now (always true without a schedule).
    pub allowed_now: bool,
    /// "Starts at 01:00" / "Pauses at 07:00", when a schedule is on.
    pub next: Option<String>,
}

/// Runs a power action (tests record it instead).
pub type PowerFn = Arc<dyn Fn(crate::automation::WhenDone) + Send + Sync>;
/// Whether something outside the HTTP queue (torrents) is still busy.
pub type BusyFn = Arc<dyn Fn() -> bool + Send + Sync>;

/// An error the window can show as is: a stable code, what happened, what to do.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct UiError {
    pub code: &'static str,
    pub message: String,
    pub hint: Option<String>,
}

impl UiError {
    /// For commands outside this module.
    pub fn new_public(
        code: &'static str,
        message: impl Into<String>,
        hint: Option<&str>,
    ) -> UiError {
        UiError::new(code, message, hint)
    }

    fn new(code: &'static str, message: impl Into<String>, hint: Option<&str>) -> UiError {
        UiError {
            code,
            message: message.into(),
            hint: hint.map(str::to_string),
        }
    }
}

pub type Emit = Arc<dyn Fn(UiEvent) + Send + Sync>;

#[derive(Debug)]
struct Running {
    cancel: Cancel,
    remove_after: bool,
    /// Stopped to start again with more networks (it turned out long, B9.5).
    replan: bool,
    /// Its own speed limit, changed live from the window.
    limit: Arc<fuselane_limits::JobLimit>,
}

/// What the New download dialog can choose besides the link and folder.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AddRequest {
    pub name: Option<String>,
    pub sha256: Option<String>,
    pub allow_duplicate: bool,
    /// Add it paused ("Download later"): it waits in the list until started.
    pub later: bool,
    /// Start by itself at this time (unix seconds, B8.5); it waits paused until then.
    #[serde(default)]
    pub start_at: Option<i64>,
    /// A file with the same name is already there: true replaces it (to the Trash)
    /// once this one is complete, false keeps both. None follows the setting.
    #[serde(default)]
    pub replace: Option<bool>,
    /// Other links to the same file (B8.9); up to 8, http or https.
    #[serde(default)]
    pub mirrors: Vec<String>,
    /// A browser session's cookies and referrer (from the extension, never the window).
    #[serde(skip)]
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub url: String,
    pub reason: String,
}

/// What a batch add did.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub added: Vec<i64>,
    pub skipped: Vec<Skipped>,
    /// The group they were put in, when asked for and two or more were added.
    pub group: Option<i64>,
}

/// `name`, or `name (2)`, `name (3)`… whichever is free in `dir`.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    (2..10_000)
        .map(|n| dir.join(fuselane_storage::names::numbered(name, n)))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// Runs a when-done action for real (Quit is the app's to do; see main.rs).
pub fn run_power_action(action: crate::automation::WhenDone) {
    use crate::automation::WhenDone;
    let result = match action {
        WhenDone::Nothing | WhenDone::Quit => Ok(()),
        WhenDone::Sleep => crate::power::sleep_now(),
        WhenDone::ShutDown => crate::power::shut_down(),
    };
    if let Err(e) = result {
        eprintln!("fuselane: couldn't {action:?} the computer: {e}");
    }
}

/// The app's backend. Cheap to share: methods take `&Arc<Self>`.
pub struct Service {
    store: Arc<Store>,
    running: Mutex<HashMap<i64, Running>>,
    emit: Mutex<Emit>,
    /// Extra listeners that live as long as the app (tray, notifications).
    listeners: Mutex<Vec<Emit>>,
    default_dir: PathBuf,
    max_running: std::sync::atomic::AtomicUsize,
    limiter: Arc<Limiter>,
    /// Limits as saved (the limiter holds the effective ones, after slow mode).
    saved_limits: Mutex<LimitsView>,
    net_prefs: Mutex<Vec<NetPref>>,
    allowances: Mutex<Vec<Allowance>>,
    usage: Mutex<HashMap<String, Usage>>,
    /// Running jobs paused because every network reached its allowance.
    allowance_paused: Mutex<std::collections::HashSet<i64>>,
    /// Look servers up through each network too (public resolvers). Off by default.
    per_network_dns: std::sync::atomic::AtomicBool,
    /// The version that ran before this one, when this launch follows an update.
    updated_from: Option<String>,
    /// Shrinks engine retry waits; only tests set it.
    retry_scale: Option<f64>,
    /// Failed downloads waiting to be tried again by themselves.
    retries: Mutex<HashMap<i64, Retry>>,
    /// Opens that arrived before the window subscribed (a double-clicked .torrent
    /// launching the app); sent once it does.
    pending_opens: Mutex<Vec<String>>,
    subscribed: std::sync::atomic::AtomicBool,
    /// Last sign-in-page check per network (2.15).
    reach: Mutex<HashMap<String, fuselane_transport::probe::Reach>>,
    /// Browser sessions for downloads handed over by the extension. Memory only:
    /// cookies are never written to disk; after a restart the download carries on
    /// without them (Fix link covers a server that then refuses).
    sessions: Mutex<HashMap<i64, fuselane_engine_http::download::Headers>>,
    automation: Mutex<crate::automation::Automation>,
    /// Jobs the schedule paused; they resume when the window opens again.
    schedule_paused: Mutex<std::collections::HashSet<i64>>,
    /// Tests pin the clock; None reads the real local time.
    clock: Mutex<Option<crate::automation::Moment>>,
    awake: Mutex<Option<crate::power::KeepAwake>>,
    /// Something ran since the queue was last empty (so "when done" can fire).
    had_work: std::sync::atomic::AtomicBool,
    pending_action: Mutex<Option<Cancel>>,
    power: Mutex<PowerFn>,
    busy_elsewhere: Mutex<Option<BusyFn>>,
    /// Seconds before a when-done action runs (tests shorten it).
    countdown: std::sync::atomic::AtomicU32,
    /// Closing the window hides it; downloads carry on from the tray.
    close_to_tray: std::sync::atomic::AtomicBool,
    /// Offer copied download links (opt-in), and the last clipboard text seen.
    watch_clipboard: std::sync::atomic::AtomicBool,
    last_clip: Mutex<Option<String>>,
    /// Opens a finished file with its usual app ("Open it" when done); set by the app.
    opener: Mutex<OpenFn>,
    /// Why mirrors weren't used, per download, from its latest run.
    mirror_notes: Mutex<HashMap<i64, Arc<Mutex<Vec<String>>>>>,
    /// "Do this one now" (B9.1): the download with every network to itself, and
    /// the ones it paused, which carry on after it.
    focus: Mutex<Option<i64>>,
    focus_held: Mutex<std::collections::HashSet<i64>>,
    /// Look for a published SHA-256 next to each new download (B9.7). On by default.
    find_checksums: std::sync::atomic::AtomicBool,
    /// "Long" in minutes, downloads known to be long, and each network's recent
    /// average speed (B9.5).
    long_minutes: std::sync::atomic::AtomicU32,
    known_long: Mutex<std::collections::HashSet<i64>>,
    net_rates: Mutex<HashMap<String, f64>>,
    /// The battery as last read, and downloads it paused (B9.10).
    battery: Mutex<Option<crate::battery::Battery>>,
    battery_paused: Mutex<std::collections::HashSet<i64>>,
}

/// Opens a file with the system's default app.
pub type OpenFn = Arc<dyn Fn(&Path) + Send + Sync>;

/// Seconds people get to cancel a sleep or shut-down.
pub const COUNTDOWN: u32 = 60;

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service")
            .field("default_dir", &self.default_dir)
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn reach_word(r: &fuselane_transport::probe::Reach) -> &'static str {
    use fuselane_transport::probe::Reach;
    match r {
        Reach::Online => "online",
        Reach::Portal { .. } => "portal",
        Reach::Offline(_) => "offline",
    }
}

/// Leaves out networks behind a sign-in page, unless that would leave none.
fn without_portals(
    all: Vec<fuselane_netif::Interface>,
    reach: &HashMap<String, fuselane_transport::probe::Reach>,
) -> Vec<fuselane_netif::Interface> {
    let open: Vec<_> = all
        .iter()
        .filter(|i| {
            !matches!(
                reach.get(&i.name),
                Some(fuselane_transport::probe::Reach::Portal { .. })
            )
        })
        .cloned()
        .collect();
    if open.is_empty() { all } else { open }
}

fn kind_word(k: fuselane_netif::Kind) -> String {
    format!("{k:?}").to_lowercase()
}

fn job_name(job: &Job) -> String {
    if let Some(f) = job.final_path.as_ref().and_then(|p| p.file_name()) {
        return f.to_string_lossy().into_owned();
    }
    if let Some(f) = &job.filename {
        return f.clone();
    }
    if let Some(f) = &job.chosen_name {
        return f.clone();
    }
    match parse_link(&job.url) {
        Ok((_, host, _, path)) => {
            let path = path.split('?').next().unwrap_or_default();
            filename_from_path(path).unwrap_or(host)
        }
        Err(_) => job.url.clone(),
    }
}

/// A link as the window may show it: a password in it is masked.
fn shown_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) if u.password().is_some() => {
            let _ = u.set_password(Some("****"));
            u.to_string()
        }
        _ => url.to_string(),
    }
}

fn view(job: &Job) -> JobView {
    JobView {
        id: job.id,
        url: shown_url(&job.url),
        name: job_name(job),
        dir: job.dir.to_string_lossy().into_owned(),
        status: job.status.as_str(),
        resumable: matches!(
            job.status,
            Status::Paused | Status::Failed { resumable: true }
        ),
        written: if job.status == Status::Completed {
            job.total.unwrap_or_else(|| job.secured_bytes())
        } else {
            job.secured_bytes()
        },
        total: job.total,
        error: job.error.clone(),
        error_action: job.error_code.clone(),
        final_path: job
            .final_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        created_at: job.created_at,
        position: job.position,
        verify: job.expected_sha256.is_some(),
        speed_limit: job.speed_limit,
        retry_in: None,
        start_at: job.start_at.filter(|_| job.status == Status::Paused),
        mirrors: job
            .mirrors
            .iter()
            .map(|m| {
                url::Url::parse(m)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_string))
                    .unwrap_or_else(|| m.clone())
            })
            .collect(),
        mirror_notes: vec![],
        focused: false,
        checksum_from: job.sha256_from.clone().filter(|f| !f.is_empty()),
        verified: job.status == Status::Completed && job.expected_sha256.is_some(),
        report: job
            .report
            .as_deref()
            .filter(|_| job.status == Status::Completed)
            .and_then(|r| serde_json::from_str::<savings::RunReport>(r).ok())
            .map(|r| savings::view(&r)),
        ready_by: job.ready_by.filter(|_| job.status != Status::Completed),
        ready_state: None,
        group_id: job.group_id,
        group_name: None,
    }
}

/// Waits before each automatic retry of a download that failed for a reason that
/// may pass (a dropped network, a busy server); a returning network skips the wait.
pub const RETRY_WAITS: [u64; 6] = [20, 60, 180, 600, 1800, 3600];

#[derive(Debug, Clone, Copy)]
struct Retry {
    attempts: usize,
    at: std::time::Instant,
}

fn store_error(e: impl std::fmt::Display) -> UiError {
    UiError::new(
        "store",
        format!("Fuselane couldn't read or save its list of downloads: {e}"),
        Some("Check that your disk has free space, then try again."),
    )
}

fn not_found(id: i64) -> UiError {
    UiError::new(
        "not-found",
        format!("There's no download {id}."),
        Some("It may have been removed already."),
    )
}

/// Packs a snapshot for the window. Mirror lanes (id + 100 per mirror) are
/// folded onto their network, so each network shows once (B8.9).
fn live(id: i64, s: &Snapshot, nets: &[(String, String, String)]) -> Live {
    let device = |lane: u32| ((lane - 1) % runner::MIRROR_ID_STEP) + 1;
    let mut order: Vec<u32> = Vec::new();
    for n in &s.networks {
        let d = device(n.id.max(1));
        if !order.contains(&d) {
            order.push(d);
        }
    }
    let index = |lane: Option<u32>| -> u16 {
        lane.and_then(|l| order.iter().position(|d| *d == device(l.max(1))))
            .map_or(0, |i| i as u16 + 1)
    };
    let mut ticks = Vec::with_capacity(s.ticks.len() * 3);
    for t in &s.ticks {
        ticks.push((t.fill.clamp(0.0, 1.0) * 100.0).round() as u16);
        ticks.push(index(t.owner));
        ticks.push(index(t.in_flight));
    }
    Live {
        id,
        written: s.written,
        total: s.total,
        rate: if s.rate.is_finite() { s.rate } else { 0.0 },
        networks: order
            .iter()
            .map(|d| {
                let lanes: Vec<_> = s
                    .networks
                    .iter()
                    .filter(|n| device(n.id.max(1)) == *d)
                    .collect();
                let (name, label, kind) = nets
                    .get(d.saturating_sub(1) as usize)
                    .cloned()
                    .unwrap_or_else(|| (format!("net{d}"), String::new(), "other".into()));
                LiveNet {
                    name,
                    label,
                    kind,
                    bytes: lanes.iter().map(|n| n.bytes).sum(),
                    rate: lanes
                        .iter()
                        .map(|n| if n.rate.is_finite() { n.rate } else { 0.0 })
                        .sum(),
                    streams: lanes.iter().map(|n| n.streams).sum(),
                    dead: lanes.iter().all(|n| n.dead),
                }
            })
            .collect(),
        ticks,
        retries: s.retries,
        hedges: s.hedges,
    }
}

impl Service {
    /// Opens the service. Jobs a crash left running come back paused (L-53).
    pub fn new(store: Store, default_dir: PathBuf) -> Result<Arc<Service>, UiError> {
        store.recover_interrupted().map_err(store_error)?;
        // Saved limits come back on launch; a damaged value is ignored, not fatal.
        let limiter = Arc::new(Limiter::default());
        // A fresh install has no saved limits: slow mode still has its default speed.
        let mut saved_limits = LimitsView {
            slow_rate: DEFAULT_SLOW,
            ..LimitsView::default()
        };
        // Which version ran last: an update is announced once (never on a fresh install).
        let current = env!("CARGO_PKG_VERSION");
        let previous = store.setting("last_version").ok().flatten();
        let updated_from = previous.filter(|p| p != current);
        let _ = store.set_setting("last_version", current);
        let store_flag = store.setting("per_network_dns").ok().flatten().as_deref() == Some("true");
        let find_checksums =
            store.setting("find_checksums").ok().flatten().as_deref() != Some("false");
        let long_minutes = store
            .setting("long_minutes")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|m| (1..=600).contains(m))
            .unwrap_or(worth::LONG_MINUTES);
        let automation: crate::automation::Automation = store
            .setting("automation")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .and_then(|a: crate::automation::Automation| a.validated().ok())
            .unwrap_or_default();
        let close_to_tray =
            store.setting("close_to_tray").ok().flatten().as_deref() == Some("true");
        let watch_clipboard =
            store.setting("watch_clipboard").ok().flatten().as_deref() == Some("true");
        let max_running = store
            .setting("max_running")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|n| (1..=MAX_RUNNING_LIMIT).contains(n))
            .unwrap_or(MAX_RUNNING);
        let allowances: Vec<Allowance> = store
            .setting("allowances")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str::<Vec<Allowance>>(&v).ok())
            .unwrap_or_default();
        let usage: HashMap<String, Usage> = store
            .setting("usage")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str::<Vec<SavedUsage>>(&v).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|u| {
                let reset = allowances
                    .iter()
                    .find(|a| a.name == u.name)
                    .map_or(1, |a| a.reset_day);
                Some((
                    u.name,
                    Usage {
                        period: Period::MonthFrom(reset),
                        start: parse_date(&u.start)?,
                        bytes: u.bytes,
                    },
                ))
            })
            .collect();
        let net_prefs: Vec<NetPref> = store
            .setting("network_prefs")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str::<Vec<NetPref>>(&v).ok())
            .map(|v| v.into_iter().filter_map(|p| p.validated().ok()).collect())
            .unwrap_or_default();
        if let Some(saved) = store
            .setting("limits")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str::<LimitsView>(&v).ok())
            .and_then(|v| v.validated().ok())
        {
            limiter.apply(&saved.to_settings());
            saved_limits = saved;
        }
        Ok(Arc::new(Service {
            store: Arc::new(store),
            running: Mutex::new(HashMap::new()),
            emit: Mutex::new(Arc::new(|_| {})),
            listeners: Mutex::new(Vec::new()),
            default_dir,
            max_running: std::sync::atomic::AtomicUsize::new(max_running),
            retry_scale: None,
            opener: Mutex::new(Arc::new(|_| {})),
            mirror_notes: Mutex::new(HashMap::new()),
            focus: Mutex::new(None),
            focus_held: Mutex::new(std::collections::HashSet::new()),
            find_checksums: std::sync::atomic::AtomicBool::new(find_checksums),
            long_minutes: std::sync::atomic::AtomicU32::new(long_minutes),
            known_long: Mutex::default(),
            net_rates: Mutex::default(),
            battery: Mutex::new(None),
            battery_paused: Mutex::default(),
            retries: Mutex::new(HashMap::new()),
            limiter,
            saved_limits: Mutex::new(saved_limits),
            net_prefs: Mutex::new(net_prefs),
            allowances: Mutex::new(allowances),
            usage: Mutex::new(usage),
            allowance_paused: Mutex::default(),
            per_network_dns: std::sync::atomic::AtomicBool::new(store_flag),
            updated_from,
            pending_opens: Mutex::default(),
            subscribed: std::sync::atomic::AtomicBool::new(false),
            reach: Mutex::default(),
            sessions: Mutex::default(),
            automation: Mutex::new(automation),
            schedule_paused: Mutex::default(),
            clock: Mutex::new(None),
            awake: Mutex::new(None),
            had_work: std::sync::atomic::AtomicBool::new(false),
            pending_action: Mutex::new(None),
            power: Mutex::new(Arc::new(run_power_action)),
            busy_elsewhere: Mutex::new(None),
            countdown: std::sync::atomic::AtomicU32::new(COUNTDOWN),
            close_to_tray: std::sync::atomic::AtomicBool::new(close_to_tray),
            watch_clipboard: std::sync::atomic::AtomicBool::new(watch_clipboard),
            last_clip: Mutex::new(None),
        }))
    }

    /// For tests: a different queue size.
    #[cfg(test)]
    pub fn with_max_running(
        store: Store,
        dir: PathBuf,
        max: usize,
    ) -> Result<Arc<Service>, UiError> {
        let s = Service::new(store, dir)?;
        let mut s = Arc::try_unwrap(s).map_err(|_| store_error("busy"))?;
        s.max_running = std::sync::atomic::AtomicUsize::new(max.max(1));
        s.retry_scale = Some(0.05);
        Ok(Arc::new(s))
    }

    /// Where events go (the window's channel). Replaces any earlier subscriber.
    pub fn subscribe(self: &Arc<Self>, emit: Emit) {
        *lock(&self.emit) = emit;
        self.publish_jobs();
        self.subscribed
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let waiting = std::mem::take(&mut *lock(&self.pending_opens));
        for target in waiting {
            self.send(UiEvent::Open { target });
        }
    }

    /// Hands something the OS asked to open to the window (now, or once it subscribes).
    pub fn open_request(&self, target: String) {
        if self.subscribed.load(std::sync::atomic::Ordering::SeqCst) {
            self.send(UiEvent::Open { target });
        } else {
            let mut q = lock(&self.pending_opens);
            if q.len() < 16 {
                q.push(target);
            }
        }
    }

    /// Speed limits and allowances, shared with torrents.
    pub fn limiter(&self) -> Arc<Limiter> {
        self.limiter.clone()
    }

    /// The shared store (torrents keep their list in it too).
    pub fn store(&self) -> Arc<Store> {
        self.store.clone()
    }

    pub fn default_dir(&self) -> &Path {
        &self.default_dir
    }

    /// Adds a listener that sees every event (the window's channel is separate).
    pub fn listen(&self, f: Emit) {
        lock(&self.listeners).push(f);
    }

    pub fn send(&self, e: UiEvent) {
        let emit = lock(&self.emit).clone();
        let listeners = lock(&self.listeners).clone();
        for l in &listeners {
            l(e.clone());
        }
        emit(e);
    }

    fn publish_jobs(&self) {
        if let Ok(jobs) = self.jobs() {
            self.send(UiEvent::Jobs { jobs });
        }
    }

    pub fn jobs(&self) -> Result<Vec<JobView>, UiError> {
        let retries = lock(&self.retries).clone();
        let notes = lock(&self.mirror_notes).clone();
        let focus = self.focused();
        let group_names: HashMap<i64, String> = self
            .store
            .groups()
            .unwrap_or_default()
            .into_iter()
            .collect();
        let now = std::time::Instant::now();
        Ok(self
            .store
            .list()
            .map_err(store_error)?
            .iter()
            .map(|j| {
                let mut v = view(j);
                v.focused = focus == Some(j.id);
                v.group_name = j.group_id.and_then(|g| group_names.get(&g).cloned());
                if v.group_name.is_none() {
                    v.group_id = None; // a group since removed
                }
                v.ready_state = self.ready_state(j).map(deadline::ReadyState::word);
                if let Some(n) = notes.get(&j.id) {
                    v.mirror_notes = lock(n).clone();
                }
                if v.status == "failed" {
                    v.retry_in = retries
                        .get(&j.id)
                        .map(|r| r.at.saturating_duration_since(now).as_secs());
                }
                v
            })
            .collect())
    }

    fn retry_wait(&self, attempt: usize) -> std::time::Duration {
        let secs = RETRY_WAITS[attempt.min(RETRY_WAITS.len() - 1)] as f64;
        std::time::Duration::from_secs_f64(secs * self.retry_scale.unwrap_or(1.0))
    }

    /// After a run: a failure that may pass is tried again later, up to
    /// `RETRY_WAITS.len()` times; anything else forgets the count.
    fn note_outcome(&self, id: i64, retry: bool) {
        let mut retries = lock(&self.retries);
        if !retry {
            retries.remove(&id);
            return;
        }
        let attempts = retries.get(&id).map_or(0, |r| r.attempts);
        if attempts >= RETRY_WAITS.len() {
            retries.remove(&id); // gave up: it waits for the person
            return;
        }
        let at = std::time::Instant::now() + self.retry_wait(attempts);
        retries.insert(
            id,
            Retry {
                attempts: attempts + 1,
                at,
            },
        );
    }

    /// Called every few seconds: tries again the failed downloads whose wait is over.
    pub fn tick_retries(self: &Arc<Self>) {
        let now = std::time::Instant::now();
        let due: Vec<i64> = lock(&self.retries)
            .iter()
            .filter(|(_, r)| r.at <= now)
            .map(|(id, _)| *id)
            .collect();
        if due.is_empty() || !self.schedule_allows() {
            return;
        }
        for id in due {
            let still_failed = self
                .store
                .get(id)
                .is_ok_and(|j| j.status == Status::Failed { resumable: true });
            if !still_failed || self.resume(id).is_err() {
                lock(&self.retries).remove(&id);
            }
        }
    }

    /// Called every few seconds: starts the downloads whose start time has come
    /// (B8.5). Ones whose time passed while Fuselane was closed start on launch.
    pub fn tick_starts(self: &Arc<Self>, now_unix: i64) {
        let due: Vec<i64> = self
            .store
            .list()
            .unwrap_or_default()
            .into_iter()
            .filter(|j| j.status == Status::Paused && j.start_at.is_some_and(|at| at <= now_unix))
            .map(|j| j.id)
            .collect();
        for id in due {
            let _ = self.resume(id);
        }
    }

    /// A network appeared or came back: waiting retries go now.
    pub fn retry_now(&self) {
        let now = std::time::Instant::now();
        for r in lock(&self.retries).values_mut() {
            r.at = now;
        }
    }

    /// A plain-text report for bug reports, built to share safely (ADR 0009: no
    /// crash-reporting service). It never contains IP addresses, links or file
    /// names; the user sees it before pasting it anywhere.
    pub fn diagnostics(&self, checks: &[(String, bool, String)]) -> String {
        use std::fmt::Write;
        let mut r = String::new();
        let _ = writeln!(
            r,
            "Fuselane {} on {} {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let _ = writeln!(r, "\nChecks:");
        for (name, ok, detail) in checks {
            let _ = writeln!(r, "  {} {name}: {detail}", if *ok { "ok" } else { "FAIL" });
        }
        let _ = writeln!(r, "\nNetworks:");
        match fuselane_netif::list() {
            Ok(list) => {
                for i in list {
                    let v4 = i.addrs.iter().filter(|a| a.is_ipv4()).count();
                    let v6 = i.addrs.len() - v4;
                    let _ = writeln!(
                        r,
                        "  {} ({}){}: {v4} IPv4, {v6} IPv6",
                        i.name,
                        kind_word(i.kind),
                        if i.usable() { ", used" } else { "" }
                    );
                }
            }
            Err(e) => {
                let _ = writeln!(r, "  couldn't list: {e}");
            }
        }
        let limits = self.limits();
        let _ = writeln!(
            r,
            "\nSpeed limits: overall {} B/s, {} per-network",
            limits.global,
            limits.networks.len()
        );
        let _ = writeln!(r, "\nRecent downloads (newest first):");
        for j in self.store.list().unwrap_or_default().iter().take(20) {
            let scheme = j.url.split("://").next().unwrap_or("?");
            let _ = writeln!(
                r,
                "  #{} {} {}{} written {} of {}",
                j.id,
                scheme,
                j.status.as_str(),
                j.error_code
                    .as_deref()
                    .map(|c| format!(" ({c})"))
                    .unwrap_or_default(),
                j.secured_bytes(),
                j.total.map_or("unknown".into(), |t| t.to_string()),
            );
        }
        r
    }

    pub fn networks(&self) -> Result<Vec<NetView>, UiError> {
        let all = fuselane_netif::list().map_err(|e| {
            UiError::new(
                "networks",
                format!("Couldn't list your networks: {e}"),
                None,
            )
        })?;
        Ok(all
            .iter()
            .map(|i| NetView {
                name: i.name.clone(),
                label: i.display_name.clone(),
                kind: kind_word(i.kind),
                usable: i.usable(),
                addrs: i.addrs.iter().map(ToString::to_string).collect(),
                reach: lock(&self.reach).get(&i.name).map(reach_word),
            })
            .collect())
    }

    /// Usable networks, leaving out ones stuck behind a sign-in page; if that
    /// leaves none, all of them (the download then says what went wrong).
    pub fn download_networks(&self) -> Result<Vec<fuselane_netif::Interface>, String> {
        let all = pick_networks(&[])?;
        Ok(without_portals(all, &lock(&self.reach)))
    }

    /// Checks every usable network for a sign-in page; true if anything changed.
    pub async fn check_reach(&self) -> bool {
        let Ok(nets) = fuselane_netif::usable() else {
            return false;
        };
        let mut set = tokio::task::JoinSet::new();
        for iface in nets {
            set.spawn(async move {
                let r = fuselane_transport::probe::check(&iface, std::time::Duration::from_secs(6))
                    .await;
                (iface.name, r)
            });
        }
        let mut found = HashMap::new();
        while let Some(Ok((name, r))) = set.join_next().await {
            found.insert(name, r);
        }
        let mut reach = lock(&self.reach);
        let changed = *reach != found;
        *reach = found;
        changed
    }

    /// Keeps the checks current: every 60 s, and within 5 s of a network appearing
    /// or going. The window hears about changes as a networks event.
    pub async fn watch_reach(self: Arc<Self>) {
        let mut last_names: Vec<String> = Vec::new();
        let mut last_check: Option<std::time::Instant> = None;
        loop {
            let mut names: Vec<String> = fuselane_netif::usable()
                .map(|v| v.into_iter().map(|i| i.name).collect())
                .unwrap_or_default();
            names.sort();
            let due = last_check.is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(60));
            if names != last_names || due {
                if names.len() > last_names.len() {
                    self.retry_now();
                }
                last_names = names;
                last_check = Some(std::time::Instant::now());
                if self.check_reach().await
                    && let Ok(networks) = self.networks()
                {
                    self.send(UiEvent::Networks { networks });
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    /// Adds a download and starts it when a slot is free.
    pub fn add(self: &Arc<Self>, url: &str, dir: Option<&str>) -> Result<i64, UiError> {
        self.add_with(url, dir, &AddRequest::default())
    }

    /// Adds a download with the person's choices. Refuses a link already in the
    /// list unless `allow_duplicate` (the window then offers "Download again").
    pub fn add_with(
        self: &Arc<Self>,
        url: &str,
        dir: Option<&str>,
        req: &AddRequest,
    ) -> Result<i64, UiError> {
        let url = url.trim();
        if url.is_empty() {
            return Err(UiError::new(
                "bad-link",
                "Paste a link to download.",
                Some("Links start with http:// or https://."),
            ));
        }
        if url.len() > 8192 {
            return Err(UiError::new(
                "bad-link",
                "That link is too long (over 8 KB).",
                Some("Copy the link again from its page."),
            ));
        }
        parse_link(url).map_err(|m| {
            UiError::new("bad-link", m, Some("Links start with http:// or https://."))
        })?;
        let mut mirrors: Vec<String> = Vec::new();
        for m in req
            .mirrors
            .iter()
            .map(|m| m.trim())
            .filter(|m| !m.is_empty())
        {
            parse_link(m).map_err(|why| {
                UiError::new(
                    "bad-mirror",
                    format!("A mirror link isn't usable: {why}"),
                    Some("Mirrors are http:// or https:// links to the same file."),
                )
            })?;
            if m != url && !mirrors.iter().any(|x| x == m) {
                mirrors.push(m.to_string());
            }
        }
        if mirrors.len() > 8 {
            return Err(UiError::new(
                "bad-mirror",
                "That's more than 8 mirrors.",
                Some("Keep the fastest few; more rarely helps."),
            ));
        }
        let dir = match dir.map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => PathBuf::from(d),
            None => self.default_dir.clone(),
        };
        if !dir.is_dir() {
            return Err(UiError::new(
                "folder-missing",
                format!("The folder \"{}\" doesn't exist.", dir.display()),
                Some("Pick another folder to save into."),
            ));
        }
        let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        let name = req
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(String::from);
        if name.as_ref().is_some_and(|n| n.len() > 255) {
            return Err(UiError::new(
                "bad-name",
                "That file name is too long (over 255 bytes).",
                Some("Pick a shorter name."),
            ));
        }
        let sha256 = match req
            .sha256
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            None => None,
            Some(s) => {
                runner::parse_sha256(s).map_err(|m| {
                    UiError::new(
                        "bad-checksum",
                        m,
                        Some("Paste the SHA-256 from the download page: 64 letters and digits."),
                    )
                })?;
                Some(s.to_ascii_lowercase())
            }
        };
        let session =
            fuselane_engine_http::download::Headers::checked(&req.headers).map_err(|m| {
                UiError::new(
                    "bad-headers",
                    format!("The browser's request can't be used: {m}."),
                    None,
                )
            })?;
        if !req.allow_duplicate
            && let Some(existing) = self
                .store
                .list()
                .map_err(store_error)?
                .into_iter()
                .find(|j| j.url == url)
        {
            return Err(UiError::new(
                "duplicate",
                format!("You already added this link ({}).", job_name(&existing)),
                Some("Download it again anyway, or open the one in your list."),
            ));
        }
        let id = self
            .store
            .create_with(url, &dir, &fuselane_core::NewJob { name, sha256 })
            .map_err(store_error)?;
        if req.later || req.start_at.is_some() {
            self.store
                .apply(id, Event::Pause, None)
                .map_err(store_error)?;
        }
        if let Some(at) = req.start_at {
            self.store.set_start_at(id, Some(at)).map_err(store_error)?;
        }
        let replace = req
            .replace
            .unwrap_or(lock(&self.automation).name_taken == crate::automation::NameTaken::Replace);
        if replace {
            self.store
                .set_replace_existing(id, true)
                .map_err(store_error)?;
        }
        if !mirrors.is_empty() {
            self.store.set_mirrors(id, &mirrors).map_err(store_error)?;
        }
        if !session.is_empty() {
            lock(&self.sessions).insert(id, session);
        }
        self.publish_jobs();
        self.pump();
        Ok(id)
    }

    /// Every download's link, one per line in queue order, for saving to a file.
    /// Importing the file adds them back (lines starting with # are ignored).
    pub fn export_text(&self) -> Result<(String, usize), UiError> {
        let mut jobs = self.store.list().map_err(store_error)?;
        jobs.sort_by_key(|j| (j.position, j.id));
        let mut seen = std::collections::HashSet::new();
        let links: Vec<&str> = jobs
            .iter()
            .map(|j| j.url.as_str())
            .filter(|u| seen.insert(*u))
            .collect();
        let mut text = format!(
            "# Fuselane downloads ({} links). Import this file in Fuselane to add them again.\n",
            links.len()
        );
        for l in &links {
            text.push_str(l);
            text.push('\n');
        }
        Ok((text, links.len()))
    }

    /// Adds every link in `text` (one per line, or mixed with other words), each
    /// pattern like `file[01-20].zip` expanded. Links already in the list and bad
    /// ones are skipped with a reason, never added twice.
    pub fn add_batch(
        self: &Arc<Self>,
        text: &str,
        dir: Option<&str>,
        later: bool,
        group: Option<&str>,
    ) -> Result<BatchResult, UiError> {
        let mut links = Vec::new();
        for found in fuselane_core::batch::links_in(text) {
            let expanded = fuselane_core::batch::expand(&found)
                .map_err(|m| UiError::new("too-many", m, None))?;
            for l in expanded {
                if !links.contains(&l) {
                    links.push(l);
                }
            }
            if links.len() > fuselane_core::batch::MAX_LINKS {
                return Err(UiError::new(
                    "too-many",
                    format!(
                        "That's more than {} links at once.",
                        fuselane_core::batch::MAX_LINKS
                    ),
                    Some("Add them in smaller groups."),
                ));
            }
        }
        if links.is_empty() {
            return Err(UiError::new(
                "bad-link",
                "There are no http:// or https:// links in that text.",
                Some(
                    "Paste one link per line, or a pattern like https://example.com/part[01-10].zip.",
                ),
            ));
        }
        let mut result = BatchResult::default();
        let mut added_links = Vec::new();
        for link in links {
            let req = AddRequest {
                later,
                ..AddRequest::default()
            };
            match self.add_with(&link, dir, &req) {
                Ok(id) => {
                    result.added.push(id);
                    added_links.push(link);
                }
                Err(e) => result.skipped.push(Skipped {
                    url: link,
                    reason: e.message,
                }),
            }
        }
        result.group = self.group_added(group, &added_links, &result.added)?;
        Ok(result)
    }

    fn now(&self) -> crate::automation::Moment {
        lock(&self.clock).unwrap_or_else(crate::automation::Moment::now)
    }

    fn schedule_allows(&self) -> bool {
        crate::automation::allowed(&lock(&self.automation).schedule, self.now())
    }

    pub fn automation_view(&self) -> AutomationView {
        use crate::automation::{allowed, clock, minutes_until_change};
        let settings = lock(&self.automation).clone();
        let at = self.now();
        let allowed_now = allowed(&settings.schedule, at);
        let next = minutes_until_change(&settings.schedule, at).map(|m| {
            let when = clock(((u32::from(at.minute) + m) % 1440) as u16);
            let days = (u32::from(at.minute) + m) / 1440;
            let day = match days {
                0 => String::new(),
                1 => " tomorrow".into(),
                n => format!(" in {n} days"),
            };
            if allowed_now {
                format!("Pauses at {when}{day}.")
            } else {
                format!("Starts at {when}{day}.")
            }
        });
        AutomationView {
            settings,
            allowed_now,
            next,
        }
    }

    /// Saves automation settings and applies them now (the schedule may pause or
    /// start downloads at once).
    pub fn set_automation(
        self: &Arc<Self>,
        a: crate::automation::Automation,
    ) -> Result<AutomationView, UiError> {
        let a = a
            .validated()
            .map_err(|m| UiError::new("bad-value", m, None))?;
        let json = serde_json::to_string(&a).map_err(store_error)?;
        self.store
            .set_setting("automation", &json)
            .map_err(store_error)?;
        if a.when_done == crate::automation::WhenDone::Nothing {
            self.cancel_when_done();
        }
        *lock(&self.automation) = a;
        self.tick_schedule();
        Ok(self.automation_view())
    }

    /// Called every few seconds: pauses running downloads when the schedule's
    /// window closes and resumes the ones it paused when it opens.
    pub fn tick_schedule(self: &Arc<Self>) {
        if self.schedule_allows() {
            let ids: Vec<i64> = lock(&self.schedule_paused).drain().collect();
            if ids.is_empty() {
                return;
            }
            for id in ids {
                if let Ok(job) = self.store.get(id)
                    && job.status == Status::Paused
                {
                    let _ = self.store.apply(id, Event::Resume, None);
                }
            }
            self.publish_jobs();
            self.send(UiEvent::Automation {
                view: self.automation_view(),
            });
            self.pump();
        } else {
            let mut paused = lock(&self.schedule_paused);
            let before = paused.len();
            for (id, r) in lock(&self.running).iter() {
                // One that would miss its deadline by waiting keeps going (B9.4).
                if self
                    .store
                    .get(*id)
                    .is_ok_and(|j| self.deadline_needs_now(&j, true))
                {
                    continue;
                }
                if paused.insert(*id) {
                    r.cancel.cancel();
                }
            }
            if paused.len() != before {
                drop(paused);
                self.send(UiEvent::Automation {
                    view: self.automation_view(),
                });
            }
        }
    }

    /// For tests: a fixed local time.
    #[cfg(test)]
    pub fn set_clock(&self, at: crate::automation::Moment) {
        *lock(&self.clock) = Some(at);
    }

    /// For tests: record power actions instead of running them, with a short countdown.
    #[cfg(test)]
    pub fn set_power(&self, f: PowerFn, countdown_secs: u32) {
        *lock(&self.power) = f;
        self.countdown
            .store(countdown_secs, std::sync::atomic::Ordering::SeqCst);
    }

    /// Lets the app run when-done actions itself (Quit closes the window).
    pub fn set_power_action(&self, f: PowerFn) {
        *lock(&self.power) = f;
    }

    /// Lets the app tell the queue that torrents are still busy.
    pub fn set_busy_elsewhere(&self, f: BusyFn) {
        *lock(&self.busy_elsewhere) = Some(f);
    }

    fn busy_elsewhere(&self) -> bool {
        lock(&self.busy_elsewhere).as_ref().is_some_and(|f| f())
    }

    /// Holds the computer awake while anything downloads (when the setting is on).
    pub fn update_awake(&self) {
        let want = lock(&self.automation).keep_awake
            && (!lock(&self.running).is_empty() || self.busy_elsewhere());
        let mut awake = lock(&self.awake);
        if want && awake.is_none() {
            *awake = crate::power::KeepAwake::start();
        } else if !want {
            *awake = None;
        }
    }

    #[cfg(test)]
    pub fn keeping_awake(&self) -> bool {
        lock(&self.awake).is_some()
    }

    /// Once nothing is left to do, starts the countdown to the chosen action.
    fn maybe_when_done(self: &Arc<Self>) {
        use crate::automation::WhenDone;
        let action = lock(&self.automation).when_done;
        if action == WhenDone::Nothing
            || !self.had_work.load(std::sync::atomic::Ordering::SeqCst)
            || !lock(&self.running).is_empty()
            || self.busy_elsewhere()
        {
            return;
        }
        let waiting = self
            .store
            .list()
            .map_or(true, |jobs| jobs.iter().any(|j| j.status == Status::Queued));
        // Queued jobs held by the schedule still count as work to do.
        if waiting {
            return;
        }
        self.had_work
            .store(false, std::sync::atomic::Ordering::SeqCst);
        let cancel = Cancel::new();
        *lock(&self.pending_action) = Some(cancel.clone());
        let seconds = self.countdown.load(std::sync::atomic::Ordering::SeqCst);
        self.send(UiEvent::WhenDone { action, seconds });
        let me = self.clone();
        tokio::spawn(async move {
            let deadline = std::time::Instant::now() + Duration::from_secs(u64::from(seconds));
            while std::time::Instant::now() < deadline {
                if cancel.is_cancelled() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if cancel.is_cancelled() || !lock(&me.running).is_empty() {
                return;
            }
            lock(&me.pending_action).take();
            let run = lock(&me.power).clone();
            run(action);
        });
    }

    /// Stops a pending sleep, shut-down or quit.
    pub fn cancel_when_done(&self) {
        if let Some(c) = lock(&self.pending_action).take() {
            c.cancel();
            self.send(UiEvent::WhenDoneCancelled);
        }
    }

    /// Moves a finished file into Video, Music, Documents… when the person turned
    /// that on and the download went to the default folder (a chosen folder is
    /// respected). Never replaces a file; a failed move leaves it where it was.
    /// Whether a file named `name` (as the engine would clean it) is already in
    /// `dir` (or the default folder), or in its type folder when sorting is on.
    pub fn name_taken(&self, dir: Option<&str>, name: &str) -> bool {
        let clean = fuselane_storage::names::sanitize(name);
        if clean.is_empty() {
            return false;
        }
        let base = dir
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .map_or_else(|| self.default_dir.clone(), PathBuf::from);
        let mut places = vec![base.join(&clean)];
        if lock(&self.automation).sort_by_type
            && dir.is_none_or(|d| d.trim().is_empty())
            && let Some(folder) = crate::automation::category(&clean)
        {
            places.push(self.default_dir.join(folder).join(&clean));
        }
        places.iter().any(|p| p.symlink_metadata().is_ok())
    }

    /// Sets how finished files are opened ("Open it" when done).
    pub fn set_opener(&self, f: OpenFn) {
        *lock(&self.opener) = f;
    }

    /// "Replace" for a name that was taken (B8.6): the engine never overwrites, so
    /// it saved "name (1).ext"; the old file goes to the Trash and the new one
    /// takes its name. Only a regular file is replaced, never a folder or a link.
    fn replace_if_asked(&self, id: i64, path: &Path, total: u64) -> PathBuf {
        let Ok(job) = self.store.get(id) else {
            return path.to_path_buf();
        };
        // The name it was meant to have: the one chosen when adding (cleaned the
        // way the engine cleans it), else the server's.
        let wanted = job
            .chosen_name
            .as_deref()
            .map(fuselane_storage::names::sanitize)
            .filter(|n| !n.is_empty())
            .or_else(|| job.filename.clone());
        let (Some(dir), Some(wanted)) = (path.parent(), wanted) else {
            return path.to_path_buf();
        };
        let target = dir.join(wanted);
        if !job.replace_existing || target == path {
            return path.to_path_buf();
        }
        let is_file = target
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_file());
        if !is_file {
            return path.to_path_buf();
        }
        // Tests delete outright: they must never fill a real Trash.
        let gone = if cfg!(test) {
            std::fs::remove_file(&target).is_ok()
        } else {
            trash::delete(&target).is_ok()
        };
        if gone && std::fs::rename(path, &target).is_ok() {
            let _ = self.store.set_finished(id, &target, total);
            return target;
        }
        path.to_path_buf()
    }

    /// "When a download finishes" (B8.7): open it, or unpack an archive next to it.
    fn after_download(self: &Arc<Self>, id: i64) {
        use crate::automation::AfterDownload;
        let action = lock(&self.automation).after_download;
        let Some(path) = self.store.get(id).ok().and_then(|j| j.final_path) else {
            return;
        };
        match action {
            AfterDownload::Nothing => {}
            AfterDownload::Open => {
                let open = lock(&self.opener).clone();
                open(&path);
            }
            AfterDownload::Unpack => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !crate::unpack::is_archive(name) {
                    return;
                }
                let me = Arc::clone(self);
                std::thread::spawn(move || {
                    let len = std::fs::metadata(&path).map_or(0, |m| m.len());
                    if let Err(why) =
                        crate::unpack::unpack(&path, crate::unpack::Limits::for_archive(len))
                    {
                        let _ = me.store.set_error(
                            id,
                            &format!("Downloaded, but it couldn't be unpacked: {why}."),
                            "unpack",
                        );
                        me.publish_jobs();
                    }
                });
            }
        }
    }

    fn sort_finished(&self, id: i64, dir: &Path, path: &Path, total: u64) {
        if !lock(&self.automation).sort_by_type {
            return;
        }
        let default = std::fs::canonicalize(&self.default_dir).unwrap_or(self.default_dir.clone());
        let dir = std::fs::canonicalize(dir).unwrap_or(dir.to_path_buf());
        if dir != default {
            return;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return;
        };
        let Some(folder) = crate::automation::category(name) else {
            return;
        };
        let target_dir = default.join(folder);
        if std::fs::create_dir_all(&target_dir).is_err() {
            return;
        }
        let target = free_name(&target_dir, name);
        if std::fs::rename(path, &target).is_ok() {
            let _ = self.store.set_finished(id, &target, total);
        }
    }

    pub fn close_to_tray(&self) -> bool {
        self.close_to_tray
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_close_to_tray(&self, on: bool) -> Result<bool, UiError> {
        self.store
            .set_setting("close_to_tray", if on { "true" } else { "false" })
            .map_err(store_error)?;
        self.close_to_tray
            .store(on, std::sync::atomic::Ordering::Relaxed);
        Ok(on)
    }

    pub fn watch_clipboard(&self) -> bool {
        self.watch_clipboard
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_watch_clipboard(&self, on: bool) -> Result<bool, UiError> {
        self.store
            .set_setting("watch_clipboard", if on { "true" } else { "false" })
            .map_err(store_error)?;
        self.watch_clipboard
            .store(on, std::sync::atomic::Ordering::Relaxed);
        Ok(on)
    }

    /// What the clipboard holds when watching starts: never offered afterwards.
    pub fn clipboard_baseline(&self, text: &str) {
        *lock(&self.last_clip) = Some(
            text.chars()
                .take(fuselane_core::clip::MAX_LEN + 1)
                .collect(),
        );
    }

    /// The clipboard now holds `text` (polled about once a second). Returns the link
    /// to offer when watching is on and a new download link was just copied that
    /// isn't in the list yet. Text copied while watching was off never counts later.
    pub fn clipboard_seen(&self, text: &str) -> Option<String> {
        let mut last = lock(&self.last_clip);
        if last.as_deref() == Some(text) {
            return None;
        }
        // Remembered (capped) only to notice the next change; never stored or logged.
        *last = Some(
            text.chars()
                .take(fuselane_core::clip::MAX_LEN + 1)
                .collect(),
        );
        drop(last);
        if !self.watch_clipboard() {
            return None;
        }
        let link = fuselane_core::clip::download_link(text)?;
        let known = self
            .store
            .list()
            .map(|jobs| jobs.iter().any(|j| j.url == link))
            .unwrap_or(false);
        (!known).then_some(link)
    }

    pub fn max_running(&self) -> usize {
        self.max_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How many downloads run at once (1–8); more start right away if allowed.
    pub fn set_max_running(self: &Arc<Self>, n: usize) -> Result<usize, UiError> {
        if !(1..=MAX_RUNNING_LIMIT).contains(&n) {
            return Err(UiError::new(
                "bad-value",
                format!("Pick between 1 and {MAX_RUNNING_LIMIT} downloads at once."),
                None,
            ));
        }
        self.store
            .set_setting("max_running", &n.to_string())
            .map_err(store_error)?;
        self.max_running
            .store(n, std::sync::atomic::Ordering::Relaxed);
        self.pump();
        Ok(n)
    }

    /// Puts these downloads first in the queue, in this order.
    pub fn reorder(self: &Arc<Self>, ids: &[i64]) -> Result<(), UiError> {
        if ids.len() > 10_000 {
            return Err(UiError::new(
                "bad-value",
                "Too many downloads to reorder.",
                None,
            ));
        }
        self.store.reorder(ids).map_err(store_error)?;
        self.publish_jobs();
        Ok(())
    }

    /// Sets one download's own speed limit (0 removes it); applies at once if running.
    pub fn set_job_limit(self: &Arc<Self>, id: i64, rate: u64) -> Result<(), UiError> {
        if rate > MAX_LIMIT {
            return Err(UiError::new(
                "bad-limit",
                "That limit is too high to be a real speed.",
                Some("Use a speed in KB/s or MB/s, or leave it empty for no limit."),
            ));
        }
        self.store.set_speed_limit(id, rate).map_err(|e| match e {
            fuselane_core::StoreError::NotFound(id) => not_found(id),
            e => store_error(e),
        })?;
        if let Some(r) = lock(&self.running).get(&id) {
            r.limit.set_rate(rate);
        }
        self.publish_jobs();
        Ok(())
    }

    pub fn pause(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        self.forget_held(id);
        if let Some(r) = lock(&self.running).get(&id) {
            r.cancel.cancel(); // the task records the pause once progress is saved
            return Ok(());
        }
        let job = self.store.get(id).map_err(|_| not_found(id))?;
        match job.status {
            Status::Queued => {
                self.store
                    .apply(id, Event::Pause, None)
                    .map_err(store_error)?;
                self.publish_jobs();
                Ok(())
            }
            Status::Paused => Ok(()),
            _ => Err(UiError::new(
                "not-running",
                "This download isn't running, so there's nothing to pause.",
                None,
            )),
        }
    }

    pub fn resume(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        if lock(&self.running).contains_key(&id) {
            return Ok(());
        }
        // Starting it by hand (or on time) ends any waiting for a start time.
        if self.store.get(id).is_ok_and(|j| j.start_at.is_some()) {
            let _ = self.store.set_start_at(id, None);
        }
        let job = match runner::job_to_resume(&self.store, &id.to_string()) {
            Ok(j) => j,
            Err(StartError::BadInput(m) | StartError::Setup(m)) => {
                return Err(match self.store.get(id) {
                    Err(_) => not_found(id),
                    Ok(_) => UiError::new(
                        "not-resumable",
                        m,
                        Some("Add the link again to download it from the start."),
                    ),
                });
            }
        };
        if job.status != Status::Queued {
            self.store
                .apply(id, Event::Resume, None)
                .map_err(store_error)?;
        }
        self.publish_jobs();
        self.pump();
        Ok(())
    }

    /// Allowances and usage for every usable network (plus any with an allowance).
    pub fn allowances(&self, today: Date) -> Vec<AllowanceView> {
        let saved = lock(&self.allowances).clone();
        let mut names: Vec<String> = fuselane_netif::list()
            .map(|l| {
                l.into_iter()
                    .filter(|i| i.usable())
                    .map(|i| i.name)
                    .collect()
            })
            .unwrap_or_default();
        for a in &saved {
            if !names.contains(&a.name) {
                names.push(a.name.clone());
            }
        }
        let usage = lock(&self.usage);
        names
            .into_iter()
            .map(|name| {
                let a = saved.iter().find(|a| a.name == name);
                let reset_day = a.map_or(1, |a| a.reset_day);
                let period = Period::MonthFrom(reset_day);
                let mut u = usage
                    .get(&name)
                    .copied()
                    .unwrap_or_else(|| Usage::new(period, today));
                u.period = period;
                u.roll(today);
                AllowanceView {
                    reached: u.reached(a.map(|a| a.bytes)),
                    allowance: a.map(|a| a.bytes),
                    reset_day,
                    used: u.bytes,
                    resets_on: date_text(next_reset(period, today)),
                    name,
                }
            })
            .collect()
    }

    /// Sets (bytes > 0) or removes (bytes = 0) a network's monthly allowance.
    pub fn set_allowance(
        self: &Arc<Self>,
        req: AllowanceRequest,
        today: Date,
    ) -> Result<Vec<AllowanceView>, UiError> {
        let bad = |m: &str| {
            UiError::new(
                "bad-allowance",
                m,
                Some("Use an amount like 5 GB and a reset day from 1 to 28."),
            )
        };
        if !valid_device(&req.name) {
            return Err(bad("That isn't a network on this computer."));
        }
        if !(1..=28).contains(&req.reset_day) {
            return Err(bad("The reset day must be from 1 to 28."));
        }
        if req.bytes > MAX_ALLOWANCE {
            return Err(bad("That allowance is too big to be real."));
        }
        let mut all = lock(&self.allowances).clone();
        all.retain(|a| a.name != req.name);
        if req.bytes > 0 {
            all.push(Allowance {
                name: req.name.clone(),
                bytes: req.bytes,
                reset_day: req.reset_day,
            });
        }
        if all.len() > 64 {
            return Err(bad("Too many allowances."));
        }
        all.sort_by(|a, b| a.name.cmp(&b.name));
        let json = serde_json::to_string(&all).map_err(store_error)?;
        self.store
            .set_setting("allowances", &json)
            .map_err(store_error)?;
        *lock(&self.allowances) = all;
        self.tick_usage(today);
        Ok(self.allowances(today))
    }

    /// Adds the bytes downloaded since the last tick to each network's usage, saves
    /// it, and makes networks past their allowance stand aside. When every usable
    /// network is past its allowance, running downloads pause with a reason.
    pub fn tick_usage(self: &Arc<Self>, today: Date) {
        let drained = self.limiter.drain_usage();
        let saved = lock(&self.allowances).clone();
        let reset_of = |name: &str| {
            saved
                .iter()
                .find(|a| a.name == name)
                .map_or(1, |a| a.reset_day)
        };
        let (blocked, snapshot) = {
            let mut usage = lock(&self.usage);
            for (name, bytes) in &drained {
                let period = Period::MonthFrom(reset_of(name));
                let u = usage
                    .entry(name.clone())
                    .or_insert_with(|| Usage::new(period, today));
                u.period = period;
                u.add(*bytes, today);
            }
            for (name, u) in usage.iter_mut() {
                u.period = Period::MonthFrom(reset_of(name));
                u.roll(today);
            }
            let blocked: Vec<String> = saved
                .iter()
                .filter(|a| usage.get(&a.name).is_some_and(|u| u.reached(Some(a.bytes))))
                .map(|a| a.name.clone())
                .collect();
            let snapshot: Vec<SavedUsage> = usage
                .iter()
                .map(|(n, u)| SavedUsage {
                    name: n.clone(),
                    start: date_text(u.start),
                    bytes: u.bytes,
                })
                .collect();
            (blocked, snapshot)
        };
        if !drained.is_empty()
            && let Ok(json) = serde_json::to_string(&snapshot)
        {
            let _ = self.store.set_setting("usage", &json);
        }
        self.limiter.set_blocked(blocked.iter().cloned());
        // Every usable network past its allowance: pause instead of waiting silently.
        let usable: Vec<String> = fuselane_netif::list()
            .map(|l| {
                l.into_iter()
                    .filter(|i| i.usable())
                    .map(|i| i.name)
                    .collect()
            })
            .unwrap_or_default();
        if !usable.is_empty() && usable.iter().all(|n| blocked.contains(n)) {
            let running = lock(&self.running);
            let mut paused = lock(&self.allowance_paused);
            for (id, r) in running.iter() {
                if paused.insert(*id) {
                    r.cancel.cancel();
                }
            }
        }
    }

    /// The previous version, if this is the first launch after an update.
    pub fn updated_from(&self) -> Option<&str> {
        self.updated_from.as_deref()
    }

    pub fn per_network_dns(&self) -> bool {
        self.per_network_dns
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Turns per-network lookups on or off; new downloads follow it.
    pub fn set_per_network_dns(&self, on: bool) -> Result<bool, UiError> {
        self.store
            .set_setting("per_network_dns", if on { "true" } else { "false" })
            .map_err(store_error)?;
        self.per_network_dns
            .store(on, std::sync::atomic::Ordering::Relaxed);
        Ok(on)
    }

    pub fn limits(&self) -> LimitsView {
        lock(&self.saved_limits).clone()
    }

    /// Saves new limits and applies them to running downloads at once.
    pub fn set_limits(&self, view: LimitsView) -> Result<LimitsView, UiError> {
        let view = view.validated()?;
        let json = serde_json::to_string(&view).map_err(store_error)?;
        self.store
            .set_setting("limits", &json)
            .map_err(store_error)?;
        self.limiter.apply(&view.to_settings());
        *lock(&self.saved_limits) = view.clone();
        Ok(view)
    }

    /// Turns slow mode on or off, keeping every other limit as saved.
    pub fn set_slow(&self, on: bool) -> Result<LimitsView, UiError> {
        let mut view = self.limits();
        view.slow = on;
        self.set_limits(view)
    }

    pub fn network_prefs(&self) -> Vec<NetPref> {
        lock(&self.net_prefs).clone()
    }

    /// Saves a network's name and colour; clearing both forgets the network.
    pub fn set_network_pref(&self, pref: NetPref) -> Result<Vec<NetPref>, UiError> {
        let pref = pref.validated()?;
        let mut all = lock(&self.net_prefs).clone();
        all.retain(|p| p.name != pref.name);
        if worth::keep(&pref) {
            all.push(pref);
        }
        if all.len() > 64 {
            return Err(UiError::new(
                "bad-network-name",
                "Too many renamed networks.",
                None,
            ));
        }
        all.sort_by(|a, b| a.name.cmp(&b.name));
        let json = serde_json::to_string(&all).map_err(store_error)?;
        self.store
            .set_setting("network_prefs", &json)
            .map_err(store_error)?;
        *lock(&self.net_prefs) = all.clone();
        Ok(all)
    }

    /// Continues a stopped download from a new link to the same file (signed links
    /// expire). The engine proves it's the same file before keeping any bytes; a
    /// different file fails with `start-over` instead of being mixed in (L-108).
    pub fn fix_link(self: &Arc<Self>, id: i64, url: &str) -> Result<(), UiError> {
        let url = url.trim();
        parse_link(url).map_err(|m| {
            UiError::new("bad-link", m, Some("Links start with http:// or https://."))
        })?;
        if lock(&self.running).contains_key(&id) {
            return Err(UiError::new(
                "not-stopped",
                "Pause the download before changing its link.",
                None,
            ));
        }
        let job = self.store.get(id).map_err(|_| not_found(id))?;
        if !matches!(
            job.status,
            Status::Paused | Status::Failed { resumable: true } | Status::Queued
        ) {
            return Err(UiError::new(
                "not-resumable",
                "This download can't continue from a new link.",
                Some("Start it again instead."),
            ));
        }
        self.store.set_url(id, url).map_err(store_error)?;
        self.resume(id)
    }

    /// Throws away a download that can't continue and starts it fresh from the
    /// same link and folder.
    pub fn start_over(self: &Arc<Self>, id: i64) -> Result<i64, UiError> {
        if lock(&self.running).contains_key(&id) {
            return Err(UiError::new(
                "not-stopped",
                "Pause the download before starting it over.",
                None,
            ));
        }
        let job = self.store.get(id).map_err(|_| not_found(id))?;
        if job.status == Status::Completed {
            return Err(UiError::new(
                "not-resumable",
                "This download already finished.",
                None,
            ));
        }
        runner::remove(&self.store, id).map_err(store_error)?;
        self.add(&job.url, Some(&job.dir.to_string_lossy()))
    }

    /// Removes a download (and its partial file). A running one is stopped first.
    pub fn remove(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        if let Some(r) = lock(&self.running).get_mut(&id) {
            r.remove_after = true;
            r.cancel.cancel();
            return Ok(());
        }
        runner::remove(&self.store, id).map_err(|_| not_found(id))?;
        self.forget_held(id);
        if self.focused() == Some(id) {
            self.unfocus();
        }
        self.publish_jobs();
        Ok(())
    }

    /// Moves a finished download's file to the Trash (Recycle Bin) and removes it
    /// from the list. Never deletes outright, so a slip can be undone.
    pub fn trash_file(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        let path = self.finished_file(id)?;
        trash::delete(&path).map_err(|e| {
            UiError::new(
                "trash-failed",
                format!("Couldn't move the file to the Trash: {e}"),
                Some("Delete it from its folder instead, then remove it from the list."),
            )
        })?;
        runner::remove(&self.store, id).map_err(|_| not_found(id))?;
        self.publish_jobs();
        Ok(())
    }

    /// Where a finished download's file is, checked to still exist. The window
    /// passes an id, never a path, so it can't open arbitrary files (L-98).
    pub fn finished_file(&self, id: i64) -> Result<PathBuf, UiError> {
        let job = self.store.get(id).map_err(|_| not_found(id))?;
        let path = match (job.status, job.final_path) {
            (Status::Completed, Some(p)) => p,
            _ => {
                return Err(UiError::new(
                    "not-finished",
                    "This download hasn't finished yet, so there's no file to show.",
                    None,
                ));
            }
        };
        if !path.is_file() {
            return Err(UiError::new(
                "file-missing",
                format!("The file is no longer at {}.", path.display()),
                Some("It may have been moved, renamed or deleted."),
            ));
        }
        Ok(path)
    }

    /// Pauses everything that's running (used when the app quits).
    pub fn pause_all(&self) {
        for r in lock(&self.running).values() {
            r.cancel.cancel();
        }
    }

    pub fn running(&self) -> usize {
        lock(&self.running).len()
    }

    /// Starts queued jobs, oldest first, while there are free slots.
    fn pump(self: &Arc<Self>) {
        if !self.battery_allows() {
            self.update_awake();
            return;
        }
        // Outside the schedule only a download that would miss its deadline runs.
        let scheduled = self.schedule_allows();
        let Ok(mut jobs) = self.store.list() else {
            return;
        };
        // Deadlines first, earliest first (B9.4); then the order chosen.
        jobs.sort_by_key(|j| (j.ready_by.unwrap_or(i64::MAX), j.position, j.id));
        let mut running = lock(&self.running);
        for job in jobs.into_iter().filter(|j| j.status == Status::Queued) {
            if running.len() >= self.max_running() {
                break;
            }
            if running.contains_key(&job.id) || !self.focus_allows(job.id) {
                continue;
            }
            if !scheduled && !self.deadline_needs_now(&job, false) {
                continue;
            }
            let cancel = Cancel::new();
            let limit = Arc::new(fuselane_limits::JobLimit::new(job.speed_limit));
            running.insert(
                job.id,
                Running {
                    cancel: cancel.clone(),
                    remove_after: false,
                    replan: false,
                    limit: limit.clone(),
                },
            );
            self.had_work
                .store(true, std::sync::atomic::Ordering::SeqCst);
            // New work cancels a pending sleep or shut-down.
            if let Some(c) = lock(&self.pending_action).take() {
                c.cancel();
            }
            let me = self.clone();
            tokio::spawn(async move { me.run(job, cancel, limit).await });
        }
        drop(running);
        self.update_awake();
    }

    async fn run(self: Arc<Self>, job: Job, cancel: Cancel, limit: Arc<fuselane_limits::JobLimit>) {
        let id = job.id;
        let job = self.with_found_checksum(job).await;
        // Networks behind a sign-in page are left out (they'd serve the login page),
        // and "long downloads only" networks wait until this one proves long (B9.5).
        let (picked, held_back) = self.networks_by_use(
            &job,
            self.without_phone_if_low(self.download_networks().unwrap_or_default()),
        );
        let nets: Vec<(String, String, String)> = picked
            .iter()
            .map(|i| (i.name.clone(), i.display_name.clone(), kind_word(i.kind)))
            .collect();
        let started = std::time::Instant::now();
        let last: Arc<Mutex<Option<Live>>> = Arc::default();
        let snapshot = {
            let me = self.clone();
            let last = last.clone();
            SnapshotFn(Arc::new(move |s: &Snapshot| {
                let l = live(id, s, &nets);
                me.note_rates(&l.networks);
                *lock(&last) = Some(l.clone());
                me.send(UiEvent::Live(l));
            }))
        };
        let opts = RunOptions {
            networks: picked.iter().map(|i| i.name.clone()).collect(),
            snapshot: Some(snapshot),
            cancel: Some(cancel),
            retry_delay_scale: self.retry_scale,
            limiter: Some(self.limiter.clone()),
            job_limit: Some(limit),
            per_network_dns: self
                .per_network_dns
                .load(std::sync::atomic::Ordering::Relaxed),
            filename: job.chosen_name.clone(),
            headers: lock(&self.sessions).get(&id).cloned().unwrap_or_default(),
            mirrors: job.mirrors.clone(),
            mirror_notes: (!job.mirrors.is_empty()).then(|| {
                let notes = Arc::new(Mutex::new(Vec::new()));
                lock(&self.mirror_notes).insert(id, notes.clone());
                notes
            }),
            sha256: job
                .expected_sha256
                .as_deref()
                .and_then(|s| runner::parse_sha256(s).ok()),
            ..RunOptions::default()
        };
        if !held_back.is_empty() {
            self.watch_if_long(id, last.clone());
        }
        let resume = job.resume();
        let outcome = runner::run(
            self.store.clone(),
            id,
            &job.url,
            job.dir.clone(),
            resume,
            opts,
        )
        .await;
        match &outcome {
            Err(e) => {
                // Setup failed before the engine started: record it, don't retry forever.
                let message = e.to_string();
                let _ = self.store.apply(id, Event::Start, None);
                let _ = self
                    .store
                    .apply(id, Event::Fail { resumable: true }, Some(&message));
                let _ = self.store.set_error_code(id, "retry");
            }
            Ok(Outcome::Completed { .. } | Outcome::Paused | Outcome::Failed { .. }) => {}
        }
        if let Ok(Outcome::Completed { report, .. }) = &outcome {
            if let Some(l) = lock(&last).take() {
                let run = savings::RunReport {
                    secs: started.elapsed().as_secs_f64(),
                    nets: l
                        .networks
                        .iter()
                        .map(|n| savings::NetBytes {
                            label: if n.label.is_empty() {
                                n.name.clone()
                            } else {
                                n.label.clone()
                            },
                            bytes: n.bytes,
                        })
                        .collect(),
                };
                if let Ok(json) = serde_json::to_string(&run) {
                    let _ = self.store.set_report(id, &json);
                }
            }
            let path = self.replace_if_asked(id, &report.path, report.total);
            self.sort_finished(id, &job.dir, &path, report.total);
            self.after_download(id);
        }
        if lock(&self.schedule_paused).contains(&id) {
            let next = self.automation_view().next.unwrap_or_default();
            let _ =
                self.store
                    .set_error(id, &format!("Waiting for the schedule. {next}"), "schedule");
        }
        if lock(&self.battery_paused).contains(&id) {
            let _ = self.store.set_error(
                id,
                "Paused: the battery is low. It carries on when you plug in.",
                "battery",
            );
        }
        if lock(&self.allowance_paused).remove(&id) {
            let _ = self.store.set_error(
                id,
                "Paused: every network reached its data allowance. It continues after the allowance resets, or raise it in Networks.",
                "allowance",
            );
        }
        let retry = match &outcome {
            Err(_) => true,
            Ok(Outcome::Failed {
                error,
                resumable: true,
            }) => runner::action_for(error) == "retry",
            Ok(_) => false,
        };
        self.note_outcome(id, retry);
        let (removed, replan) = lock(&self.running)
            .remove(&id)
            .map_or((false, false), |r| (r.remove_after, r.replan));
        // It turned out long: carry on at once with the networks held back.
        if replan && matches!(&outcome, Ok(Outcome::Paused)) {
            let _ = self.store.apply(id, Event::Resume, None);
        }
        if !matches!(&outcome, Ok(Outcome::Paused)) || removed {
            lock(&self.known_long).remove(&id);
        }
        if let Some(f) = self.focused().filter(|f| *f != id)
            && let Ok(j) = self.store.get(f)
        {
            self.note_held(&job_name(&j));
        }
        self.focus_ended(id);
        if removed {
            let _ = runner::remove(&self.store, id);
        }
        // A finished or removed download needs its session no more.
        if removed || matches!(&outcome, Ok(Outcome::Completed { .. })) {
            lock(&self.sessions).remove(&id);
        }
        self.publish_jobs();
        self.pump();
        self.maybe_when_done();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuselane_testkit::{Content, Fault, RangeServer, Rule};
    use std::time::{Duration, Instant};

    const KB: u64 = 1024;

    struct Harness {
        svc: Arc<Service>,
        events: Arc<Mutex<Vec<UiEvent>>>,
        dir: tempfile::TempDir,
    }

    fn harness(max: usize) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("fuselane.db")).unwrap();
        let svc = Service::with_max_running(store, dir.path().to_path_buf(), max).unwrap();
        // Test servers count requests and script faults; a checksum lookup would
        // use them up. The checksum test turns it back on.
        svc.find_checksums
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let events: Arc<Mutex<Vec<UiEvent>>> = Arc::default();
        svc.subscribe({
            let events = events.clone();
            Arc::new(move |e| lock(&events).push(e))
        });
        Harness { svc, events, dir }
    }

    impl Harness {
        fn job(&self, id: i64) -> JobView {
            self.svc
                .jobs()
                .unwrap()
                .into_iter()
                .find(|j| j.id == id)
                .unwrap()
        }

        async fn wait(&self, what: &str, f: impl Fn(&Harness) -> bool) {
            let start = Instant::now();
            while !f(self) {
                assert!(
                    start.elapsed() < Duration::from_secs(20),
                    "timed out waiting for {what}: {:?}",
                    self.svc
                        .jobs()
                        .unwrap()
                        .iter()
                        .map(|j| (j.id, j.status, j.written, j.error.clone()))
                        .collect::<Vec<_>>()
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }

        fn lives(&self, id: i64) -> Vec<Live> {
            lock(&self.events)
                .iter()
                .filter_map(|e| match e {
                    UiEvent::Live(l) if l.id == id => Some(l.clone()),
                    _ => None,
                })
                .collect()
        }
    }

    fn link(server: &RangeServer) -> String {
        format!("http://{}{}", server.addr(), server.path())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_completes_and_streams_live_updates() {
        let content = Content::new(3 * 1024 * KB, 91);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(1500 * KB),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        let job = h.job(id);
        let path = PathBuf::from(job.final_path.unwrap());
        assert_eq!(
            fuselane_testkit::sha256_file(&path).unwrap(),
            content.sha256()
        );
        assert_eq!(job.written, content.size);
        let lives = h.lives(id);
        assert!(!lives.is_empty(), "no live updates");
        for l in &lives {
            assert_eq!(l.ticks.len() % 3, 0);
            assert!(
                l.ticks
                    .chunks(3)
                    .all(|t| t[0] <= 100 && (t[1] as usize) <= l.networks.len())
            );
            assert!(l.networks.iter().all(|n| !n.name.is_empty()));
        }
        // The window gets a JSON it can parse: camelCase, tagged.
        let json = serde_json::to_string(&UiEvent::Live(lives[0].clone())).unwrap();
        assert!(json.starts_with(r#"{"type":"live","id":"#), "{json}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bad_links_and_folders_are_refused_before_anything_is_saved() {
        let h = harness(3);
        for bad in [
            "",
            "   ",
            "ftp://example.com/f",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "magnet:?xt=urn:btih:abc",
            "not a link",
            &format!("https://example.com/{}", "a".repeat(9000)),
        ] {
            let e = h.svc.add(bad, None).unwrap_err();
            assert_eq!(e.code, "bad-link", "{bad:?}");
            assert!(e.hint.is_some());
        }
        let e = h
            .svc
            .add("https://example.com/f", Some("/definitely/not/a/folder"))
            .unwrap_err();
        assert_eq!(e.code, "folder-missing");
        // A file is not a folder either.
        let file = h.dir.path().join("plain.txt");
        std::fs::write(&file, b"x").unwrap();
        let e = h
            .svc
            .add("https://example.com/f", file.to_str())
            .unwrap_err();
        assert_eq!(e.code, "folder-missing");
        assert!(h.svc.jobs().unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_then_resume_is_byte_exact() {
        let content = Content::new(4 * 1024 * KB, 92);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(600 * KB),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("some progress", |h| {
            h.lives(id).iter().any(|l| l.written > 256 * KB)
        })
        .await;
        h.svc.pause(id).unwrap();
        h.wait("paused", |h| {
            h.job(id).status == "paused" && h.svc.running() == 0
        })
        .await;
        assert!(h.job(id).resumable);
        // Pausing twice is harmless.
        h.svc.pause(id).unwrap();
        h.svc.resume(id).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        let path = PathBuf::from(h.job(id).final_path.unwrap());
        assert_eq!(
            fuselane_testkit::sha256_file(&path).unwrap(),
            content.sha256()
        );
        // A finished download can't be resumed or paused.
        assert_eq!(h.svc.resume(id).unwrap_err().code, "not-resumable");
        assert_eq!(h.svc.pause(id).unwrap_err().code, "not-running");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copied_download_links_are_offered_once_and_only_when_watching() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fuselane.db");
        let svc = Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        // Nothing listens there, so the added download fails fast and harmlessly.
        let iso = "http://127.0.0.1:9/os.iso";
        // Off by default: nothing is offered, and what was copied meanwhile is old news.
        assert!(!svc.watch_clipboard());
        assert_eq!(svc.clipboard_seen(iso), None);
        svc.set_watch_clipboard(true).unwrap();
        assert_eq!(svc.clipboard_seen(iso), None, "copied before turning it on");
        assert_eq!(svc.clipboard_seen("some text"), None);
        assert_eq!(svc.clipboard_seen("https://example.com/blog"), None);
        assert_eq!(svc.clipboard_seen(iso).as_deref(), Some(iso));
        assert_eq!(svc.clipboard_seen(iso), None, "once per copy");
        // A link already in the list isn't offered again.
        svc.add(iso, None).unwrap();
        svc.clipboard_seen("x");
        assert_eq!(svc.clipboard_seen(iso), None);
        // The choice is kept.
        drop(svc);
        let svc = Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        assert!(svc.watch_clipboard());
        svc.set_watch_clipboard(false).unwrap();
        assert_eq!(svc.clipboard_seen("https://example.com/b.zip"), None);
    }

    #[test]
    fn passwords_in_links_are_masked_for_the_window() {
        assert_eq!(
            shown_url("https://me:secret@example.com/a.iso"),
            "https://me:****@example.com/a.iso"
        );
        assert_eq!(
            shown_url("https://example.com/a.iso?x=1"),
            "https://example.com/a.iso?x=1"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_list_exports_as_links_and_imports_back() {
        let dir = tempfile::tempdir().unwrap();
        let svc = Service::new(
            Store::open(&dir.path().join("a.db")).unwrap(),
            dir.path().to_path_buf(),
        )
        .unwrap();
        let later = AddRequest {
            later: true,
            ..AddRequest::default()
        };
        for l in ["http://127.0.0.1:9/a.iso", "http://127.0.0.1:9/b.zip"] {
            svc.add_with(l, None, &later).unwrap();
        }
        let (text, n) = svc.export_text().unwrap();
        assert_eq!(n, 2);
        assert!(text.starts_with("# Fuselane downloads (2 links)"));
        assert!(text.ends_with("http://127.0.0.1:9/a.iso\nhttp://127.0.0.1:9/b.zip\n"));
        // Into an empty list: both come back, waiting.
        let other = Service::new(
            Store::open(&dir.path().join("b.db")).unwrap(),
            dir.path().to_path_buf(),
        )
        .unwrap();
        let r = other.add_batch(&text, None, true, None).unwrap();
        assert_eq!(r.added.len(), 2);
        assert!(r.skipped.is_empty());
        // Importing twice skips what's already there.
        let again = other.add_batch(&text, None, true, None).unwrap();
        assert_eq!((again.added.len(), again.skipped.len()), (0, 2));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn download_later_waits_until_started_and_then_finishes() {
        let content = Content::new(256 * KB, 95);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let req = AddRequest {
            later: true,
            ..AddRequest::default()
        };
        let id = h.svc.add_with(&link(&server), None, &req).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(h.job(id).status, "paused");
        assert!(h.job(id).resumable);
        assert_eq!(h.svc.running(), 0);
        h.svc.resume(id).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        // A batch can wait too.
        let text = format!("{0}?a\n{0}?b", link(&server));
        let r = h.svc.add_batch(&text, None, true, None).unwrap();
        assert_eq!(r.added.len(), 2);
        for id in r.added {
            assert_eq!(h.job(id).status, "paused");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_with_a_start_time_waits_then_starts_by_itself() {
        let content = Content::new(256 * KB, 97);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let at = 2_000_000_000; // a fixed "now" for the test's clock
        let req = AddRequest {
            start_at: Some(at),
            ..AddRequest::default()
        };
        let id = h.svc.add_with(&link(&server), None, &req).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(h.job(id).status, "paused");
        assert_eq!(h.job(id).start_at, Some(at));
        // A tick a minute early does nothing.
        h.svc.tick_starts(at - 60);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!((h.job(id).status, h.svc.running()), ("paused", 0));
        // On time: it starts, the time is cleared, and it finishes.
        h.svc.tick_starts(at);
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        assert_eq!(h.job(id).start_at, None);

        // Started by hand before its time: the schedule is dropped, not kept.
        let req = AddRequest {
            start_at: Some(at + 3600),
            allow_duplicate: true,
            ..AddRequest::default()
        };
        let id = h
            .svc
            .add_with(&format!("{}?again", link(&server)), None, &req)
            .unwrap();
        h.svc.resume(id).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        assert_eq!(h.svc.store.get(id).unwrap().start_at, None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_that_fails_for_a_passing_reason_tries_again_by_itself() {
        let content = Content::new(256 * KB, 96);
        let server = RangeServer::start(content).await.unwrap();
        // The server is unreachable at first (the network dropped), then fine.
        server.add_rule(Rule {
            skip: 0,
            times: 4,
            fault: Fault::Reset,
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("the failure", |h| h.job(id).status == "failed")
            .await;
        let job = h.job(id);
        assert_eq!(job.error_action.as_deref(), Some("retry"));
        assert!(job.retry_in.is_some(), "a retry is planned");
        // The app's 5-second tick; tests shrink the waits.
        let start = Instant::now();
        while h.job(id).status != "completed" {
            assert!(start.elapsed() < Duration::from_secs(30), "{:?}", h.job(id));
            h.svc.tick_retries();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert_eq!(h.job(id).retry_in, None);
        assert!(lock(&h.svc.retries).is_empty(), "success forgets the count");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn only_a_finished_file_can_go_to_the_trash() {
        let h = harness(1);
        let req = AddRequest {
            later: true,
            ..AddRequest::default()
        };
        let id = h
            .svc
            .add_with("http://127.0.0.1:9/a.iso", None, &req)
            .unwrap();
        assert_eq!(h.svc.trash_file(id).unwrap_err().code, "not-finished");
        assert_eq!(h.job(id).status, "paused", "still listed");
        assert_eq!(h.svc.trash_file(9999).unwrap_err().code, "not-found");
    }

    #[test]
    fn retries_stop_after_the_last_wait() {
        let dir = tempfile::tempdir().unwrap();
        let svc = Service::new(
            Store::open(&dir.path().join("fuselane.db")).unwrap(),
            dir.path().to_path_buf(),
        )
        .unwrap();
        for n in 1..=RETRY_WAITS.len() {
            svc.note_outcome(7, true);
            assert_eq!(lock(&svc.retries)[&7].attempts, n);
        }
        svc.note_outcome(7, true);
        assert!(lock(&svc.retries).is_empty(), "gave up after the last wait");
        svc.note_outcome(8, true);
        svc.note_outcome(8, false);
        assert!(
            lock(&svc.retries).is_empty(),
            "any other outcome forgets it"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_limit_applies_while_running_and_is_kept() {
        let content = Content::new(2 * 1024 * KB, 94);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        // 256 KiB/s from the start: 2 MiB can't finish in under a few seconds.
        h.svc.set_job_limit(id, 256 * KB).unwrap();
        assert_eq!(h.job(id).speed_limit, 256 * KB);
        let start = Instant::now();
        h.wait("some progress", |h| {
            h.lives(id).iter().any(|l| l.written > 128 * KB)
        })
        .await;
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_ne!(h.job(id).status, "completed", "limit ignored");
        // Lifting it lets the rest arrive at full speed.
        h.svc.set_job_limit(id, 0).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        assert!(start.elapsed() < Duration::from_secs(8));
        assert_eq!(h.job(id).speed_limit, 0);
        assert_eq!(
            h.svc.set_job_limit(id, MAX_LIMIT + 1).unwrap_err().code,
            "bad-limit"
        );
        assert_eq!(h.svc.set_job_limit(9999, 1).unwrap_err().code, "not-found");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn removing_a_running_download_deletes_its_partial_file() {
        let content = Content::new(4 * 1024 * KB, 93);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(300 * KB),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("some progress", |h| !h.lives(id).is_empty()).await;
        h.svc.remove(id).unwrap();
        h.wait("removal", |h| {
            h.svc.jobs().unwrap().is_empty() && h.svc.running() == 0
        })
        .await;
        let leftovers: Vec<_> = std::fs::read_dir(h.dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "fuselane"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "partial file left behind: {leftovers:?}"
        );
        assert_eq!(h.svc.remove(id).unwrap_err().code, "not-found");
        assert_eq!(h.svc.pause(999).unwrap_err().code, "not-found");
        assert_eq!(h.svc.resume(999).unwrap_err().code, "not-found");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_queue_runs_one_at_a_time_when_asked() {
        let a = Content::new(1024 * KB, 94);
        let b = Content::new(1024 * KB, 95);
        let sa = RangeServer::start(a).await.unwrap();
        let sb = RangeServer::start(b).await.unwrap();
        sa.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(800 * KB),
        });
        let h = harness(1);
        let ia = h.svc.add(&link(&sa), None).unwrap();
        let ib = h.svc.add(&link(&sb), None).unwrap();
        h.wait("first running", |h| h.job(ia).status == "running")
            .await;
        assert_eq!(h.job(ib).status, "queued");
        assert_eq!(h.svc.running(), 1);
        h.wait("both done", |h| {
            h.job(ia).status == "completed" && h.job(ib).status == "completed"
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn do_this_one_now_holds_the_others_then_they_carry_on() {
        let slow = |seed| async move {
            let s = RangeServer::start(Content::new(1024 * KB, seed))
                .await
                .unwrap();
            s.add_rule(Rule {
                skip: 1,
                times: u32::MAX,
                fault: Fault::Throttle(400 * KB),
            });
            s
        };
        let (sa, sb, sc) = (slow(141).await, slow(142).await, slow(143).await);
        let h = harness(2);
        let ia = h.svc.add(&link(&sa), None).unwrap();
        let ib = h.svc.add(&link(&sb), None).unwrap();
        let ic = h.svc.add(&link(&sc), None).unwrap();
        h.wait("two running", |h| {
            h.job(ia).status == "running" && h.job(ib).status == "running"
        })
        .await;
        assert_eq!(h.job(ic).status, "queued");

        h.svc.focus(ic).unwrap();
        h.wait("only the focused one runs", |h| {
            h.job(ic).status == "running"
                && h.job(ia).status == "paused"
                && h.job(ib).status == "paused"
        })
        .await;
        assert!(h.job(ic).focused);
        assert_eq!(h.svc.running(), 1, "it has every network to itself");
        assert!(h.job(ia).error.unwrap_or_default().contains("goes first"));
        assert_eq!(h.job(ia).error_action.as_deref(), Some("focus"));

        h.wait("all done, the held ones by themselves", |h| {
            [ia, ib, ic].iter().all(|i| h.job(*i).status == "completed")
        })
        .await;
        assert_eq!(h.svc.focused(), None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_paused_by_hand_while_held_stays_paused() {
        let sa = RangeServer::start(Content::new(1024 * KB, 151))
            .await
            .unwrap();
        sa.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(300 * KB),
        });
        let sb = RangeServer::start(Content::new(256 * KB, 152))
            .await
            .unwrap();
        sb.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(128 * KB),
        });
        let h = harness(2);
        let ia = h.svc.add(&link(&sa), None).unwrap();
        h.wait("running", |h| h.job(ia).status == "running").await;
        let ib = h.svc.add(&link(&sb), None).unwrap();
        h.svc.focus(ib).unwrap();
        h.wait("held", |h| h.job(ia).status == "paused").await;
        h.svc.pause(ia).unwrap();
        h.wait("focus done", |h| h.job(ib).status == "completed")
            .await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(h.job(ia).status, "paused", "a pause by hand is kept");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unreachable_server_fails_with_a_plain_message_instead_of_hanging() {
        // Bind then drop a listener so nothing answers on the port.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let h = harness(3);
        let id = h
            .svc
            .add(&format!("http://127.0.0.1:{port}/f.bin"), None)
            .unwrap();
        h.wait("failure", |h| h.job(id).status.starts_with("failed"))
            .await;
        let job = h.job(id);
        assert_eq!(job.name, "f.bin");
        let msg = job.error.unwrap_or_default();
        assert!(msg.contains("Couldn't reach"), "{msg}");
        assert!(job.resumable);
        assert_eq!(h.svc.running(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_server_that_lies_about_the_file_is_reported_not_saved() {
        let content = Content::new(1024 * KB, 96);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::Status(404, None),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("failure", |h| h.job(id).status.starts_with("failed"))
            .await;
        assert!(h.job(id).error.unwrap_or_default().contains("404"));
        assert!(h.job(id).final_path.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_same_link_twice_at_once_gives_two_whole_files() {
        let content = Content::new(2 * 1024 * KB, 97);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let a = h.svc.add(&link(&server), None).unwrap();
        let again = AddRequest {
            allow_duplicate: true,
            ..AddRequest::default()
        };
        let b = h.svc.add_with(&link(&server), None, &again).unwrap();
        h.wait("both done", |h| {
            h.job(a).status == "completed" && h.job(b).status == "completed"
        })
        .await;
        let (pa, pb) = (h.job(a).final_path.unwrap(), h.job(b).final_path.unwrap());
        assert_ne!(pa, pb, "the second must not overwrite the first");
        for p in [pa, pb] {
            assert_eq!(
                fuselane_testkit::sha256_file(Path::new(&p)).unwrap(),
                content.sha256()
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_link_already_in_the_list_is_refused_unless_asked_for_again() {
        let content = Content::new(64 * KB, 120);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let a = h.svc.add(&link(&server), None).unwrap();
        let err = h
            .svc
            .add(&format!("  {}  ", link(&server)), None)
            .unwrap_err();
        assert_eq!(err.code, "duplicate");
        assert!(err.message.contains("already added"), "{}", err.message);
        assert!(err.hint.as_deref().unwrap_or("").contains("again"));
        assert_eq!(h.svc.jobs().unwrap().len(), 1, "nothing was added");
        let again = AddRequest {
            allow_duplicate: true,
            ..AddRequest::default()
        };
        let b = h.svc.add_with(&link(&server), None, &again).unwrap();
        assert_ne!(a, b);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_chosen_name_and_checksum_are_used() {
        let content = Content::new(300 * KB, 121);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let good = AddRequest {
            name: Some("  report final.bin ".into()),
            sha256: Some(
                content
                    .sha256()
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect(),
            ),
            allow_duplicate: false,
            later: false,
            start_at: None,
            replace: None,
            mirrors: vec![],
            headers: vec![],
        };
        let id = h.svc.add_with(&link(&server), None, &good).unwrap();
        assert_eq!(h.job(id).name, "report final.bin", "shown before it starts");
        assert!(h.job(id).verify);
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        let path = PathBuf::from(h.job(id).final_path.unwrap());
        assert_eq!(path.file_name().unwrap(), "report final.bin");

        // The wrong checksum: the file is refused and never published.
        let bad = AddRequest {
            sha256: Some("0".repeat(64)),
            allow_duplicate: true,
            ..AddRequest::default()
        };
        let id2 = h.svc.add_with(&link(&server), None, &bad).unwrap();
        h.wait("failure", |h| h.job(id2).status.starts_with("failed"))
            .await;
        let err = h.job(id2).error.unwrap_or_default();
        assert!(err.contains("SHA-256"), "{err}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_checksum_published_next_to_the_file_is_found_and_checked() {
        let hex =
            |c: &Content| -> String { c.sha256().iter().map(|b| format!("{b:02x}")).collect() };
        // Right: SHA256SUMS lists this file among others.
        let content = Content::new(300 * KB, 161);
        let server = RangeServer::start(content).await.unwrap();
        server.serve_file(
            "/SHA256SUMS",
            format!(
                "{}  other.bin\n{} *file.bin\n",
                "a".repeat(64),
                hex(&content)
            ),
        );
        let h = harness(3);
        h.svc.set_find_checksums(true).unwrap();
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        let j = h.job(id);
        assert!(j.verified, "checked against the published hash");
        assert_eq!(j.checksum_from.as_deref(), Some("SHA256SUMS"));

        // Wrong: a damaged file is refused, never published.
        let other = Content::new(200 * KB, 162);
        let bad = RangeServer::start(other).await.unwrap();
        bad.serve_file("/file.bin.sha256", "0".repeat(64));
        let id2 = h.svc.add(&link(&bad), None).unwrap();
        h.wait("failure", |h| h.job(id2).status.starts_with("failed"))
            .await;
        assert!(h.job(id2).error.unwrap_or_default().contains("SHA-256"));
        assert_eq!(h.job(id2).checksum_from.as_deref(), Some("file.bin.sha256"));

        // None published: it downloads as before, and the lookup isn't repeated.
        let plain = RangeServer::start(Content::new(100 * KB, 163))
            .await
            .unwrap();
        let id3 = h.svc.add(&link(&plain), None).unwrap();
        h.wait("plain", |h| h.job(id3).status == "completed").await;
        assert!(!h.job(id3).verified);
        assert_eq!(
            h.svc.store.get(id3).unwrap().sha256_from.as_deref(),
            Some("")
        );

        // Off: nothing is looked up.
        h.svc.set_find_checksums(false).unwrap();
        let off = RangeServer::start(content).await.unwrap();
        off.serve_file("/SHA256SUMS", format!("{}  file.bin\n", hex(&content)));
        let id4 = h.svc.add(&link(&off), None).unwrap();
        h.wait("off", |h| h.job(id4).status == "completed").await;
        assert!(!h.job(id4).verified);
        assert_eq!(h.svc.store.get(id4).unwrap().sha256_from, None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_already_downloaded_is_pointed_out_only_while_it_is_there() {
        let content = Content::new(120 * KB, 171);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        assert_eq!(
            h.svc.already_have("file.bin", Some(120 * KB)).unwrap(),
            None
        );
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("done", |h| h.job(id).status == "completed").await;
        let have = h
            .svc
            .already_have("FILE.bin", Some(120 * KB))
            .unwrap()
            .unwrap();
        assert_eq!(
            (have.id, have.size, have.name.as_str()),
            (id, 120 * KB, "file.bin")
        );
        // Another size, an unknown size or another name is a different file.
        assert_eq!(
            h.svc.already_have("file.bin", Some(121 * KB)).unwrap(),
            None
        );
        assert_eq!(h.svc.already_have("file.bin", None).unwrap(), None);
        assert_eq!(
            h.svc.already_have("other.bin", Some(120 * KB)).unwrap(),
            None
        );
        // Gone from disk: nothing to point at.
        std::fs::remove_file(&have.path).unwrap();
        assert_eq!(
            h.svc.already_have("file.bin", Some(120 * KB)).unwrap(),
            None
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_finished_download_keeps_what_each_network_carried() {
        let server = RangeServer::start(Content::new(400 * KB, 181))
            .await
            .unwrap();
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        assert_eq!(h.job(id).report, None, "nothing to say before it finishes");
        h.wait("done", |h| h.job(id).status == "completed").await;
        let r = h.job(id).report.expect("a report");
        assert_eq!(r.nets.iter().map(|n| n.bytes).sum::<u64>(), 400 * KB);
        assert!(r.nets.iter().all(|n| !n.label.is_empty()));
        assert!(r.secs > 0.0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_found_to_be_long_carries_on_by_itself_with_more_networks() {
        let content = Content::new(1024 * KB, 191);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(300 * KB),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("running", |h| h.job(id).status == "running").await;
        tokio::time::sleep(Duration::from_millis(400)).await;
        // What watch_if_long does once the speed shows the download is long.
        lock(&h.svc.known_long).insert(id);
        if let Some(r) = lock(&h.svc.running).get_mut(&id) {
            r.replan = true;
            r.cancel.cancel();
        }
        h.wait("finished without anyone resuming it", |h| {
            h.job(id).status == "completed"
        })
        .await;
        let path = PathBuf::from(h.job(id).final_path.unwrap());
        assert_eq!(
            fuselane_testkit::sha256_file(&path).unwrap(),
            content.sha256(),
            "byte-exact across the restart"
        );
        assert!(lock(&h.svc.known_long).is_empty(), "forgotten once done");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn on_low_battery_downloads_pause_until_plugged_in_when_asked() {
        use crate::battery::Battery;
        let content = Content::new(1024 * KB, 201);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(200 * KB),
        });
        let h = harness(3);
        let mut a = h.svc.automation_view().settings;
        a.low_battery = crate::automation::LowBattery::Pause;
        h.svc.set_automation(a).unwrap();
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("running", |h| h.job(id).status == "running").await;

        // Plugged in at 10%: nothing happens.
        h.svc.tick_battery(Some(Battery {
            percent: 10,
            plugged_in: true,
        }));
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(h.job(id).status, "running");

        // Unplugged at 15%: it pauses and says why; nothing new starts.
        h.svc.tick_battery(Some(Battery {
            percent: 15,
            plugged_in: false,
        }));
        h.wait("paused", |h| h.job(id).status == "paused").await;
        assert_eq!(h.job(id).error_action.as_deref(), Some("battery"));
        let other = RangeServer::start(Content::new(64 * KB, 202))
            .await
            .unwrap();
        let id2 = h.svc.add(&link(&other), None).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(h.job(id2).status, "queued");

        // Plugged in: both carry on by themselves.
        h.svc.tick_battery(Some(Battery {
            percent: 16,
            plugged_in: true,
        }));
        h.wait("both done", |h| {
            h.job(id).status == "completed" && h.job(id2).status == "completed"
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_earliest_deadline_goes_first_and_an_urgent_one_ignores_the_schedule() {
        let now = deadline::unix_now();
        let slow = RangeServer::start(Content::new(512 * KB, 211))
            .await
            .unwrap();
        slow.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(256 * KB),
        });
        let sb = RangeServer::start(Content::new(64 * KB, 212))
            .await
            .unwrap();
        let sc = RangeServer::start(Content::new(64 * KB, 213))
            .await
            .unwrap();
        let h = harness(1);
        let ia = h.svc.add(&link(&slow), None).unwrap();
        h.wait("first running", |h| h.job(ia).status == "running")
            .await;
        let ib = h.svc.add(&link(&sb), None).unwrap();
        let ic = h.svc.add(&link(&sc), None).unwrap();
        h.svc.set_ready_by(ib, Some(now + 7200)).unwrap();
        h.svc.set_ready_by(ic, Some(now + 5400)).unwrap();
        assert_eq!(h.job(ic).ready_by, Some(now + 5400));
        assert_eq!(h.job(ic).ready_state, Some("on-track"));
        h.wait("the earlier deadline started first", |h| {
            h.job(ic).status != "queued" && h.job(ib).status == "queued"
                || h.job(ib).status == "completed" && h.job(ic).status == "completed"
        })
        .await;
        h.wait("all done", |h| h.job(ib).status == "completed")
            .await;
        assert_eq!(h.job(ic).ready_by, None, "nothing to show once done");

        // Outside the schedule: only the download that would miss its deadline runs.
        h.svc.set_clock(noon());
        h.svc.set_automation(overnight_only()).unwrap();
        let sd = RangeServer::start(Content::new(64 * KB, 214))
            .await
            .unwrap();
        let se = RangeServer::start(Content::new(64 * KB, 215))
            .await
            .unwrap();
        let id = h.svc.add(&link(&sd), None).unwrap();
        let ie = h.svc.add(&link(&se), None).unwrap();
        h.svc.set_ready_by(ie, Some(now + 600)).unwrap();
        assert_eq!(h.job(ie).ready_state, Some("at-risk"));
        h.wait("urgent one done", |h| h.job(ie).status == "completed")
            .await;
        assert_eq!(h.job(id).status, "queued", "the other waits for the window");
        assert!(
            h.svc.set_ready_by(id, Some(now - 10)).is_err(),
            "not in the past"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn links_added_together_stay_together_as_a_group() {
        let a = RangeServer::start(Content::new(256 * KB, 221))
            .await
            .unwrap();
        let b = RangeServer::start(Content::new(256 * KB, 222))
            .await
            .unwrap();
        let h = harness(3);
        let text = format!("{}\n{}\n", link(&a), link(&b));
        let r = h.svc.add_batch(&text, None, true, Some("")).unwrap();
        let g = r.group.expect("a group");
        let [ia, ib] = [r.added[0], r.added[1]];
        assert_eq!(h.job(ia).group_id, Some(g));
        assert_eq!(
            h.job(ib).group_name.as_deref(),
            Some("2 files from 127.0.0.1")
        );
        h.svc.rename_group(g, "  Season 1 ").unwrap();
        assert_eq!(h.job(ia).group_name.as_deref(), Some("Season 1"));
        assert!(h.svc.rename_group(g, "   ").is_err());

        // Added "later": resume all starts both; pause all stops both.
        h.svc.resume_group(g).unwrap();
        h.wait("both done", |h| {
            h.job(ia).status == "completed" && h.job(ib).status == "completed"
        })
        .await;

        // Without a name asked for, or with one link, there's no group.
        let c = RangeServer::start(Content::new(64 * KB, 223))
            .await
            .unwrap();
        let one = h.svc.add_batch(&link(&c), None, true, Some("x")).unwrap();
        assert_eq!(one.group, None);
        h.svc.ungroup(g).unwrap();
        assert_eq!(h.job(ia).group_id, None);
        assert!(h.svc.pause_group(g).is_err(), "gone");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bad_names_and_checksums_are_refused_before_saving() {
        let h = harness(3);
        let link = "http://127.0.0.1:9/f.bin";
        for (req, code) in [
            (
                AddRequest {
                    sha256: Some("abc".into()),
                    ..AddRequest::default()
                },
                "bad-checksum",
            ),
            (
                AddRequest {
                    sha256: Some("g".repeat(64)),
                    ..AddRequest::default()
                },
                "bad-checksum",
            ),
            (
                AddRequest {
                    name: Some("x".repeat(300)),
                    ..AddRequest::default()
                },
                "bad-name",
            ),
        ] {
            assert_eq!(h.svc.add_with(link, None, &req).unwrap_err().code, code);
        }
        assert!(h.svc.jobs().unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pasted_text_and_patterns_add_many_downloads_at_once() {
        let h = harness(1);
        let text = "Parts:\nhttp://127.0.0.1:9/part[1-3].bin\nhttp://127.0.0.1:9/extra.bin, http://127.0.0.1:9/part2.bin";
        let r = h.svc.add_batch(text, None, false, None).unwrap();
        assert_eq!(r.added.len(), 4, "{r:?}");
        assert!(
            r.skipped.is_empty(),
            "the repeat of part2 was merged: {r:?}"
        );
        // Again: everything is already in the list.
        let again = h.svc.add_batch(text, None, false, None).unwrap();
        assert!(again.added.is_empty());
        assert_eq!(again.skipped.len(), 4);
        assert!(again.skipped.iter().all(|s| s.reason.contains("already")));
        assert_eq!(
            h.svc
                .add_batch("no links", None, false, None)
                .unwrap_err()
                .code,
            "bad-link"
        );
        assert_eq!(
            h.svc
                .add_batch("http://127.0.0.1:9/[1-5000]", None, false, None)
                .unwrap_err()
                .code,
            "too-many"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn downloads_at_once_is_a_saved_setting_within_limits() {
        let h = harness(3);
        assert_eq!(h.svc.set_max_running(5).unwrap(), 5);
        assert_eq!(h.svc.max_running(), 5);
        for bad in [0, 9, 100] {
            assert_eq!(h.svc.set_max_running(bad).unwrap_err().code, "bad-value");
        }
        assert_eq!(h.svc.max_running(), 5, "a refused value changes nothing");
        // A fresh service on the same database starts with the saved value.
        let store = Store::open(&h.dir.path().join("fuselane.db")).unwrap();
        let again = Service::new(store, h.dir.path().to_path_buf()).unwrap();
        assert_eq!(again.max_running(), 5);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_queue_starts_jobs_in_the_order_chosen() {
        let content = Content::new(64 * KB, 122);
        let server = RangeServer::start(content).await.unwrap();
        // One at a time, and the first one held so the rest stay queued.
        server.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::Throttle(16 * KB),
        });
        let h = harness(1);
        let ids: Vec<i64> = (0..3)
            .map(|i| {
                h.svc
                    .add(&format!("{}?n={i}", link(&server)), None)
                    .unwrap()
            })
            .collect();
        h.wait("first running", |h| h.job(ids[0]).status == "running")
            .await;
        h.svc.reorder(&[ids[2]]).unwrap();
        let pos: Vec<i64> = ids.iter().map(|&id| h.job(id).position).collect();
        assert!(pos[2] < pos[1], "moved ahead: {pos:?}");
        h.svc.pause(ids[0]).unwrap();
        h.wait("the moved one starts next", |h| {
            h.job(ids[2]).status == "running"
        })
        .await;
        assert_eq!(h.job(ids[1]).status, "queued");
    }

    fn overnight_only() -> crate::automation::Automation {
        crate::automation::Automation {
            schedule: crate::automation::Schedule {
                enabled: true,
                start: 60,
                stop: 7 * 60,
                days: [true; 7],
            },
            ..crate::automation::Automation::default()
        }
    }

    fn noon() -> crate::automation::Moment {
        crate::automation::Moment {
            weekday: 2,
            minute: 12 * 60,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_schedule_holds_downloads_until_its_window_and_pauses_at_its_end() {
        let content = Content::new(512 * KB, 130);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(64 * KB),
        });
        let h = harness(3);
        h.svc.set_clock(noon());
        let view = h.svc.set_automation(overnight_only()).unwrap();
        assert!(!view.allowed_now);
        assert_eq!(view.next.as_deref(), Some("Starts at 01:00 tomorrow."));
        let id = h.svc.add(&link(&server), None).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            h.job(id).status,
            "queued",
            "outside the window nothing starts"
        );

        // 01:30: the window opens.
        h.svc.set_clock(crate::automation::Moment {
            weekday: 3,
            minute: 90,
        });
        h.svc.tick_schedule();
        h.svc.resume(id).unwrap(); // nudges the queue, as the tick's pump does
        h.wait("running", |h| h.job(id).status == "running").await;
        assert_eq!(
            h.svc.automation_view().next.as_deref(),
            Some("Pauses at 07:00.")
        );

        // 07:00: it closes; the running download pauses and says why.
        h.svc.set_clock(crate::automation::Moment {
            weekday: 3,
            minute: 7 * 60,
        });
        h.svc.tick_schedule();
        h.wait("paused", |h| h.job(id).status == "paused").await;
        assert!(h.job(id).error.unwrap_or_default().contains("schedule"));

        // Next night it carries on by itself.
        h.svc.set_clock(crate::automation::Moment {
            weekday: 4,
            minute: 61,
        });
        h.svc.tick_schedule();
        h.wait("resumed", |h| {
            matches!(h.job(id).status, "running" | "completed")
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn when_everything_finishes_the_chosen_action_runs_after_a_countdown() {
        let content = Content::new(64 * KB, 131);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let ran: Arc<Mutex<Vec<crate::automation::WhenDone>>> = Arc::default();
        h.svc.set_power(
            {
                let ran = ran.clone();
                Arc::new(move |a| lock(&ran).push(a))
            },
            1,
        );
        h.svc
            .set_automation(crate::automation::Automation {
                when_done: crate::automation::WhenDone::Sleep,
                ..crate::automation::Automation::default()
            })
            .unwrap();
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("done", |h| h.job(id).status == "completed").await;
        // The store says completed a moment before the countdown is announced.
        h.wait("the window to be told, so it can offer Cancel", |h| {
            lock(&h.events)
                .iter()
                .any(|e| matches!(e, UiEvent::WhenDone { seconds: 1, .. }))
        })
        .await;
        let t = Instant::now();
        while lock(&ran).is_empty() {
            assert!(t.elapsed() < Duration::from_secs(5), "never ran");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(*lock(&ran), vec![crate::automation::WhenDone::Sleep]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_cancelled_countdown_never_runs_its_action() {
        let content = Content::new(64 * KB, 132);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let ran: Arc<Mutex<Vec<crate::automation::WhenDone>>> = Arc::default();
        h.svc.set_power(
            {
                let ran = ran.clone();
                Arc::new(move |a| lock(&ran).push(a))
            },
            1,
        );
        h.svc
            .set_automation(crate::automation::Automation {
                when_done: crate::automation::WhenDone::ShutDown,
                ..crate::automation::Automation::default()
            })
            .unwrap();
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("countdown", |h| {
            h.job(id).status == "completed"
                && lock(&h.events)
                    .iter()
                    .any(|e| matches!(e, UiEvent::WhenDone { .. }))
        })
        .await;
        h.svc.cancel_when_done();
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert!(lock(&ran).is_empty(), "cancelled: {:?}", lock(&ran));
        assert!(
            lock(&h.events)
                .iter()
                .any(|e| matches!(e, UiEvent::WhenDoneCancelled))
        );
    }

    #[test]
    fn mirror_lanes_show_as_their_network() {
        use fuselane_engine_http::download::{NetSnapshot, TickSnapshot};
        let lane = |id, bytes, rate, dead| NetSnapshot {
            id,
            bytes,
            rate,
            streams: 2,
            dead,
        };
        let s = Snapshot {
            written: 0,
            total: Some(100),
            rate: 0.0,
            // Wi-Fi (1) and Ethernet (2), each also reaching one mirror (101, 102).
            networks: vec![
                lane(1, 10, 1.0, false),
                lane(2, 20, 2.0, false),
                lane(101, 5, 0.5, true),
                lane(102, 7, 0.7, false),
            ],
            ticks: vec![TickSnapshot {
                fill: 1.0,
                owner: Some(102),
                in_flight: Some(101),
            }],
            retries: 0,
            hedges: 0,
        };
        let nets = vec![
            ("en1".to_string(), "Wi-Fi".to_string(), "wifi".to_string()),
            (
                "en0".to_string(),
                "Ethernet".to_string(),
                "ethernet".to_string(),
            ),
        ];
        let l = live(1, &s, &nets);
        assert_eq!(l.networks.len(), 2);
        assert_eq!(
            (l.networks[0].name.as_str(), l.networks[0].bytes),
            ("en1", 15)
        );
        assert_eq!(
            (l.networks[1].name.as_str(), l.networks[1].bytes),
            ("en0", 27)
        );
        assert!((l.networks[1].rate - 2.7).abs() < 1e-9);
        assert_eq!(l.networks[0].streams, 4);
        assert!(
            !l.networks[0].dead,
            "a dead mirror lane doesn't make the network dead"
        );
        // Ring ticks point at the network, not the lane.
        assert_eq!(&l.ticks[..], &[100, 2, 1]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mirrors_are_checked_when_added_and_kept_with_the_download() {
        let h = harness(3);
        let req = |m: &[&str]| AddRequest {
            later: true,
            allow_duplicate: true,
            mirrors: m.iter().map(|s| s.to_string()).collect(),
            ..AddRequest::default()
        };
        let bad = h
            .svc
            .add_with(
                "https://example.com/a.iso",
                None,
                &req(&["ftp://x.example/a.iso"]),
            )
            .unwrap_err();
        assert_eq!(bad.code, "bad-mirror");
        let id = h
            .svc
            .add_with(
                "https://example.com/a.iso",
                None,
                // The main link and a repeat are dropped quietly.
                &req(&[
                    "https://m1.example/a.iso",
                    " https://example.com/a.iso ",
                    "https://m1.example/a.iso",
                    "https://m2.example/pub/a.iso",
                ]),
            )
            .unwrap();
        assert_eq!(
            h.svc.store.get(id).unwrap().mirrors,
            vec!["https://m1.example/a.iso", "https://m2.example/pub/a.iso"]
        );
        assert_eq!(h.job(id).mirrors, vec!["m1.example", "m2.example"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_taken_name_keeps_both_or_replaces_the_old_file() {
        let content = Content::new(64 * KB, 141);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let old = h.dir.path().join("report.bin");
        std::fs::write(&old, b"the old one").unwrap();
        assert!(h.svc.name_taken(None, "report.bin"));
        assert!(!h.svc.name_taken(None, "other.bin"));
        let req = |replace| AddRequest {
            name: Some("report.bin".into()),
            allow_duplicate: true,
            replace,
            ..AddRequest::default()
        };
        // Keep both (the default when nobody chose): the new one is numbered.
        let a = h.svc.add_with(&link(&server), None, &req(None)).unwrap();
        h.wait("a", |h| h.job(a).status == "completed").await;
        let pa = PathBuf::from(h.job(a).final_path.unwrap());
        assert_eq!(pa.file_name().unwrap(), "report (1).bin");
        assert_eq!(std::fs::read(&old).unwrap(), b"the old one");
        // Replace: the old file goes (to the Trash in the app) and the new one takes its name.
        let b = h
            .svc
            .add_with(&link(&server), None, &req(Some(true)))
            .unwrap();
        h.wait("b", |h| {
            h.job(b)
                .final_path
                .is_some_and(|p| p.ends_with("report.bin"))
        })
        .await;
        assert_eq!(
            fuselane_testkit::sha256_file(&old).unwrap(),
            content.sha256()
        );
        assert!(!h.dir.path().join("report (2).bin").exists());
        // A folder with the name is never replaced.
        std::fs::create_dir(h.dir.path().join("folder.bin")).unwrap();
        let c = h
            .svc
            .add_with(
                &link(&server),
                None,
                &AddRequest {
                    name: Some("folder.bin".into()),
                    allow_duplicate: true,
                    replace: Some(true),
                    ..AddRequest::default()
                },
            )
            .unwrap();
        h.wait("c", |h| h.job(c).status == "completed").await;
        assert!(h.dir.path().join("folder.bin").is_dir());
        assert!(h.job(c).final_path.unwrap().ends_with("folder (1).bin"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_finished_download_can_open_or_unpack() {
        let content = Content::new(16 * KB, 142);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let opened = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
        {
            let opened = opened.clone();
            h.svc.set_opener(Arc::new(move |p: &Path| {
                opened.lock().unwrap().push(p.into())
            }));
        }
        h.svc
            .set_automation(crate::automation::Automation {
                after_download: crate::automation::AfterDownload::Open,
                ..crate::automation::Automation::default()
            })
            .unwrap();
        let a = h.svc.add(&link(&server), None).unwrap();
        h.wait("a", |h| h.job(a).status == "completed").await;
        h.wait("opened", |_| !opened.lock().unwrap().is_empty())
            .await;
        assert_eq!(
            opened.lock().unwrap()[0],
            PathBuf::from(h.job(a).final_path.unwrap())
        );

        // Unpack: a finished zip unpacks next to itself; anything else is left alone.
        h.svc
            .set_automation(crate::automation::Automation {
                after_download: crate::automation::AfterDownload::Unpack,
                ..crate::automation::Automation::default()
            })
            .unwrap();
        let zip_path = h.dir.path().join("pack.zip");
        {
            use std::io::Write;
            let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            z.start_file("inside.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"unpacked").unwrap();
            z.finish().unwrap();
        }
        let id = h
            .svc
            .store
            .create("https://example.com/pack.zip", h.dir.path())
            .unwrap();
        h.svc.store.set_finished(id, &zip_path, 1).unwrap();
        h.svc.after_download(id);
        let inside = h.dir.path().join("pack").join("inside.txt");
        h.wait("unpacked", |_| inside.exists()).await;
        assert_eq!(std::fs::read(&inside).unwrap(), b"unpacked");
        // A damaged archive says so on the download, in plain words.
        let bad = h.dir.path().join("broken.zip");
        std::fs::write(&bad, b"not a zip at all").unwrap();
        let id = h
            .svc
            .store
            .create("https://example.com/broken.zip", h.dir.path())
            .unwrap();
        h.svc.store.set_finished(id, &bad, 16).unwrap();
        h.svc.after_download(id);
        h.wait("unpack error", |h| {
            h.svc
                .store
                .get(id)
                .unwrap()
                .error
                .is_some_and(|e| e.contains("couldn't be unpacked"))
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn finished_files_sort_into_folders_by_type_only_in_the_default_folder() {
        let video = Content::new(64 * KB, 133);
        let server = RangeServer::start(video).await.unwrap();
        let h = harness(3);
        h.svc
            .set_automation(crate::automation::Automation {
                sort_by_type: true,
                ..crate::automation::Automation::default()
            })
            .unwrap();
        let named = |name: &str| AddRequest {
            name: Some(name.into()),
            allow_duplicate: true,
            ..AddRequest::default()
        };
        let a = h
            .svc
            .add_with(&link(&server), None, &named("trip.mp4"))
            .unwrap();
        // "Completed" is recorded a moment before the file moves, so wait for the move.
        let in_video = |h: &Harness, id| {
            h.job(id)
                .final_path
                .is_some_and(|p| Path::new(&p).parent().is_some_and(|d| d.ends_with("Video")))
        };
        h.wait("a sorted", |h| in_video(h, a)).await;
        let pa = PathBuf::from(h.job(a).final_path.unwrap());
        let root = std::fs::canonicalize(h.dir.path()).unwrap();
        assert_eq!(pa, root.join("Video").join("trip.mp4"));
        assert!(pa.exists());

        // A folder the person chose is left alone.
        let chosen = h.dir.path().join("Mine");
        std::fs::create_dir(&chosen).unwrap();
        let b = h
            .svc
            .add_with(
                &link(&server),
                Some(chosen.to_str().unwrap()),
                &named("trip.mp4"),
            )
            .unwrap();
        h.wait("b done", |h| h.job(b).status == "completed").await;
        let pb = PathBuf::from(h.job(b).final_path.unwrap());
        assert_eq!(
            pb.parent().unwrap(),
            std::fs::canonicalize(&chosen).unwrap()
        );

        // A second file of the same name never replaces the first.
        let c = h
            .svc
            .add_with(&link(&server), None, &named("trip.mp4"))
            .unwrap();
        h.wait("c sorted", |h| in_video(h, c)).await;
        let pc = PathBuf::from(h.job(c).final_path.unwrap());
        assert_eq!(pc.parent().unwrap(), root.join("Video"));
        assert_ne!(pc, pa);
        assert!(pa.exists() && pc.exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_computer_is_kept_awake_only_while_downloading() {
        let content = Content::new(256 * KB, 134);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(128 * KB),
        });
        let h = harness(3);
        assert!(!h.svc.keeping_awake());
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("running", |h| h.job(id).status == "running").await;
        #[cfg(target_os = "macos")]
        assert!(h.svc.keeping_awake(), "macOS always can");
        h.wait("done", |h| h.job(id).status == "completed").await;
        h.svc.update_awake();
        assert!(!h.svc.keeping_awake());
        // Turned off: never held, even while running.
        h.svc
            .set_automation(crate::automation::Automation {
                keep_awake: false,
                ..crate::automation::Automation::default()
            })
            .unwrap();
        let id2 = h.svc.add(&format!("{}?2", link(&server)), None).unwrap();
        h.wait("running 2", |h| h.job(id2).status == "running")
            .await;
        assert!(!h.svc.keeping_awake());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn automation_settings_are_checked_and_remembered() {
        let h = harness(3);
        let mut bad = overnight_only();
        bad.schedule.days = [false; 7];
        assert_eq!(h.svc.set_automation(bad).unwrap_err().code, "bad-value");
        let good = crate::automation::Automation {
            when_done: crate::automation::WhenDone::Quit,
            sort_by_type: true,
            ..overnight_only()
        };
        h.svc.set_automation(good.clone()).unwrap();
        let store = Store::open(&h.dir.path().join("fuselane.db")).unwrap();
        let again = Service::new(store, h.dir.path().to_path_buf()).unwrap();
        assert_eq!(again.automation_view().settings, good);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn empty_files_and_servers_without_ranges_still_finish() {
        let empty = Content::new(0, 98);
        let se = RangeServer::start(empty).await.unwrap();
        let whole = Content::new(700 * KB, 99);
        let sw = RangeServer::start(whole).await.unwrap();
        sw.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::IgnoreRange,
        });
        let h = harness(3);
        let ie = h.svc.add(&link(&se), None).unwrap();
        let iw = h.svc.add(&link(&sw), None).unwrap();
        h.wait("both done", |h| {
            let (e, w) = (h.job(ie), h.job(iw));
            (e.status == "completed" || e.status.starts_with("failed"))
                && (w.status == "completed" || w.status.starts_with("failed"))
        })
        .await;
        let e = h.job(ie);
        assert_eq!(e.status, "completed", "{:?}", e.error);
        assert_eq!(std::fs::metadata(e.final_path.unwrap()).unwrap().len(), 0);
        let w = h.job(iw);
        assert_eq!(w.status, "completed", "{:?}", w.error);
        assert_eq!(
            fuselane_testkit::sha256_file(Path::new(&w.final_path.unwrap())).unwrap(),
            whole.sha256()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_flood_of_downloads_never_runs_more_than_the_queue_allows() {
        let content = Content::new(256 * KB, 100);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::Throttle(2048 * KB),
        });
        let h = harness(3);
        let ids: Vec<i64> = (0..30)
            .map(|i| {
                h.svc
                    .add(&format!("{}?n={i}", link(&server)), None)
                    .unwrap()
            })
            .collect();
        let mut peak = 0;
        let start = Instant::now();
        loop {
            let jobs = h.svc.jobs().unwrap();
            let running = jobs.iter().filter(|j| j.status == "running").count();
            peak = peak.max(running).max(h.svc.running());
            if jobs.iter().all(|j| j.status == "completed") {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "flood didn't finish"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(peak <= 3, "ran {peak} at once");
        assert_eq!(ids.len(), 30);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn waiting_jobs_can_be_paused_removed_and_resumed() {
        let content = Content::new(2 * 1024 * KB, 101);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(1024 * KB),
        });
        let h = harness(1);
        let first = h.svc.add(&link(&server), None).unwrap();
        let waiting = h.svc.add(&format!("{}?w", link(&server)), None).unwrap();
        let doomed = h.svc.add(&format!("{}?d", link(&server)), None).unwrap();
        h.wait("first running", |h| h.job(first).status == "running")
            .await;
        h.svc.pause(waiting).unwrap();
        assert_eq!(h.job(waiting).status, "paused");
        h.svc.remove(doomed).unwrap();
        assert!(h.svc.jobs().unwrap().iter().all(|j| j.id != doomed));
        // Resuming a running job is a harmless no-op.
        h.svc.resume(first).unwrap();
        h.svc.resume(waiting).unwrap();
        h.wait("all done", |h| {
            h.job(first).status == "completed" && h.job(waiting).status == "completed"
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn only_finished_files_that_still_exist_can_be_revealed() {
        let content = Content::new(64 * KB, 102);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        assert_eq!(h.svc.finished_file(404).unwrap_err().code, "not-found");
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("done", |h| h.job(id).status == "completed").await;
        let path = h.svc.finished_file(id).unwrap();
        assert!(path.starts_with(std::fs::canonicalize(h.dir.path()).unwrap()));
        std::fs::remove_file(&path).unwrap();
        let e = h.svc.finished_file(id).unwrap_err();
        assert_eq!(e.code, "file-missing");
        assert!(e.hint.is_some());
        // An unfinished download has no file to show.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let bad = h
            .svc
            .add(&format!("http://127.0.0.1:{port}/x"), None)
            .unwrap();
        h.wait("failed", |h| h.job(bad).status.starts_with("failed"))
            .await;
        assert_eq!(h.svc.finished_file(bad).unwrap_err().code, "not-finished");
    }

    /// The field names `apps/desktop/src/lib/types.ts` declares for an interface.
    fn ts_fields(src: &str, name: &str) -> Vec<String> {
        let start = src
            .find(&format!("export interface {name} {{"))
            .unwrap_or_else(|| panic!("types.ts has no interface {name}"));
        let body = &src[start..];
        let body = &body[body.find('{').unwrap() + 1..body.find("\n}").unwrap()];
        let mut out: Vec<String> = body
            .lines()
            .map(str::trim)
            .filter(|l| {
                !l.is_empty() && !l.starts_with("//") && !l.starts_with("/*") && !l.starts_with('*')
            })
            .filter_map(|l| l.split(':').next())
            .map(|f| f.trim_end_matches('?').to_string())
            .collect();
        out.sort();
        out
    }

    fn json_fields(v: &impl Serialize) -> Vec<String> {
        let mut keys: Vec<String> = serde_json::to_value(v)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_window_types_match_what_the_backend_sends() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/types.ts"))
                .unwrap();
        let job = JobView {
            id: 1,
            url: "u".into(),
            name: "n".into(),
            dir: "d".into(),
            status: "queued",
            resumable: false,
            written: 0,
            total: None,
            error: None,
            error_action: None,
            final_path: None,
            created_at: 0,
            position: 0,
            verify: false,
            speed_limit: 0,
            retry_in: None,
            start_at: None,
            mirrors: vec![],
            mirror_notes: vec![],
            ..JobView::default()
        };
        let net = NetView {
            name: "en0".into(),
            label: "Wi-Fi".into(),
            kind: "wifi".into(),
            usable: true,
            addrs: vec![],
            reach: Some("online"),
        };
        let lnet = LiveNet {
            name: "en0".into(),
            label: "Wi-Fi".into(),
            kind: "wifi".into(),
            bytes: 0,
            rate: 0.0,
            streams: 0,
            dead: false,
        };
        let live = Live {
            id: 1,
            written: 0,
            total: None,
            rate: 0.0,
            networks: vec![],
            ticks: vec![],
            retries: 0,
            hedges: 0,
        };
        let err = UiError::new("c", "m", None);
        let lim = LimitsView {
            global: 1,
            networks: vec![],
            ..LimitsView::default()
        };
        let nl = NetLimit {
            name: "en0".into(),
            rate: 1,
        };
        let pv = PreviewView {
            filename: "f".into(),
            total: None,
            splittable: true,
        };
        for (name, got) in [
            ("JobView", json_fields(&job)),
            ("NetView", json_fields(&net)),
            ("LiveNet", json_fields(&lnet)),
            ("Live", json_fields(&live)),
            ("UiError", json_fields(&err)),
            ("PreviewView", json_fields(&pv)),
            ("LimitsView", json_fields(&lim)),
            ("NetLimit", json_fields(&nl)),
            (
                "AllowanceView",
                json_fields(&AllowanceView {
                    name: "en0".into(),
                    allowance: None,
                    reset_day: 1,
                    used: 0,
                    resets_on: "2026-11-01".into(),
                    reached: false,
                }),
            ),
            (
                "NetPref",
                json_fields(&NetPref {
                    name: "en0".into(),
                    label: None,
                    lane: None,
                    use_for: NetUse::Long,
                }),
            ),
        ] {
            assert_eq!(
                got,
                ts_fields(&src, name),
                "{name}: types.ts and service.rs disagree"
            );
        }
        // Events are tagged exactly as the window switches on them.
        let jobs = serde_json::to_value(UiEvent::Jobs { jobs: vec![] }).unwrap();
        assert_eq!(jobs["type"], "jobs");
        assert!(
            src.contains("{ type: 'jobs'; jobs: JobView[] }")
                && src.contains("{ type: 'live' } & Live")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_expired_link_can_be_fixed_and_the_download_continues() {
        let content = Content::new(3 * 1024 * KB, 103);
        let old = RangeServer::start(content).await.unwrap();
        // Short answers make the download need many requests; after 12 of them every
        // request is refused, as when a signed URL expires mid-download.
        old.add_rule(Rule {
            skip: 1,
            times: 12,
            fault: Fault::CapRange(64 * KB),
        });
        old.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Status(403, None),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&old), None).unwrap();
        h.wait("expired", |h| h.job(id).status.starts_with("failed"))
            .await;
        let failed = h.job(id);
        assert_eq!(
            failed.error_action.as_deref(),
            Some("fix-link"),
            "{:?}",
            failed.error
        );
        assert!(failed.resumable);
        // Negative cases first: a bad link, an unknown id.
        assert_eq!(
            h.svc.fix_link(id, "ftp://x/y").unwrap_err().code,
            "bad-link"
        );
        assert_eq!(
            h.svc.fix_link(999, "http://x/y").unwrap_err().code,
            "not-found"
        );
        // A fresh link to the same file continues and ends byte-exact.
        let fresh = RangeServer::start(content).await.unwrap();
        h.svc.fix_link(id, &link(&fresh)).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        let done = h.job(id);
        assert_eq!(done.url, link(&fresh));
        assert_eq!(done.error_action, None);
        assert_eq!(
            fuselane_testkit::sha256_file(Path::new(&done.final_path.unwrap())).unwrap(),
            content.sha256()
        );
        assert_eq!(
            h.svc.fix_link(id, &link(&fresh)).unwrap_err().code,
            "not-resumable"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_new_link_to_a_different_file_is_refused_then_started_over() {
        let a = Content::new(2 * 1024 * KB, 104);
        let old = RangeServer::start(a).await.unwrap();
        // Short answers make the download need many requests; after 12 of them every
        // request is refused, as when a signed URL expires mid-download.
        old.add_rule(Rule {
            skip: 1,
            times: 12,
            fault: Fault::CapRange(64 * KB),
        });
        old.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Status(403, None),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&old), None).unwrap();
        h.wait("expired", |h| h.job(id).status.starts_with("failed"))
            .await;
        // Same size, different bytes and validator: must never be mixed in.
        let other = Content::new(2 * 1024 * KB, 105);
        let wrong = RangeServer::start(other).await.unwrap();
        wrong.set_etag("\"a-different-file\"");
        h.svc.fix_link(id, &link(&wrong)).unwrap();
        h.wait("refused", |h| h.job(id).status == "failed-final")
            .await;
        assert_eq!(h.job(id).error_action.as_deref(), Some("start-over"));
        let fresh = h.svc.start_over(id).unwrap();
        h.wait("fresh done", |h| h.job(fresh).status == "completed")
            .await;
        assert!(
            h.svc.jobs().unwrap().iter().all(|j| j.id != id),
            "old job removed"
        );
        assert_eq!(
            fuselane_testkit::sha256_file(Path::new(&h.job(fresh).final_path.unwrap())).unwrap(),
            other.sha256()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn limits_are_validated_saved_applied_and_restored() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fuselane.db");
        let content = Content::new(1024 * KB, 106);
        let server = RangeServer::start(content).await.unwrap();
        {
            let svc = Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
            assert_eq!(
                svc.limits(),
                LimitsView {
                    slow_rate: DEFAULT_SLOW,
                    ..LimitsView::default()
                }
            );
            for bad in [
                LimitsView {
                    global: MAX_LIMIT + 1,
                    networks: vec![],
                    ..LimitsView::default()
                },
                LimitsView {
                    global: 0,
                    networks: vec![NetLimit {
                        name: " ".into(),
                        rate: 1,
                    }],
                    ..LimitsView::default()
                },
                LimitsView {
                    global: 0,
                    networks: vec![NetLimit {
                        name: "en0\n".into(),
                        rate: 1,
                    }],
                    ..LimitsView::default()
                },
                LimitsView {
                    global: 0,
                    networks: vec![
                        NetLimit {
                            name: "en0".into(),
                            rate: 1,
                        },
                        NetLimit {
                            name: "en0".into(),
                            rate: 2,
                        },
                    ],
                    ..LimitsView::default()
                },
            ] {
                assert_eq!(svc.set_limits(bad).unwrap_err().code, "bad-limit");
            }
            let saved = svc
                .set_limits(LimitsView {
                    global: 512 * KB,
                    networks: vec![
                        NetLimit {
                            name: "en9".into(),
                            rate: 0,
                        },
                        NetLimit {
                            name: "en0".into(),
                            rate: 64 * KB,
                        },
                    ],
                    ..LimitsView::default()
                })
                .unwrap();
            assert_eq!(
                saved.networks,
                vec![NetLimit {
                    name: "en0".into(),
                    rate: 64 * KB
                }],
                "0 means no limit"
            );
            // A running download obeys the overall limit: 1 MiB at 512 KiB/s.
            let start = Instant::now();
            let id = svc.add(&link(&server), None).unwrap();
            while svc.jobs().unwrap()[0].status != "completed" {
                assert!(
                    start.elapsed() < Duration::from_secs(20),
                    "{:?}",
                    svc.jobs().unwrap()[0]
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            let secs = start.elapsed().as_secs_f64();
            assert!(secs > 1.4, "limit ignored: {secs:.2} s");
            let _ = id;
        }
        // Restarting brings the limits back.
        let svc = Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        assert_eq!(svc.limits().global, 512 * KB);
        assert_eq!(svc.limits().networks.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn diagnostics_never_leak_addresses_links_or_names() {
        let content = Content::new(64 * KB, 107);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let secret = format!(
            "http://{}/private-report-name.pdf?token=hunter2",
            server.addr()
        );
        let id = h.svc.add(&secret, None).unwrap();
        h.wait("finished", |h| {
            h.job(id).status != "queued" && h.job(id).status != "running"
        })
        .await;
        let report = h.svc.diagnostics(&[("tls".into(), true, "ready".into())]);
        assert!(report.contains(env!("CARGO_PKG_VERSION")));
        assert!(report.contains("ok tls: ready"));
        for leak in ["127.0.0.1", "private-report-name", "hunter2", "token"] {
            assert!(
                !report.contains(leak),
                "diagnostics leaked {leak:?}:\n{report}"
            );
        }
        for iface in fuselane_netif::list().unwrap() {
            for a in iface.addrs {
                assert!(!report.contains(&a.to_string()), "leaked address {a}");
            }
        }
        assert!(report.contains(&format!("#{id} http")));
    }

    #[test]
    fn slow_mode_caps_without_forgetting_the_normal_limit() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fuselane.db");
        let svc = Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        svc.set_limits(LimitsView {
            global: 8 * 1024 * KB,
            ..LimitsView::default()
        })
        .unwrap();
        let on = svc.set_slow(true).unwrap();
        assert!(on.slow);
        assert_eq!(on.global, 8 * 1024 * KB, "the normal limit is kept");
        assert_eq!(
            on.to_settings().global,
            DEFAULT_SLOW,
            "the slower of the two applies"
        );
        // A slow cap above the normal limit doesn't speed anything up.
        let mut v = svc.limits();
        v.slow_rate = 64 * 1024 * KB;
        assert_eq!(
            svc.set_limits(v).unwrap().to_settings().global,
            8 * 1024 * KB
        );
        let off = svc.set_slow(false).unwrap();
        assert_eq!(off.to_settings().global, 8 * 1024 * KB);
        // With no normal limit, slow mode alone applies, and survives a restart.
        svc.set_limits(LimitsView {
            global: 0,
            slow: true,
            slow_rate: 256 * KB,
            ..LimitsView::default()
        })
        .unwrap();
        drop(svc);
        let svc = Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        assert!(svc.limits().slow);
        assert_eq!(svc.limits().to_settings().global, 256 * KB);
        assert_eq!(
            svc.set_limits(LimitsView {
                slow: true,
                slow_rate: MAX_LIMIT + 1,
                ..LimitsView::default()
            })
            .unwrap_err()
            .code,
            "bad-limit"
        );
    }

    #[test]
    fn networks_can_be_renamed_and_recoloured_safely() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fuselane.db");
        let svc = Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        let pref = |name: &str, label: Option<&str>, lane: Option<&str>| NetPref {
            name: name.into(),
            label: label.map(Into::into),
            lane: lane.map(Into::into),
            use_for: NetUse::Always,
        };
        let all = svc
            .set_network_pref(pref("en0", Some("  Home Wi-Fi  "), Some("mint")))
            .unwrap();
        assert_eq!(
            all,
            vec![pref("en0", Some("Home Wi-Fi"), Some("mint"))],
            "trimmed"
        );
        for (bad, code) in [
            (pref("", Some("x"), None), "bad-network-name"),
            (pref("en0\n", Some("x"), None), "bad-network-name"),
            (pref("en0", Some(&"x".repeat(41)), None), "bad-network-name"),
            (pref("en0", Some("tab\there"), None), "bad-network-name"),
            (pref("en0", None, Some("orange")), "bad-network-color"),
            (pref("en0", None, Some("fuse")), "bad-network-color"),
        ] {
            assert_eq!(svc.set_network_pref(bad).unwrap_err().code, code);
        }
        // Clearing both forgets the network; others survive a restart.
        svc.set_network_pref(pref("en5", None, Some("iris")))
            .unwrap();
        svc.set_network_pref(pref("en0", Some(" "), None)).unwrap();
        drop(svc);
        let svc = Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        assert_eq!(svc.network_prefs(), vec![pref("en5", None, Some("iris"))]);
    }

    #[test]
    fn an_update_is_announced_once_and_never_on_a_fresh_install() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fuselane.db");
        let open = || Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        assert_eq!(open().updated_from(), None, "fresh install");
        assert_eq!(open().updated_from(), None, "same version again");
        // Pretend an older version ran last.
        Store::open(&db)
            .unwrap()
            .set_setting("last_version", "0.0.1-old")
            .unwrap();
        assert_eq!(open().updated_from(), Some("0.0.1-old"));
        assert_eq!(open().updated_from(), None, "only once");
        // Slow mode has its default speed without anything saved.
        assert_eq!(open().limits().slow_rate, DEFAULT_SLOW);
    }

    fn day(y: i32, m: u8, d: u8) -> Date {
        Date {
            year: y,
            month: m,
            day: d,
        }
    }

    #[test]
    fn allowance_requests_are_validated() {
        let h = harness(3);
        let today = day(2026, 10, 8);
        let req = |name: &str, bytes: u64, reset_day: u8| AllowanceRequest {
            name: name.into(),
            bytes,
            reset_day,
        };
        for bad in [
            req("", 1, 1),
            req("en0\n", 1, 1),
            req("en0", 1, 0),
            req("en0", 1, 29),
            req("en0", MAX_ALLOWANCE + 1, 1),
        ] {
            assert_eq!(
                h.svc.set_allowance(bad, today).unwrap_err().code,
                "bad-allowance"
            );
        }
        let views = h
            .svc
            .set_allowance(req("en77", 5 * 1024 * 1024 * KB, 15), today)
            .unwrap();
        let v = views.iter().find(|v| v.name == "en77").unwrap();
        assert_eq!(v.allowance, Some(5 * 1024 * 1024 * KB));
        assert_eq!(v.resets_on, "2026-10-15");
        assert!(!v.reached);
        // Zero removes it.
        let views = h.svc.set_allowance(req("en77", 0, 15), today).unwrap();
        assert!(views.iter().all(|v| v.name != "en77"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn when_every_network_hits_its_allowance_downloads_pause_until_the_reset() {
        let usable: Vec<String> = fuselane_netif::list()
            .unwrap()
            .into_iter()
            .filter(|i| i.usable())
            .map(|i| i.name)
            .collect();
        assert!(!usable.is_empty(), "test needs a network");
        let content = Content::new(4 * 1024 * KB, 108);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(256 * KB),
        });
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fuselane.db");
        let svc = Service::with_max_running(Store::open(&db).unwrap(), dir.path().to_path_buf(), 3)
            .unwrap();
        let october = day(2026, 10, 8);
        for n in &usable {
            svc.set_allowance(
                AllowanceRequest {
                    name: n.clone(),
                    bytes: 128 * KB,
                    reset_day: 1,
                },
                october,
            )
            .unwrap();
        }
        let id = svc.add(&link(&server), None).unwrap();
        let start = Instant::now();
        loop {
            svc.tick_usage(october);
            let job = svc
                .jobs()
                .unwrap()
                .into_iter()
                .find(|j| j.id == id)
                .unwrap();
            if job.status == "paused" && svc.running() == 0 {
                assert_eq!(job.error_action.as_deref(), Some("allowance"));
                assert!(job.error.unwrap_or_default().contains("data allowance"));
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "never paused: {job:?}"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let views = svc.allowances(october);
        assert!(
            views
                .iter()
                .filter(|v| usable.contains(&v.name))
                .all(|v| v.reached)
        );
        // Usage survives a restart.
        let used: u64 = views.iter().map(|v| v.used).sum();
        drop(svc);
        let svc = Service::with_max_running(Store::open(&db).unwrap(), dir.path().to_path_buf(), 3)
            .unwrap();
        assert_eq!(
            svc.allowances(october).iter().map(|v| v.used).sum::<u64>(),
            used
        );
        // A new month resets the count; resuming finishes the file.
        let november = day(2026, 11, 1);
        svc.tick_usage(november);
        assert!(
            svc.allowances(november)
                .iter()
                .all(|v| !v.reached && v.used == 0)
        );
        for n in &usable {
            svc.set_allowance(
                AllowanceRequest {
                    name: n.clone(),
                    bytes: 0,
                    reset_day: 1,
                },
                november,
            )
            .unwrap();
        }
        svc.resume(id).unwrap();
        let start = Instant::now();
        while svc
            .jobs()
            .unwrap()
            .iter()
            .find(|j| j.id == id)
            .unwrap()
            .status
            != "completed"
        {
            svc.tick_usage(november);
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "didn't finish after the reset"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let path = svc
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == id)
            .unwrap()
            .final_path
            .unwrap();
        assert_eq!(
            fuselane_testkit::sha256_file(Path::new(&path)).unwrap(),
            content.sha256()
        );
    }

    #[test]
    fn per_network_lookups_are_off_by_default_and_remembered() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fuselane.db");
        let open = || Service::new(Store::open(&db).unwrap(), dir.path().to_path_buf()).unwrap();
        assert!(!open().per_network_dns(), "private by default");
        open().set_per_network_dns(true).unwrap();
        assert!(open().per_network_dns());
        open().set_per_network_dns(false).unwrap();
        assert!(!open().per_network_dns());
    }

    #[test]
    fn opens_wait_for_the_window_then_arrive_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("fuselane.db")).unwrap();
        let svc = Service::new(store, dir.path().to_path_buf()).unwrap();
        svc.open_request("magnet:?xt=urn:btih:a".into());
        svc.open_request("/tmp/b.torrent".into());
        for i in 0..40 {
            svc.open_request(format!("extra {i}"));
        }
        let events: Arc<Mutex<Vec<UiEvent>>> = Arc::default();
        svc.subscribe({
            let events = events.clone();
            Arc::new(move |e| lock(&events).push(e))
        });
        svc.open_request("magnet:?xt=urn:btih:c".into());
        let opens: Vec<String> = lock(&events)
            .iter()
            .filter_map(|e| match e {
                UiEvent::Open { target } => Some(target.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(opens.len(), 17, "16 queued at most, then live");
        assert_eq!(opens[0], "magnet:?xt=urn:btih:a");
        assert_eq!(opens[1], "/tmp/b.torrent");
        assert_eq!(opens[16], "magnet:?xt=urn:btih:c");
    }

    #[test]
    fn networks_behind_a_sign_in_page_are_left_out_unless_none_remain() {
        use fuselane_transport::probe::Reach;
        let net = |name: &str| fuselane_netif::Interface {
            name: name.into(),
            display_name: name.into(),
            index: 1,
            kind: fuselane_netif::Kind::Wifi,
            addrs: vec!["10.0.0.2".parse().unwrap()],
        };
        let mut reach = HashMap::new();
        reach.insert("en1".to_string(), Reach::Portal { location: None });
        reach.insert("en0".to_string(), Reach::Online);
        let names =
            |v: Vec<fuselane_netif::Interface>| v.into_iter().map(|i| i.name).collect::<Vec<_>>();
        assert_eq!(
            names(without_portals(
                vec![net("en0"), net("en1"), net("en7")],
                &reach
            )),
            vec!["en0", "en7"]
        );
        assert_eq!(
            names(without_portals(vec![net("en1")], &reach)),
            vec!["en1"],
            "all stuck: try anyway"
        );
        assert_eq!(reach_word(&Reach::Offline("x".into())), "offline");
    }

    #[test]
    fn crashed_jobs_come_back_paused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fuselane.db");
        {
            let store = Store::open(&path).unwrap();
            let id = store.create("http://x/f", dir.path()).unwrap();
            store.apply(id, Event::Start, None).unwrap();
        }
        let svc = Service::new(Store::open(&path).unwrap(), dir.path().to_path_buf()).unwrap();
        assert_eq!(svc.jobs().unwrap()[0].status, "paused");
    }
}
