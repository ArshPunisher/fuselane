//! Torrents in the desktop app (ADR 0006). Separate from the HTTP queue: a
//! torrent's life is different (peers, file choice, recheck on restart). The
//! engine starts on first use, so people who never add a torrent never run DHT.
//!
//! Finished torrents stop and release their files by default (seeding is opt-in,
//! P5 5.7): chosen files stay, edge-piece bytes in unchosen files are deleted
//! (L-69), and the row stays as "completed".

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TorrentNetView {
    pub name: String,
    pub peers: usize,
    pub received: u64,
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
}

struct Entry {
    torrent: Option<Torrent>,
    base: PathBuf,
    added_at: i64,
    last: TorrentView,
    prev: (u64, Instant),
}

pub type NetSource = Arc<dyn Fn() -> Result<Vec<Interface>, String> + Send + Sync>;

pub struct Torrents {
    store: Arc<Store>,
    /// Holds `<id>.torrent` files and the DHT node list.
    state_dir: PathBuf,
    default_dir: PathBuf,
    networks: NetSource,
    dht: bool,
    engine: OnceCell<Arc<TorrentEngine>>,
    entries: tokio::sync::Mutex<Vec<Entry>>,
    pending: Mutex<Vec<Listing>>,
    emit: Emit,
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
        emit: Emit,
    ) -> Arc<Torrents> {
        Arc::new(Torrents {
            store,
            state_dir,
            default_dir,
            networks,
            dht,
            engine: OnceCell::new(),
            entries: tokio::sync::Mutex::new(Vec::new()),
            pending: Mutex::new(Vec::new()),
            emit,
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
        let engine = self.engine().await?;
        let id = listing.info_hash.clone();
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
            base,
            added_at,
            last,
            prev: (0, Instant::now()),
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
        let sel = t.selected();
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
        engine
            .select(&t, files.into_iter().collect())
            .await
            .map_err(ui)?;
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
            if delete_files {
                engine.remove(t, true).await
            } else {
                engine.release(t).await
            }
            .map_err(ui)?;
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
            engine.delete_files(&listing);
        }
        if valid_id(id) {
            let _ = std::fs::remove_file(self.state_dir.join(format!("{id}.torrent")));
        }
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
        let mut entries = self.entries.lock().await;
        let mut active = false;
        let mut released = false;
        for e in entries.iter_mut() {
            let Some(t) = e.torrent.clone() else { continue };
            let p = t.progress();
            let elapsed = e.prev.1.elapsed().as_secs_f64().max(0.001);
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let rate = (p.done.saturating_sub(e.prev.0) as f64 / elapsed) as u64;
            e.prev = (p.done, Instant::now());
            e.last = view_of(&t, e.added_at, rate);
            active |= matches!(p.phase, Phase::Checking | Phase::Downloading);
            if release_finished
                && p.phase == Phase::Seeding
                && p.done == p.total
                && let Some(engine) = &engine
            {
                let mut last = e.last.clone();
                let _ = engine.release(t).await;
                last.rate = 0;
                last.status = "completed".into();
                e.last = last;
                e.torrent = None;
                released = true;
            }
        }
        drop(entries);
        if released {
            self.save().await;
        }
        active || released
    }

    async fn publish(&self) {
        let torrents = self.list().await;
        (self.emit)(UiEvent::Torrents { torrents });
    }

    /// Called every second: refreshes, releases finished torrents, and tells the
    /// window when something moved.
    pub async fn tick(&self) {
        if self.engine.get().is_none() {
            return;
        }
        if self.refresh(true).await {
            self.publish().await;
        }
    }

    async fn save(&self) {
        let entries = self.entries.lock().await;
        let saved: Vec<Saved> = entries
            .iter()
            .map(|e| Saved {
                id: e.last.id.clone(),
                base: e.base.clone(),
                selected: e
                    .torrent
                    .as_ref()
                    .map(|t| t.selected().into_iter().collect())
                    .unwrap_or_default(),
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
            if !valid_id(&s.id) {
                continue;
            }
            if let Some(done) = s.completed {
                self.entries.lock().await.push(Entry {
                    torrent: None,
                    base: s.base,
                    added_at: s.added_at,
                    last: done,
                    prev: (0, Instant::now()),
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
            };
            match engine
                .add(Source::File(bytes), Some(s.base.clone()), vec![], opts)
                .await
            {
                Ok(t) => {
                    let last = view_of(&t, s.added_at, 0);
                    self.entries.lock().await.push(Entry {
                        torrent: Some(t),
                        base: s.base,
                        added_at: s.added_at,
                        last,
                        prev: (0, Instant::now()),
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
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::Duration;

    use librqbit::{
        AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session,
        SessionOptions,
    };

    fn ts_fields(src: &str, name: &str) -> Vec<String> {
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

    fn json_fields(v: &impl Serialize) -> Vec<String> {
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
            credited: 0,
        };
        let file = TorrentFileView {
            index: 0,
            path: "a".into(),
            size: 1,
            selected: true,
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
        for (name, got) in [
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
    fn only_real_info_hashes_name_files() {
        assert!(valid_id(&"a".repeat(40)));
        assert!(!valid_id("../../etc/passwd"));
        assert!(!valid_id(&"A".repeat(40)));
        assert!(!valid_id(&"a".repeat(41)));
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
        let events: Arc<Mutex<Vec<UiEvent>>> = Arc::default();
        let sink = events.clone();
        let tor = Torrents::new(
            store.clone(),
            state.clone(),
            downloads.clone(),
            loopback(),
            false,
            Arc::new(move |e| sink.lock().unwrap().push(e)),
        );
        Setup {
            tor,
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
        .unwrap_or_else(|_| panic!("never reached {status}"))
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
    async fn a_torrent_downloads_the_chosen_files_then_releases_and_survives_a_restart() {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn a_paused_torrent_comes_back_paused_with_its_file_choice() {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn bad_torrent_files_get_specific_errors() {
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
