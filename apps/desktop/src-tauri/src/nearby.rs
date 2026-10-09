//! Nearby in the app (B8.11, docs/03-architecture/NEARBY.md): who is on the
//! network, who may see this computer, trusted devices, asking before files
//! arrive, and sending. The protocol itself is in `fuselane-nearby`.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fuselane_nearby::server::{Decision, Ended, Host};
use fuselane_nearby::{
    DeviceInfo, Discovery, Identity, Outgoing, PrepareUpload, SendError, Server, Target,
};
use serde::{Deserialize, Serialize};

use crate::service::{Emit, UiError, UiEvent};

/// How long "Everyone" lasts before it falls back to trusted devices only.
pub const EVERYONE_FOR: Duration = Duration::from_secs(10 * 60);
/// Devices not heard from for this long leave the list.
const FORGET_AFTER: Duration = Duration::from_secs(10 * 60);

/// A device on the network, as the window shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub fingerprint: String,
    pub alias: String,
    /// mobile | desktop | web | headless | server
    pub kind: String,
    /// "Fuselane", "Windows", "Samsung"… as it says.
    pub model: Option<String>,
    pub trusted: bool,
    /// Another Fuselane (shows the check words).
    pub fuselane: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Trusted {
    pub fingerprint: String,
    pub alias: String,
    /// When it was trusted (yyyy-mm-dd).
    pub since: String,
}

/// Someone asking to send files here, waiting for an answer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RequestView {
    pub id: u64,
    pub alias: String,
    pub kind: String,
    pub model: Option<String>,
    pub files: Vec<String>,
    pub total: u64,
    /// Four check words, when both sides are Fuselane and the sender was verified.
    pub words: Option<Vec<String>>,
    /// Its identity was checked by connecting back to it.
    pub verified: bool,
}

/// A transfer to or from a device.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TransferView {
    pub id: String,
    /// "out" or "in"
    pub direction: &'static str,
    pub device: String,
    pub name: String,
    pub size: u64,
    pub done: u64,
    /// asking | sending | receiving | done | declined | failed | cancelled
    pub state: &'static str,
    pub error: Option<String>,
    /// While asking: the words the other screen should show too.
    pub words: Option<Vec<String>>,
    /// Where a received file was saved.
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NearbyView {
    pub on: bool,
    pub me: String,
    /// Seconds left of "Everyone"; None when only trusted devices can see this.
    pub everyone_for: Option<u64>,
    pub devices: Vec<DeviceView>,
    pub trusted: Vec<Trusted>,
    pub transfers: Vec<TransferView>,
    pub request: Option<RequestView>,
    /// Why Nearby isn't working, when it isn't.
    pub problem: Option<String>,
}

struct Seen {
    info: DeviceInfo,
    addr: SocketAddr,
    at: Instant,
}

struct Pending {
    view: RequestView,
    answer: tokio::sync::oneshot::Sender<(bool, bool)>,
    fingerprint: Option<String>,
}

struct Out {
    view: TransferView,
    sent: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
}

/// The source of local IPv4 addresses Nearby listens on.
pub type Addrs = Arc<dyn Fn() -> Vec<Ipv4Addr> + Send + Sync>;

pub struct Nearby {
    state_dir: PathBuf,
    inbox: PathBuf,
    store: Arc<fuselane_core::Store>,
    addrs: Addrs,
    emit: Emit,
    identity: Mutex<Option<Identity>>,
    server: tokio::sync::Mutex<Option<Server>>,
    discovery: tokio::sync::Mutex<Option<Arc<Discovery>>>,
    port: AtomicU64,
    everyone_until: Mutex<Option<Instant>>,
    devices: Mutex<HashMap<String, Seen>>,
    trusted: Mutex<Vec<Trusted>>,
    pending: Mutex<Option<Pending>>,
    next_request: AtomicU64,
    outgoing: Mutex<Vec<Out>>,
    incoming: Mutex<HashMap<String, TransferView>>,
    problem: Mutex<Option<String>>,
    /// The discovery port (tests use their own).
    discovery_port: u16,
    alias: String,
}

impl std::fmt::Debug for Nearby {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Nearby")
            .field("alias", &self.alias)
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn err(code: &'static str, message: impl Into<String>, hint: Option<&str>) -> UiError {
    UiError {
        code,
        message: message.into(),
        hint: hint.map(str::to_string),
    }
}

/// This computer's name as people know it ("Arsh's MacBook Pro").
pub fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        && o.status.success()
    {
        let n = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !n.is_empty() {
            return n;
        }
    }
    if let Ok(n) = std::env::var("COMPUTERNAME") {
        return n;
    }
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Fuselane".into())
}

fn is_fuselane(info: &DeviceInfo) -> bool {
    info.device_model
        .as_deref()
        .is_some_and(|m| m.starts_with("Fuselane"))
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

impl Nearby {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Arc<fuselane_core::Store>,
        state_dir: PathBuf,
        inbox: PathBuf,
        addrs: Addrs,
        emit: Emit,
        alias: String,
        discovery_port: u16,
    ) -> Arc<Nearby> {
        let trusted = store
            .setting("nearby_trusted")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();
        Arc::new(Nearby {
            state_dir,
            inbox,
            store,
            addrs,
            emit,
            identity: Mutex::new(None),
            server: tokio::sync::Mutex::new(None),
            discovery: tokio::sync::Mutex::new(None),
            port: AtomicU64::new(0),
            everyone_until: Mutex::new(None),
            devices: Mutex::new(HashMap::new()),
            trusted: Mutex::new(trusted),
            pending: Mutex::new(None),
            next_request: AtomicU64::new(1),
            outgoing: Mutex::new(Vec::new()),
            incoming: Mutex::new(HashMap::new()),
            problem: Mutex::new(None),
            discovery_port,
            alias,
        })
    }

    fn fingerprint(&self) -> String {
        lock(&self.identity)
            .as_ref()
            .map(|i| i.fingerprint.clone())
            .unwrap_or_default()
    }

    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            alias: self.alias.chars().take(64).collect(),
            version: fuselane_nearby::proto::VERSION.into(),
            device_model: Some(format!("Fuselane ({})", std::env::consts::OS)),
            device_type: Some("desktop".into()),
            fingerprint: self.fingerprint(),
            port: u16::try_from(self.port.load(Ordering::Relaxed))
                .unwrap_or(fuselane_nearby::proto::PORT),
            protocol: "https".into(),
            download: false,
        }
    }

    fn everyone(&self) -> Option<Duration> {
        let mut u = lock(&self.everyone_until);
        match *u {
            Some(t) if t > Instant::now() => Some(t - Instant::now()),
            Some(_) => {
                *u = None;
                None
            }
            None => None,
        }
    }

    fn is_trusted(&self, fp: &str) -> bool {
        lock(&self.trusted)
            .iter()
            .any(|t| t.fingerprint.eq_ignore_ascii_case(fp))
    }

    pub fn running(&self) -> bool {
        self.port.load(Ordering::Relaxed) != 0
    }

    /// Starts the receiver and discovery (the first time the Send page opens).
    pub async fn start(self: &Arc<Self>) -> Result<NearbyView, UiError> {
        if self.running() {
            return Ok(self.view());
        }
        let id = Identity::load_or_create(&self.state_dir).map_err(|e| {
            err(
                "nearby-identity",
                format!("Nearby couldn't set up this computer's identity ({e})."),
                None,
            )
        })?;
        *lock(&self.identity) = Some(id.clone());
        let server = Server::start(&id, Arc::new(Receiver(Arc::downgrade(self))), fuselane_nearby::proto::PORT)
            .await
            .map_err(|e| {
                err(
                    "nearby-port",
                    format!("Nearby couldn't start receiving ({e})."),
                    Some("Another app may be using the port. Quit LocalSend on this computer and try again."),
                )
            })?;
        self.port
            .store(u64::from(server.addr.port()), Ordering::Relaxed);
        *self.server.lock().await = Some(server);
        match Discovery::start(&(self.addrs)(), self.discovery_port) {
            Ok(d) => {
                let d = Arc::new(d);
                *self.discovery.lock().await = Some(d.clone());
                let me = Arc::downgrade(self);
                tokio::spawn(async move {
                    while let Ok((from, a)) = d.recv().await {
                        let Some(me) = me.upgrade() else { break };
                        me.heard(from, a).await;
                    }
                });
            }
            Err(e) => {
                *lock(&self.problem) = Some(format!(
                    "Devices can't be found right now: {e}. Join Wi-Fi or plug in Ethernet."
                ));
            }
        }
        self.publish();
        Ok(self.view())
    }

    /// An announcement arrived: list the device, and answer it when it may see us.
    async fn heard(self: &Arc<Self>, from: SocketAddr, a: fuselane_nearby::proto::Announcement) {
        if a.info.fingerprint.eq_ignore_ascii_case(&self.fingerprint()) {
            return; // ourselves
        }
        let addr = SocketAddr::new(from.ip(), a.info.port);
        let fp = a.info.fingerprint.clone();
        lock(&self.devices).insert(
            fp.clone(),
            Seen {
                info: a.info.clone(),
                addr,
                at: Instant::now(),
            },
        );
        self.publish();
        if a.announce && (self.everyone().is_some() || self.is_trusted(&fp)) {
            let me = self.info();
            let t = Target {
                addr,
                fingerprint: (!fp.is_empty()).then_some(fp),
            };
            let _ = fuselane_nearby::client::register(&me, &t).await;
        }
    }

    /// Who can see this computer: everyone for 10 minutes, or trusted devices only.
    pub async fn set_everyone(self: &Arc<Self>, on: bool) -> NearbyView {
        *lock(&self.everyone_until) = on.then(|| Instant::now() + EVERYONE_FOR);
        if on {
            self.announce().await;
        }
        self.publish();
        self.view()
    }

    async fn announce(&self) {
        if let Some(d) = self.discovery.lock().await.clone() {
            let _ = d
                .announce(&fuselane_nearby::proto::Announcement {
                    info: self.info(),
                    announce: true,
                })
                .await;
        }
    }

    /// Every few seconds: re-announce while visible to everyone, drop devices not
    /// heard from in a while, and refresh sending progress.
    pub async fn tick(self: &Arc<Self>) {
        if !self.running() {
            return;
        }
        if self.everyone().is_some() {
            self.announce().await;
        }
        lock(&self.devices).retain(|_, d| d.at.elapsed() < FORGET_AFTER);
        for o in lock(&self.outgoing).iter_mut() {
            o.view.done = o.sent.load(Ordering::Relaxed).min(o.view.size);
        }
        self.publish();
    }

    pub fn view(&self) -> NearbyView {
        let trusted = lock(&self.trusted).clone();
        let mut devices: Vec<DeviceView> = lock(&self.devices)
            .iter()
            .map(|(fp, s)| DeviceView {
                fingerprint: fp.clone(),
                alias: s.info.alias.clone(),
                kind: s
                    .info
                    .device_type
                    .clone()
                    .unwrap_or_else(|| "desktop".into()),
                model: s.info.device_model.clone(),
                trusted: trusted
                    .iter()
                    .any(|t| t.fingerprint.eq_ignore_ascii_case(fp)),
                fuselane: is_fuselane(&s.info),
            })
            .collect();
        devices.sort_by(|a, b| b.trusted.cmp(&a.trusted).then(a.alias.cmp(&b.alias)));
        let mut transfers: Vec<TransferView> = lock(&self.outgoing)
            .iter()
            .map(|o| o.view.clone())
            .collect();
        transfers.extend(lock(&self.incoming).values().cloned());
        NearbyView {
            on: self.running(),
            me: self.alias.clone(),
            everyone_for: self.everyone().map(|d| d.as_secs()),
            devices,
            trusted,
            transfers,
            request: lock(&self.pending).as_ref().map(|p| p.view.clone()),
            problem: lock(&self.problem).clone(),
        }
    }

    fn publish(&self) {
        (self.emit)(UiEvent::Nearby { view: self.view() });
    }

    fn save_trusted(&self) {
        if let Ok(json) = serde_json::to_string(&*lock(&self.trusted)) {
            let _ = self.store.set_setting("nearby_trusted", &json);
        }
    }

    fn trust(&self, fp: &str, alias: &str) {
        let mut t = lock(&self.trusted);
        if !t.iter().any(|x| x.fingerprint.eq_ignore_ascii_case(fp)) {
            t.push(Trusted {
                fingerprint: fp.to_ascii_uppercase(),
                alias: alias.to_string(),
                since: today(),
            });
        }
        drop(t);
        self.save_trusted();
    }

    pub fn forget(&self, fingerprint: &str) -> NearbyView {
        lock(&self.trusted).retain(|t| !t.fingerprint.eq_ignore_ascii_case(fingerprint));
        self.save_trusted();
        self.publish();
        self.view()
    }

    /// The person's answer to an incoming request.
    pub fn answer(&self, id: u64, accept: bool, trust: bool) -> Result<NearbyView, UiError> {
        let p = lock(&self.pending)
            .take_if(|p| p.view.id == id)
            .ok_or_else(|| err("nearby-gone", "That request has ended.", None))?;
        if accept
            && trust
            && let Some(fp) = &p.fingerprint
        {
            self.trust(fp, &p.view.alias);
        }
        let _ = p.answer.send((accept, trust));
        self.publish();
        Ok(self.view())
    }

    /// Sends `paths` to a device on the list.
    pub async fn send(
        self: &Arc<Self>,
        fingerprint: &str,
        paths: Vec<String>,
    ) -> Result<NearbyView, UiError> {
        let (info, addr) = lock(&self.devices)
            .get(fingerprint)
            .map(|s| (s.info.clone(), s.addr))
            .ok_or_else(|| {
                err(
                    "nearby-gone",
                    "That device isn't on the network anymore.",
                    Some("Ask them to open Fuselane or LocalSend, then try again."),
                )
            })?;
        let mut files = vec![];
        for p in paths {
            let path = PathBuf::from(p.trim());
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
                    "Only files can be sent for now.",
                    Some("Zip the folder and send the zip."),
                ));
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            files.push(Outgoing {
                path,
                name,
                size: meta.len(),
                mime: "application/octet-stream".into(),
            });
        }
        if files.is_empty() {
            return Err(err("send-missing", "Pick at least one file to send.", None));
        }
        let id = format!("out-{}", self.next_request.fetch_add(1, Ordering::Relaxed));
        let words = is_fuselane(&info).then(|| {
            fuselane_nearby::check_words(&self.fingerprint(), fingerprint)
                .iter()
                .map(|w| (*w).to_string())
                .collect()
        });
        let (sent, cancel) = (
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(false)),
        );
        let name = if files.len() == 1 {
            files[0].name.clone()
        } else {
            format!("{} files", files.len())
        };
        lock(&self.outgoing).push(Out {
            view: TransferView {
                id: id.clone(),
                direction: "out",
                device: info.alias.clone(),
                name,
                size: files.iter().map(|f| f.size).sum(),
                done: 0,
                state: "asking",
                error: None,
                words,
                path: None,
            },
            sent: sent.clone(),
            cancel: cancel.clone(),
        });
        self.publish();
        let me = self.clone();
        let target = Target {
            addr,
            fingerprint: (!fingerprint.is_empty()).then(|| fingerprint.to_string()),
        };
        tokio::spawn(async move {
            let my = me.info();
            // Once bytes flow, it's no longer "asking".
            let watch = {
                let (me, id, sent) = (me.clone(), id.clone(), sent.clone());
                tokio::spawn(async move {
                    loop {
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        if sent.load(Ordering::Relaxed) > 0 {
                            if let Some(o) = lock(&me.outgoing).iter_mut().find(|o| o.view.id == id)
                                && o.view.state == "asking"
                            {
                                o.view.state = "sending";
                                o.view.words = None;
                            }
                            me.publish();
                            break;
                        }
                    }
                })
            };
            let r = fuselane_nearby::client::send(&my, &target, &files, sent.clone(), cancel).await;
            watch.abort();
            if let Some(o) = lock(&me.outgoing).iter_mut().find(|o| o.view.id == id) {
                o.view.done = sent.load(Ordering::Relaxed).min(o.view.size);
                o.view.words = None;
                match r {
                    Ok(_) => {
                        o.view.state = "done";
                        o.view.done = o.view.size;
                    }
                    Err(SendError::Declined) => o.view.state = "declined",
                    Err(SendError::Cancelled) => o.view.state = "cancelled",
                    Err(e) => {
                        o.view.state = "failed";
                        o.view.error = Some(format!("Couldn't send: {e}."));
                    }
                }
            }
            me.publish();
        });
        Ok(self.view())
    }

    pub fn cancel(&self, id: &str) {
        if let Some(o) = lock(&self.outgoing).iter().find(|o| o.view.id == id) {
            o.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Clears finished transfers from the list.
    pub fn clear(&self, id: &str) -> NearbyView {
        lock(&self.outgoing)
            .retain(|o| o.view.id != id || matches!(o.view.state, "asking" | "sending"));
        lock(&self.incoming).retain(|k, v| k != id || v.state == "receiving");
        self.publish();
        self.view()
    }

    pub fn received_path(&self, id: &str) -> Option<PathBuf> {
        lock(&self.incoming)
            .get(id)
            .and_then(|v| v.path.clone())
            .map(PathBuf::from)
    }
}

/// The receiving server's view of the app (held weakly so the app can go away).
struct Receiver(std::sync::Weak<Nearby>);

impl Host for Receiver {
    fn me(&self) -> DeviceInfo {
        self.0
            .upgrade()
            .map(|n| n.info())
            .unwrap_or_else(|| DeviceInfo {
                alias: "Fuselane".into(),
                version: fuselane_nearby::proto::VERSION.into(),
                device_model: None,
                device_type: None,
                fingerprint: String::new(),
                port: fuselane_nearby::proto::PORT,
                protocol: "https".into(),
                download: false,
            })
    }

    fn seen(&self, from: SocketAddr, info: DeviceInfo, verified: Option<String>) {
        let Some(n) = self.0.upgrade() else { return };
        // Only a verified fingerprint names a device that registers itself.
        let Some(fp) = verified else { return };
        if fp.eq_ignore_ascii_case(&n.fingerprint()) {
            return;
        }
        let addr = SocketAddr::new(from.ip(), info.port);
        lock(&n.devices).insert(
            fp,
            Seen {
                info,
                addr,
                at: Instant::now(),
            },
        );
        n.publish();
    }

    fn decide<'a>(
        &'a self,
        _from: SocketAddr,
        verified: Option<String>,
        req: &'a PrepareUpload,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Decision> + Send + 'a>> {
        Box::pin(async move {
            let Some(n) = self.0.upgrade() else {
                return Decision::Decline;
            };
            let trusted = verified.as_deref().is_some_and(|fp| n.is_trusted(fp));
            if trusted {
                return Decision::Accept {
                    dir: n.inbox.clone(),
                    only: None,
                };
            }
            // Trusted only: nobody else may send.
            if n.everyone().is_none() {
                return Decision::Decline;
            }
            if lock(&n.pending).is_some() {
                return Decision::Busy;
            }
            let words = match (&verified, is_fuselane(&req.info)) {
                (Some(fp), true) => Some(
                    fuselane_nearby::check_words(&n.fingerprint(), fp)
                        .iter()
                        .map(|w| (*w).to_string())
                        .collect(),
                ),
                _ => None,
            };
            let (tx, rx) = tokio::sync::oneshot::channel();
            let id = n.next_request.fetch_add(1, Ordering::Relaxed);
            *lock(&n.pending) = Some(Pending {
                view: RequestView {
                    id,
                    alias: req.info.alias.clone(),
                    kind: req
                        .info
                        .device_type
                        .clone()
                        .unwrap_or_else(|| "desktop".into()),
                    model: req.info.device_model.clone(),
                    files: req
                        .files
                        .values()
                        .map(|f| f.file_name.clone())
                        .take(50)
                        .collect(),
                    total: req.files.values().map(|f| f.size).sum(),
                    words,
                    verified: verified.is_some(),
                },
                answer: tx,
                fingerprint: verified,
            });
            n.publish();
            let answer = tokio::time::timeout(Duration::from_secs(110), rx).await;
            // Unanswered: the request goes away by itself.
            if lock(&n.pending).as_ref().is_some_and(|p| p.view.id == id) {
                lock(&n.pending).take();
                n.publish();
            }
            match answer {
                Ok(Ok((true, _))) => Decision::Accept {
                    dir: n.inbox.clone(),
                    only: None,
                },
                _ => Decision::Decline,
            }
        })
    }

    fn started(&self, session: &str, from: &DeviceInfo, names: &[String], total: u64) {
        let Some(n) = self.0.upgrade() else { return };
        lock(&n.incoming).insert(
            session.to_string(),
            TransferView {
                id: session.to_string(),
                direction: "in",
                device: from.alias.clone(),
                name: if names.len() == 1 {
                    names[0].clone()
                } else {
                    format!("{} files", names.len())
                },
                size: total,
                done: 0,
                state: "receiving",
                error: None,
                words: None,
                path: None,
            },
        );
        n.publish();
    }

    fn progress(&self, session: &str, _file: &str, written: u64) {
        let Some(n) = self.0.upgrade() else { return };
        if let Some(v) = lock(&n.incoming).get_mut(session) {
            v.done = written.max(v.done);
        }
    }

    fn file_done(&self, session: &str, _file: &str, path: &Path) {
        let Some(n) = self.0.upgrade() else { return };
        let mut inc = lock(&n.incoming);
        let v = inc
            .entry(session.to_string())
            .or_insert_with(|| TransferView {
                id: session.to_string(),
                direction: "in",
                device: String::new(),
                name: String::new(),
                size: 0,
                done: 0,
                state: "receiving",
                error: None,
                words: None,
                path: None,
            });
        // One file: its saved name (it may have been numbered). Several: the folder.
        if !v.name.ends_with(" files") {
            v.name = path
                .file_name()
                .map(|x| x.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
        v.path = Some(path.display().to_string());
        drop(inc);
        n.publish();
    }

    fn ended(&self, session: &str, how: Ended) {
        let Some(n) = self.0.upgrade() else { return };
        if let Some(v) = lock(&n.incoming).get_mut(session) {
            match how {
                Ended::Done => {
                    v.state = "done";
                    v.done = v.size;
                }
                Ended::Cancelled => v.state = "cancelled",
                Ended::Failed(why) => {
                    v.state = "failed";
                    v.error = Some(format!("It didn't arrive: {why}."));
                }
            }
        }
        n.publish();
    }
}

/// IPv4 addresses on networks where other devices can be (not phone tethers).
pub fn lan_addrs() -> Vec<Ipv4Addr> {
    fuselane_netif::list()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| {
            i.usable()
                && matches!(
                    i.kind,
                    fuselane_netif::Kind::Wifi
                        | fuselane_netif::Kind::Ethernet
                        | fuselane_netif::Kind::Other
                )
        })
        .flat_map(|i| i.addrs)
        .filter_map(|a| match a {
            IpAddr::V4(v4) if !v4.is_loopback() => Some(v4),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Side {
        n: Arc<Nearby>,
        inbox: tempfile::TempDir,
        _state: tempfile::TempDir,
    }

    async fn side(name: &str) -> Side {
        let state = tempfile::tempdir().unwrap();
        let inbox = tempfile::tempdir().unwrap();
        let store = Arc::new(fuselane_core::Store::open(&state.path().join("db")).unwrap());
        let n = Nearby::new(
            store,
            state.path().to_path_buf(),
            inbox.path().to_path_buf(),
            Arc::new(Vec::new),
            Arc::new(|_| {}),
            name.into(),
            54_000 + (std::process::id() % 900) as u16,
        );
        n.start().await.unwrap();
        Side {
            n,
            inbox,
            _state: state,
        }
    }

    fn addr(s: &Side) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], s.n.info().port))
    }

    /// `a` introduces itself to `b` (what answering an announcement does).
    async fn introduce(a: &Side, b: &Side) {
        let t = Target {
            addr: addr(b),
            fingerprint: Some(b.n.fingerprint()),
        };
        fuselane_nearby::client::register(&a.n.info(), &t)
            .await
            .unwrap();
    }

    async fn until(what: &str, f: impl Fn() -> bool) {
        for _ in 0..200 {
            if f() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("timed out waiting for {what}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn asking_trusting_and_forgetting_a_device() {
        let a = side("Maya's MacBook Air").await;
        let b = side("STUDIO-PC").await;
        introduce(&a, &b).await;
        let fa = a.n.fingerprint();
        until("b to list a", || {
            b.n.view().devices.iter().any(|d| d.fingerprint == fa)
        })
        .await;
        let listed =
            b.n.view()
                .devices
                .into_iter()
                .find(|d| d.fingerprint == fa)
                .unwrap();
        assert!(listed.fuselane && !listed.trusted);
        assert_eq!(listed.alias, "Maya's MacBook Air");

        let out = tempfile::tempdir().unwrap();
        let file = out.path().join("Holiday video.mov");
        std::fs::write(&file, vec![9u8; 300_000]).unwrap();
        let path = file.display().to_string();

        // Trusted only (the default): an unknown device is refused.
        b.n.send(&fa, vec![path.clone()]).await.unwrap();
        until("a decline", || {
            b.n.view().transfers.iter().any(|t| t.state == "declined")
        })
        .await;
        assert_eq!(std::fs::read_dir(a.inbox.path()).unwrap().count(), 0);

        // Everyone: a asks, with the same words on both screens.
        a.n.set_everyone(true).await;
        assert!(a.n.view().everyone_for.is_some_and(|s| s > 590));
        b.n.send(&fa, vec![path.clone()]).await.unwrap();
        until("the request", || a.n.view().request.is_some()).await;
        let req = a.n.view().request.unwrap();
        assert_eq!(req.alias, "STUDIO-PC");
        assert_eq!(req.files, vec!["Holiday video.mov"]);
        assert!(req.verified);
        let asking =
            b.n.view()
                .transfers
                .into_iter()
                .find(|t| t.state == "asking")
                .unwrap();
        assert_eq!(req.words, asking.words, "both screens show the same words");
        a.n.answer(req.id, true, true).unwrap();
        let saved = a.inbox.path().join("Holiday video.mov");
        until("the file", || {
            saved.exists() && a.n.view().transfers.iter().any(|t| t.state == "done")
        })
        .await;
        assert_eq!(std::fs::read(&saved).unwrap(), vec![9u8; 300_000]);
        assert_eq!(a.n.view().trusted.len(), 1);

        // Trusted now: the next file arrives without asking, even when not visible.
        a.n.set_everyone(false).await;
        std::fs::write(&file, b"second").unwrap();
        b.n.send(&fa, vec![path.clone()]).await.unwrap();
        let second = a.inbox.path().join("Holiday video (2).mov");
        until("the second file", || second.exists()).await;
        assert!(a.n.view().request.is_none());

        // Forgotten: asked again (and refused, since only trusted devices may send).
        let fb = a.n.view().trusted[0].fingerprint.clone();
        a.n.forget(&fb);
        let before =
            b.n.view()
                .transfers
                .iter()
                .filter(|t| t.state == "declined")
                .count();
        b.n.send(&fa, vec![path]).await.unwrap();
        until("a decline", || {
            b.n.view()
                .transfers
                .iter()
                .filter(|t| t.state == "declined")
                .count()
                > before
        })
        .await;
    }
}
