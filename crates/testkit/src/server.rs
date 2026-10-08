//! A hostile HTTP/1.1 range server for engine tests (TESTING.md §1 L2).
//!
//! Serves a [`Content`] at `/file.bin` and follows a script of [`Rule`]s: each
//! rule skips some requests, then applies a [`Fault`] a number of times. Every
//! request is logged so tests can assert what the client actually sent.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, StreamBody, combinators::BoxBody};
use hyper::body::Frame;
use hyper::header::{CONTENT_LENGTH, CONTENT_RANGE, ETAG, HeaderValue, RANGE};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

use crate::content::Content;

/// One way to misbehave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fault {
    /// Answer 200 with the whole file, ignoring Range.
    IgnoreRange,
    /// Answer a shorter range than asked (at most N bytes), honestly labelled.
    CapRange(u64),
    /// Label and send the range starting N bytes earlier than asked.
    WrongStart(u64),
    /// Label and send N bytes past the requested end.
    Overrun(u64),
    /// Promise the full range, then send only N bytes and close.
    ShortBody(u64),
    /// Send N body bytes, then go silent with the connection open.
    StallAt(u64),
    /// Answer with this status (and optional Retry-After) and no body.
    Status(u16, Option<String>),
    /// 206 without a Content-Range header.
    NoContentRange,
    /// Send at most N bytes per second.
    Throttle(u64),
    /// Close the connection without answering.
    Reset,
}

/// Skip `skip` matching requests, then apply `fault` to the next `times` requests.
#[derive(Debug, Clone)]
pub struct Rule {
    pub skip: u32,
    pub times: u32,
    pub fault: Fault,
}

impl Rule {
    pub fn once(fault: Fault) -> Rule {
        Rule {
            skip: 0,
            times: 1,
            fault,
        }
    }
    pub fn always(fault: Fault) -> Rule {
        Rule {
            skip: 0,
            times: u32::MAX,
            fault,
        }
    }
}

/// What the client sent.
#[derive(Debug, Clone)]
pub struct RequestLog {
    pub range: Option<String>,
    pub if_range: Option<String>,
    pub accept_encoding: Option<String>,
    pub fault: Option<Fault>,
}

#[derive(Debug)]
struct State {
    content: Content,
    etag: String,
    rules: Vec<Rule>,
    log: Vec<RequestLog>,
    /// Bodies being sent right now, and the most at once.
    active: u32,
    peak: u32,
    /// Refuse (503) a new transfer when this many are already running (CDN-style).
    max_concurrent: Option<u32>,
    /// Swap to this file (and ETag) atomically when request number N arrives (1-based).
    swap_at: Option<(usize, Content, String)>,
}

/// A running server; stops when dropped (the runtime task is aborted).
#[derive(Debug)]
pub struct RangeServer {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for RangeServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl RangeServer {
    /// Starts serving `content` on 127.0.0.1 with a random port.
    pub async fn start(content: Content) -> std::io::Result<RangeServer> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let state = Arc::new(Mutex::new(State {
            content,
            etag: "\"v1\"".into(),
            rules: vec![],
            log: vec![],
            active: 0,
            peak: 0,
            max_concurrent: None,
            swap_at: None,
        }));
        let st = state.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let st = st.clone();
                tokio::spawn(async move {
                    let svc = service_fn(move |req| handle(st.clone(), req));
                    let _ = hyper::server::conn::http1::Builder::new()
                        .keep_alive(true)
                        .serve_connection(TokioIo::new(stream), svc)
                        .await;
                });
            }
        });
        Ok(RangeServer { addr, state, task })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn path(&self) -> &'static str {
        "/file.bin"
    }

    pub fn add_rule(&self, rule: Rule) {
        self.lock().rules.push(rule);
    }

    /// Changes the ETag from now on (same bytes: a load balancer relabelling).
    pub fn set_etag(&self, etag: &str) {
        self.lock().etag = etag.to_string();
    }

    /// Replaces the file (a real new version on the server).
    pub fn set_content(&self, content: Content, etag: &str) {
        let mut s = self.lock();
        s.content = content;
        s.etag = etag.to_string();
    }

    /// Refuse new transfers with 503 while `n` are already being sent.
    pub fn set_max_concurrent(&self, n: Option<u32>) {
        self.lock().max_concurrent = n;
    }

    /// Atomically replaces the file and ETag when request number `n` arrives (1-based),
    /// so tests can change the file at an exact point in a download.
    pub fn swap_at_request(&self, n: usize, content: Content, etag: &str) {
        self.lock().swap_at = Some((n, content, etag.to_string()));
    }

    /// Most bodies sent at the same time so far.
    pub fn peak_concurrent(&self) -> u32 {
        self.lock().peak
    }

    pub fn requests(&self) -> Vec<RequestLog> {
        self.lock().log.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

type Body = BoxBody<Bytes, std::io::Error>;

fn empty() -> Body {
    http_body_util::Empty::new()
        .map_err(|never| match never {})
        .boxed()
}

/// Parses `bytes=a-b` / `bytes=a-` (single range only).
fn parse_range(v: &str, size: u64) -> Option<(u64, u64)> {
    let r = v.strip_prefix("bytes=")?;
    let (a, b) = r.split_once('-')?;
    let first: u64 = a.trim().parse().ok()?;
    let last: u64 = if b.trim().is_empty() {
        size.checked_sub(1)?
    } else {
        b.trim().parse::<u64>().ok()?.min(size.checked_sub(1)?)
    };
    (first <= last).then_some((first, last))
}

async fn handle(
    state: Arc<Mutex<State>>,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Body>, std::io::Error> {
    let header = |name| {
        req.headers()
            .get(name)
            .and_then(|v: &HeaderValue| v.to_str().ok())
            .map(str::to_string)
    };
    let (content, etag, fault) = {
        let mut s = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((n, c, e)) = s.swap_at.clone()
            && s.log.len() + 1 >= n
        {
            s.content = c;
            s.etag = e;
            s.swap_at = None;
        }
        let mut fault = None;
        let busy =
            s.max_concurrent.is_some_and(|m| s.active >= m) && req.headers().contains_key(RANGE);
        if busy {
            fault = Some(Fault::Status(503, None));
        }
        for rule in s.rules.iter_mut().filter(|_| !busy) {
            if rule.times == 0 {
                continue;
            }
            if rule.skip > 0 {
                rule.skip -= 1;
                continue;
            }
            rule.times -= 1;
            fault = Some(rule.fault.clone());
            break;
        }
        s.log.push(RequestLog {
            range: header(RANGE),
            if_range: header(hyper::header::IF_RANGE),
            accept_encoding: header(hyper::header::ACCEPT_ENCODING),
            fault: fault.clone(),
        });
        (s.content, s.etag.clone(), fault)
    };

    if fault == Some(Fault::Reset) {
        return Err(std::io::Error::other("reset by test server"));
    }
    if let Some(Fault::Status(code, retry_after)) = &fault {
        let mut res = Response::builder().status(*code).header(ETAG, &etag);
        if let Some(ra) = retry_after {
            res = res.header(hyper::header::RETRY_AFTER, ra);
        }
        return Ok(res.body(empty()).unwrap_or_else(|_| Response::new(empty())));
    }

    let size = content.size;
    let requested = header(RANGE).and_then(|r| parse_range(&r, size));
    if header(RANGE).is_some() && requested.is_none() {
        // Unsatisfiable (including any range on an empty file).
        return Ok(Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(CONTENT_RANGE, format!("bytes */{size}"))
            .header(ETAG, &etag)
            .body(empty())
            .unwrap_or_else(|_| Response::new(empty())));
    }

    // RFC 9110 §13.1.5: a Range is honoured only if If-Range matches the current
    // representation exactly (strong comparison); otherwise the whole file is sent.
    let if_range_ok =
        header(hyper::header::IF_RANGE).is_none_or(|v| v == etag && !v.starts_with("W/"));
    let requested = if if_range_ok { requested } else { None };
    let (status, mut first, mut last) = match (requested, &fault) {
        (_, Some(Fault::IgnoreRange)) | (None, _) => (200, 0, size.saturating_sub(1)),
        (Some((a, b)), _) => (206, a, b),
    };
    match &fault {
        Some(Fault::CapRange(n)) if status == 206 => last = last.min(first + n.saturating_sub(1)),
        Some(Fault::WrongStart(n)) if status == 206 => first = first.saturating_sub(*n),
        Some(Fault::Overrun(n)) if status == 206 => last = (last + n).min(size.saturating_sub(1)),
        _ => {}
    }
    let len = if size == 0 { 0 } else { last - first + 1 };
    let (send, stall, throttle) = match &fault {
        Some(Fault::ShortBody(n)) => (len.min(*n), false, None),
        Some(Fault::StallAt(n)) => (len.min(*n), true, None),
        Some(Fault::Throttle(bps)) => (len, false, Some(*bps)),
        _ => (len, false, None),
    };

    let mut res = Response::builder()
        .status(status)
        .header(ETAG, &etag)
        .header(CONTENT_LENGTH, len)
        .header(hyper::header::ACCEPT_RANGES, "bytes");
    if status == 206 && fault != Some(Fault::NoContentRange) {
        res = res.header(CONTENT_RANGE, format!("bytes {first}-{last}/{size}"));
    }

    let (tx, rx) = mpsc::channel::<Result<Frame<Bytes>, std::io::Error>>(4);
    let guard = ActiveGuard::new(state.clone());
    tokio::spawn(async move {
        let _guard = guard;
        let mut off = first;
        let end = first + send;
        while off < end {
            let n = (end - off).min(64 * 1024);
            // Throttle: pace in 1/20 s slices, sleeping *before* each slice, so even a
            // small body takes its real time and a finished body isn't counted as busy.
            let n = throttle.map_or(n, |bps| n.min((bps / 20).max(1)));
            if let Some(bps) = throttle {
                tokio::time::sleep(Duration::from_millis(1000 * n / bps.max(1))).await;
            }
            let mut buf = vec![0u8; n as usize];
            content.fill(off, &mut buf);
            if tx.send(Ok(Frame::data(Bytes::from(buf)))).await.is_err() {
                return;
            }
            off += n;
        }
        if stall {
            // Hold the connection open, silent, until the client gives up.
            let _keep = tx;
            tokio::time::sleep(Duration::from_secs(3600)).await;
        } else if send < len {
            let _ = tx.send(Err(std::io::Error::other("short body"))).await;
        }
    });
    let body = StreamBody::new(tokio_stream_from(rx)).boxed();
    Ok(res.body(body).unwrap_or_else(|_| Response::new(empty())))
}

/// Counts a body as active from creation until its sender task ends.
struct ActiveGuard(Arc<Mutex<State>>);

impl ActiveGuard {
    fn new(state: Arc<Mutex<State>>) -> ActiveGuard {
        {
            let mut s = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            s.active += 1;
            s.peak = s.peak.max(s.active);
        }
        ActiveGuard(state)
    }
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        let mut s = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.active = s.active.saturating_sub(1);
    }
}

/// Adapts an mpsc receiver into a `Stream` without pulling in tokio-stream.
fn tokio_stream_from<T>(mut rx: mpsc::Receiver<T>) -> impl futures_core::Stream<Item = T> {
    futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx))
}
