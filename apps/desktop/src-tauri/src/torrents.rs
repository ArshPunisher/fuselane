//! Torrents in the desktop app (ADR 0006). Separate from the HTTP queue: a
//! torrent's life is different (peers, file choice, recheck on restart). The
//! engine starts on first use, so people who never add a torrent never run DHT.
//!
//! Finished torrents stop and release their files by default (seeding is opt-in,
//! P5 5.7): chosen files stay, edge-piece bytes in unchosen files are deleted
//! (L-69), and the row stays as "completed".

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use fuselane_core::Store;
use fuselane_engine_torrent::{
    AddOptions, EngineOptions, Listing, Phase, Source, Torrent, TorrentEngine, TorrentError,
};
use fuselane_netif::Interface;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::service::{Emit, UiError, UiEvent};

/// Finding a magnet's file list needs peers; give up after this long.
const METADATA_WAIT: std::time::Duration = std::time::Duration::from_secs(90);
/// Inspected torrents waiting for the user to pick files.
const MAX_PENDING: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TorrentView {
    /// The info hash: stable across restarts.
    pub id: String,
    pub name: String,
    pub folder: String,
    /// checking | downloading | paused | completed | failed
    pub status: String,
    pub done: u64,
    pub total: u64,
    pub uploaded: u64,
    /// Bytes per second over the last tick.
    pub rate: u64,
    pub error: Option<String>,
    pub file_count: usize,
    pub selected_count: usize,
    pub networks: Vec<TorrentNetView>,
    pub added_at: i64,
}

/// How soon a torrent file is fetched (B8.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    /// Not downloaded.
    Skip,
    /// Waits until everything else is done.
    Low,
    Normal,
    /// Fetched before the others (and what Play asks for).
    High,
}

/// Which of the wanted files the engine fetches now: every finished one stays
/// selected, and of the unfinished only the highest priority present. Low files
/// therefore wait for the rest, High ones go first.
pub fn tiered(
    wanted: &HashSet<usize>,
    prio: &HashMap<usize, Priority>,
    done: &dyn Fn(usize) -> bool,
) -> HashSet<usize> {
    let level = |i: usize| prio.get(&i).copied().unwrap_or(Priority::Normal);
    match wanted
        .iter()
        .filter(|i| !done(**i))
        .map(|i| level(*i))
        .max()
    {
        None => wanted.clone(),
        Some(top) => wanted
            .iter()
            .copied()
            .filter(|i| done(*i) || level(*i) >= top)
            .collect(),
    }
}

/// "Has this file fully arrived?" for [`tiered`], from the torrent's progress.
fn done_fn(t: &Torrent) -> impl Fn(usize) -> bool + use<> {
    let have = t.file_progress();
    let lens: Vec<u64> = t.listing().files.iter().map(|f| f.len).collect();
    move |i| {
        lens.get(i)
            .is_some_and(|len| have.get(i).is_some_and(|h| h >= len))
    }
}

/// When a torrent's peers were last read, and each one's bytes received and sent.
type PeerBytes = (Instant, HashMap<String, (u64, u64)>);

/// One connected peer, for the torrent's Peers tab.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PeerView {
    pub addr: String,
    pub client: Option<String>,
    /// Device name of our network it's on; None when it connected to us.
    pub network: Option<String>,
    /// Bytes per second from it and to it since the last look.
    pub down: u64,
    pub up: u64,
    pub received: u64,
    pub sent: u64,
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TorrentNetView {
    pub name: String,
    pub peers: usize,
    pub received: u64,
    /// Bytes per second received on this network over the last tick.
    #[serde(default)]
    pub rate: u64,
    /// Verified bytes credited to this network; all networks sum to `done`.
    pub credited: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFileView {
    pub index: usize,
    pub path: String,
    pub size: u64,
    pub selected: bool,
    /// Bytes of it that have arrived and passed their check.
    pub done: u64,
    pub priority: Priority,
}

/// What a torrent holds, before it starts: the user picks files from this.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListingView {
    /// Pass back to `add`; it is the info hash.
    pub token: String,
    pub name: String,
    pub folder: String,
    pub total: u64,
    pub files: Vec<TorrentFileView>,
}

/// Sharing back after a download (P5 5.7). Off by default; when on, it stops at
/// whichever limit comes first, and never uses metered networks while no torrent
/// is downloading.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeedSettings {
    pub enabled: bool,
    /// Stop after uploading this many times the download's size.
    pub ratio: f64,
    /// Stop after sharing this many minutes.
    pub minutes: u32,
}

impl Default for SeedSettings {
    fn default() -> Self {
        SeedSettings {
            enabled: false,
            ratio: 1.0,
            minutes: 60,
        }
    }
}

impl SeedSettings {
    fn validated(self) -> Result<SeedSettings, UiError> {
        if !self.ratio.is_finite() || !(0.1..=10.0).contains(&self.ratio) {
            return Err(UiError {
                code: "bad-ratio",
                message: "The sharing ratio must be between 0.1 and 10.".into(),
                hint: Some("1 means upload as much as you downloaded.".into()),
            });
        }
        if !(1..=10_080).contains(&self.minutes) {
            return Err(UiError {
                code: "bad-minutes",
                message: "Sharing time must be between 1 minute and 7 days (10080 minutes).".into(),
                hint: None,
            });
        }
        Ok(self)
    }
}

/// Saved across restarts (setting "torrents"); the .torrent itself is a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    id: String,
    base: PathBuf,
    selected: Vec<usize>,
    paused: bool,
    added_at: i64,
    /// Set once finished and released: the final view, shown as is.
    completed: Option<TorrentView>,
    /// File priorities other than Normal (B8.10).
    #[serde(default)]
    priorities: Vec<(usize, Priority)>,
}

struct Entry {
    torrent: Option<Torrent>,
    /// Every network it uses had reached its data allowance at the last tick.
    starved: bool,
    /// When the download finished (sharing time counts from here).
    finished_at: Option<Instant>,
    base: PathBuf,
    added_at: i64,
    last: TorrentView,
    prev: (u64, Instant),
    /// The files the person wants, once priorities are in play (else the
    /// engine's selection is the whole story).
    wanted: Option<HashSet<usize>>,
    /// Priorities other than Normal.
    prio: HashMap<usize, Priority>,
}

pub type NetSource = Arc<dyn Fn() -> Result<Vec<Interface>, String> + Send + Sync>;

pub struct Torrents {
    store: Arc<Store>,
    /// Holds `<id>.torrent` files and the DHT node list.
    state_dir: PathBuf,
    default_dir: PathBuf,
    networks: NetSource,
    dht: bool,
    limiter: Option<Arc<fuselane_limits::Limiter>>,
    seed: Mutex<SeedSettings>,
    /// When the network list was last re-read (torrents follow plug and unplug).
    nets_checked: Mutex<Option<Instant>>,
    engine: OnceCell<Arc<TorrentEngine>>,
    entries: tokio::sync::Mutex<Vec<Entry>>,
    pending: Mutex<Vec<Listing>>,
    emit: Emit,
    /// Something is checking or downloading (keeps the computer awake, holds off
    /// "when done" actions). Updated every tick.
    busy: std::sync::atomic::AtomicBool,
    /// The last peers snapshot per torrent, to turn byte counts into speeds.
    peer_bytes: Mutex<HashMap<String, PeerBytes>>,
}

impl std::fmt::Debug for Torrents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Torrents")
            .field("state_dir", &self.state_dir)
            .finish_non_exhaustive()
    }
}

fn ui(e: TorrentError) -> UiError {
    let (code, hint) = match &e {
        TorrentError::NotAMagnet => (
            "not-a-magnet",
            Some("Copy the whole magnet link and paste it again."),
        ),
        TorrentError::FileTooBig(_) => (
            "torrent-too-big",
            Some("Pick the .torrent file, not the download itself."),
        ),
        TorrentError::NoNetworks => (
            "no-networks",
            Some("Join a Wi-Fi network, plug in Ethernet, or tether a phone."),
        ),
        TorrentError::FileExists(_) => (
            "file-exists",
            Some("Move the existing file, or choose another folder."),
        ),
        TorrentError::UnsafePath(_) => (
            "unsafe-torrent",
            Some("Get the torrent from somewhere you trust."),
        ),
        TorrentError::AlreadyAdded => ("already-added", None),
        TorrentError::NothingSelected => ("nothing-selected", None),
        TorrentError::NoSuchFile(_) => ("no-such-file", None),
        TorrentError::Invalid(_) => (
            "invalid-torrent",
            Some("The file may be damaged. Download the .torrent again."),
        ),
        TorrentError::Engine(_) => (
            "torrent-engine",
            Some("Try again. If it keeps happening, restart Fuselane."),
        ),
    };
    UiError {
        code,
        message: e.to_string(),
        hint: hint.map(str::to_string),
    }
}

/// Moves a removed torrent's files to the Trash (B8.2), with the engine's path
/// checks: only the torrent's own regular files, never through a link.
fn to_trash(engine: &TorrentEngine, listing: &Listing) -> Result<(), UiError> {
    let done = engine.delete_files_with(listing, |p| {
        // Unit tests delete outright: they must never fill the developer's (or a
        // CI runner's) Trash.
        if cfg!(test) {
            std::fs::remove_file(p)
        } else {
            trash::delete(p).map_err(std::io::Error::other)
        }
    });
    if done.skipped.is_empty() {
        Ok(())
    } else {
        Err(UiError {
            code: "trash-failed",
            message: format!(
                "Removed from the list, but {} file{} couldn't be moved to the Trash.",
                done.skipped.len(),
                if done.skipped.len() == 1 { "" } else { "s" }
            ),
            hint: Some(format!(
                "Delete what's left in {} yourself.",
                listing.folder.display()
            )),
        })
    }
}

fn not_found() -> UiError {
    UiError {
        code: "not-found",
        message: "That torrent isn't in your list any more.".into(),
        hint: None,
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Info hashes are 40 lowercase hex characters; anything else never reaches the disk.
fn valid_id(id: &str) -> bool {
    id.len() == 40
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Networks to keep new peers off: metered ones (phone tethers, cellular) while
/// torrents only share. While anything downloads, every network helps.
fn metered_to_avoid(nets: &[Interface], seeding: bool, downloading: bool) -> Vec<String> {
    if !seeding || downloading {
        return Vec::new();
    }
    nets.iter()
        .filter(|i| {
            matches!(
                i.kind,
                fuselane_netif::Kind::Tether | fuselane_netif::Kind::Cellular
            )
        })
        .map(|i| i.name.clone())
        .collect()
}

/// A released torrent's final view: done, nothing moving, no peers.
fn freeze(v: &mut TorrentView) {
    v.rate = 0;
    v.status = "completed".into();
    for n in &mut v.networks {
        n.peers = 0;
        n.rate = 0;
    }
}

/// Sets each network's speed from the bytes it received since the last tick and
/// returns the torrent's speed as their sum, so the big number always equals the
/// lanes shown under it (B8.1). The verified-bytes delta jumps a whole piece at a
/// time; it is only the fallback when no network reports received bytes.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn lane_rates(
    nets: &mut [TorrentNetView],
    before: &[(String, u64)],
    verified_delta: u64,
    elapsed: f64,
) -> u64 {
    let per_sec = |bytes: u64| (bytes as f64 / elapsed.max(0.001)) as u64;
    for n in nets.iter_mut() {
        let was = before
            .iter()
            .find(|(name, _)| *name == n.name)
            .map_or(n.received, |(_, r)| *r);
        n.rate = per_sec(n.received.saturating_sub(was));
    }
    if nets.iter().all(|n| n.received == 0) {
        return per_sec(verified_delta);
    }
    nets.iter().map(|n| n.rate).sum()
}

fn view_of(t: &Torrent, added_at: i64, rate: u64) -> TorrentView {
    let p = t.progress();
    let status = match p.phase {
        Phase::Checking => "checking",
        Phase::Downloading => "downloading",
        // Finished but not yet released (a moment, or while seeding later).
        Phase::Seeding => "completed",
        Phase::Paused => "paused",
        Phase::Failed => "failed",
    };
    let listing = t.listing();
    TorrentView {
        id: t.info_hash(),
        name: listing.name.clone(),
        folder: listing.folder.display().to_string(),
        status: status.into(),
        done: p.done,
        total: p.total,
        uploaded: p.uploaded,
        rate,
        error: p.error,
        file_count: listing.files.iter().filter(|f| !f.padding).count(),
        selected_count: t.selected().len(),
        networks: t
            .networks()
            .into_iter()
            .map(|n| TorrentNetView {
                name: n.name,
                peers: n.peers,
                received: n.received,
                rate: 0,
                credited: n.credited,
            })
            .collect(),
        added_at,
    }
}

impl Torrents {
    pub fn new(
        store: Arc<Store>,
        state_dir: PathBuf,
        default_dir: PathBuf,
        networks: NetSource,
        dht: bool,
        limiter: Option<Arc<fuselane_limits::Limiter>>,
        emit: Emit,
    ) -> Arc<Torrents> {
        let seed = store
            .setting("torrent_seeding")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str::<SeedSettings>(&v).ok())
            .and_then(|s| s.validated().ok())
            .unwrap_or_default();
        Arc::new(Torrents {
            store,
            state_dir,
            default_dir,
            networks,
            dht,
            limiter,
            seed: Mutex::new(seed),
            nets_checked: Mutex::new(None),
            engine: OnceCell::new(),
            entries: tokio::sync::Mutex::new(Vec::new()),
            pending: Mutex::new(Vec::new()),
            emit,
            busy: std::sync::atomic::AtomicBool::new(false),
            peer_bytes: Mutex::new(HashMap::new()),
        })
    }

    async fn engine(&self) -> Result<Arc<TorrentEngine>, UiError> {
        self.engine
            .get_or_try_init(|| async {
                let networks = (self.networks)().map_err(|m| UiError {
                    code: "no-networks",
                    message: m,
                    hint: None,
                })?;
                std::fs::create_dir_all(&self.state_dir).map_err(|e| UiError {
                    code: "torrent-engine",
                    message: format!("Couldn't create Fuselane's torrent folder: {e}"),
                    hint: None,
                })?;
                TorrentEngine::start(EngineOptions {
                    download_dir: self.default_dir.clone(),
                    networks,
                    dht: self.dht,
                    listen: None,
                    state_dir: Some(self.state_dir.clone()),
                    limiter: self.limiter.clone(),
                    upnp: false,
                    local_discovery: false,
                })
                .await
                .map(Arc::new)
                .map_err(ui)
            })
            .await
            .cloned()
    }

    fn remember(&self, l: Listing) -> ListingView {
        let view = ListingView {
            token: l.info_hash.clone(),
            name: l.name.clone(),
            folder: l.folder.display().to_string(),
            total: l.total(),
            files: l
                .files
                .iter()
                .enumerate()
                .filter(|(_, f)| !f.padding)
                .map(|(i, f)| TorrentFileView {
                    index: i,
                    path: f.parts.join("/"),
                    size: f.len,
                    selected: true,
                    done: 0,
                    priority: Priority::Normal,
                })
                .collect(),
        };
        let mut p = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        p.retain(|x| x.info_hash != l.info_hash);
        p.push(l);
        if p.len() > MAX_PENDING {
            p.remove(0);
        }
        view
    }

    fn folder(&self, dir: Option<&str>) -> PathBuf {
        dir.filter(|d| !d.trim().is_empty())
            .map_or_else(|| self.default_dir.clone(), PathBuf::from)
    }

    pub async fn inspect_magnet(
        &self,
        magnet: &str,
        dir: Option<&str>,
    ) -> Result<ListingView, UiError> {
        let engine = self.engine().await?;
        let listing = tokio::time::timeout(
            METADATA_WAIT,
            engine.inspect(
                Source::Magnet(magnet.to_owned()),
                Some(self.folder(dir)),
                vec![],
            ),
        )
        .await
        .map_err(|_| UiError {
            code: "no-peers",
            message: "Nobody sharing this torrent could be found yet.".into(),
            hint: Some(
                "Try again later; torrents with few people sharing them can take a while.".into(),
            ),
        })?
        .map_err(ui)?;
        Ok(self.remember(listing))
    }

    pub async fn inspect_file(
        &self,
        path: &Path,
        dir: Option<&str>,
    ) -> Result<ListingView, UiError> {
        let meta = std::fs::metadata(path).map_err(|e| UiError {
            code: "torrent-unreadable",
            message: format!("Couldn't open that file: {e}"),
            hint: None,
        })?;
        if !meta.is_file() {
            return Err(UiError {
                code: "torrent-unreadable",
                message: "That is a folder, not a .torrent file.".into(),
                hint: None,
            });
        }
        let max = fuselane_engine_torrent::engine::MAX_TORRENT_FILE as u64;
        if meta.len() > max {
            return Err(ui(TorrentError::FileTooBig(
                usize::try_from(meta.len()).unwrap_or(usize::MAX),
            )));
        }
        let bytes = std::fs::read(path).map_err(|e| UiError {
            code: "torrent-unreadable",
            message: format!("Couldn't read that file: {e}"),
            hint: None,
        })?;
        self.inspect_bytes(bytes, dir).await
    }

    /// A .torrent's contents, for files dropped on the window (no path is known).
    pub async fn inspect_bytes(
        &self,
        bytes: Vec<u8>,
        dir: Option<&str>,
    ) -> Result<ListingView, UiError> {
        if bytes.len() > fuselane_engine_torrent::engine::MAX_TORRENT_FILE {
            return Err(ui(TorrentError::FileTooBig(bytes.len())));
        }
        let engine = self.engine().await?;
        let listing = engine
            .inspect(Source::File(bytes), Some(self.folder(dir)), vec![])
            .await
            .map_err(ui)?;
        Ok(self.remember(listing))
    }

    /// Starts an inspected torrent with the chosen files.
    pub async fn add(&self, token: &str, files: Vec<usize>) -> Result<String, UiError> {
        let listing = {
            let mut p = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let i = p
                .iter()
                .position(|l| l.info_hash == token)
                .ok_or_else(|| UiError {
                    code: "expired",
                    message: "That torrent's details were cleared. Add it again.".into(),
                    hint: None,
                })?;
            p.remove(i)
        };
        let base = listing.base().to_path_buf();
        let id = listing.info_hash.clone();
        // A finished torrent is no longer in the engine but is still in the list.
        if self.entries.lock().await.iter().any(|e| e.last.id == id) {
            return Err(ui(TorrentError::AlreadyAdded));
        }
        let engine = self.engine().await?;
        std::fs::write(
            self.state_dir.join(format!("{id}.torrent")),
            listing.torrent_bytes(),
        )
        .map_err(|e| UiError {
            code: "torrent-engine",
            message: format!("Couldn't save the torrent: {e}"),
            hint: None,
        })?;
        let t = engine
            .add_listed(
                listing,
                AddOptions {
                    only: Some(files.into_iter().collect()),
                    ..Default::default()
                },
            )
            .await
            .map_err(ui)?;
        let added_at = now();
        let last = view_of(&t, added_at, 0);
        self.entries.lock().await.push(Entry {
            torrent: Some(t),
            starved: false,
            finished_at: None,
            base,
            added_at,
            last,
            prev: (0, Instant::now()),
            wanted: None,
            prio: HashMap::new(),
        });
        self.save().await;
        self.publish().await;
        Ok(id)
    }

    pub async fn list(&self) -> Vec<TorrentView> {
        self.entries
            .lock()
            .await
            .iter()
            .map(|e| e.last.clone())
            .collect()
    }

    pub async fn files(&self, id: &str) -> Result<Vec<TorrentFileView>, UiError> {
        let entries = self.entries.lock().await;
        let e = entries
            .iter()
            .find(|e| e.last.id == id)
            .ok_or_else(not_found)?;
        let Some(t) = &e.torrent else {
            return Ok(Vec::new());
        };
        let sel = e.wanted.clone().unwrap_or_else(|| t.selected());
        let have = t.file_progress();
        Ok(t.listing()
            .files
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.padding)
            .map(|(i, f)| TorrentFileView {
                index: i,
                path: f.parts.join("/"),
                size: f.len,
                selected: sel.contains(&i),
                done: have.get(i).copied().unwrap_or(0).min(f.len),
                priority: if sel.contains(&i) {
                    e.prio.get(&i).copied().unwrap_or(Priority::Normal)
                } else {
                    Priority::Skip
                },
            })
            .collect())
    }

    async fn live(&self, id: &str) -> Result<(Arc<TorrentEngine>, Torrent), UiError> {
        let entries = self.entries.lock().await;
        let e = entries
            .iter()
            .find(|e| e.last.id == id)
            .ok_or_else(not_found)?;
        let t = e.torrent.clone().ok_or_else(|| UiError {
            code: "finished",
            message: "This torrent has finished.".into(),
            hint: None,
        })?;
        drop(entries);
        Ok((self.engine().await?, t))
    }

    /// The peers connected to a torrent now, fastest first, with their speeds
    /// since the window last asked (it asks about once a second while showing them).
    pub async fn peers(&self, id: &str) -> Result<Vec<PeerView>, UiError> {
        let Ok((_, t)) = self.live(id).await else {
            return Ok(Vec::new());
        };
        let now = Instant::now();
        let peers = t.peers();
        let mut last = self.peer_bytes.lock().unwrap_or_else(|p| p.into_inner());
        let prev = last.remove(id);
        let secs = prev
            .as_ref()
            .map_or(0.0, |(at, _)| now.duration_since(*at).as_secs_f64());
        let rate = |was: Option<u64>, is: u64| -> u64 {
            match (was, secs > 0.05) {
                (Some(w), true) => (is.saturating_sub(w) as f64 / secs) as u64,
                _ => 0,
            }
        };
        let mut views: Vec<PeerView> = peers
            .iter()
            .map(|p| {
                let was = prev.as_ref().and_then(|(_, m)| m.get(&p.addr));
                PeerView {
                    addr: p.addr.clone(),
                    client: p.client.clone(),
                    network: p.network.clone(),
                    down: rate(was.map(|w| w.0), p.received),
                    up: rate(was.map(|w| w.1), p.sent),
                    received: p.received,
                    sent: p.sent,
                    kind: p.kind,
                }
            })
            .collect();
        views.sort_by(|a, b| {
            (b.down + b.up)
                .cmp(&(a.down + a.up))
                .then(b.received.cmp(&a.received))
                .then(a.addr.cmp(&b.addr))
        });
        last.insert(
            id.to_string(),
            (
                now,
                peers
                    .into_iter()
                    .map(|p| (p.addr, (p.received, p.sent)))
                    .collect(),
            ),
        );
        Ok(views)
    }

    /// How complete each slice of the torrent is (0 to 100), for its pieces map.
    pub async fn pieces(&self, id: &str, cells: usize) -> Result<Vec<u8>, UiError> {
        let cells = cells.clamp(1, 2048);
        match self.live(id).await {
            Ok((engine, t)) => Ok(engine.piece_map(&t, cells)),
            // A finished torrent has let go of its pieces: all of it is here.
            Err(e) if e.code == "finished" => Ok(vec![100; cells.min(256)]),
            Err(e) => Err(e),
        }
    }

    pub async fn pause(&self, id: &str) -> Result<(), UiError> {
        let (engine, t) = self.live(id).await?;
        engine.pause(&t).await.map_err(ui)?;
        self.after_change().await;
        Ok(())
    }

    pub async fn resume(&self, id: &str) -> Result<(), UiError> {
        let (engine, t) = self.live(id).await?;
        engine.resume(&t).await.map_err(ui)?;
        self.after_change().await;
        Ok(())
    }

    pub async fn select(&self, id: &str, files: Vec<usize>) -> Result<(), UiError> {
        let (engine, t) = self.live(id).await?;
        let files: HashSet<usize> = files.into_iter().collect();
        let tiers = {
            let mut entries = self.entries.lock().await;
            let e = entries
                .iter_mut()
                .find(|e| e.last.id == id)
                .ok_or_else(not_found)?;
            if e.wanted.is_some() {
                e.prio.retain(|i, _| files.contains(i));
                e.wanted = Some(files.clone());
                Some(tiered(&files, &e.prio, &done_fn(&t)))
            } else {
                None
            }
        };
        engine
            .select(&t, tiers.unwrap_or(files))
            .await
            .map_err(ui)?;
        self.after_change().await;
        Ok(())
    }

    /// One file of a live torrent, read in order as it arrives (for Play).
    pub async fn open_stream(
        &self,
        id: &str,
        file: usize,
    ) -> Option<(Box<dyn fuselane_engine_torrent::FileReader>, u64, String)> {
        let (engine, t) = self.live(id).await.ok()?;
        let f = t.listing().files.get(file)?.clone();
        let reader = engine.stream(&t, file).await.ok()?;
        Some((reader, f.len, f.parts.last().cloned().unwrap_or_default()))
    }

    /// The name of file `file` in a live torrent, if there is one.
    pub async fn file_name(&self, id: &str, file: usize) -> Option<String> {
        let (_, t) = self.live(id).await.ok()?;
        t.listing()
            .files
            .get(file)
            .and_then(|f| f.parts.last().cloned())
    }

    /// Sets one file's priority (B8.10). Skip leaves it out; at least one file
    /// must stay wanted.
    pub async fn set_priority(&self, id: &str, file: usize, p: Priority) -> Result<(), UiError> {
        let (engine, t) = self.live(id).await?;
        if t.listing().files.get(file).is_none_or(|f| f.padding) {
            return Err(UiError {
                code: "no-such-file",
                message: "That file isn't in this torrent.".into(),
                hint: None,
            });
        }
        let next = {
            let mut entries = self.entries.lock().await;
            let e = entries
                .iter_mut()
                .find(|e| e.last.id == id)
                .ok_or_else(not_found)?;
            let mut wanted = e.wanted.clone().unwrap_or_else(|| t.selected());
            match p {
                Priority::Skip => {
                    wanted.remove(&file);
                }
                _ => {
                    wanted.insert(file);
                }
            }
            if wanted.is_empty() {
                return Err(UiError {
                    code: "nothing-selected",
                    message: "At least one file has to be downloaded.".into(),
                    hint: Some("Pick another file first, or remove the torrent.".into()),
                });
            }
            if matches!(p, Priority::Skip | Priority::Normal) {
                e.prio.remove(&file);
            } else {
                e.prio.insert(file, p);
            }
            e.wanted = Some(wanted.clone());
            tiered(&wanted, &e.prio, &done_fn(&t))
        };
        engine.select(&t, next).await.map_err(ui)?;
        self.after_change().await;
        Ok(())
    }

    /// Removes a torrent from the list; with `delete_files`, its files too.
    pub async fn remove(&self, id: &str, delete_files: bool) -> Result<(), UiError> {
        let entry = {
            let mut entries = self.entries.lock().await;
            let i = entries
                .iter()
                .position(|e| e.last.id == id)
                .ok_or_else(not_found)?;
            entries.remove(i)
        };
        if let Some(t) = entry.torrent {
            let engine = self.engine().await?;
            let listing = t.listing().clone();
            engine.release(t).await.map_err(ui)?;
            if delete_files {
                to_trash(&engine, &listing)?;
            }
        } else if delete_files && valid_id(id) {
            // Already released: read the saved .torrent again for its exact file list.
            let bytes = std::fs::read(self.state_dir.join(format!("{id}.torrent"))).map_err(|_| UiError {
                code: "torrent-unreadable",
                message: "Fuselane no longer has this torrent's file list, so it can't delete the files.".into(),
                hint: Some("Delete them in your file manager.".into()),
            })?;
            let engine = self.engine().await?;
            let listing = engine
                .inspect(Source::File(bytes), Some(entry.base.clone()), vec![])
                .await
                .map_err(ui)?;
            to_trash(&engine, &listing)?;
        }
        if valid_id(id) {
            let _ = std::fs::remove_file(self.state_dir.join(format!("{id}.torrent")));
        }
        self.save().await;
        self.publish().await;
        Ok(())
    }

    pub fn seed_settings(&self) -> SeedSettings {
        self.seed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub async fn set_seed_settings(&self, s: SeedSettings) -> Result<SeedSettings, UiError> {
        let s = s.validated()?;
        let json = serde_json::to_string(&s).map_err(|e| UiError {
            code: "store",
            message: format!("Couldn't save the setting: {e}"),
            hint: None,
        })?;
        self.store
            .set_setting("torrent_seeding", &json)
            .map_err(|e| UiError {
                code: "store",
                message: format!("Couldn't save the setting: {e}"),
                hint: None,
            })?;
        *self
            .seed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = s.clone();
        // Turning sharing off releases seeding torrents now, not at the next limit.
        self.refresh(true).await;
        self.save().await;
        self.publish().await;
        Ok(s)
    }

    /// Stops sharing a finished torrent now; its files stay.
    pub async fn stop_sharing(&self, id: &str) -> Result<(), UiError> {
        let engine = self.engine().await?;
        let mut entries = self.entries.lock().await;
        let e = entries
            .iter_mut()
            .find(|e| e.last.id == id)
            .ok_or_else(not_found)?;
        if e.last.status != "seeding" {
            return Err(UiError {
                code: "not-sharing",
                message: "This torrent isn't sharing.".into(),
                hint: None,
            });
        }
        if let Some(t) = e.torrent.take() {
            engine.release(t).await.map_err(ui)?;
        }
        freeze(&mut e.last);
        drop(entries);
        self.save().await;
        self.publish().await;
        Ok(())
    }

    /// Where a torrent's files are, for "Show in Finder".
    pub async fn folder_of(&self, id: &str) -> Result<PathBuf, UiError> {
        let entries = self.entries.lock().await;
        let e = entries
            .iter()
            .find(|e| e.last.id == id)
            .ok_or_else(not_found)?;
        Ok(PathBuf::from(&e.last.folder))
    }

    async fn after_change(&self) {
        self.refresh(false).await;
        self.save().await;
        self.publish().await;
    }

    /// Updates every view, releases finished torrents, and returns whether any is active.
    async fn refresh(&self, release_finished: bool) -> bool {
        let engine = self.engine.get().cloned();
        let seed = self.seed_settings();
        let mut entries = self.entries.lock().await;
        let mut active = false;
        let mut released = false;
        let mut seeding = false;
        let mut downloading = false;
        for e in entries.iter_mut() {
            let Some(t) = e.torrent.clone() else { continue };
            let p = t.progress();
            let elapsed = e.prev.1.elapsed().as_secs_f64().max(0.001);
            let verified_delta = p.done.saturating_sub(e.prev.0);
            e.prev = (p.done, Instant::now());
            let before: Vec<(String, u64)> = e
                .last
                .networks
                .iter()
                .map(|n| (n.name.clone(), n.received))
                .collect();
            e.last = view_of(&t, e.added_at, 0);
            e.last.rate = lane_rates(&mut e.last.networks, &before, verified_delta, elapsed);
            // Priorities (B8.10): move on to the next tier as one finishes, and the
            // torrent counts as done only once every wanted file is.
            let mut more_to_come = false;
            if let Some(wanted) = e.wanted.clone() {
                let done = done_fn(&t);
                let next = tiered(&wanted, &e.prio, &done);
                if next != t.selected()
                    && let Some(engine) = &engine
                {
                    let _ = engine.select(&t, next).await;
                }
                if wanted.iter().any(|i| !done(*i)) {
                    more_to_come = true;
                    let have = t.file_progress();
                    let files = &t.listing().files;
                    let len = |i: &usize| files.get(*i).map_or(0, |f| f.len);
                    e.last.total = wanted.iter().map(len).sum();
                    e.last.done = wanted
                        .iter()
                        .map(|i| have.get(*i).copied().unwrap_or(0).min(len(i)))
                        .sum();
                    if e.last.status == "completed" {
                        e.last.status = "downloading".into();
                    }
                }
            }
            let starved = self.all_networks_used_up(&e.last);
            // An allowance was raised or reset: reconnect now instead of waiting out
            // librqbit's growing retry timers.
            if e.starved
                && !starved
                && p.phase == Phase::Downloading
                && let Some(engine) = &engine
            {
                let _ = engine.reconnect(&t).await;
            }
            e.starved = starved;
            if p.phase == Phase::Downloading && starved {
                e.last.error = Some(
                    "Every network has reached its data allowance, so this torrent is waiting. Raise an allowance on the Networks page, or wait for it to reset."
                        .into(),
                );
            }
            active |= more_to_come || matches!(p.phase, Phase::Checking | Phase::Downloading);
            let finished = !more_to_come && p.phase == Phase::Seeding && p.done == p.total;
            if finished {
                let at = *e.finished_at.get_or_insert_with(Instant::now);
                let share = &seed;
                let enough = !share.enabled
                    || p.uploaded as f64 >= share.ratio * p.total as f64
                    || at.elapsed().as_secs() >= u64::from(share.minutes) * 60;
                if !enough {
                    e.last.status = "seeding".into();
                    seeding = true;
                } else if release_finished && let Some(engine) = &engine {
                    let _ = engine.release(t).await;
                    freeze(&mut e.last);
                    e.torrent = None;
                    released = true;
                }
            }
            downloading |= more_to_come || matches!(p.phase, Phase::Checking | Phase::Downloading);
        }
        // Metered guard: while torrents only share, phone tethers and cellular take
        // no new peers. While something downloads, every network helps (bonding).
        if let Some(engine) = &engine {
            let avoid = metered_to_avoid(&engine.interfaces(), seeding, downloading);
            engine.avoid_networks(avoid);
        }
        active = active || seeding;
        drop(entries);
        if released {
            self.save().await;
        }
        active || released
    }

    /// Every 10 s, hands the engine the networks present now, so a phone tethered
    /// mid-torrent joins in and an unplugged network stops taking peers.
    fn follow_networks(&self, engine: &TorrentEngine) {
        {
            let mut last = self
                .nets_checked
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if last.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(10)) {
                return;
            }
            *last = Some(Instant::now());
        }
        // A failed or empty listing is ignored: keep what we had rather than nothing.
        if let Ok(now) = (self.networks)()
            && !now.is_empty()
        {
            engine.set_networks(now);
        }
    }

    /// True when the limiter blocks every network this torrent uses.
    fn all_networks_used_up(&self, v: &TorrentView) -> bool {
        self.limiter.as_ref().is_some_and(|l| {
            !v.networks.is_empty() && v.networks.iter().all(|n| l.blocked(&n.name))
        })
    }

    async fn publish(&self) {
        let torrents = self.list().await;
        (self.emit)(UiEvent::Torrents { torrents });
    }

    /// Called every second: refreshes, releases finished torrents, and tells the
    /// window when something moved.
    pub async fn tick(&self) {
        let Some(engine) = self.engine.get().cloned() else {
            return;
        };
        self.follow_networks(&engine);
        if self.refresh(true).await {
            self.publish().await;
        }
        let busy = self
            .entries
            .lock()
            .await
            .iter()
            .any(|e| matches!(e.last.status.as_str(), "checking" | "downloading"));
        self.busy.store(busy, std::sync::atomic::Ordering::Relaxed);
    }

    /// Whether any torrent is checking or downloading (as of the last tick).
    pub fn busy(&self) -> bool {
        self.busy.load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn save(&self) {
        let entries = self.entries.lock().await;
        let saved: Vec<Saved> = entries
            .iter()
            .map(|e| Saved {
                id: e.last.id.clone(),
                base: e.base.clone(),
                selected: match (&e.wanted, &e.torrent) {
                    (Some(w), _) => w.iter().copied().collect(),
                    (None, Some(t)) => t.selected().into_iter().collect(),
                    (None, None) => vec![],
                },
                priorities: e.prio.iter().map(|(i, p)| (*i, *p)).collect(),
                paused: e.last.status == "paused",
                added_at: e.added_at,
                completed: e.torrent.is_none().then(|| e.last.clone()),
            })
            .collect();
        drop(entries);
        if let Ok(json) = serde_json::to_string(&saved) {
            let _ = self.store.set_setting("torrents", &json);
        }
    }

    /// Brings saved torrents back after a launch. Unfinished ones are rechecked
    /// from disk (resume) and come back paused if they were paused.
    pub async fn restore(&self) {
        let saved: Vec<Saved> = self
            .store
            .setting("torrents")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();
        if saved.is_empty() {
            return;
        }
        for s in saved {
            // Skip anything already added this launch (restore runs alongside new adds).
            if !valid_id(&s.id) || self.entries.lock().await.iter().any(|e| e.last.id == s.id) {
                continue;
            }
            if let Some(done) = s.completed {
                self.entries.lock().await.push(Entry {
                    torrent: None,
                    starved: false,
                    finished_at: None,
                    base: s.base,
                    added_at: s.added_at,
                    last: done,
                    prev: (0, Instant::now()),
                    wanted: None,
                    prio: HashMap::new(),
                });
                continue;
            }
            let Ok(bytes) = std::fs::read(self.state_dir.join(format!("{}.torrent", s.id))) else {
                continue;
            };
            let Ok(engine) = self.engine().await else {
                return;
            };
            let opts = AddOptions {
                only: (!s.selected.is_empty()).then(|| s.selected.iter().copied().collect()),
                paused: s.paused,
                resume: true,
                storage: None,
            };
            match engine
                .add(Source::File(bytes), Some(s.base.clone()), vec![], opts)
                .await
            {
                Ok(t) => {
                    let last = view_of(&t, s.added_at, 0);
                    self.entries.lock().await.push(Entry {
                        torrent: Some(t),
                        starved: false,
                        finished_at: None,
                        base: s.base,
                        added_at: s.added_at,
                        last,
                        prev: (0, Instant::now()),
                        wanted: (!s.priorities.is_empty())
                            .then(|| s.selected.iter().copied().collect()),
                        prio: s.priorities.iter().copied().collect(),
                    });
                }
                Err(e) => eprintln!("fuselane: couldn't bring back a torrent: {e}"),
            }
        }
        self.save().await;
        self.publish().await;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;

    use librqbit::{
        AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session,
        SessionOptions,
    };

    pub(crate) fn ts_fields(src: &str, name: &str) -> Vec<String> {
        let start = src
            .find(&format!("export interface {name} {{"))
            .unwrap_or_else(|| panic!("no {name}"));
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

    pub(crate) fn json_fields(v: &impl Serialize) -> Vec<String> {
        let mut out: Vec<String> = serde_json::to_value(v)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        out.sort();
        out
    }

    #[test]
    fn the_window_types_match_what_the_backend_sends() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/types.ts"))
                .unwrap();
        let net = TorrentNetView {
            name: "en0".into(),
            peers: 0,
            received: 0,
            rate: 0,
            credited: 0,
        };
        let file = TorrentFileView {
            index: 0,
            path: "a".into(),
            size: 1,
            selected: true,
            done: 0,
            priority: Priority::Normal,
        };
        let view = TorrentView {
            id: "a".repeat(40),
            name: "n".into(),
            folder: "f".into(),
            status: "paused".into(),
            done: 0,
            total: 0,
            uploaded: 0,
            rate: 0,
            error: None,
            file_count: 1,
            selected_count: 1,
            networks: vec![],
            added_at: 0,
        };
        let listing = ListingView {
            token: "t".into(),
            name: "n".into(),
            folder: "f".into(),
            total: 1,
            files: vec![],
        };
        let peer = PeerView {
            addr: "1.2.3.4:5".into(),
            client: None,
            network: None,
            down: 0,
            up: 0,
            received: 0,
            sent: 0,
            kind: "tcp",
        };
        for (name, got) in [
            ("PeerView", json_fields(&peer)),
            ("TorrentView", json_fields(&view)),
            ("TorrentNetView", json_fields(&net)),
            ("TorrentFileView", json_fields(&file)),
            ("ListingView", json_fields(&listing)),
        ] {
            assert_eq!(
                ts_fields(&src, name),
                got,
                "{name} differs between types.ts and torrents.rs"
            );
        }
        // The status words the window knows are the ones view_of produces.
        for s in ["checking", "downloading", "paused", "completed", "failed"] {
            assert!(src.contains(&format!("'{s}'")), "{s}");
        }
    }

    #[test]
    fn the_speed_is_the_sum_of_the_networks() {
        let net = |name: &str, received: u64| TorrentNetView {
            name: name.into(),
            peers: 1,
            received,
            rate: 0,
            credited: 0,
        };
        let mut nets = vec![
            net("Wi-Fi", 3_000_000),
            net("Ethernet", 7_000_000),
            net("New", 500_000),
        ];
        let before = vec![
            ("Wi-Fi".to_string(), 1_000_000),
            ("Ethernet".to_string(), 2_000_000),
        ];
        // A whole 4 MiB piece got verified this tick: it must not move the number.
        let total = lane_rates(&mut nets, &before, 4 << 20, 2.0);
        assert_eq!(nets[0].rate, 1_000_000);
        assert_eq!(nets[1].rate, 2_500_000);
        // A network that just joined starts from zero instead of counting its total.
        assert_eq!(nets[2].rate, 0);
        assert_eq!(total, nets.iter().map(|n| n.rate).sum::<u64>());
        // No per-network counts (nothing received yet): fall back to verified bytes.
        let mut idle = vec![net("Wi-Fi", 0)];
        assert_eq!(lane_rates(&mut idle, &[], 2_000, 2.0), 1_000);
    }

    #[test]
    fn only_real_info_hashes_name_files() {
        assert!(valid_id(&"a".repeat(40)));
        assert!(!valid_id("../../etc/passwd"));
        assert!(!valid_id(&"A".repeat(40)));
        assert!(!valid_id(&"a".repeat(41)));
    }

    /// librqbit's and the proxy's debug logs, shown by nextest only when a test fails.
    fn logs() {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("librqbit=debug,librqbit_core=info,fuselane_engine_torrent=debug")
            .with_test_writer()
            .try_init();
    }

    fn payload(len: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }

    /// A librqbit seeder for folder `T` with three files; the torrent file is
    /// written to `<root>/T.torrent`.
    async fn seeder(root: &Path) -> (Arc<Session>, SocketAddr, PathBuf) {
        let folder = root.join("T");
        std::fs::create_dir_all(&folder).unwrap();
        for (i, n) in ["a.bin", "b.bin", "c.bin"].iter().enumerate() {
            std::fs::write(folder.join(n), payload(90_000 + i * 7_001, i as u64 + 3)).unwrap();
        }
        let spawner = librqbit::spawn_utils::BlockingSpawner::new(2);
        let torrent = librqbit::create_torrent(
            &folder,
            CreateTorrentOptions {
                name: Some("T"),
                trackers: vec![],
                piece_length: Some(32 * 1024),
            },
            &spawner,
        )
        .await
        .unwrap();
        let bytes = torrent.as_bytes().unwrap();
        let file = root.join("T.torrent");
        std::fs::write(&file, &bytes).unwrap();
        let session = Session::new_with_opts(
            root.to_path_buf(),
            SessionOptions {
                dht: None,
                persistence: None,
                fastresume: false,
                disable_local_service_discovery: true,
                listen: Some(ListenerOptions {
                    listen_addr: (Ipv4Addr::LOCALHOST, 0).into(),
                    enable_upnp_port_forwarding: false,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let h = session
            .add_torrent(
                AddTorrent::TorrentFileBytes(bytes),
                Some(AddTorrentOptions {
                    output_folder: Some(folder.to_string_lossy().into_owned()),
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .unwrap()
            .into_handle()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(30), h.wait_until_completed())
            .await
            .unwrap()
            .unwrap();
        let addr = session.listen_addr().unwrap();
        (session, addr, file)
    }

    struct Setup {
        tor: Arc<Torrents>,
        limiter: Arc<fuselane_limits::Limiter>,
        events: Arc<Mutex<Vec<UiEvent>>>,
        store: Arc<Store>,
        state: PathBuf,
        downloads: PathBuf,
        _dir: tempfile::TempDir,
    }

    fn loopback() -> NetSource {
        Arc::new(|| {
            Ok(vec![Interface {
                name: "lo0".into(),
                display_name: "Loopback".into(),
                index: 1,
                kind: fuselane_netif::Kind::Loopback,
                addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
            }])
        })
    }

    fn setup() -> Setup {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&dir.path().join("fuselane.db")).unwrap());
        let state = dir.path().join("state");
        let downloads = dir.path().join("dl");
        std::fs::create_dir_all(&downloads).unwrap();
        reopen(store, state, downloads, dir)
    }

    fn reopen(
        store: Arc<Store>,
        state: PathBuf,
        downloads: PathBuf,
        dir: tempfile::TempDir,
    ) -> Setup {
        let limiter = Arc::new(fuselane_limits::Limiter::default());
        let events: Arc<Mutex<Vec<UiEvent>>> = Arc::default();
        let sink = events.clone();
        let tor = Torrents::new(
            store.clone(),
            state.clone(),
            downloads.clone(),
            loopback(),
            false,
            Some(limiter.clone()),
            Arc::new(move |e| sink.lock().unwrap().push(e)),
        );
        Setup {
            tor,
            limiter,
            events,
            store,
            state,
            downloads,
            _dir: dir,
        }
    }

    async fn until(tor: &Torrents, id: &str, status: &str) -> TorrentView {
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                tor.tick().await;
                if let Some(v) = tor
                    .list()
                    .await
                    .into_iter()
                    .find(|v| v.id == id && v.status == status)
                {
                    return v;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            // Say where it got stuck, so a CI-only failure can be read from the log.
            let last = describe_entries(tor);
            panic!("never reached {status}; last seen: {last}")
        })
    }

    fn describe_entries(tor: &Torrents) -> String {
        let entries = tor.entries.try_lock();
        match entries {
            Ok(e) => e
                .iter()
                .map(|e| {
                    let p = e.torrent.as_ref().map(|t| format!("{:?}", t.progress()));
                    format!(
                        "{} {}/{} err={:?} live={p:?}",
                        e.last.status, e.last.done, e.last.total, e.last.error
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
            Err(_) => "busy".into(),
        }
    }

    /// Adds the seeder's torrent with the given files, pointing the engine at the seeder.
    async fn add(s: &Setup, file: &Path, peer: SocketAddr, pick: &[&str]) -> String {
        let listing = s.tor.inspect_file(file, None).await.unwrap();
        assert_eq!(listing.name, "T");
        assert_eq!(listing.files.len(), 3);
        assert_eq!(
            listing.total,
            listing.files.iter().map(|f| f.size).sum::<u64>()
        );
        let files = listing
            .files
            .iter()
            .filter(|f| pick.iter().any(|p| f.path == *p))
            .map(|f| f.index)
            .collect();
        // The engine's own inspect has no peers; hand the seeder over directly.
        let engine = s.tor.engine().await.unwrap();
        {
            let mut p = s.tor.pending.lock().unwrap();
            p.clear();
        }
        let l = engine
            .inspect(
                Source::File(std::fs::read(file).unwrap()),
                Some(s.downloads.clone()),
                vec![peer],
            )
            .await
            .unwrap();
        s.tor.remember(l);
        s.tor.add(&listing.token, files).await.unwrap()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn peers_and_the_pieces_map_follow_a_download() {
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        let id = add(&s, &file, addr, &["a.bin", "b.bin", "c.bin"]).await;
        // While it runs: the seeder is a peer on the loopback network.
        let mut seen = None;
        for _ in 0..200 {
            if let Some(p) = s.tor.peers(&id).await.unwrap().into_iter().next() {
                seen = Some(p);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        if let Some(p) = seen {
            assert_eq!(p.addr, addr.to_string());
            assert_eq!(p.network.as_deref(), Some("lo0"));
        }
        until(&s.tor, &id, "completed").await;
        let map = s.tor.pieces(&id, 32).await.unwrap();
        assert!(!map.is_empty() && map.iter().all(|&c| c == 100), "{map:?}");
        assert!(
            s.tor.peers(&id).await.unwrap().is_empty(),
            "a finished torrent has let go"
        );
        assert_eq!(
            s.tor
                .pieces("0".repeat(40).as_str(), 8)
                .await
                .unwrap_err()
                .code,
            "not-found"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_torrent_downloads_the_chosen_files_then_releases_and_survives_a_restart() {
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        let id = add(&s, &file, addr, &["a.bin", "c.bin"]).await;
        let done = until(&s.tor, &id, "completed").await;
        assert_eq!(done.done, done.total);
        assert_eq!((done.file_count, done.selected_count), (3, 2));
        assert_eq!(
            done.networks.iter().map(|n| n.credited).sum::<u64>(),
            done.done
        );
        assert!(
            done.networks.iter().all(|n| n.peers == 0),
            "released: no peers"
        );
        let t = s.downloads.join("T");
        for n in ["a.bin", "c.bin"] {
            assert_eq!(
                std::fs::read(t.join(n)).unwrap(),
                std::fs::read(seed.path().join("T").join(n)).unwrap(),
                "{n}"
            );
        }
        assert!(
            !t.join("b.bin").exists(),
            "the unchosen file was cleaned up"
        );
        assert!(
            s.events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, UiEvent::Torrents { .. }))
        );
        // Bad ids and bad actions on a finished torrent get clear errors.
        assert_eq!(s.tor.pause("nope").await.unwrap_err().code, "not-found");
        assert_eq!(s.tor.pause(&id).await.unwrap_err().code, "finished");

        // Restoring while the torrent is already listed must not add it twice.
        s.tor.restore().await;
        assert_eq!(s.tor.list().await.len(), 1);

        // Restart: the finished row comes back as it was.
        let again = reopen(
            s.store.clone(),
            s.state.clone(),
            s.downloads.clone(),
            tempfile::tempdir().unwrap(),
        );
        again.tor.restore().await;
        let list = again.tor.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(
            (list[0].status.as_str(), list[0].done),
            ("completed", done.done)
        );

        // Removing with files deletes what was downloaded, and nothing else.
        std::fs::write(t.join("mine.txt"), b"user file").unwrap();
        again.tor.remove(&id, true).await.unwrap();
        assert!(!t.join("a.bin").exists() && !t.join("c.bin").exists());
        assert!(t.join("mine.txt").exists());
        assert!(again.tor.list().await.is_empty());
        assert!(!again.state.join(format!("{id}.torrent")).exists());
    }

    #[test]
    fn priorities_fetch_high_first_and_low_last() {
        let wanted: HashSet<usize> = [0, 1, 2, 3].into();
        let prio: HashMap<usize, Priority> = [(0, Priority::High), (3, Priority::Low)].into();
        let none_done = |_: usize| false;
        assert_eq!(tiered(&wanted, &prio, &none_done), [0].into());
        // The High file is done: the Normal ones (1, 2) join, Low still waits.
        let first_done = |i: usize| i == 0;
        assert_eq!(tiered(&wanted, &prio, &first_done), [0, 1, 2].into());
        let all_but_low = |i: usize| i != 3;
        assert_eq!(tiered(&wanted, &prio, &all_but_low), [0, 1, 2, 3].into());
        // Everything done: the whole choice stays selected (for sharing).
        let all = |_: usize| true;
        assert_eq!(tiered(&wanted, &prio, &all), wanted);
        // No priorities: exactly the choice, so nothing changes for most people.
        assert_eq!(tiered(&wanted, &HashMap::new(), &none_done), wanted);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn priorities_and_skip_still_end_with_every_wanted_file_exact() {
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        let id = add(&s, &file, addr, &["a.bin", "b.bin", "c.bin"]).await;
        let listed = s.tor.files(&id).await.unwrap();
        let index = |name: &str| listed.iter().find(|f| f.path == name).map(|f| f.index);
        let (a, b, c) = (index("a.bin"), index("b.bin"), index("c.bin"));
        // Raced with the download on purpose: whatever was fetched already, the
        // end state must hold. (A finished torrent has no live files to change.)
        if let (Some(a), Some(b), Some(c)) = (a, b, c) {
            let _ = s.tor.set_priority(&id, c, Priority::High).await;
            let _ = s.tor.set_priority(&id, a, Priority::Low).await;
            let _ = s.tor.set_priority(&id, b, Priority::Skip).await;
            if let Ok(files) = s.tor.files(&id).await
                && !files.is_empty()
            {
                let p = |i| files.iter().find(|f| f.index == i).unwrap().priority;
                assert_eq!(
                    (p(a), p(b), p(c)),
                    (Priority::Low, Priority::Skip, Priority::High)
                );
                // Nothing left to want is refused.
                let _ = s.tor.set_priority(&id, a, Priority::Skip).await;
                let err = s
                    .tor
                    .set_priority(&id, c, Priority::Skip)
                    .await
                    .unwrap_err();
                assert_eq!(err.code, "nothing-selected");
                s.tor.set_priority(&id, a, Priority::Low).await.unwrap();
            }
        }
        let done = until(&s.tor, &id, "completed").await;
        assert_eq!(done.done, done.total);
        let t = s.downloads.join("T");
        for n in ["a.bin", "c.bin"] {
            assert_eq!(
                std::fs::read(t.join(n)).unwrap(),
                std::fs::read(seed.path().join("T").join(n)).unwrap(),
                "{n}"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_file_streams_in_order_while_it_downloads() {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        let id = add(&s, &file, addr, &["a.bin", "b.bin", "c.bin"]).await;
        let c = s
            .tor
            .files(&id)
            .await
            .unwrap()
            .into_iter()
            .find(|f| f.path == "c.bin")
            .map(|f| f.index);
        let want = std::fs::read(seed.path().join("T").join("c.bin")).unwrap();
        // It may already have finished (a local seeder is quick): then there is
        // nothing live to stream, which the app answers with "open the file".
        if let Some(c) = c
            && let Some((mut r, len, name)) = s.tor.open_stream(&id, c).await
        {
            assert_eq!((len, name.as_str()), (want.len() as u64, "c.bin"));
            let mut got = vec![];
            tokio::time::timeout(Duration::from_secs(30), r.read_to_end(&mut got))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(got, want);
            // Seeking (what a player does) reads from there on.
            r.seek(std::io::SeekFrom::Start(1000)).await.unwrap();
            let mut tail = vec![0u8; 100];
            r.read_exact(&mut tail).await.unwrap();
            assert_eq!(&tail[..], &want[1000..1100]);
        }
        until(&s.tor, &id, "completed").await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_paused_torrent_comes_back_paused_with_its_file_choice() {
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        let id = add(&s, &file, addr, &["a.bin", "b.bin", "c.bin"]).await;
        s.tor.pause(&id).await.unwrap();
        assert_eq!(s.tor.list().await[0].status, "paused");
        drop(s.tor);

        let again = reopen(
            s.store.clone(),
            s.state.clone(),
            s.downloads.clone(),
            tempfile::tempdir().unwrap(),
        );
        again.tor.restore().await;
        let v = again.tor.list().await;
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].status, "paused", "{v:?}");
        assert_eq!(
            again
                .tor
                .files(&id)
                .await
                .unwrap()
                .iter()
                .filter(|f| f.selected)
                .count(),
            3
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn used_up_allowances_pause_torrents_with_a_reason() {
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        // Slow, so the torrent is still downloading when the allowance runs out.
        s.limiter.apply(&fuselane_limits::LimitSettings {
            global: 32 * 1024,
            networks: vec![],
        });
        let id = add(&s, &file, addr, &["a.bin", "b.bin", "c.bin"]).await;
        until(&s.tor, &id, "downloading").await;
        s.limiter.set_blocked(["lo0".to_string()]);
        s.tor.tick().await;
        let v = s.tor.list().await.into_iter().find(|v| v.id == id).unwrap();
        assert!(
            v.error
                .as_deref()
                .is_some_and(|e| e.contains("data allowance")),
            "{v:?}"
        );
        // Lifted: the reason goes away and it finishes.
        s.limiter.set_blocked(Vec::new());
        s.limiter.apply(&fuselane_limits::LimitSettings::default());
        let done = until(&s.tor, &id, "completed").await;
        assert_eq!(done.error, None);
    }

    #[test]
    fn metered_networks_are_avoided_only_while_just_sharing() {
        let net = |name: &str, kind| Interface {
            name: name.into(),
            display_name: name.into(),
            index: 1,
            kind,
            addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        };
        let nets = [
            net("en0", fuselane_netif::Kind::Wifi),
            net("en7", fuselane_netif::Kind::Tether),
            net("pdp0", fuselane_netif::Kind::Cellular),
        ];
        assert_eq!(metered_to_avoid(&nets, true, false), vec!["en7", "pdp0"]);
        assert!(
            metered_to_avoid(&nets, true, true).is_empty(),
            "downloading: bond everything"
        );
        assert!(metered_to_avoid(&nets, false, false).is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sharing_settings_are_checked_and_remembered() {
        logs();
        let s = setup();
        assert_eq!(s.tor.seed_settings(), SeedSettings::default());
        assert!(!s.tor.seed_settings().enabled, "off by default");
        for (ratio, minutes, code) in [
            (0.0, 60, "bad-ratio"),
            (10.5, 60, "bad-ratio"),
            (f64::NAN, 60, "bad-ratio"),
            (1.0, 0, "bad-minutes"),
            (1.0, 10_081, "bad-minutes"),
        ] {
            let e = s
                .tor
                .set_seed_settings(SeedSettings {
                    enabled: true,
                    ratio,
                    minutes,
                })
                .await
                .unwrap_err();
            assert_eq!(e.code, code, "{ratio} {minutes}");
        }
        assert_eq!(
            s.tor.seed_settings(),
            SeedSettings::default(),
            "a refused value changes nothing"
        );
        let want = SeedSettings {
            enabled: true,
            ratio: 2.5,
            minutes: 90,
        };
        assert_eq!(s.tor.set_seed_settings(want.clone()).await.unwrap(), want);
        let again = reopen(
            s.store.clone(),
            s.state.clone(),
            s.downloads.clone(),
            tempfile::tempdir().unwrap(),
        );
        assert_eq!(again.tor.seed_settings(), want);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn with_sharing_on_a_finished_torrent_seeds_until_a_limit_or_stop() {
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        s.tor
            .set_seed_settings(SeedSettings {
                enabled: true,
                ratio: 1.0,
                minutes: 30,
            })
            .await
            .unwrap();
        let id = add(&s, &file, addr, &["a.bin", "b.bin", "c.bin"]).await;
        let v = until(&s.tor, &id, "seeding").await;
        assert_eq!(v.done, v.total);
        assert!(
            s.tor.list().await[0].status == "seeding",
            "not released while sharing"
        );
        assert_eq!(s.tor.pause("x").await.unwrap_err().code, "not-found");

        // The time limit passes: released, files kept.
        {
            let mut entries = s.tor.entries.lock().await;
            entries[0].finished_at = Some(Instant::now() - std::time::Duration::from_secs(31 * 60));
        }
        let done = until(&s.tor, &id, "completed").await;
        assert!(done.networks.iter().all(|n| n.peers == 0 && n.rate == 0));
        assert!(s.downloads.join("T").join("a.bin").exists());
        assert_eq!(
            s.tor.stop_sharing(&id).await.unwrap_err().code,
            "not-sharing"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn stop_sharing_releases_at_once_and_a_finished_torrent_cant_be_added_twice() {
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        s.tor
            .set_seed_settings(SeedSettings {
                enabled: true,
                ratio: 5.0,
                minutes: 600,
            })
            .await
            .unwrap();
        let id = add(&s, &file, addr, &["a.bin"]).await;
        until(&s.tor, &id, "seeding").await;
        s.tor.stop_sharing(&id).await.unwrap();
        assert_eq!(s.tor.list().await[0].status, "completed");

        // The same torrent again, after it was released: refused, never a second row.
        let listing = s.tor.inspect_file(&file, None).await.unwrap();
        let again = s.tor.add(&listing.token, vec![0]).await.unwrap_err();
        assert_eq!(again.code, "already-added");
        assert_eq!(s.tor.list().await.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn turning_sharing_off_releases_seeding_torrents_at_once() {
        logs();
        let seed = tempfile::tempdir().unwrap();
        let (_seeder, addr, file) = seeder(seed.path()).await;
        let s = setup();
        s.tor
            .set_seed_settings(SeedSettings {
                enabled: true,
                ratio: 5.0,
                minutes: 600,
            })
            .await
            .unwrap();
        let id = add(&s, &file, addr, &["a.bin", "c.bin"]).await;
        until(&s.tor, &id, "seeding").await;
        s.tor
            .set_seed_settings(SeedSettings {
                enabled: false,
                ratio: 5.0,
                minutes: 600,
            })
            .await
            .unwrap();
        let v = s.tor.list().await;
        assert_eq!(v[0].status, "completed", "{v:?}");
        assert!(
            !s.downloads.join("T").join("b.bin").exists(),
            "unchosen file cleaned up on release"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn torrents_follow_networks_coming_and_going() {
        logs();
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&dir.path().join("fuselane.db")).unwrap());
        let net = |name: &str| Interface {
            name: name.into(),
            display_name: name.into(),
            index: 1,
            kind: fuselane_netif::Kind::Ethernet,
            addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        };
        let list = Arc::new(Mutex::new(vec![net("en0")]));
        let source: NetSource = {
            let list = list.clone();
            Arc::new(move || Ok(list.lock().unwrap().clone()))
        };
        let tor = Torrents::new(
            store,
            dir.path().join("state"),
            dir.path().to_path_buf(),
            source,
            false,
            None,
            Arc::new(|_| {}),
        );
        let engine = tor.engine().await.unwrap();
        let names = || {
            engine
                .interfaces()
                .into_iter()
                .map(|i| i.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(), vec!["en0"]);
        *list.lock().unwrap() = vec![net("en0"), net("en7")];
        tor.tick().await;
        assert_eq!(names(), vec!["en0", "en7"], "a tethered phone joins");
        // Not re-read again within 10 s.
        *list.lock().unwrap() = vec![net("en7")];
        tor.tick().await;
        assert_eq!(names(), vec!["en0", "en7"]);
        *tor.nets_checked.lock().unwrap() = None;
        tor.tick().await;
        assert_eq!(
            names(),
            vec!["en7"],
            "an unplugged network stops taking peers"
        );
        // An empty listing (a blip) keeps what was there.
        list.lock().unwrap().clear();
        *tor.nets_checked.lock().unwrap() = None;
        tor.tick().await;
        assert_eq!(names(), vec!["en7"]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bad_torrent_files_get_specific_errors() {
        logs();
        let s = setup();
        let dir = tempfile::tempdir().unwrap();
        let missing = s
            .tor
            .inspect_file(&dir.path().join("none.torrent"), None)
            .await
            .unwrap_err();
        assert_eq!(missing.code, "torrent-unreadable");
        assert_eq!(
            s.tor.inspect_file(dir.path(), None).await.unwrap_err().code,
            "torrent-unreadable"
        );
        let big = dir.path().join("big.torrent");
        std::fs::File::create(&big)
            .unwrap()
            .set_len(9 * 1024 * 1024)
            .unwrap();
        let e = s.tor.inspect_file(&big, None).await.unwrap_err();
        assert_eq!(e.code, "torrent-too-big");
        assert!(e.message.contains("8 MiB"), "{}", e.message);
        let e = s
            .tor
            .inspect_bytes(vec![0; 8 * 1024 * 1024 + 1], None)
            .await
            .unwrap_err();
        assert_eq!(e.code, "torrent-too-big");
        assert_eq!(
            s.tor
                .inspect_bytes(Vec::new(), None)
                .await
                .unwrap_err()
                .code,
            "invalid-torrent"
        );
        let junk = dir.path().join("junk.torrent");
        std::fs::write(&junk, b"<html>not a torrent</html>").unwrap();
        assert_eq!(
            s.tor.inspect_file(&junk, None).await.unwrap_err().code,
            "invalid-torrent"
        );
        assert_eq!(
            s.tor
                .inspect_magnet("https://example.com", None)
                .await
                .unwrap_err()
                .code,
            "not-a-magnet"
        );
        assert_eq!(
            s.tor.add(&"0".repeat(40), vec![0]).await.unwrap_err().code,
            "expired"
        );
        assert!(s.tor.list().await.is_empty());
    }
}
