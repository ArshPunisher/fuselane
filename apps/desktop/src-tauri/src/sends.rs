//! Fuse Send in the app (STEPS 6.8, FUSE-SEND.md): share a file straight from this
//! computer with a link, and receive from someone else's link. No server: the
//! sender's Fuselane seeds an encrypted torrent and the receiver's finds it.
//!
//! Its own torrent engine, separate from ordinary torrents, because a sender must
//! be reachable: it listens, asks the router to forward the port (UPnP) and looks
//! for receivers on the same network.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use fuselane_core::Store;
use fuselane_engine_torrent::{EngineOptions, Torrent, TorrentEngine};
use fuselane_send::link::Link;
use fuselane_send::share::{self, Prepared, Receiving};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, OnceCell};

use crate::service::{Emit, UiError, UiEvent};
use crate::torrents::NetSource;

/// One file being shared, as the window shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareView {
    pub id: String,
    pub name: String,
    pub size: u64,
    /// The link to send, once the file is ready.
    pub link: Option<String>,
    /// preparing, sharing, sent (stopped after one full copy), changed, failed
    pub state: &'static str,
    /// How far preparing has got (0 to 1).
    pub prepared: f64,
    pub sent: u64,
    pub peers: usize,
    /// Stop sharing once a full copy has been sent.
    pub once: bool,
    pub error: Option<String>,
}

/// One file being received.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiveView {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub done: u64,
    /// finding, receiving, checking, done, failed
    pub state: &'static str,
    pub path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Saved {
    path: PathBuf,
    name: String,
    size: u64,
    /// The link's token: info-hash and key. Kept in Fuselane's own data folder,
    /// next to everything else it remembers, so the same link keeps working.
    token: String,
    #[serde(default)]
    once: bool,
}

struct ShareEntry {
    view: ShareView,
    progress: Arc<AtomicU64>,
    torrent: Option<Torrent>,
}

struct ReceiveEntry {
    view: ReceiveView,
    torrent: Option<Torrent>,
}

pub struct Sends {
    store: Arc<Store>,
    dir: PathBuf,
    default_dir: PathBuf,
    networks: NetSource,
    limiter: Option<Arc<fuselane_limits::Limiter>>,
    /// DHT, UPnP and local discovery on (the app); off in tests, which stay offline.
    reachable: bool,
    engine: OnceCell<Arc<TorrentEngine>>,
    shares: Mutex<Vec<ShareEntry>>,
    receives: Mutex<Vec<ReceiveEntry>>,
    emit: Emit,
    busy: AtomicBool,
}

impl std::fmt::Debug for Sends {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sends")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

fn err(code: &'static str, message: impl Into<String>, hint: Option<&str>) -> UiError {
    UiError {
        code,
        message: message.into(),
        hint: hint.map(String::from),
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

const PREP_SCALE: f64 = 1_000_000.0;

impl Sends {
    pub fn new(
        store: Arc<Store>,
        dir: PathBuf,
        default_dir: PathBuf,
        networks: NetSource,
        limiter: Option<Arc<fuselane_limits::Limiter>>,
        reachable: bool,
        emit: Emit,
    ) -> Arc<Sends> {
        Arc::new(Sends {
            store,
            dir,
            default_dir,
            networks,
            limiter,
            reachable,
            engine: OnceCell::new(),
            shares: Mutex::new(Vec::new()),
            receives: Mutex::new(Vec::new()),
            emit,
            busy: AtomicBool::new(false),
        })
    }

    async fn engine(&self) -> Result<Arc<TorrentEngine>, UiError> {
        self.engine
            .get_or_try_init(|| async {
                let networks = (self.networks)().map_err(|m| err("no-networks", m, None))?;
                std::fs::create_dir_all(&self.dir).map_err(|e| {
                    err(
                        "send-engine",
                        format!("Couldn't create Fuselane's send folder: {e}"),
                        None,
                    )
                })?;
                TorrentEngine::start(EngineOptions {
                    download_dir: self.default_dir.clone(),
                    networks,
                    dht: self.reachable,
                    // Any free port on every address: receivers connect to us.
                    listen: Some(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))),
                    state_dir: Some(self.dir.join("dht")),
                    limiter: self.limiter.clone(),
                    upnp: self.reachable,
                    local_discovery: self.reachable,
                })
                .await
                .map(Arc::new)
                .map_err(|e| err("send-engine", e.to_string(), None))
            })
            .await
            .cloned()
    }

    /// Whether anything is preparing or receiving (keeps the computer awake).
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    pub async fn views(&self) -> (Vec<ShareView>, Vec<ReceiveView>) {
        let shares = self
            .shares
            .lock()
            .await
            .iter()
            .map(|s| s.view.clone())
            .collect();
        let receives = self
            .receives
            .lock()
            .await
            .iter()
            .map(|r| r.view.clone())
            .collect();
        (shares, receives)
    }

    async fn publish(&self) {
        let (shares, receives) = self.views().await;
        (self.emit)(UiEvent::Sends { shares, receives });
    }

    fn saved(&self) -> Vec<Saved> {
        self.store
            .setting("fuse_send_shares")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    }

    fn write_saved(&self, list: &[Saved]) {
        if let Ok(json) = serde_json::to_string(list) {
            let _ = self.store.set_setting("fuse_send_shares", &json);
        }
    }

    fn files_for(&self, id: &str) -> (PathBuf, PathBuf) {
        (
            self.dir.join(format!("{id}.head")),
            self.dir.join(format!("{id}.torrent")),
        )
    }

    fn remember(&self, path: &Path, p: &Prepared) -> std::io::Result<()> {
        let id = hex(&p.link.info_hash);
        let (head, torrent) = self.files_for(&id);
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(head, &p.head)?;
        std::fs::write(torrent, &p.torrent)?;
        let mut list = self.saved();
        list.retain(|s| !same_share(s, &id));
        list.push(Saved {
            path: path.to_path_buf(),
            name: p.name.clone(),
            size: p.size,
            token: p.link.token(),
            once: false,
        });
        self.write_saved(&list);
        Ok(())
    }

    fn forget(&self, id: &str) {
        let mut list = self.saved();
        list.retain(|s| !same_share(s, id));
        self.write_saved(&list);
        let (head, torrent) = self.files_for(id);
        let _ = std::fs::remove_file(head);
        let _ = std::fs::remove_file(torrent);
    }

    /// Starts sharing a file: prepares it (two reads, with progress), then seeds.
    pub async fn send(self: &Arc<Self>, path: &str) -> Result<String, UiError> {
        let path = PathBuf::from(path.trim());
        let meta = std::fs::metadata(&path).map_err(|e| {
            err(
                "send-missing",
                format!("Fuselane couldn't open \"{}\" ({e}).", path.display()),
                Some("Check the file is still there and pick it again."),
            )
        })?;
        if meta.is_dir() {
            return Err(err(
                "send-folder",
                "Only single files can be sent for now.",
                Some("Zip the folder and send the zip."),
            ));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // A temporary id until the info-hash is known.
        let temp = next_temp();
        let progress = Arc::new(AtomicU64::new(0));
        self.shares.lock().await.push(ShareEntry {
            view: ShareView {
                id: temp.clone(),
                name: name.clone(),
                size: meta.len(),
                link: None,
                state: "preparing",
                prepared: 0.0,
                sent: 0,
                peers: 0,
                once: false,
                error: None,
            },
            progress: progress.clone(),
            torrent: None,
        });
        self.busy.store(true, Ordering::Relaxed);
        self.publish().await;
        let me = self.clone();
        let id = temp.clone();
        tokio::spawn(async move {
            let p2 = path.clone();
            let prog = progress.clone();
            let prepared = tokio::task::spawn_blocking(move || {
                share::prepare(&p2, |done, total| {
                    let f = if total == 0 {
                        1.0
                    } else {
                        done as f64 / total as f64
                    };
                    prog.store((f * PREP_SCALE) as u64, Ordering::Relaxed);
                })
            })
            .await;
            let result: Result<(Prepared, Torrent), String> = match prepared {
                Err(_) => Err("Preparing stopped unexpectedly. Try again.".into()),
                Ok(Err(e)) => Err(e.to_string()),
                Ok(Ok(p)) => match me.engine().await {
                    Err(e) => Err(e.message),
                    Ok(engine) => match share::seed(&engine, &p, &path).await {
                        Err(e) => Err(e.to_string()),
                        Ok(t) => {
                            if let Err(e) = me.remember(&path, &p) {
                                eprintln!("fuselane: couldn't save the share: {e}");
                            }
                            Ok((p, t))
                        }
                    },
                },
            };
            {
                let mut shares = me.shares.lock().await;
                if let Some(s) = shares.iter_mut().find(|s| s.view.id == id) {
                    match result {
                        Ok((p, t)) => {
                            s.view.id = hex(&p.link.info_hash);
                            s.view.link = Some(p.link.url());
                            s.view.state = "sharing";
                            s.view.prepared = 1.0;
                            s.torrent = Some(t);
                        }
                        Err(m) => {
                            s.view.state = "failed";
                            s.view.error = Some(m);
                        }
                    }
                }
            }
            me.refresh_busy().await;
            me.publish().await;
        });
        Ok(temp)
    }

    /// Whether to stop sharing once a full copy has been sent (kept across restarts).
    pub async fn set_once(&self, id: &str, on: bool) -> Result<(), UiError> {
        {
            let mut shares = self.shares.lock().await;
            let s = shares
                .iter_mut()
                .find(|s| s.view.id == id)
                .ok_or_else(|| err("not-found", "That share is no longer in the list.", None))?;
            s.view.once = on;
        }
        let mut list = self.saved();
        for s in list.iter_mut().filter(|s| same_share(s, id)) {
            s.once = on;
        }
        self.write_saved(&list);
        self.tick().await;
        self.publish().await;
        Ok(())
    }

    /// Stops sharing; the file itself is never touched.
    pub async fn stop(&self, id: &str) -> Result<(), UiError> {
        let entry = {
            let mut shares = self.shares.lock().await;
            let i = shares
                .iter()
                .position(|s| s.view.id == id)
                .ok_or_else(|| err("not-found", "That share is no longer in the list.", None))?;
            shares.remove(i)
        };
        if let (Some(t), Some(engine)) = (entry.torrent, self.engine.get()) {
            let _ = engine.remove(t, false).await;
        }
        self.forget(id);
        self.publish().await;
        Ok(())
    }

    /// Opens a link someone sent and starts receiving into `dir` (or Downloads).
    pub async fn receive(
        self: &Arc<Self>,
        text: &str,
        dir: Option<&str>,
    ) -> Result<String, UiError> {
        self.receive_from(text, dir, vec![], share::LOOKUP).await
    }

    /// `receive`, trying `peers` first and looking for `lookup` (tests point it at a sender).
    async fn receive_from(
        self: &Arc<Self>,
        text: &str,
        dir: Option<&str>,
        peers: Vec<SocketAddr>,
        lookup: std::time::Duration,
    ) -> Result<String, UiError> {
        let link = Link::parse(text).map_err(|e| {
            err(
                "bad-send-link",
                e.to_string(),
                Some("Fuse Send links start with https://arshpunisher.github.io/fuselane/s#."),
            )
        })?;
        let dir = match dir.map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => PathBuf::from(d),
            None => self.default_dir.clone(),
        };
        if !dir.is_dir() {
            return Err(err(
                "folder-missing",
                format!("The folder \"{}\" doesn't exist.", dir.display()),
                Some("Pick another folder to save into."),
            ));
        }
        let id = hex(&link.info_hash);
        {
            let mut receives = self.receives.lock().await;
            if receives
                .iter()
                .any(|r| r.view.id == id && r.view.state != "failed")
            {
                return Err(err(
                    "duplicate",
                    "You're already receiving this link.",
                    None,
                ));
            }
            receives.retain(|r| r.view.id != id);
            receives.push(ReceiveEntry {
                view: ReceiveView {
                    id: id.clone(),
                    name: "Looking for the sender…".into(),
                    size: 0,
                    done: 0,
                    state: "finding",
                    path: None,
                    error: None,
                },
                torrent: None,
            });
        }
        self.busy.store(true, Ordering::Relaxed);
        self.publish().await;
        let me = self.clone();
        let id2 = id.clone();
        tokio::spawn(async move {
            let started = match me.engine().await {
                Err(e) => Err(e.message),
                Ok(engine) => share::receive(&engine, &link, &dir, peers, lookup)
                    .await
                    .map_err(|e| e.to_string()),
            };
            match started {
                Err(m) => me.fail_receive(&id2, m).await,
                Ok(r) => me.watch_receive(&id2, r).await,
            }
            me.refresh_busy().await;
            me.publish().await;
        });
        Ok(id)
    }

    async fn fail_receive(&self, id: &str, message: String) {
        if let Some(r) = self
            .receives
            .lock()
            .await
            .iter_mut()
            .find(|r| r.view.id == id)
        {
            r.view.state = "failed";
            r.view.error = Some(message);
        }
    }

    async fn watch_receive(&self, id: &str, r: Receiving) {
        let torrent = r.torrent.clone();
        if let Some(e) = self
            .receives
            .lock()
            .await
            .iter_mut()
            .find(|e| e.view.id == id)
        {
            e.view.state = "receiving";
            e.view.size = r.size;
            e.view.name = "Receiving…".into();
            e.torrent = Some(torrent.clone());
        }
        self.publish().await;
        if let Err(e) = torrent.finished().await {
            self.fail_receive(id, e.to_string()).await;
            return;
        }
        if let Some(e) = self
            .receives
            .lock()
            .await
            .iter_mut()
            .find(|e| e.view.id == id)
        {
            e.view.state = "checking";
            e.view.done = e.view.size;
        }
        self.publish().await;
        if let Some(engine) = self.engine.get() {
            let _ = engine.remove(torrent, false).await;
        }
        match tokio::task::spawn_blocking(move || r.finish()).await {
            Ok(Ok(path)) => {
                if let Some(e) = self
                    .receives
                    .lock()
                    .await
                    .iter_mut()
                    .find(|e| e.view.id == id)
                {
                    e.view.state = "done";
                    e.view.name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    e.view.path = Some(path.display().to_string());
                    e.torrent = None;
                }
            }
            Ok(Err(m)) => self.fail_receive(id, m.to_string()).await,
            Err(_) => {
                self.fail_receive(id, "Checking stopped unexpectedly.".into())
                    .await
            }
        }
    }

    /// Where a finished receive was saved.
    pub async fn received_path(&self, id: &str) -> Result<PathBuf, UiError> {
        self.receives
            .lock()
            .await
            .iter()
            .find(|r| r.view.id == id)
            .and_then(|r| r.view.path.clone())
            .map(PathBuf::from)
            .ok_or_else(|| err("not-found", "That file is no longer in the list.", None))
    }

    /// Removes a finished or failed receive from the list (the file stays).
    pub async fn dismiss(&self, id: &str) {
        self.receives.lock().await.retain(|r| {
            r.view.id != id || matches!(r.view.state, "finding" | "receiving" | "checking")
        });
        self.publish().await;
    }

    async fn refresh_busy(&self) {
        let busy = self
            .shares
            .lock()
            .await
            .iter()
            .any(|s| s.view.state == "preparing")
            || self
                .receives
                .lock()
                .await
                .iter()
                .any(|r| matches!(r.view.state, "finding" | "receiving" | "checking"));
        self.busy.store(busy, Ordering::Relaxed);
    }

    /// Called every second: progress for preparing, sharing and receiving.
    pub async fn tick(&self) {
        let mut changed = false;
        let mut done = Vec::new();
        for s in self.shares.lock().await.iter_mut() {
            let next = match (&s.torrent, s.view.state) {
                (_, "preparing") => {
                    let f = s.progress.load(Ordering::Relaxed) as f64 / PREP_SCALE;
                    (f, s.view.sent, s.view.peers)
                }
                (Some(t), "sharing") => {
                    let p = t.progress();
                    let peers = t.networks().iter().map(|n| n.peers).sum();
                    (1.0, p.uploaded, peers)
                }
                _ => continue,
            };
            if (next.0 - s.view.prepared).abs() > 0.001
                || next.1 != s.view.sent
                || next.2 != s.view.peers
            {
                s.view.prepared = next.0;
                s.view.sent = next.1;
                s.view.peers = next.2;
                changed = true;
            }
            // The torrent carries the 1 KiB sealed header too.
            let full = s.view.size + fuselane_send::crypt::HEADER_BLOCK as u64;
            if s.view.once && s.view.state == "sharing" && s.view.sent >= full {
                s.view.state = "sent";
                s.view.link = None;
                s.view.peers = 0;
                if let Some(t) = s.torrent.take() {
                    done.push((s.view.id.clone(), t));
                }
                changed = true;
            }
        }
        for (id, t) in done {
            if let Some(engine) = self.engine.get() {
                let _ = engine.remove(t, false).await;
            }
            self.forget(&id);
        }
        for r in self.receives.lock().await.iter_mut() {
            if let (Some(t), "receiving") = (&r.torrent, r.view.state) {
                let done = t.progress().done.min(r.view.size);
                if done != r.view.done {
                    r.view.done = done;
                    changed = true;
                }
            }
        }
        if changed {
            self.publish().await;
        }
    }

    /// Shares from before a restart start again from their saved header and torrent.
    /// A file that changed or vanished is shown as such and no longer shared.
    pub async fn restore(self: &Arc<Self>) {
        let list = self.saved();
        if list.is_empty() {
            return;
        }
        let Ok(engine) = self.engine().await else {
            return;
        };
        for s in list {
            let Ok(link) = Link::parse(&s.token) else {
                continue;
            };
            let id = hex(&link.info_hash);
            let (head, torrent) = self.files_for(&id);
            let restored = match (
                std::fs::read(&head),
                std::fs::read(&torrent),
                std::fs::metadata(&s.path),
            ) {
                (Ok(head), Ok(torrent), Ok(m)) if m.len() == s.size => {
                    let p = Prepared {
                        link: link.clone(),
                        torrent,
                        head,
                        size: s.size,
                        name: s.name.clone(),
                    };
                    share::seed(&engine, &p, &s.path).await.ok()
                }
                _ => None,
            };
            let mut view = ShareView {
                id: id.clone(),
                name: s.name.clone(),
                size: s.size,
                link: Some(link.url()),
                state: "sharing",
                prepared: 1.0,
                sent: 0,
                peers: 0,
                once: s.once,
                error: None,
            };
            if restored.is_none() {
                view.state = "changed";
                view.link = None;
                view.error = Some(format!(
                    "\"{}\" was moved, deleted or changed after it was shared. Share it again for a new link.",
                    s.name
                ));
                self.forget(&id);
            }
            self.shares.lock().await.push(ShareEntry {
                view,
                progress: Arc::new(AtomicU64::new(0)),
                torrent: restored,
            });
        }
        self.publish().await;
    }
}

fn same_share(s: &Saved, id: &str) -> bool {
    Link::parse(&s.token).is_ok_and(|l| hex(&l.info_hash) == id)
}

/// Temporary ids for shares still preparing (their info-hash isn't known yet).
fn next_temp() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    format!("prep-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::time::Duration;

    use fuselane_netif::Interface;

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

    struct Side {
        sends: Arc<Sends>,
        store: Arc<Store>,
        root: PathBuf,
    }

    fn side(root: &Path) -> Side {
        std::fs::create_dir_all(root.join("dl")).unwrap();
        let store = Arc::new(Store::open(&root.join("fuselane.db")).unwrap());
        Side {
            sends: reopen(&store, root),
            store,
            root: root.to_path_buf(),
        }
    }

    fn reopen(store: &Arc<Store>, root: &Path) -> Arc<Sends> {
        Sends::new(
            store.clone(),
            root.join("shares"),
            root.join("dl"),
            loopback(),
            None,
            false,
            Arc::new(|_| {}),
        )
    }

    async fn until<T>(what: &str, f: impl AsyncFnMut() -> Option<T>) -> T {
        until_for(what, 600, f).await
    }

    async fn until_for<T>(what: &str, ticks: u32, mut f: impl AsyncFnMut() -> Option<T>) -> T {
        for _ in 0..ticks {
            if let Some(v) = f().await {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("timed out waiting for {what}");
    }

    async fn shared(s: &Sends) -> ShareView {
        until("the share", async || {
            let (shares, _) = s.views().await;
            shares.into_iter().find(|v| v.state != "preparing")
        })
        .await
    }

    fn payload(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 31 % 251) as u8).collect()
    }

    #[test]
    fn the_window_types_match_what_the_backend_sends() {
        use crate::torrents::tests::{json_fields, ts_fields};
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/types.ts"))
                .unwrap();
        let share = ShareView {
            id: String::new(),
            name: String::new(),
            size: 0,
            link: None,
            state: "sharing",
            prepared: 1.0,
            sent: 0,
            peers: 0,
            once: false,
            error: None,
        };
        let receive = ReceiveView {
            id: String::new(),
            name: String::new(),
            size: 0,
            done: 0,
            state: "done",
            path: None,
            error: None,
        };
        assert_eq!(json_fields(&share), ts_fields(&src, "ShareView"));
        assert_eq!(json_fields(&receive), ts_fields(&src, "ReceiveView"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_file_sent_from_one_fuselane_arrives_whole_in_another() {
        let tmp = tempfile::tempdir().unwrap();
        let a = side(&tmp.path().join("a"));
        let b = side(&tmp.path().join("b"));
        let file = a.root.join("holiday.mov");
        let data = payload(3 * 1024 * 1024 + 17);
        std::fs::write(&file, &data).unwrap();

        a.sends.send(file.to_str().unwrap()).await.unwrap();
        let share = shared(&a.sends).await;
        assert_eq!(share.state, "sharing", "{:?}", share.error);
        assert_eq!(share.name, "holiday.mov");
        // Stop once a full copy is out (kept across restarts).
        a.sends.set_once(&share.id, true).await.unwrap();
        let link = share.link.clone().unwrap();
        assert!(link.starts_with("https://arshpunisher.github.io/fuselane/s#"));

        let port = a.sends.engine.get().unwrap().listen_addr().unwrap().port();
        let peer = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        b.sends
            .receive_from(&link, None, vec![peer], Duration::from_secs(20))
            .await
            .unwrap();
        let done = until("the file to arrive", async || {
            b.sends.tick().await;
            let (_, receives) = b.sends.views().await;
            let r = receives.into_iter().next()?;
            assert_ne!(r.state, "failed", "{:?}", r.error);
            (r.state == "done").then_some(r)
        })
        .await;
        assert_eq!(done.name, "holiday.mov");
        assert_eq!(std::fs::read(done.path.unwrap()).unwrap(), data);
        assert!(!b.sends.busy());
        // No hidden leftovers next to the file.
        let names: Vec<String> = std::fs::read_dir(b.root.join("dl"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["holiday.mov"]);
        // The sender stopped by itself after the full copy.
        let stopped = until("the share to stop", async || {
            a.sends.tick().await;
            let (shares, _) = a.sends.views().await;
            shares.into_iter().find(|s| s.state == "sent")
        })
        .await;
        assert!(stopped.link.is_none());
        let again = reopen(&a.store, &a.root);
        again.restore().await;
        assert!(
            again.views().await.0.is_empty(),
            "a sent share isn't restored"
        );
    }

    /// The app's real setup: DHT, UPnP and local discovery on the real networks, no
    /// address given. Needs the internet or a LAN, so it's run by hand:
    /// `cargo nextest run -p fuselane-desktop --run-ignored only found_without`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "uses the real network"]
    async fn a_share_is_found_without_an_address() {
        // RUST_LOG=librqbit=debug,librqbit_lsd=trace shows how the peers met.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();
        let tmp = tempfile::tempdir().unwrap();
        let real = |root: &Path| {
            std::fs::create_dir_all(root.join("dl")).unwrap();
            let store = Arc::new(Store::open(&root.join("fuselane.db")).unwrap());
            Sends::new(
                store,
                root.join("shares"),
                root.join("dl"),
                Arc::new(|| fuselane_core::runner::pick_networks(&[])),
                None,
                true,
                Arc::new(|_| {}),
            )
        };
        let (a_root, b_root) = (tmp.path().join("a"), tmp.path().join("b"));
        let (a, b) = (real(&a_root), real(&b_root));
        let file = a_root.join("found.bin");
        let data = payload(2 * 1024 * 1024 + 3);
        std::fs::write(&file, &data).unwrap();
        a.send(file.to_str().unwrap()).await.unwrap();
        let link = shared(&a).await.link.unwrap();
        let start = std::time::Instant::now();
        b.receive(&link, None).await.unwrap();
        let done = until_for("the file to arrive", 3000, async || {
            b.tick().await;
            let (_, r) = b.views().await;
            let r = r.into_iter().next()?;
            assert_ne!(r.state, "failed", "{:?}", r.error);
            (r.state == "done").then_some(r)
        })
        .await;
        eprintln!("found and received in {:?}", start.elapsed());
        assert_eq!(std::fs::read(done.path.unwrap()).unwrap(), data);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn shares_survive_a_restart_until_the_file_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let a = side(tmp.path());
        let file = a.root.join("notes.txt");
        std::fs::write(&file, payload(40_000)).unwrap();
        a.sends.send(file.to_str().unwrap()).await.unwrap();
        let first = shared(&a.sends).await;
        assert_eq!(first.state, "sharing");

        // Same file after a restart: the same link keeps working.
        let again = reopen(&a.store, &a.root);
        again.restore().await;
        let (shares, _) = again.views().await;
        assert_eq!(shares.len(), 1);
        assert_eq!(shares[0].state, "sharing");
        assert_eq!(shares[0].link, first.link);

        // The file changed: shown as such, not shared, and forgotten next time.
        std::fs::write(&file, payload(1_000)).unwrap();
        let later = reopen(&a.store, &a.root);
        later.restore().await;
        let (shares, _) = later.views().await;
        assert_eq!(shares[0].state, "changed");
        assert!(shares[0].link.is_none());
        assert!(shares[0].error.as_deref().unwrap().contains("notes.txt"));
        let last = reopen(&a.store, &a.root);
        last.restore().await;
        assert!(last.views().await.0.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stopping_a_share_forgets_it_and_leaves_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let a = side(tmp.path());
        let file = a.root.join("x.bin");
        std::fs::write(&file, payload(5_000)).unwrap();
        a.sends.send(file.to_str().unwrap()).await.unwrap();
        let share = shared(&a.sends).await;
        a.sends.stop(&share.id).await.unwrap();
        assert!(a.sends.views().await.0.is_empty());
        assert!(file.exists());
        let again = reopen(&a.store, &a.root);
        again.restore().await;
        assert!(again.views().await.0.is_empty());
        assert_eq!(a.sends.stop(&share.id).await.unwrap_err().code, "not-found");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bad_input_gets_a_clear_error() {
        let tmp = tempfile::tempdir().unwrap();
        let a = side(tmp.path());
        let missing = a.sends.send("/no/such/file.zip").await.unwrap_err();
        assert_eq!(missing.code, "send-missing");
        assert!(missing.hint.is_some());
        let folder = a.sends.send(a.root.to_str().unwrap()).await.unwrap_err();
        assert_eq!(folder.code, "send-folder");
        let link = a
            .sends
            .receive("https://example.com/s#nope", None)
            .await
            .unwrap_err();
        assert_eq!(link.code, "bad-send-link");
        assert!(a.sends.views().await.0.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_offline_sender_is_reported_and_can_be_dismissed() {
        let tmp = tempfile::tempdir().unwrap();
        let a = side(&tmp.path().join("a"));
        let b = side(&tmp.path().join("b"));
        let file = a.root.join("x.bin");
        std::fs::write(&file, payload(5_000)).unwrap();
        a.sends.send(file.to_str().unwrap()).await.unwrap();
        let link = shared(&a.sends).await.link.unwrap();
        // The only address to try answers nothing (the sender's computer is off).
        let dead = SocketAddr::from((Ipv4Addr::LOCALHOST, 1));
        let id = b
            .sends
            .receive_from(&link, None, vec![dead], Duration::from_millis(500))
            .await
            .unwrap();
        let failed = until("the lookup to give up", async || {
            let (_, r) = b.sends.views().await;
            r.into_iter().find(|r| r.state == "failed")
        })
        .await;
        let e = failed.error.unwrap();
        assert!(e.contains("couldn't reach the sender"), "{e}");
        b.sends.dismiss(&id).await;
        assert!(b.sends.views().await.1.is_empty());
    }
}
