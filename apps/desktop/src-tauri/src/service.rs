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

/// Downloads that run at once; the rest wait their turn (L-53).
pub const MAX_RUNNING: usize = 3;

/// One row in the transfers list.
#[derive(Debug, Clone, Serialize, PartialEq)]
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
    let url = url.trim();
    parse_link(url)
        .map_err(|m| UiError::new("bad-link", m, Some("Links start with http:// or https://.")))?;
    let p = runner::preview(url)
        .await
        .map_err(|m| UiError::new("preview-failed", m, None))?;
    Ok(PreviewView {
        filename: p.filename,
        total: p.total,
        splittable: p.splittable,
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiEvent {
    Jobs { jobs: Vec<JobView> },
    Live(Live),
}

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
}

/// The app's backend. Cheap to share: methods take `&Arc<Self>`.
pub struct Service {
    store: Arc<Store>,
    running: Mutex<HashMap<i64, Running>>,
    emit: Mutex<Emit>,
    /// Extra listeners that live as long as the app (tray, notifications).
    listeners: Mutex<Vec<Emit>>,
    default_dir: PathBuf,
    max_running: usize,
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
}

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
    match parse_link(&job.url) {
        Ok((_, host, _, path)) => {
            let path = path.split('?').next().unwrap_or_default();
            filename_from_path(path).unwrap_or(host)
        }
        Err(_) => job.url.clone(),
    }
}

fn view(job: &Job) -> JobView {
    JobView {
        id: job.id,
        url: job.url.clone(),
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
    }
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

/// Packs a snapshot for the window.
fn live(id: i64, s: &Snapshot, nets: &[(String, String, String)]) -> Live {
    let index = |net: Option<u32>| -> u16 {
        net.and_then(|n| s.networks.iter().position(|x| x.id == n))
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
        networks: s
            .networks
            .iter()
            .map(|n| {
                let (name, label, kind) = nets
                    .get(n.id.saturating_sub(1) as usize)
                    .cloned()
                    .unwrap_or_else(|| (format!("net{}", n.id), String::new(), "other".into()));
                LiveNet {
                    name,
                    label,
                    kind,
                    bytes: n.bytes,
                    rate: if n.rate.is_finite() { n.rate } else { 0.0 },
                    streams: n.streams,
                    dead: n.dead,
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
            max_running: MAX_RUNNING,
            retry_scale: None,
            limiter,
            saved_limits: Mutex::new(saved_limits),
            net_prefs: Mutex::new(net_prefs),
            allowances: Mutex::new(allowances),
            usage: Mutex::new(usage),
            allowance_paused: Mutex::default(),
            per_network_dns: std::sync::atomic::AtomicBool::new(store_flag),
            updated_from,
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
        s.max_running = max.max(1);
        s.retry_scale = Some(0.05);
        Ok(Arc::new(s))
    }

    /// Where events go (the window's channel). Replaces any earlier subscriber.
    pub fn subscribe(self: &Arc<Self>, emit: Emit) {
        *lock(&self.emit) = emit;
        self.publish_jobs();
    }

    pub fn default_dir(&self) -> &Path {
        &self.default_dir
    }

    /// Adds a listener that sees every event (the window's channel is separate).
    pub fn listen(&self, f: Emit) {
        lock(&self.listeners).push(f);
    }

    fn send(&self, e: UiEvent) {
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
        Ok(self
            .store
            .list()
            .map_err(store_error)?
            .iter()
            .map(view)
            .collect())
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
            })
            .collect())
    }

    /// Adds a download and starts it when a slot is free.
    pub fn add(self: &Arc<Self>, url: &str, dir: Option<&str>) -> Result<i64, UiError> {
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
        let id = self.store.create(url, &dir).map_err(store_error)?;
        self.publish_jobs();
        self.pump();
        Ok(id)
    }

    pub fn pause(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
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
        if pref.label.is_some() || pref.lane.is_some() {
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
        let Ok(mut jobs) = self.store.list() else {
            return;
        };
        jobs.sort_by_key(|j| (j.created_at, j.id));
        let mut running = lock(&self.running);
        for job in jobs.into_iter().filter(|j| j.status == Status::Queued) {
            if running.len() >= self.max_running {
                break;
            }
            if running.contains_key(&job.id) {
                continue;
            }
            let cancel = Cancel::new();
            running.insert(
                job.id,
                Running {
                    cancel: cancel.clone(),
                    remove_after: false,
                },
            );
            let me = self.clone();
            tokio::spawn(async move { me.run(job, cancel).await });
        }
    }

    async fn run(self: Arc<Self>, job: Job, cancel: Cancel) {
        let id = job.id;
        let picked = pick_networks(&[]).unwrap_or_default();
        let nets: Vec<(String, String, String)> = picked
            .iter()
            .map(|i| (i.name.clone(), i.display_name.clone(), kind_word(i.kind)))
            .collect();
        let snapshot = {
            let me = self.clone();
            SnapshotFn(Arc::new(move |s: &Snapshot| {
                me.send(UiEvent::Live(live(id, s, &nets)));
            }))
        };
        let opts = RunOptions {
            networks: picked.iter().map(|i| i.name.clone()).collect(),
            snapshot: Some(snapshot),
            cancel: Some(cancel),
            retry_delay_scale: self.retry_scale,
            limiter: Some(self.limiter.clone()),
            per_network_dns: self
                .per_network_dns
                .load(std::sync::atomic::Ordering::Relaxed),
            ..RunOptions::default()
        };
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
        match outcome {
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
        if lock(&self.allowance_paused).remove(&id) {
            let _ = self.store.set_error(
                id,
                "Paused: every network reached its data allowance. It continues after the allowance resets, or raise it in Networks.",
                "allowance",
            );
        }
        let removed = lock(&self.running)
            .remove(&id)
            .is_some_and(|r| r.remove_after);
        if removed {
            let _ = runner::remove(&self.store, id);
        }
        self.publish_jobs();
        self.pump();
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
        let b = h.svc.add(&link(&server), None).unwrap();
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
            .map(|_| h.svc.add(&link(&server), None).unwrap())
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
        let waiting = h.svc.add(&link(&server), None).unwrap();
        let doomed = h.svc.add(&link(&server), None).unwrap();
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
        };
        let net = NetView {
            name: "en0".into(),
            label: "Wi-Fi".into(),
            kind: "wifi".into(),
            usable: true,
            addrs: vec![],
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
