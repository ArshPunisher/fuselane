//! The multi-network HTTP downloader (ENGINE-DOWNLOAD.md §1–9).
//!
//! probe → plan → claim staging file → streams pull work from the pure
//! scheduler → every response checked → bytes written at offsets → publish.
//! Networks are abstract connectors here; `transport` supplies pinned sockets
//! in production, tests supply plain or faulty ones.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::client::conn::http1::SendRequest;
use hyper::header::{
    ACCEPT_ENCODING, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG, HOST,
    HeaderName, HeaderValue, IF_RANGE, LAST_MODIFIED, LOCATION, RANGE, RETRY_AFTER, USER_AGENT,
};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;

use fuselane_storage::staging::{Staging, StagingError};

use crate::headers::{self, ContentRange, RangeError};
use crate::plan::Plan;
pub use crate::retry::LaneTrouble;
use crate::retry::{Decision, DiskFailure, Failure, StreamRetry};
use crate::scheduler::{self, Attempt, Block, HedgePolicy, NetId, Requester, StreamId};
use crate::throttle::{self, ThrottlePolicy};

/// Anything a connector can hand back.
pub trait Io: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static> Io for T {}

pub type BoxIo = Box<dyn Io>;
pub type ConnectFuture = Pin<Box<dyn Future<Output = std::io::Result<BoxIo>> + Send>>;
/// Opens a connection to the server through one network.
pub type Connect = Arc<dyn Fn(SocketAddr) -> ConnectFuture + Send + Sync>;

/// One usable network.
#[derive(Clone)]
pub struct Network {
    pub id: NetId,
    pub name: String,
    pub connect: Connect,
    /// This lane fetches from a mirror instead of the main source (B8.9). Mirror
    /// lanes come after the main ones; `connect` reaches the mirror's server.
    pub mirror: Option<Arc<Mirror>>,
}

/// Another server with the same file (B8.9). Every response from it must match
/// the main file's exact size; a mismatch, a refusal or a changed file only
/// retires the lanes using it, never the download.
#[derive(Debug, Clone)]
pub struct Mirror {
    pub source: Source,
    /// Its own validator for If-Range (validators differ from server to server).
    pub if_range: Option<String>,
    /// Its own normalized ETag, checked on each of its responses.
    pub etag: Option<String>,
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Network")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

/// Where the file lives (plain HTTP here; TLS comes with `transport`).
#[derive(Debug, Clone)]
pub struct Source {
    pub addr: SocketAddr,
    pub host: String,
    pub path: String,
}

/// One network's part of a [`Snapshot`].
#[derive(Debug, Clone, PartialEq)]
pub struct NetSnapshot {
    pub id: NetId,
    pub bytes: u64,
    /// Bytes per second over the last few seconds (L-30).
    pub rate: f64,
    pub streams: u32,
    pub dead: bool,
}

/// One tick of the Fuse Core ring: a group of blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TickSnapshot {
    /// 0.0..=1.0 of this tick's bytes secured.
    pub fill: f32,
    /// The network that fetched most of it so far.
    pub owner: Option<NetId>,
    /// A network currently fetching inside it.
    pub in_flight: Option<NetId>,
}

/// What the UI draws, emitted a few times a second (deltas come later, L-34).
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub written: u64,
    pub total: Option<u64>,
    pub rate: f64,
    pub networks: Vec<NetSnapshot>,
    /// At most `TICKS` ticks, in file order.
    pub ticks: Vec<TickSnapshot>,
    pub retries: u64,
    pub hedges: u64,
}

/// Ring resolution: blocks are grouped into at most this many ticks.
pub const TICKS: usize = 180;

#[derive(Clone)]
pub struct SnapshotFn(pub Arc<dyn Fn(&Snapshot) + Send + Sync>);

impl std::fmt::Debug for SnapshotFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SnapshotFn")
    }
}

/// Called with (bytes written so far, total if known). Keep it cheap; it runs on the data path.
#[derive(Clone)]
pub struct ProgressFn(pub Arc<dyn Fn(u64, Option<u64>) + Send + Sync>);

impl std::fmt::Debug for ProgressFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProgressFn")
    }
}

/// Something worth telling the person about one network during a download.
#[derive(Debug, Clone, PartialEq)]
pub enum LaneEvent {
    /// `name` slowed to `rate` bytes per second (its best was `best`; 0 when
    /// never measured) while another network kept going. It gets no new work and
    /// hands back what it held until a check finds it fast again (STEPS 8.2).
    Throttled {
        net: NetId,
        name: String,
        rate: f64,
        best: f64,
    },
    /// A check found it fast again (`rate` bytes per second on one stream).
    Restored { net: NetId, name: String, rate: f64 },
    /// It can't connect, for a reason its connector put in words (a proxy
    /// turning down the login). `lasting`: it stopped trying for this download.
    Trouble {
        net: NetId,
        name: String,
        message: String,
        lasting: bool,
    },
}

/// Receives [`LaneEvent`]s. Called outside the engine's locks; keep it cheap.
#[derive(Clone)]
pub struct LaneEventFn(pub Arc<dyn Fn(&LaneEvent) + Send + Sync>);

impl std::fmt::Debug for LaneEventFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LaneEventFn")
    }
}

/// Stops a running download cleanly (pause). Cheap to clone.
#[derive(Clone, Default)]
pub struct Cancel {
    flag: Arc<AtomicBool>,
    notify: Arc<tokio::sync::Notify>,
}

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

impl std::fmt::Debug for Cancel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cancel")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// What must be saved to resume later. Emitted only after the staging file is fsynced,
/// so every byte counted in `secured` is durable (L-55).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub staging_path: PathBuf,
    pub filename: String,
    pub total: Option<u64>,
    pub block_size: u64,
    /// Bytes secured from the start of each block.
    pub secured: Vec<u64>,
    pub raw_etag: Option<String>,
    pub last_modified: Option<String>,
}

impl Checkpoint {
    pub fn secured_bytes(&self) -> u64 {
        self.secured.iter().sum()
    }
}

/// Receives checkpoints (the core persists them).
#[derive(Clone)]
pub struct CheckpointFn(pub Arc<dyn Fn(&Checkpoint) + Send + Sync>);

impl std::fmt::Debug for CheckpointFn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CheckpointFn")
    }
}

/// A saved download to continue. Treated as untrusted input (L-53).
#[derive(Debug, Clone)]
pub struct Resume {
    pub staging_path: PathBuf,
    pub total: u64,
    pub block_size: u64,
    pub secured: Vec<u64>,
    pub raw_etag: Option<String>,
    pub last_modified: Option<String>,
}

impl From<Checkpoint> for Option<Resume> {
    fn from(c: Checkpoint) -> Option<Resume> {
        Some(Resume {
            staging_path: c.staging_path,
            total: c.total?,
            block_size: c.block_size,
            secured: c.secured,
            raw_etag: c.raw_etag,
            last_modified: c.last_modified,
        })
    }
}

/// Request headers a logged-in browser session needs (its cookies, a referrer),
/// already checked by [`Headers::checked`]. Never printed: `Debug` shows the names only.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Headers(Vec<(HeaderName, HeaderValue)>);

impl std::fmt::Debug for Headers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(|(n, _)| n.as_str()))
            .finish()
    }
}

/// The only headers a download may carry from a browser. Everything that shapes the
/// request itself (Host, Range, encodings, connection handling) stays Fuselane's.
const ALLOWED: &[&str] = &[
    "cookie",
    "authorization",
    "referer",
    "user-agent",
    "accept",
    "accept-language",
];
/// Cookies can be long; anything bigger than this is not a normal session.
const MAX_HEADER_BYTES: usize = 16 * 1024;

impl Headers {
    /// The same headers without the ones that prove who you are (cookies, a login),
    /// for following a redirect to another site.
    pub fn without_credentials(&self) -> Headers {
        Headers(
            self.0
                .iter()
                .filter(|(n, _)| n.as_str() != "cookie" && n.as_str() != "authorization")
                .cloned()
                .collect(),
        )
    }

    /// The same headers with an HTTP Basic login (from a link like
    /// `https://user:pass@host/file`), replacing any other login.
    pub fn with_login(&self, user: &str, password: &str) -> Result<Headers, String> {
        use base64::Engine;
        let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{password}"));
        let value = HeaderValue::from_str(&format!("Basic {token}"))
            .map_err(|_| "the login in the link isn't valid".to_string())?;
        let mut out: Vec<_> = self
            .0
            .iter()
            .filter(|(n, _)| n.as_str() != "authorization")
            .cloned()
            .collect();
        out.push((hyper::header::AUTHORIZATION, value));
        Ok(Headers(out))
    }

    /// Keeps allowed headers with valid values; anything else is an error naming it.
    pub fn checked(raw: &[(String, String)]) -> Result<Headers, String> {
        let mut out = Vec::new();
        let mut total = 0;
        for (name, value) in raw {
            let lower = name.trim().to_ascii_lowercase();
            if !ALLOWED.contains(&lower.as_str()) {
                return Err(format!("the {name} header can't be passed on"));
            }
            total += lower.len() + value.len();
            if total > MAX_HEADER_BYTES {
                return Err("the headers are too large (over 16 KB)".into());
            }
            let n = HeaderName::from_bytes(lower.as_bytes())
                .map_err(|_| format!("{name} isn't a valid header name"))?;
            // HeaderValue refuses CR, LF and NUL, so nothing can be smuggled in.
            let v = HeaderValue::from_str(value.trim())
                .map_err(|_| format!("the {name} header has characters that aren't allowed"))?;
            out.retain(|(o, _): &(HeaderName, HeaderValue)| o != n);
            out.push((n, v));
        }
        Ok(Headers(out))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Carries something that proves who you are (a cookie or a login).
    pub fn has_credentials(&self) -> bool {
        self.without_credentials().0.len() != self.0.len()
    }
}

/// Every tunable in one place (CLAUDE.md coding conventions).
#[derive(Debug, Clone)]
pub struct Tuning {
    pub connect_timeout: Duration,
    pub first_byte_timeout: Duration,
    pub idle_timeout: Duration,
    pub streams_per_network: u32,
    /// Tests use small blocks to exercise many of them.
    pub block_size: Option<u64>,
    pub hedge: HedgePolicy,
    /// Bytes compared when a validator changes (L-05).
    pub confirm_sample: u64,
    pub user_agent: String,
    /// Multiplies every retry wait (tests compress time; production uses 1.0).
    pub retry_delay_scale: f64,
    pub progress: Option<ProgressFn>,
    /// Auto (grow 8 → 32 while every stream is served) or keep `streams_per_network` fixed.
    pub auto_streams: bool,
    /// How often the stream-count controller runs.
    pub controller_tick: Duration,
    pub checkpoint: Option<CheckpointFn>,
    /// fsync + checkpoint at most this often while running (and always on stop).
    pub checkpoint_every: Duration,
    pub cancel: Option<Cancel>,
    /// When given, the finished file must have this SHA-256 or it isn't published (step 2.32).
    pub expected_sha256: Option<[u8; 32]>,
    /// Windows of secured bytes re-checked against the server on resume (lying checkpoints).
    pub resume_samples: u32,
    pub snapshot: Option<SnapshotFn>,
    pub snapshot_every: Duration,
    /// Live speed limits shared with other downloads (by network name).
    pub limiter: Option<Arc<fuselane_limits::Limiter>>,
    /// This download's own limit, changeable while it runs.
    pub job_limit: Option<Arc<fuselane_limits::JobLimit>>,
    /// The name the person chose, instead of the server's (made safe first).
    pub filename: Option<String>,
    /// A browser session's cookies and referrer, sent on every request.
    pub headers: Headers,
    /// When a network counts as throttled, and how often it's tried again.
    pub throttle: ThrottlePolicy,
    /// Hears when a network is benched as throttled or comes back.
    pub lane_events: Option<LaneEventFn>,
}

impl Default for Tuning {
    fn default() -> Self {
        Tuning {
            connect_timeout: Duration::from_secs(10),
            first_byte_timeout: Duration::from_secs(20),
            idle_timeout: Duration::from_secs(20),
            streams_per_network: crate::concurrency::START,
            block_size: None,
            hedge: HedgePolicy::default(),
            confirm_sample: 16 * 1024,
            user_agent: format!("Fuselane/{}", env!("CARGO_PKG_VERSION")),
            retry_delay_scale: 1.0,
            progress: None,
            auto_streams: true,
            controller_tick: Duration::from_millis(500),
            checkpoint: None,
            checkpoint_every: Duration::from_secs(15),
            cancel: None,
            expected_sha256: None,
            resume_samples: 4,
            snapshot: None,
            snapshot_every: Duration::from_millis(200),
            filename: None,
            headers: Headers::default(),
            limiter: None,
            job_limit: None,
            throttle: ThrottlePolicy::default(),
            lane_events: None,
        }
    }
}

/// How a download ended badly. Each maps to one user message (ERRORS.md).
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("no network could reach the server: {0}")]
    Unreachable(String),
    #[error("the server answered the first request with status {0}")]
    ProbeStatus(u16),
    /// The file is somewhere else (3xx with a Location); the caller follows it,
    /// since a new host needs new connections.
    #[error("the server moved the file (status {status})")]
    Redirect { status: u16, location: String },
    #[error("this link stopped working (status {0}); paste a fresh link to continue")]
    LinkExpired(u16),
    #[error("the file on the server changed since the download started")]
    VersionChanged,
    #[error("the disk failed: {0:?}")]
    Disk(DiskFailure),
    #[error("every network failed; last error: {0}")]
    AllNetworksFailed(String),
    /// The disk can't hold what's left of the file (checked before writing).
    #[error("not enough free space: need {needed} bytes, {free} free")]
    NoSpace { needed: u64, free: u64 },
    #[error("the downloaded file doesn't match the expected SHA-256")]
    ChecksumMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },
    #[error("paused; progress is saved")]
    Paused,
    #[error("this download can't be resumed: {0}")]
    NotResumable(String),
    /// No network could connect, and a connector said why (a proxy refusing
    /// the login). Already plain words with what to do.
    #[error("{0}")]
    Lane(String),
    #[error(transparent)]
    Staging(#[from] StagingError),
}

/// What happened, for the UI and for tests.
#[derive(Debug, Clone)]
pub struct Report {
    pub path: PathBuf,
    pub total: u64,
    pub bytes_by_network: HashMap<NetId, u64>,
    pub retries: u64,
    pub hedges: u64,
    /// Most requests in flight at once on each network (what the controller allowed).
    pub peak_streams: HashMap<NetId, u32>,
}

/// What the probe learned.
#[derive(Debug, Clone)]
pub struct Probe {
    pub total: Option<u64>,
    pub ranges: bool,
    /// Normalized, for comparing responses (L-06).
    pub etag: Option<String>,
    /// Exactly as the server sent it: the only valid If-Range value (RFC 9110 §13.1.5).
    pub raw_etag: Option<String>,
    pub last_modified: Option<String>,
    pub filename: String,
    /// The server's Content-Type, lowercased, without parameters.
    pub content_type: Option<String>,
}

impl Probe {
    /// A web page rather than a file: what a server answers with when it wants
    /// someone to sign in first.
    pub fn is_web_page(&self) -> bool {
        matches!(
            self.content_type.as_deref(),
            Some("text/html" | "application/xhtml+xml")
        )
    }
}

// ---------- one HTTP exchange ----------

struct Conn {
    sender: SendRequest<Empty<Bytes>>,
}

async fn connect(net: &Network, src: &Source, t: &Tuning) -> Result<Conn, Failure> {
    let io = tokio::time::timeout(t.connect_timeout, (net.connect)(src.addr))
        .await
        .map_err(|_| Failure::Connection)?
        .map_err(|e| {
            LaneTrouble::of(&e).map_or(Failure::Connection, |t| Failure::Lane(t.clone()))
        })?;
    let (sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(io))
        .await
        .map_err(|_| Failure::Connection)?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    Ok(Conn { sender })
}

fn request(
    src: &Source,
    t: &Tuning,
    range: Option<(u64, Option<u64>)>,
    if_range: Option<&str>,
) -> Request<Empty<Bytes>> {
    let mut b = Request::get(&src.path)
        .header(HOST, &src.host)
        .header(ACCEPT_ENCODING, "identity");
    // The browser's own User-Agent, when given, so the server sees the same client.
    if !t.headers.0.iter().any(|(n, _)| n == USER_AGENT) {
        b = b.header(USER_AGENT, &t.user_agent);
    }
    for (n, v) in &t.headers.0 {
        b = b.header(n, v);
    }
    if let Some((first, last)) = range {
        b = b.header(
            RANGE,
            match last {
                Some(l) => format!("bytes={first}-{l}"),
                None => format!("bytes={first}-"),
            },
        );
    }
    if let Some(v) = if_range {
        b = b.header(IF_RANGE, v);
    }
    b.body(Empty::new())
        .unwrap_or_else(|_| Request::new(Empty::new()))
}

async fn send(
    conn: &mut Conn,
    req: Request<Empty<Bytes>>,
    t: &Tuning,
) -> Result<Response<hyper::body::Incoming>, Failure> {
    conn.sender.ready().await.map_err(|_| Failure::Connection)?;
    tokio::time::timeout(t.first_byte_timeout, conn.sender.send_request(req))
        .await
        .map_err(|_| Failure::Connection)?
        .map_err(|_| Failure::Connection)
}

fn header(
    res: &Response<hyper::body::Incoming>,
    name: hyper::header::HeaderName,
) -> Option<String> {
    res.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Reads bytes `first..=last` of the file from `src` through `net`, or None if
/// the server doesn't answer with exactly that range. Used to spot-check that a
/// mirror serves the same bytes as the main server before it may help (B8.9).
pub async fn read_range(
    src: &Source,
    net: &Network,
    t: &Tuning,
    first: u64,
    last: u64,
) -> Option<Vec<u8>> {
    let mut c = connect(net, src, t).await.ok()?;
    let res = send(&mut c, request(src, t, Some((first, Some(last))), None), t)
        .await
        .ok()?;
    if res.status().as_u16() != 206 {
        return None;
    }
    let body = res.into_body().collect().await.ok()?.to_bytes();
    (body.len() as u64 == last - first + 1).then(|| body.to_vec())
}

/// Reads a whole small file (at most `max` bytes) from `src` through `net`, or
/// None if it isn't there, is bigger, or the server misbehaves. Used for the
/// checksum lists published next to downloads (B9.7).
pub async fn read_small(src: &Source, net: &Network, t: &Tuning, max: usize) -> Option<Vec<u8>> {
    let mut c = connect(net, src, t).await.ok()?;
    let res = send(&mut c, request(src, t, None, None), t).await.ok()?;
    if res.status().as_u16() != 200 {
        return None;
    }
    let declared = res
        .headers()
        .get(hyper::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| n > max as u64) {
        return None;
    }
    let body = http_body_util::Limited::new(res.into_body(), max)
        .collect()
        .await
        .ok()?
        .to_bytes();
    Some(body.to_vec())
}

/// Busy answers the probe waits out before reporting the status.
const PROBE_BUSY_RETRIES: u32 = 3;
/// Longest Retry-After the probe honours (a server asking for an hour is answered later).
const PROBE_RETRY_AFTER_CAP_MS: u64 = 30_000;

/// Probes with `Range: bytes=0-0` on the first network that answers (L-01, L-04).
pub async fn probe(src: &Source, networks: &[Network], t: &Tuning) -> Result<Probe, JobError> {
    let mut last = String::from("no networks");
    // A reason a connector gave (a proxy turning down the login) beats "couldn't connect".
    let mut trouble: Option<String> = None;
    for net in networks.iter().filter(|n| n.mirror.is_none()) {
        let mut busy = 0;
        let res = loop {
            let mut conn = match connect(net, src, t).await {
                Ok(c) => c,
                Err(Failure::Lane(why)) => {
                    trouble = Some(why.message);
                    break None;
                }
                Err(_) => {
                    last = format!("{} couldn't connect", net.name);
                    break None;
                }
            };
            let res = match send(&mut conn, request(src, t, Some((0, Some(0))), None), t).await {
                Ok(r) => r,
                Err(_) => {
                    last = format!("{} got no answer", net.name);
                    break None;
                }
            };
            let status = res.status().as_u16();
            // A busy server (429, 503...) gets a few patient retries before we give up,
            // honouring Retry-After (L-110): one rate-limit answer must not end a download.
            if crate::retry::is_busy(status) && busy < PROBE_BUSY_RETRIES {
                let wait = header(&res, hyper::header::RETRY_AFTER)
                    .and_then(|v| crate::retry::parse_retry_after(&v, now_unix()))
                    .unwrap_or(2_000 << busy)
                    .min(PROBE_RETRY_AFTER_CAP_MS);
                busy += 1;
                drop(res);
                tokio::time::sleep(scaled(wait, t)).await;
                continue;
            }
            break Some(res);
        };
        let Some(res) = res else { continue };
        let status = res.status().as_u16();
        if (300..400).contains(&status)
            && status != 304
            && let Some(location) = header(&res, LOCATION).filter(|l| !l.trim().is_empty())
        {
            return Err(JobError::Redirect {
                status,
                location: location.trim().to_string(),
            });
        }
        let raw_etag = header(&res, ETAG)
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty());
        let etag = raw_etag
            .as_deref()
            .map(headers::normalize_etag)
            .filter(|e| !e.is_empty());
        let last_modified = header(&res, LAST_MODIFIED);
        let disposition = header(&res, CONTENT_DISPOSITION)
            .and_then(|v| headers::content_disposition_filename(&v));
        let from_path = headers::filename_from_path(src.path.split('?').next().unwrap_or(""));
        let filename = disposition
            .or(from_path)
            .unwrap_or_else(|| "download".into());
        let content_type = header(&res, CONTENT_TYPE).map(|v| {
            v.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        });
        let cr = header(&res, CONTENT_RANGE).and_then(|v| headers::parse_content_range(&v));
        let cl = header(&res, CONTENT_LENGTH).and_then(|v| v.parse::<u64>().ok());
        let (total, ranges) = match (status, cr) {
            (206, Some(ContentRange::Bytes { total, .. })) => (total, total.is_some()),
            (416, Some(ContentRange::Unsatisfied { total: 0 })) => (Some(0), false),
            (200, _) => (cl, false), // Content-Length is trusted only on a 200 (L-04)
            (s, _) => return Err(JobError::ProbeStatus(s)),
        };
        drop(res);
        return Ok(Probe {
            total,
            ranges,
            etag,
            raw_etag,
            last_modified,
            filename,
            content_type,
        });
    }
    Err(trouble.map_or(JobError::Unreachable(last), JobError::Lane))
}

// ---------- shared state ----------

#[derive(Debug, Default)]
struct BlockExtra {
    /// Byte intervals within the block that some attempt wrote: [from, to).
    written: Vec<(u64, u64)>,
}

struct Shared {
    plan: Plan,
    blocks: Vec<Block>,
    extra: Vec<BlockExtra>,
    idle: HashMap<NetId, u32>,
    dead_networks: HashSet<NetId>,
    /// Consecutive link refusals per network (status, count), reset by delivered bytes.
    link_refused: HashMap<NetId, (u16, u32)>,
    /// Normalized ETag that responses must match (updated when a relabel is confirmed).
    accepted_etag: Option<String>,
    /// If-Range value: the raw strong ETag, else Last-Modified, else none
    /// (weak ETags can't be used in If-Range; a normalized one makes servers send 200).
    if_range: Option<String>,
    bytes_by_network: HashMap<NetId, u64>,
    fatal: Option<JobError>,
    last_error: String,
    retries: u64,
    hedges: u64,
    /// For an unknown size: bytes received when the single attempt finished.
    unknown_total: Option<u64>,
    /// Per-network stream bookkeeping for the concurrency controller.
    nets: HashMap<NetId, NetStats>,
    peak_streams: HashMap<NetId, u32>,
    /// Bytes downloaded twice because a hedge raced an attempt (diagnostics).
    wasted_bytes: u64,
    /// Per block, the furthest offset any attempt has written.
    written_max: Vec<u64>,
    /// Bytes each network delivered into each block (for the ring's colours).
    block_net_bytes: Vec<Vec<(NetId, u64)>>,
    meters: HashMap<NetId, crate::measure::Meter>,
    total_meter: crate::measure::Meter,
    /// Every byte each network received, races included: its real speed (8.2).
    raw_bytes: HashMap<NetId, u64>,
    /// Networks benched as throttled: they get no work while others can carry it.
    benched: HashSet<NetId>,
    /// A check may start on these benched networks (the first stream to ask runs it)...
    probe_open: HashSet<NetId>,
    /// ...and this stream is running it.
    prober: HashMap<NetId, StreamId>,
    /// Connector reasons already passed on, per network.
    told: HashSet<(NetId, String)>,
}

#[derive(Debug, Default)]
struct NetStats {
    live: u32,
    /// Streams to retire at their next pick (graceful: their current attempt finishes).
    retire: u32,
    answered: HashSet<StreamId>,
    served_tick: HashSet<StreamId>,
    refused_tick: HashSet<StreamId>,
}

impl Shared {
    fn all_complete(&self) -> bool {
        self.blocks.iter().all(Block::complete)
    }

    /// Whether `stream` on `net` must stand aside: its network is benched as
    /// throttled, it isn't running the network's check, and another network can
    /// carry the work (a benched network still works when it's the last one).
    fn resting(&self, ctx: &Ctx, net: NetId, stream: StreamId) -> bool {
        self.benched.contains(&net)
            && self.prober.get(&net) != Some(&stream)
            && ctx.lanes.iter().any(|(id, name)| {
                *id != net
                    && !self.dead_networks.contains(id)
                    && !self.benched.contains(id)
                    && self.nets.get(id).is_some_and(|n| n.live > 0)
                    && !ctx.tuning.limiter.as_ref().is_some_and(|l| l.blocked(name))
            })
    }

    /// Advances `secured` through contiguous written intervals.
    fn settle(&mut self, b: usize) {
        let blk = &mut self.blocks[b];
        let ex = &mut self.extra[b];
        let mut moved = true;
        while moved {
            moved = false;
            for &(from, to) in &ex.written {
                if from <= blk.secured && to > blk.secured {
                    blk.secured = to.min(blk.len);
                    moved = true;
                }
            }
        }
        ex.written.retain(|&(_, to)| to > blk.secured);
    }
}

struct Ctx {
    src: Source,
    tuning: Tuning,
    shared: Mutex<Shared>,
    file: std::fs::File,
    wake: tokio::sync::Notify,
    stop: AtomicBool,
    live_networks: usize,
    epoch: Instant,
    written_total: AtomicU64,
    staging_path: PathBuf,
    filename: String,
    last_modified: Option<String>,
    /// Every lane's id and network name.
    lanes: Vec<(NetId, String)>,
}

impl Ctx {
    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// ---------- one stream ----------

/// Keeps a network's live-stream count right however the stream ends.
struct LiveGuard {
    ctx: Arc<Ctx>,
    net: NetId,
}

impl Drop for LiveGuard {
    fn drop(&mut self) {
        let mut s = self.ctx.lock();
        if let Some(n) = s.nets.get_mut(&self.net) {
            n.live = n.live.saturating_sub(1);
        }
    }
}

async fn run_stream(ctx: Arc<Ctx>, net: Network, stream: StreamId) {
    ctx.lock().nets.entry(net.id).or_default().live += 1;
    let _live = LiveGuard {
        ctx: ctx.clone(),
        net: net.id,
    };
    let mut conn: Option<Conn> = None;
    let mut retry = StreamRetry::default();
    let mut last_rate: Option<f64> = None;
    let mut jitter_seed = u64::from(stream).wrapping_mul(0x9E37_79B9) | 1;
    loop {
        if ctx.stop.load(Ordering::Acquire) {
            return;
        }
        // A network past its data allowance stands aside, ready to rejoin when the
        // allowance resets or is raised.
        if ctx
            .tuning
            .limiter
            .as_ref()
            .is_some_and(|l| l.blocked(&net.name))
        {
            {
                let s = ctx.lock();
                if s.fatal.is_some() || s.all_complete() || s.dead_networks.contains(&net.id) {
                    return; // nothing left to stand aside for
                }
            }
            conn = None;
            tokio::select! {
                _ = ctx.wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(500)) => {}
            }
            continue;
        }
        // A network benched as throttled rests while the others carry the work,
        // except for the one stream running its check (8.2).
        let rest = {
            let mut s = ctx.lock();
            if s.benched.contains(&net.id)
                && s.prober.get(&net.id) != Some(&stream)
                && s.probe_open.remove(&net.id)
            {
                s.prober.insert(net.id, stream);
                last_rate = None; // judge the check on its own, not the crawl before it
            }
            s.resting(&ctx, net.id, stream) && s.fatal.is_none() && !s.all_complete()
        };
        if rest {
            conn = None;
            tokio::select! {
                _ = ctx.wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(500)) => {}
            }
            continue;
        }
        // Pick work (synchronously, under the lock: nothing can yield between choosing and registering).
        let work = {
            let mut s = ctx.lock();
            if let Some(n) = s.nets.get_mut(&net.id)
                && n.retire > 0
            {
                n.retire -= 1; // the controller asked for fewer streams on this network
                return;
            }
            if s.fatal.is_some() || s.all_complete() || s.dead_networks.contains(&net.id) {
                return;
            }
            let idle: Vec<NetId> = s
                .idle
                .iter()
                .filter(|(n, c)| **c > 0 && **n != net.id && !s.dead_networks.contains(n))
                .map(|(n, _)| *n)
                .collect();
            let snap = scheduler::Snapshot {
                now_ms: ctx.now_ms(),
                blocks: &s.blocks,
                idle_networks: &idle,
                disk_behind: false,
                policy: ctx.tuning.hedge,
            };
            let w = scheduler::pick_work(
                &snap,
                Requester {
                    stream,
                    network: net.id,
                    rate: last_rate,
                },
            );
            if let Some(w) = w {
                let now = ctx.now_ms();
                let blk = &mut s.blocks[w.block];
                blk.attempts.push(Attempt {
                    stream,
                    network: net.id,
                    started_ms: now,
                    from: w.from,
                    position: w.from,
                    rate: 0.0,
                });
                if w.hedge {
                    blk.hedges += 1;
                    s.hedges += 1;
                }
                if let Some(c) = s.idle.get_mut(&net.id) {
                    *c = c.saturating_sub(1);
                }
                let in_flight = s
                    .blocks
                    .iter()
                    .flat_map(|b| &b.attempts)
                    .filter(|a| a.network == net.id)
                    .count() as u32;
                let peak = s.peak_streams.entry(net.id).or_default();
                *peak = (*peak).max(in_flight);
            }
            w
        };
        let Some(work) = work else {
            *ctx.lock().idle.entry(net.id).or_default() += 1;
            tokio::select! {
                _ = ctx.wake.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
            if let Some(c) = ctx.lock().idle.get_mut(&net.id) {
                *c = c.saturating_sub(1);
            }
            continue;
        };

        let outcome = fetch_block(&ctx, &net, stream, work, &mut conn).await;

        // Settle the attempt.
        let (block_done, delivered) = {
            let mut s = ctx.lock();
            let blk = &mut s.blocks[work.block];
            let pos = blk
                .attempts
                .iter()
                .find(|a| a.stream == stream)
                .map_or(work.from, |a| a.position);
            blk.attempts.retain(|a| a.stream != stream);
            let delivered = pos > work.from;
            if delivered {
                s.extra[work.block].written.push((work.from, pos));
            }
            s.settle(work.block);
            let blk = &mut s.blocks[work.block];
            if !delivered && outcome.is_err() {
                blk.avoid = Some(net.id);
            } else if delivered {
                blk.avoid = None;
            }
            (blk.complete(), delivered)
        };
        if block_done {
            ctx.wake.notify_waiters();
        }
        match outcome {
            Ok(rate) => {
                retry.progressed();
                last_rate = Some(rate);
                ctx.lock().link_refused.remove(&net.id);
            }
            Err(Outcome::Lost) => {} // another attempt finished this block first
            Err(Outcome::Failed(failure)) => {
                conn = None; // never reuse a connection after a failure
                if delivered {
                    retry.progressed();
                }
                jitter_seed ^= jitter_seed << 13;
                jitter_seed ^= jitter_seed >> 7;
                jitter_seed ^= jitter_seed << 17;
                let jitter = (jitter_seed % 1000) as f64 / 1000.0;
                let silent = !delivered
                    && ctx
                        .lock()
                        .bytes_by_network
                        .get(&net.id)
                        .copied()
                        .unwrap_or(0)
                        == 0;
                let (decision, signals) = retry.decide(
                    &failure,
                    ctx.now_ms(),
                    silent && ctx.live_networks > 1,
                    jitter,
                );
                if let Failure::Lane(t) = &failure {
                    tell_trouble(&ctx, &net, t);
                }
                // A mirror that refuses, or whose file looks different, just stops
                // helping: its lanes retire and the main server carries on (B8.9).
                if net.mirror.is_some()
                    && (signals.link_refused.is_some()
                        || matches!(
                            decision,
                            Decision::FailAndDiscard
                                | Decision::ConfirmBytes
                                | Decision::FailNetwork
                        ))
                {
                    let mut s = ctx.lock();
                    s.retries += 1;
                    s.last_error = format!("{} (mirror): {failure:?}", net.name);
                    s.dead_networks.insert(net.id);
                    drop(s);
                    ctx.wake.notify_waiters();
                    return;
                }
                {
                    let mut s = ctx.lock();
                    s.retries += 1;
                    s.last_error = match &failure {
                        Failure::Lane(t) => t.message.clone(),
                        _ => format!("{}: {failure:?}", net.name),
                    };
                    if signals.refused {
                        s.nets
                            .entry(net.id)
                            .or_default()
                            .refused_tick
                            .insert(stream);
                    }
                    if let Some(code) = signals.link_refused {
                        let entry = s.link_refused.entry(net.id).or_insert((code, 0));
                        *entry = (code, entry.1 + 1);
                        // Expired only when every live network keeps refusing: brief 403s
                        // from rate limiting must not end a download (chaos seed 43). A
                        // network benched as throttled isn't asking, so it can't vote.
                        let dead = s.dead_networks.clone();
                        let resting = s.benched.iter().filter(|n| !dead.contains(n)).count();
                        let persistent = s
                            .link_refused
                            .iter()
                            .filter(|(n, (_, c))| {
                                !dead.contains(n)
                                    && !s.benched.contains(n)
                                    && *c >= LINK_EXPIRED_AFTER
                            })
                            .count()
                            + dead.len()
                            + resting
                            >= ctx.live_networks;
                        if persistent {
                            s.fatal.get_or_insert(JobError::LinkExpired(code));
                        }
                    } else if delivered {
                        s.link_refused.remove(&net.id);
                    }
                }
                match decision {
                    Decision::Retry { delay_ms, .. } => {
                        tokio::time::sleep(scaled(delay_ms, &ctx.tuning)).await
                    }
                    Decision::Unreachable { retry_ms } => {
                        tokio::time::sleep(scaled(retry_ms, &ctx.tuning)).await
                    }
                    Decision::Retire => return,
                    Decision::FailNetwork => {
                        ctx.lock().dead_networks.insert(net.id);
                        return;
                    }
                    Decision::PauseJob(d) => {
                        ctx.lock().fatal.get_or_insert(JobError::Disk(d));
                        ctx.wake.notify_waiters();
                        return;
                    }
                    Decision::FailAndDiscard => {
                        ctx.lock().fatal.get_or_insert(JobError::VersionChanged);
                        ctx.wake.notify_waiters();
                        return;
                    }
                    Decision::ConfirmBytes => {
                        if !confirm_same_bytes(&ctx, &net).await {
                            ctx.lock().fatal.get_or_insert(JobError::VersionChanged);
                            ctx.wake.notify_waiters();
                            return;
                        }
                    }
                }
            }
        }
    }
}

/// Consecutive refusals on every network before a link counts as expired.
const LINK_EXPIRED_AFTER: u32 = 4;

/// Passes a connector's reason on to the listener, once per network and reason.
fn tell_trouble(ctx: &Ctx, net: &Network, t: &LaneTrouble) {
    let first = ctx.lock().told.insert((net.id, t.message.clone()));
    if first && let Some(sink) = &ctx.tuning.lane_events {
        (sink.0)(&LaneEvent::Trouble {
            net: net.id,
            name: net.name.clone(),
            message: t.message.clone(),
            lasting: t.lasting,
        });
    }
}

/// One tick of throttle detection (8.2): feeds the detector each network's
/// bytes and attempts, applies what it decides, and tells the listener.
fn watch_throttling(
    ctx: &Arc<Ctx>,
    networks: &[Network],
    groups: &HashMap<NetId, u32>,
    detector: &mut throttle::Detector,
) {
    let limits = ctx.tuning.limiter.as_ref().map(|l| l.settings());
    let shaped = limits.as_ref().is_some_and(|l| l.global > 0)
        || ctx.tuning.job_limit.as_ref().is_some_and(|j| j.rate() > 0);
    let now = ctx.now_ms();
    let mut events = vec![];
    let wake = {
        let mut s = ctx.lock();
        let mut attempts: HashMap<NetId, u32> = HashMap::new();
        for a in s.blocks.iter().flat_map(|b| &b.attempts) {
            *attempts.entry(a.network).or_default() += 1;
        }
        let samples: Vec<throttle::Sample> = networks
            .iter()
            .map(|n| throttle::Sample {
                net: n.id,
                group: groups.get(&n.id).copied().unwrap_or(0),
                bytes: s.raw_bytes.get(&n.id).copied().unwrap_or(0),
                attempts: attempts.get(&n.id).copied().unwrap_or(0),
                limited: limits
                    .as_ref()
                    .is_some_and(|l| l.networks.iter().any(|(name, r)| *name == n.name && *r > 0)),
                out: s.dead_networks.contains(&n.id)
                    || ctx
                        .tuning
                        .limiter
                        .as_ref()
                        .is_some_and(|l| l.blocked(&n.name)),
            })
            .collect();
        let total = s.plan.total;
        let name = |id: NetId| {
            networks
                .iter()
                .find(|n| n.id == id)
                .map(|n| n.name.clone())
                .unwrap_or_default()
        };
        for change in detector.tick(now, &samples, total, shaped) {
            match change {
                throttle::Change::Bench { net, rate, best } => {
                    s.benched.insert(net);
                    s.probe_open.remove(&net);
                    s.prober.remove(&net);
                    events.push(LaneEvent::Throttled {
                        net,
                        name: name(net),
                        rate,
                        best,
                    });
                }
                throttle::Change::Probe { net } => {
                    s.probe_open.insert(net);
                }
                throttle::Change::Restore { net, rate } => {
                    s.benched.remove(&net);
                    s.probe_open.remove(&net);
                    s.prober.remove(&net);
                    events.push(LaneEvent::Restored {
                        net,
                        name: name(net),
                        rate,
                    });
                }
                throttle::Change::Rest { net, .. } => {
                    // Still slow (or nothing to measure): the check's stream rests too.
                    s.probe_open.remove(&net);
                    s.prober.remove(&net);
                }
            }
        }
        // Wake resting streams: one may take a check, or all may go back to work.
        !events.is_empty() || !s.probe_open.is_empty()
    };
    if wake {
        ctx.wake.notify_waiters();
    }
    if let Some(sink) = &ctx.tuning.lane_events {
        for e in &events {
            (sink.0)(e);
        }
    }
}

fn scaled(ms: u64, t: &Tuning) -> Duration {
    Duration::from_millis((ms as f64 * t.retry_delay_scale.clamp(0.0, 1.0)) as u64)
}

/// Reads the server's current ETag/Last-Modified with a 1-byte request and makes it
/// the accepted validator. False when the server can't be asked.
async fn adopt_current_validator(ctx: &Arc<Ctx>, net: &Network) -> bool {
    let Ok(mut c) = connect(net, &ctx.src, &ctx.tuning).await else {
        return false;
    };
    let Ok(res) = send(
        &mut c,
        request(&ctx.src, &ctx.tuning, Some((0, Some(0))), None),
        &ctx.tuning,
    )
    .await
    else {
        return false;
    };
    if !matches!(res.status().as_u16(), 200 | 206) {
        return false;
    }
    let raw = header(&res, ETAG)
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty());
    let lm = header(&res, LAST_MODIFIED);
    let mut s = ctx.lock();
    s.accepted_etag = raw.as_deref().map(headers::normalize_etag);
    s.if_range = raw.filter(|e| !is_weak(e)).or(lm);
    true
}

/// fsyncs the staging file, then hands the durable progress to the checkpoint sink (L-55).
async fn emit_checkpoint(ctx: &Arc<Ctx>) {
    let Some(sink) = ctx.tuning.checkpoint.clone() else {
        return;
    };
    // Read progress *before* syncing: everything counted is then covered by the fsync.
    let cp = {
        let s = ctx.lock();
        Checkpoint {
            staging_path: ctx.staging_path.clone(),
            filename: ctx.filename.clone(),
            total: s.plan.total,
            block_size: s.plan.block_size,
            // Count what live attempts have already written too: those bytes are on disk and
            // covered by the fsync below, so a crash loses at most the unsynced tail.
            secured: s
                .blocks
                .iter()
                .zip(&s.extra)
                .map(|(b, ex)| {
                    let mut intervals: Vec<(u64, u64)> = ex.written.clone();
                    intervals.extend(b.attempts.iter().map(|a| (a.from, a.position)));
                    let mut secured = b.secured;
                    let mut moved = true;
                    while moved {
                        moved = false;
                        for &(from, to) in &intervals {
                            if from <= secured && to > secured {
                                secured = to.min(b.len);
                                moved = true;
                            }
                        }
                    }
                    secured
                })
                .collect(),
            raw_etag: s.if_range.clone().filter(|v| v.starts_with('"')),
            last_modified: ctx.last_modified.clone(),
        }
    };
    let Ok(file) = ctx.file.try_clone() else {
        return;
    };
    if tokio::task::spawn_blocking(move || file.sync_data())
        .await
        .ok()
        .and_then(Result::ok)
        .is_some()
    {
        (sink.0)(&cp);
    }
}

/// Fails early with the numbers when the disk can't hold `needed` more bytes.
fn check_space(dir: &Path, needed: u64) -> Result<(), JobError> {
    let free = fuselane_storage::free::free_space(dir).ok();
    if fuselane_storage::free::fits(free, needed) {
        Ok(())
    } else {
        Err(JobError::NoSpace {
            needed,
            free: free.unwrap_or(0),
        })
    }
}

/// Builds the UI's view of the job (cheap: one pass over blocks, grouped into ticks).
fn make_snapshot(ctx: &Arc<Ctx>, networks: &[Network]) -> Snapshot {
    let now = ctx.now_ms();
    let mut s = ctx.lock();
    let n_blocks = s.blocks.len().max(1);
    let n_ticks = n_blocks.min(TICKS);
    let mut ticks = vec![
        TickSnapshot {
            fill: 0.0,
            owner: None,
            in_flight: None
        };
        n_ticks
    ];
    let mut tick_len = vec![0u64; n_ticks];
    let mut tick_secured = vec![0u64; n_ticks];
    let mut tick_owner: Vec<HashMap<NetId, u64>> = vec![HashMap::new(); n_ticks];
    for (i, b) in s.blocks.iter().enumerate() {
        let t = i * n_ticks / n_blocks;
        tick_len[t] += b.len;
        let written = b
            .attempts
            .iter()
            .map(|a| a.position)
            .fold(b.secured, u64::max)
            .min(b.len);
        tick_secured[t] += written;
        if let Some(a) = b.attempts.first() {
            ticks[t].in_flight = Some(a.network);
        }
        for (net, bytes) in s.block_net_bytes.get(i).into_iter().flatten() {
            *tick_owner[t].entry(*net).or_default() += bytes;
        }
    }
    for (t, tick) in ticks.iter_mut().enumerate() {
        tick.fill = if tick_len[t] == 0 {
            0.0
        } else {
            (tick_secured[t] as f64 / tick_len[t] as f64) as f32
        };
        tick.owner = tick_owner[t]
            .iter()
            .max_by_key(|(_, b)| **b)
            .map(|(n, _)| *n);
    }
    let dead = s.dead_networks.clone();
    let bytes = s.bytes_by_network.clone();
    let streams: HashMap<NetId, u32> = s.nets.iter().map(|(n, st)| (*n, st.live)).collect();
    let nets = networks
        .iter()
        .map(|n| NetSnapshot {
            id: n.id,
            bytes: bytes.get(&n.id).copied().unwrap_or(0),
            rate: s.meters.get_mut(&n.id).map_or(0.0, |m| m.rate(now)),
            streams: streams.get(&n.id).copied().unwrap_or(0),
            dead: dead.contains(&n.id),
        })
        .collect();
    Snapshot {
        written: ctx.written_total.load(Ordering::Relaxed),
        total: s.plan.total,
        rate: s.total_meter.rate(now),
        networks: nets,
        ticks,
        retries: s.retries,
        hedges: s.hedges,
    }
}

fn sha256_of(path: &Path) -> std::io::Result<[u8; 32]> {
    use sha2::Digest;
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

/// Compares up to `resume_samples` windows of secured bytes with the server.
/// Returns false if any differs; true if they match or nothing is secured.
async fn verify_resumed(ctx: &Arc<Ctx>, net: &Network) -> bool {
    let windows: Vec<(u64, u64)> = {
        let s = ctx.lock();
        let secured: Vec<(u64, u64)> = s
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.secured > 0)
            .filter_map(|(i, b)| s.plan.block(i as u64).map(|(start, _)| (start, b.secured)))
            .collect();
        if secured.is_empty() {
            return true;
        }
        let n = ctx.tuning.resume_samples.max(1) as usize;
        let step = (secured.len() / n).max(1);
        secured
            .iter()
            .step_by(step)
            .take(n)
            .map(|&(start, len)| (start, len.min(ctx.tuning.confirm_sample)))
            .collect()
    };
    let Ok(mut c) = connect(net, &ctx.src, &ctx.tuning).await else {
        return true;
    }; // can't check now: the per-response checks still apply
    for (start, len) in windows {
        let Ok(res) = send(
            &mut c,
            request(
                &ctx.src,
                &ctx.tuning,
                Some((start, Some(start + len - 1))),
                None,
            ),
            &ctx.tuning,
        )
        .await
        else {
            return true;
        };
        if res.status().as_u16() != 206 {
            return true;
        }
        let Ok(body) = res.into_body().collect().await.map(|b| b.to_bytes()) else {
            return true;
        };
        let mut disk = vec![0u8; body.len()];
        let Ok(file) = ctx.file.try_clone() else {
            return true;
        };
        let read =
            tokio::task::spawn_blocking(move || read_at(&file, start, &mut disk).map(|()| disk))
                .await;
        match read {
            Ok(Ok(disk)) if disk == body.as_ref() => {}
            Ok(Ok(_)) => return false,
            _ => return true,
        }
    }
    true
}

fn is_weak(etag: &str) -> bool {
    etag.starts_with("W/") || etag.starts_with("w/")
}

enum Outcome {
    /// Another attempt completed the block; this one stopped early.
    Lost,
    Failed(Failure),
}

/// Fetches `[work.from, len)` of a block; returns the attempt's rate (bytes/s).
async fn fetch_block(
    ctx: &Arc<Ctx>,
    net: &Network,
    stream: StreamId,
    work: scheduler::Work,
    conn: &mut Option<Conn>,
) -> Result<f64, Outcome> {
    let (block_start, block_len, total, etag, if_range) = {
        let s = ctx.lock();
        let (start, len) = s.plan.block(work.block as u64).unwrap_or((0, 0));
        (
            start,
            len,
            s.plan.total,
            s.accepted_etag.clone(),
            s.if_range.clone(),
        )
    };
    // A mirror lane asks its own server with that server's validators.
    let (src, etag, if_range) = match &net.mirror {
        Some(m) => (&m.source, m.etag.clone(), m.if_range.clone()),
        None => (&ctx.src, etag, if_range),
    };
    if conn.is_none() {
        *conn = Some(
            connect(net, src, &ctx.tuning)
                .await
                .map_err(Outcome::Failed)?,
        );
    }
    let Some(c) = conn.as_mut() else {
        return Err(Outcome::Failed(Failure::Connection));
    };

    let unknown = total.is_none();
    let want_first = block_start + work.from;
    let want_last = if unknown {
        u64::MAX
    } else {
        block_start + block_len - 1
    };
    let range = if unknown && want_first == 0 {
        None
    } else {
        Some((want_first, (!unknown).then_some(want_last)))
    };
    let res = send(
        c,
        request(src, &ctx.tuning, range, if_range.as_deref()),
        &ctx.tuning,
    )
    .await
    .map_err(Outcome::Failed)?;
    let status = res.status().as_u16();

    if !(status == 200 || status == 206) {
        let retry_after_ms =
            header(&res, RETRY_AFTER).and_then(|v| crate::retry::parse_retry_after(&v, now_unix()));
        return Err(Outcome::Failed(Failure::Status {
            code: status,
            retry_after_ms,
        }));
    }
    // Version check on every response, before writing anything (L-05).
    let got_etag = header(&res, ETAG)
        .map(|e| headers::normalize_etag(&e))
        .filter(|e| !e.is_empty());
    // A 200 to a request carrying If-Range means the validator no longer matches
    // (RFC 9110 §13.1.5): the file may have changed. Size proves it; otherwise sample.
    if status == 200 && want_first > 0 && if_range.is_some() {
        let length = header(&res, CONTENT_LENGTH).and_then(|v| v.parse::<u64>().ok());
        *conn = None;
        let size_changed = matches!((length, total), (Some(l), Some(t)) if l != t);
        return Err(Outcome::Failed(Failure::VersionChanged { size_changed }));
    }
    let accepted = headers::check_range_response(
        want_first,
        want_last,
        total,
        status,
        header(&res, CONTENT_RANGE).as_deref(),
        header(&res, CONTENT_LENGTH).and_then(|v| v.parse().ok()),
    )
    .map_err(|e| match e {
        RangeError::SizeChanged { .. } => {
            Outcome::Failed(Failure::VersionChanged { size_changed: true })
        }
        other => Outcome::Failed(Failure::BadResponse(other)),
    })?;
    if let (Some(want), Some(got)) = (&etag, &got_etag)
        && want != got
    {
        *conn = None;
        return Err(Outcome::Failed(Failure::VersionChanged {
            size_changed: false,
        }));
    }
    if unknown && status == 200 && want_first > 0 {
        return Err(Outcome::Failed(Failure::BadResponse(
            RangeError::RangeIgnored,
        )));
    }

    // Stream the body to disk at fixed offsets.
    let expected = if unknown {
        None
    } else {
        Some(accepted.last - accepted.first + 1)
    };
    let mut body = res.into_body();
    let mut received: u64 = 0;
    let started = Instant::now();
    loop {
        let lost = {
            let s = ctx.lock();
            s.blocks[work.block].complete()
                || s.fatal.is_some()
                || ctx.stop.load(Ordering::Acquire)
                // Benched as throttled: hand the rest of the block to the others (8.2).
                || s.resting(ctx, net.id, stream)
        };
        if lost {
            *conn = None;
            return Err(Outcome::Lost);
        }
        let frame = match tokio::time::timeout(ctx.tuning.idle_timeout, body.frame()).await {
            Err(_) => return Err(Outcome::Failed(Failure::Connection)), // silent: stall watchdog (L-09)
            Ok(None) => break,
            Ok(Some(Err(_))) => return Err(Outcome::Failed(Failure::Connection)),
            Ok(Some(Ok(f))) => f,
        };
        let Ok(mut data) = frame.into_data() else {
            continue;
        };
        if let Some(exp) = expected
            && received + data.len() as u64 > exp
        {
            // Overlong body: keep only what belongs to the range, then fail (L-02).
            data.truncate((exp - received) as usize);
            write_chunk(ctx, accepted.first + received, &data)
                .await
                .map_err(Outcome::Failed)?;
            advance(
                ctx,
                work.block,
                stream,
                net.id,
                work.from + received + data.len() as u64,
                data.len() as u64,
            );
            return Err(Outcome::Failed(Failure::BadResponse(RangeError::Overrun {
                want: exp,
                got: received + data.len() as u64,
            })));
        }
        write_chunk(ctx, accepted.first + received, &data)
            .await
            .map_err(Outcome::Failed)?;
        received += data.len() as u64;
        advance(
            ctx,
            work.block,
            stream,
            net.id,
            work.from + received,
            data.len() as u64,
        );
        // Speed limits: wait off the debt before reading more (TCP slows the server).
        let mut wait = std::time::Duration::ZERO;
        if let Some(limiter) = &ctx.tuning.limiter {
            wait = limiter.take(&net.name, data.len() as u64);
            // Allowance reached mid-block: hand the rest back to the other networks.
            if limiter.blocked(&net.name) {
                *conn = None;
                return Err(Outcome::Lost);
            }
        }
        if let Some(job) = &ctx.tuning.job_limit {
            wait = wait.max(job.take(data.len() as u64));
        }
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
    }
    if let Some(exp) = expected
        && received < exp
    {
        // A capped range (honestly labelled) is fine: what's written is kept and the rest refetched.
        let capped = accepted.last < want_last;
        if !capped {
            *conn = None;
            return Err(Outcome::Failed(Failure::BadResponse(
                RangeError::LengthMismatch {
                    first: accepted.first,
                    last: accepted.last,
                    length: received,
                },
            )));
        }
    }
    if unknown {
        let mut s = ctx.lock();
        s.unknown_total = Some(received);
        s.blocks[0].len = received;
    }
    Ok(received as f64 / started.elapsed().as_secs_f64().max(1e-3))
}

fn advance(ctx: &Ctx, block: usize, stream: StreamId, net: NetId, position: u64, new_bytes: u64) {
    let mut s = ctx.lock();
    let now = ctx.now_ms();
    // Only bytes past the block's furthest written point are new. A hedge rewrites
    // what the attempt it races already wrote: that network still did the work (its
    // own speed counts it) but the file gains nothing, so it isn't credited (L-117).
    // The furthest point ever written survives failed attempts, whose bytes stay on
    // disk while a retry rewrites them from the last secured point.
    let frontier = s
        .written_max
        .get(block)
        .copied()
        .unwrap_or(0)
        .max(s.blocks[block].secured);
    let useful = position.saturating_sub(frontier).min(new_bytes);
    if let Some(m) = s.written_max.get_mut(block) {
        *m = (*m).max(position);
    }
    s.wasted_bytes += new_bytes - useful;
    *s.raw_bytes.entry(net).or_default() += new_bytes;
    s.meters.entry(net).or_default().add(new_bytes, now);
    s.total_meter.add(useful, now);
    if let Some(row) = s.block_net_bytes.get_mut(block) {
        match row.iter_mut().find(|(n, _)| *n == net) {
            Some((_, b)) => *b += useful,
            None => row.push((net, useful)),
        }
    }
    let stats = s.nets.entry(net).or_default();
    stats.served_tick.insert(stream);
    stats.answered.insert(stream);
    if let Some(a) = s.blocks[block]
        .attempts
        .iter_mut()
        .find(|a| a.stream == stream)
    {
        a.position = a.position.max(position);
    }
    *s.bytes_by_network.entry(net).or_default() += useful;
    let total = s.plan.total;
    drop(s);
    let written = ctx.written_total.fetch_add(useful, Ordering::Relaxed) + useful;
    if let Some(p) = &ctx.tuning.progress {
        (p.0)(written, total);
    }
}

async fn write_chunk(ctx: &Arc<Ctx>, offset: u64, data: &Bytes) -> Result<(), Failure> {
    let file = ctx
        .file
        .try_clone()
        .map_err(|e| Failure::Disk(disk_kind(&e)))?;
    let data = data.clone();
    tokio::task::spawn_blocking(move || Staging::write_at(&file, offset, &data))
        .await
        .map_err(|_| Failure::Disk(DiskFailure::Io))?
        .map_err(|e| Failure::Disk(disk_kind(&e)))
}

fn disk_kind(e: &std::io::Error) -> DiskFailure {
    match e.kind() {
        std::io::ErrorKind::StorageFull => DiskFailure::NoSpace,
        std::io::ErrorKind::QuotaExceeded => DiskFailure::QuotaExceeded,
        std::io::ErrorKind::ReadOnlyFilesystem => DiskFailure::ReadOnly,
        std::io::ErrorKind::PermissionDenied => DiskFailure::PermissionDenied,
        std::io::ErrorKind::NotFound => DiskFailure::DriveMissing,
        _ => DiskFailure::Io,
    }
}

/// A validator changed: refetch a sample of bytes already on disk and compare (L-05).
/// Same bytes → accept the new label; different → it's a different file.
async fn confirm_same_bytes(ctx: &Arc<Ctx>, net: &Network) -> bool {
    let (secured_at, len) = {
        let s = ctx.lock();
        let found = s
            .blocks
            .iter()
            .enumerate()
            .find(|(_, b)| b.secured > 0)
            .map(|(i, b)| (s.plan.block(i as u64).map_or(0, |(st, _)| st), b.secured));
        match found {
            Some((start, secured)) => (start, secured.min(ctx.tuning.confirm_sample)),
            None => (0, 0),
        }
    };
    if len == 0 {
        // Nothing on disk yet, so nothing can be mixed up: adopt the server's current
        // validator. (Returning without adopting it made every If-Range mismatch loop.)
        return adopt_current_validator(ctx, net).await;
    }
    let Ok(mut c) = connect(net, &ctx.src, &ctx.tuning).await else {
        return false;
    };
    let Ok(res) = send(
        &mut c,
        request(
            &ctx.src,
            &ctx.tuning,
            Some((secured_at, Some(secured_at + len - 1))),
            None,
        ),
        &ctx.tuning,
    )
    .await
    else {
        return false;
    };
    if res.status().as_u16() != 206 {
        return false;
    }
    let new_raw = header(&res, ETAG).map(|e| e.trim().to_string());
    let new_etag = new_raw.as_deref().map(headers::normalize_etag);
    let Ok(bytes) = res.into_body().collect().await.map(|b| b.to_bytes()) else {
        return false;
    };
    let mut on_disk = vec![0u8; bytes.len()];
    let file = match ctx.file.try_clone() {
        Ok(f) => f,
        Err(_) => return false,
    };
    let read = tokio::task::spawn_blocking(move || {
        read_at(&file, secured_at, &mut on_disk).map(|()| on_disk)
    })
    .await;
    match read {
        Ok(Ok(disk)) if disk == bytes.as_ref() && bytes.len() as u64 == len => {
            let mut s = ctx.lock();
            s.accepted_etag = new_etag;
            s.if_range = new_raw
                .filter(|e| !is_weak(e))
                .or(s.if_range.take().filter(|v| !v.starts_with('"')));
            true
        }
        _ => false,
    }
}

#[cfg(unix)]
fn read_at(file: &std::fs::File, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.read_exact_at(buf, offset)
}

#[cfg(windows)]
fn read_at(file: &std::fs::File, mut offset: u64, mut buf: &mut [u8]) -> std::io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        let n = file.seek_read(buf, offset)?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        buf = &mut buf[n..];
        offset += n as u64;
    }
    Ok(())
}

// ---------- the job ----------

/// Downloads `src` over `networks` into `dir`.
pub async fn download(
    src: Source,
    networks: Vec<Network>,
    dir: &Path,
    tuning: Tuning,
) -> Result<Report, JobError> {
    download_with(src, networks, dir, tuning, None).await
}

/// Like [`download`], continuing from a saved [`Resume`] when given.
pub async fn download_with(
    src: Source,
    networks: Vec<Network>,
    dir: &Path,
    tuning: Tuning,
    resume: Option<Resume>,
) -> Result<Report, JobError> {
    if networks.is_empty() {
        return Err(JobError::Unreachable("no networks selected".into()));
    }
    let probe = probe(&src, &networks, &tuning).await?;
    let splittable = probe.ranges && probe.total.is_some_and(|t| t > 0);
    let mut plan = Plan::new(probe.total, splittable, networks.len() as u32);
    if let (Some(bs), Some(total), true) = (tuning.block_size, probe.total, splittable) {
        let bs = bs.max(1);
        plan = Plan {
            total: Some(total),
            block_size: bs,
            blocks: total.div_ceil(bs),
        };
    }
    // Resume: the server must still have the same file (size proof), and ranges (L-42, L-53).
    let mut resumed_secured: Option<Vec<u64>> = None;
    let mut resumed_validators: Option<(Option<String>, Option<String>)> = None;
    // Room for the whole file before creating it (resumes check what's left, below).
    if resume.is_none()
        && let Some(total) = probe.total
    {
        check_space(dir, total)?;
    }
    let staging = match &resume {
        Some(r) => {
            if probe.total != Some(r.total) {
                let _ = std::fs::remove_file(&r.staging_path); // a different file now
                return Err(JobError::VersionChanged);
            }
            if !splittable {
                return Err(JobError::NotResumable(
                    "the server no longer supports resuming".into(),
                ));
            }
            let staging = match Staging::reopen(&r.staging_path, Some(r.total)) {
                Ok(s) => s,
                Err(StagingError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(JobError::NotResumable("the partial file is missing".into()));
                }
                Err(e) => return Err(e.into()),
            };
            plan = Plan {
                total: Some(r.total),
                block_size: r.block_size.max(1),
                blocks: r.total.div_ceil(r.block_size.max(1)),
            };
            // Reconcile with what's really on disk: never trust progress past the file's length.
            let on_disk = std::fs::metadata(&r.staging_path)
                .map(|m| m.len())
                .unwrap_or(0);
            if on_disk < r.total {
                staging
                    .handle()
                    .and_then(|f| f.set_len(r.total))
                    .map_err(StagingError::from)?;
            }
            let valid = r.secured.len() as u64 == plan.blocks;
            resumed_secured = Some(
                (0..plan.blocks)
                    .map(|i| {
                        let (start, len) = plan.block(i).unwrap_or((0, 0));
                        let saved = if valid { r.secured[i as usize] } else { 0 };
                        saved.min(len).min(on_disk.saturating_sub(start))
                    })
                    .collect(),
            );
            resumed_validators = Some((r.raw_etag.clone(), r.last_modified.clone()));
            let have: u64 = resumed_secured.iter().flatten().sum();
            check_space(dir, r.total.saturating_sub(have))?;
            staging
        }
        None => {
            // A chosen name wins, made safe like a server's; an unusable one falls back.
            let chosen = tuning
                .filename
                .as_deref()
                .map(fuselane_storage::names::sanitize)
                .filter(|n| !n.is_empty() && n != "download");
            Staging::create(
                dir,
                chosen.as_deref().unwrap_or(&probe.filename),
                probe.total,
            )?
        }
    };
    let file = staging.handle().map_err(StagingError::from)?;

    if probe.total == Some(0) {
        let path = staging.publish(0)?;
        return Ok(Report {
            path,
            total: 0,
            bytes_by_network: HashMap::new(),
            retries: 0,
            hedges: 0,
            peak_streams: HashMap::new(),
        });
    }

    let blocks: Vec<Block> = (0..plan.blocks)
        .map(|i| Block {
            len: plan.block(i).map_or(0, |(_, l)| l),
            secured: resumed_secured
                .as_ref()
                .and_then(|v| v.get(i as usize).copied())
                .unwrap_or(0),
            attempts: vec![],
            avoid: None,
            hedges: 0,
        })
        .collect();
    let n_blocks = blocks.len();
    // Non-splittable downloads run one stream on the first network (L-84 in Plexo's list).
    let networks: Vec<Network> = if splittable {
        networks
    } else {
        networks.into_iter().take(1).collect()
    };
    let ctx = Arc::new(Ctx {
        src,
        tuning: tuning.clone(),
        shared: Mutex::new(Shared {
            plan,
            blocks,
            extra: (0..n_blocks).map(|_| BlockExtra::default()).collect(),
            idle: HashMap::new(),
            dead_networks: HashSet::new(),
            link_refused: HashMap::new(),
            // On resume, keep the *saved* validators: if the server's differ, the first
            // segment triggers the byte-sampling check (L-05).
            accepted_etag: match &resumed_validators {
                Some((raw, _)) => raw.as_deref().map(headers::normalize_etag),
                None => probe.etag.clone(),
            },
            if_range: match &resumed_validators {
                Some((raw, lm)) => raw.clone().filter(|e| !is_weak(e)).or(lm.clone()),
                None => probe
                    .raw_etag
                    .clone()
                    .filter(|e| !is_weak(e))
                    .or(probe.last_modified.clone()),
            },
            bytes_by_network: HashMap::new(),
            fatal: None,
            last_error: String::new(),
            retries: 0,
            hedges: 0,
            unknown_total: None,
            nets: HashMap::new(),
            peak_streams: HashMap::new(),
            wasted_bytes: 0,
            written_max: vec![0; n_blocks],
            block_net_bytes: vec![Vec::new(); n_blocks],
            meters: HashMap::new(),
            total_meter: crate::measure::Meter::default(),
            raw_bytes: HashMap::new(),
            benched: HashSet::new(),
            probe_open: HashSet::new(),
            prober: HashMap::new(),
            told: HashSet::new(),
        }),
        file,
        wake: tokio::sync::Notify::new(),
        stop: AtomicBool::new(false),
        live_networks: networks.len(),
        epoch: Instant::now(),
        written_total: AtomicU64::new(0),
        staging_path: staging.path().to_path_buf(),
        filename: staging.name().to_string(),
        last_modified: resumed_validators
            .as_ref()
            .map_or(probe.last_modified.clone(), |(_, lm)| lm.clone()),
        lanes: networks.iter().map(|n| (n.id, n.name.clone())).collect(),
    });

    // A resumed download re-checks a few windows of what the checkpoint calls secured.
    // If any differs from the server, the checkpoint lied (corrupt row, edited file):
    // distrust all of it and fetch everything again (L-53).
    if resume.is_some() && !verify_resumed(&ctx, &networks[0]).await {
        let saved = resumed_validators
            .as_ref()
            .and_then(|(raw, _)| raw.as_deref().map(headers::normalize_etag));
        if saved.is_some() && saved != probe.etag {
            // New label *and* different bytes: the file on the server changed.
            drop(staging.discard());
            return Err(JobError::VersionChanged);
        }
        // Same label, different bytes: the checkpoint (or the disk) lied. Start over.
        let mut s = ctx.lock();
        for b in &mut s.blocks {
            b.secured = 0;
        }
    }

    // Interleave stream starts across networks (L-23).
    let mut controller =
        crate::concurrency::Controller::new(if tuning.auto_streams && splittable {
            None
        } else {
            Some(if splittable {
                tuning.streams_per_network
            } else {
                1
            })
        });
    let mut next_id: HashMap<NetId, u32> = HashMap::new();
    let mut new_id = |net: NetId, index: usize| {
        let k = next_id.entry(net).or_insert(0);
        *k += 1;
        (index as u32) * 10_000 + *k
    };
    let groups: Vec<Vec<(Network, StreamId)>> = networks
        .iter()
        .enumerate()
        .map(|(ni, n)| {
            let start = if tuning.auto_streams && splittable {
                controller.starting(n.id)
            } else {
                controller.limit(n.id)
            };
            (0..start).map(|_| (n.clone(), new_id(n.id, ni))).collect()
        })
        .collect();
    let mut tasks = tokio::task::JoinSet::new();
    for (net, id) in crate::plan::interleave(&groups) {
        tasks.spawn(run_stream(ctx.clone(), net, id));
    }

    // The supervisor: tick the controller until every stream has finished.
    let mut last_checkpoint = Instant::now();
    let mut snap_ticker =
        tokio::time::interval(tuning.snapshot_every.max(Duration::from_millis(16)));
    snap_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let cancel = tuning.cancel.clone().unwrap_or_default();
    let mut ticker = tokio::time::interval(tuning.controller_tick);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut detector = throttle::Detector::new(tuning.throttle);
    // Lanes to the same server share a group: mirrors are only compared with
    // lanes to the same mirror.
    let groups: HashMap<NetId, u32> = networks
        .iter()
        .map(|n| {
            let group = n.mirror.as_ref().map_or(0, |m| {
                networks
                    .iter()
                    .filter_map(|o| o.mirror.as_ref())
                    .position(|o| Arc::ptr_eq(o, m))
                    .map_or(0, |i| i as u32 + 1)
            });
            (n.id, group)
        })
        .collect();
    loop {
        tokio::select! {
            joined = tasks.join_next() => {
                if joined.is_none() { break; }
            }
            _ = snap_ticker.tick(), if tuning.snapshot.is_some() => {
                if let Some(sink) = &tuning.snapshot {
                    let snap = make_snapshot(&ctx, &networks);
                    (sink.0)(&snap);
                }
            }
            _ = cancel.notify.notified() => {
                ctx.stop.store(true, Ordering::Release);
                ctx.wake.notify_waiters();
            }
            _ = ticker.tick() => {
                if cancel.is_cancelled() {
                    ctx.stop.store(true, Ordering::Release);
                    ctx.wake.notify_waiters();
                }
                if last_checkpoint.elapsed() >= tuning.checkpoint_every {
                    last_checkpoint = Instant::now();
                    emit_checkpoint(&ctx).await;
                }
                let (ticks, spare) = {
                    let mut s = ctx.lock();
                    let spare = s.blocks.iter().filter(|b| !b.complete() && b.attempts.is_empty()).count() as u32;
                    let dead = s.dead_networks.clone();
                    let ticks: Vec<crate::concurrency::NetTick> = networks
                        .iter()
                        .filter(|n| !dead.contains(&n.id))
                        .map(|n| {
                            let st = s.nets.entry(n.id).or_default();
                            let streams = st.live.saturating_sub(st.retire);
                            let tick = crate::concurrency::NetTick {
                                id: n.id,
                                streams,
                                answered: (st.answered.len() as u32).min(streams),
                                refused: st.refused_tick.len() as u32,
                                served: st.served_tick.len() as u32,
                            };
                            st.served_tick.clear();
                            st.refused_tick.clear();
                            tick
                        })
                        .collect();
                    (ticks, spare)
                };
                watch_throttling(&ctx, &networks, &groups, &mut detector);
                if ticks.is_empty() { continue; }
                let now = ctx.now_ms();
                for action in controller.tick(now, &ticks, spare, crate::concurrency::Disk::Unknown) {
                    match action {
                        crate::concurrency::Action::Add { net, count } => {
                            let Some((index, network)) = networks.iter().enumerate().find(|(_, n)| n.id == net) else { continue };
                            for _ in 0..count {
                                tasks.spawn(run_stream(ctx.clone(), network.clone(), new_id(net, index)));
                            }
                        }
                        crate::concurrency::Action::Retire { net, count } => {
                            ctx.lock().nets.entry(net).or_default().retire += count;
                        }
                    }
                }
            }
        }
    }
    ctx.stop.store(true, Ordering::Release);
    // One last snapshot so the UI's final picture (who fetched which part, each
    // network's share) includes the bytes after the last periodic one.
    if let Some(sink) = &tuning.snapshot {
        (sink.0)(&make_snapshot(&ctx, &networks));
    }

    let (fatal, complete, report_bytes, retries, hedges, last_error, unknown_total, peak_streams) = {
        let mut s = ctx.lock();
        (
            s.fatal.take(),
            s.all_complete(),
            s.bytes_by_network.clone(),
            s.retries,
            s.hedges,
            s.last_error.clone(),
            s.unknown_total,
            s.peak_streams.clone(),
        )
    };
    if let Some(e) = fatal {
        if matches!(e, JobError::VersionChanged) {
            let _ = staging.discard(); // a different file: the partial data is useless (L-05)
        } else {
            emit_checkpoint(&ctx).await; // keep what we have for a later resume
        }
        return Err(e);
    }
    if !complete {
        emit_checkpoint(&ctx).await;
        if cancel.is_cancelled() {
            return Err(JobError::Paused);
        }
        return Err(JobError::AllNetworksFailed(last_error));
    }
    let total = probe.total.or(unknown_total).unwrap_or(0);
    if let Some(expected) = tuning.expected_sha256 {
        let path = ctx.staging_path.clone();
        let actual = tokio::task::spawn_blocking(move || sha256_of(&path))
            .await
            .map_err(|_| JobError::Disk(DiskFailure::Io))?
            .map_err(|_| JobError::Disk(DiskFailure::Io))?;
        if actual != expected {
            return Err(JobError::ChecksumMismatch { expected, actual }); // staging kept for inspection
        }
    }
    let path = staging.publish(total)?;
    Ok(Report {
        path,
        total,
        bytes_by_network: report_bytes,
        retries,
        hedges,
        peak_streams,
    })
}
