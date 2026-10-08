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
    ACCEPT_ENCODING, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, ETAG, HOST, IF_RANGE,
    LAST_MODIFIED, RANGE, RETRY_AFTER, USER_AGENT,
};
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;

use fuselane_storage::staging::{Staging, StagingError};

use crate::headers::{self, ContentRange, RangeError};
use crate::plan::Plan;
use crate::retry::{Decision, DiskFailure, Failure, StreamRetry};
use crate::scheduler::{self, Attempt, Block, HedgePolicy, NetId, Requester, StreamId};

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
    #[error("this link stopped working (status {0}); paste a fresh link to continue")]
    LinkExpired(u16),
    #[error("the file on the server changed since the download started")]
    VersionChanged,
    #[error("the disk failed: {0:?}")]
    Disk(DiskFailure),
    #[error("every network failed; last error: {0}")]
    AllNetworksFailed(String),
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
}

/// What the probe learned.
#[derive(Debug, Clone)]
pub struct Probe {
    pub total: Option<u64>,
    pub ranges: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub filename: String,
}

// ---------- one HTTP exchange ----------

struct Conn {
    sender: SendRequest<Empty<Bytes>>,
}

async fn connect(net: &Network, src: &Source, t: &Tuning) -> Result<Conn, Failure> {
    let io = tokio::time::timeout(t.connect_timeout, (net.connect)(src.addr))
        .await
        .map_err(|_| Failure::Connection)?
        .map_err(|_| Failure::Connection)?;
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
        .header(USER_AGENT, &t.user_agent)
        .header(ACCEPT_ENCODING, "identity");
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

/// Probes with `Range: bytes=0-0` on the first network that answers (L-01, L-04).
pub async fn probe(src: &Source, networks: &[Network], t: &Tuning) -> Result<Probe, JobError> {
    let mut last = String::from("no networks");
    for net in networks {
        let mut conn = match connect(net, src, t).await {
            Ok(c) => c,
            Err(_) => {
                last = format!("{} couldn't connect", net.name);
                continue;
            }
        };
        let res = match send(&mut conn, request(src, t, Some((0, Some(0))), None), t).await {
            Ok(r) => r,
            Err(_) => {
                last = format!("{} got no answer", net.name);
                continue;
            }
        };
        let status = res.status().as_u16();
        let etag = header(&res, ETAG)
            .map(|e| headers::normalize_etag(&e))
            .filter(|e| !e.is_empty());
        let last_modified = header(&res, LAST_MODIFIED);
        let disposition = header(&res, CONTENT_DISPOSITION)
            .and_then(|v| headers::content_disposition_filename(&v));
        let from_path = headers::filename_from_path(src.path.split('?').next().unwrap_or(""));
        let filename = disposition
            .or(from_path)
            .unwrap_or_else(|| "download".into());
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
            last_modified,
            filename,
        });
    }
    Err(JobError::Unreachable(last))
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
    accepted_etag: Option<String>,
    bytes_by_network: HashMap<NetId, u64>,
    fatal: Option<JobError>,
    last_error: String,
    retries: u64,
    hedges: u64,
    /// For an unknown size: bytes received when the single attempt finished.
    unknown_total: Option<u64>,
}

impl Shared {
    fn all_complete(&self) -> bool {
        self.blocks.iter().all(Block::complete)
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

async fn run_stream(ctx: Arc<Ctx>, net: Network, stream: StreamId) {
    let mut conn: Option<Conn> = None;
    let mut retry = StreamRetry::default();
    let mut last_rate: Option<f64> = None;
    let mut jitter_seed = u64::from(stream).wrapping_mul(0x9E37_79B9) | 1;
    loop {
        if ctx.stop.load(Ordering::Acquire) {
            return;
        }
        // Pick work (synchronously, under the lock: nothing can yield between choosing and registering).
        let work = {
            let mut s = ctx.lock();
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
                {
                    let mut s = ctx.lock();
                    s.retries += 1;
                    s.last_error = format!("{}: {failure:?}", net.name);
                    if let Some(code) = signals.link_refused {
                        let entry = s.link_refused.entry(net.id).or_insert((code, 0));
                        *entry = (code, entry.1 + 1);
                        // Expired only when every live network keeps refusing: brief 403s
                        // from rate limiting must not end a download (chaos seed 43).
                        let dead = s.dead_networks.clone();
                        let persistent = s
                            .link_refused
                            .iter()
                            .filter(|(n, (_, c))| !dead.contains(n) && *c >= LINK_EXPIRED_AFTER)
                            .count()
                            + dead.len()
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

fn scaled(ms: u64, t: &Tuning) -> Duration {
    Duration::from_millis((ms as f64 * t.retry_delay_scale.clamp(0.0, 1.0)) as u64)
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
    let (block_start, block_len, total, etag) = {
        let s = ctx.lock();
        let (start, len) = s.plan.block(work.block as u64).unwrap_or((0, 0));
        (start, len, s.plan.total, s.accepted_etag.clone())
    };
    if conn.is_none() {
        *conn = Some(
            connect(net, &ctx.src, &ctx.tuning)
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
        request(&ctx.src, &ctx.tuning, range, etag.as_deref()),
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
            s.blocks[work.block].complete() || s.fatal.is_some()
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
    if let Some(a) = s.blocks[block]
        .attempts
        .iter_mut()
        .find(|a| a.stream == stream)
    {
        a.position = a.position.max(position);
    }
    *s.bytes_by_network.entry(net).or_default() += new_bytes;
    ctx.written_total.fetch_add(new_bytes, Ordering::Relaxed);
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
            None => return true, // nothing on disk yet: nothing to mix up, just take the new label
        }
    };
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
    let new_etag = header(&res, ETAG).map(|e| headers::normalize_etag(&e));
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
            ctx.lock().accepted_etag = new_etag;
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
    let staging = Staging::create(dir, &probe.filename, probe.total)?;
    let file = staging.handle().map_err(StagingError::from)?;

    if probe.total == Some(0) {
        let path = staging.publish(0)?;
        return Ok(Report {
            path,
            total: 0,
            bytes_by_network: HashMap::new(),
            retries: 0,
            hedges: 0,
        });
    }

    let blocks: Vec<Block> = (0..plan.blocks)
        .map(|i| Block {
            len: plan.block(i).map_or(0, |(_, l)| l),
            secured: 0,
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
            accepted_etag: probe.etag.clone().or(probe.last_modified.clone()),
            bytes_by_network: HashMap::new(),
            fatal: None,
            last_error: String::new(),
            retries: 0,
            hedges: 0,
            unknown_total: None,
        }),
        file,
        wake: tokio::sync::Notify::new(),
        stop: AtomicBool::new(false),
        live_networks: networks.len(),
        epoch: Instant::now(),
        written_total: AtomicU64::new(0),
    });

    // Interleave stream starts across networks (L-23).
    let per_net = if splittable {
        tuning.streams_per_network.clamp(1, crate::concurrency::MAX)
    } else {
        1
    };
    let groups: Vec<Vec<(Network, StreamId)>> = networks
        .iter()
        .enumerate()
        .map(|(ni, n)| {
            (0..per_net)
                .map(|k| (n.clone(), (ni as u32) * 1000 + k))
                .collect()
        })
        .collect();
    let mut tasks = Vec::new();
    for (net, id) in crate::plan::interleave(&groups) {
        tasks.push(tokio::spawn(run_stream(ctx.clone(), net, id)));
    }
    for t in tasks {
        let _ = t.await;
    }
    ctx.stop.store(true, Ordering::Release);

    let (fatal, complete, report_bytes, retries, hedges, last_error, unknown_total) = {
        let mut s = ctx.lock();
        (
            s.fatal.take(),
            s.all_complete(),
            s.bytes_by_network.clone(),
            s.retries,
            s.hedges,
            s.last_error.clone(),
            s.unknown_total,
        )
    };
    if let Some(e) = fatal {
        if matches!(e, JobError::VersionChanged) {
            let _ = staging.discard(); // a different file: the partial data is useless (L-05)
        }
        return Err(e);
    }
    if !complete {
        return Err(JobError::AllNetworksFailed(last_error));
    }
    let total = probe.total.or(unknown_total).unwrap_or(0);
    let path = staging.publish(total)?;
    Ok(Report {
        path,
        total,
        bytes_by_network: report_bytes,
        retries,
        hedges,
    })
}
