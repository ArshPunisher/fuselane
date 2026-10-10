//! Remote control (8.7): an aria2-compatible JSON-RPC endpoint, so tools that
//! already speak aria2 (AriaNg, the "Aria2" browser extensions, phone remote
//! apps, media automation) can hand downloads to Fuselane and watch them.
//!
//! Off unless the person turns it on. Every call needs the secret
//! (`"token:<secret>"` as the first parameter, as aria2 does). It listens on
//! this computer only unless they allow the local network, and refuses
//! requests whose Host is a name other than `localhost`, which stops a web
//! page from reaching it through DNS rebinding.
//!
//! The protocol lives here; [`Host`] is what the app provides. Over HTTP POST
//! only (no WebSocket yet): AriaNg works with its "HTTP" setting.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use serde_json::{Value, json};

/// The aria2 version this endpoint behaves like, for clients that check it.
pub const ARIA2_VERSION: &str = "1.37.0";
/// Largest request accepted.
pub const MAX_BODY: usize = 1024 * 1024;
/// aria2's own default port.
pub const DEFAULT_PORT: u16 = 6800;

/// A download as aria2 tools see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: i64,
    pub url: String,
    pub mirrors: Vec<String>,
    pub dir: String,
    pub name: String,
    pub status: Status,
    pub total: Option<u64>,
    pub done: u64,
    /// Bytes per second right now.
    pub speed: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Waiting,
    Paused,
    Complete,
    Error,
}

impl Status {
    fn word(self) -> &'static str {
        match self {
            Status::Active => "active",
            Status::Waiting => "waiting",
            Status::Paused => "paused",
            Status::Complete => "complete",
            Status::Error => "error",
        }
    }

    fn stopped(self) -> bool {
        matches!(self, Status::Complete | Status::Error)
    }
}

/// `aria2.addUri`: the links (the first is the download, the rest mirrors).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Add {
    pub uris: Vec<String>,
    pub dir: Option<String>,
    pub out: Option<String>,
    pub paused: bool,
}

/// What the app provides. Errors are sentences for the person.
pub trait Host: Send + Sync + 'static {
    fn jobs(&self) -> Vec<Job>;
    fn add(&self, add: Add) -> Result<i64, String>;
    fn pause(&self, id: i64) -> Result<(), String>;
    fn resume(&self, id: i64) -> Result<(), String>;
    /// Takes it out of the list (the file stays where it is).
    fn remove(&self, id: i64) -> Result<(), String>;
    fn default_dir(&self) -> String;
    fn max_running(&self) -> usize;
    fn set_max_running(&self, n: usize) -> Result<(), String>;
}

/// A JSON-RPC failure: HTTP status, aria2 error code and message.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Fault {
    http: StatusCode,
    code: i64,
    message: String,
}

fn fault(message: impl Into<String>) -> Fault {
    Fault {
        http: StatusCode::BAD_REQUEST,
        code: 1,
        message: message.into(),
    }
}

/// The GID aria2 tools use: 16 hex digits.
pub fn gid(id: i64) -> String {
    format!("{id:016x}")
}

fn id_of(v: Option<&Value>) -> Result<i64, Fault> {
    let s = v
        .and_then(Value::as_str)
        .ok_or_else(|| fault("A GID is needed."))?;
    i64::from_str_radix(s, 16)
        .ok()
        .filter(|_| s.len() <= 16)
        .ok_or_else(|| fault(format!("GID {s} is not found")))
}

/// A random secret for a new setup: 32 hex digits.
pub fn new_secret() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Compares without stopping at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

const METHODS: &[&str] = &[
    "aria2.addUri",
    "aria2.remove",
    "aria2.forceRemove",
    "aria2.pause",
    "aria2.forcePause",
    "aria2.pauseAll",
    "aria2.forcePauseAll",
    "aria2.unpause",
    "aria2.unpauseAll",
    "aria2.tellStatus",
    "aria2.getUris",
    "aria2.getFiles",
    "aria2.getPeers",
    "aria2.getServers",
    "aria2.tellActive",
    "aria2.tellWaiting",
    "aria2.tellStopped",
    "aria2.getOption",
    "aria2.changeOption",
    "aria2.getGlobalOption",
    "aria2.changeGlobalOption",
    "aria2.getGlobalStat",
    "aria2.purgeDownloadResult",
    "aria2.removeDownloadResult",
    "aria2.getVersion",
    "aria2.getSessionInfo",
    "aria2.saveSession",
    "system.multicall",
    "system.listMethods",
    "system.listNotifications",
];

/// The protocol, without the network: one parsed request in, the reply out.
pub struct Rpc<H> {
    host: Arc<H>,
    secret: String,
    session: String,
}

impl<H> std::fmt::Debug for Rpc<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rpc").finish_non_exhaustive()
    }
}

impl<H: Host> Rpc<H> {
    pub fn new(host: Arc<H>, secret: String) -> Self {
        Rpc {
            host,
            secret,
            session: new_secret(),
        }
    }

    /// Handles a request body: one call or a batch. Returns the HTTP status
    /// and the JSON reply.
    pub fn handle(&self, body: &[u8]) -> (StatusCode, Value) {
        let Ok(req) = serde_json::from_slice::<Value>(body) else {
            return (
                StatusCode::BAD_REQUEST,
                json!({"jsonrpc": "2.0", "id": null,
                       "error": {"code": -32700, "message": "Parse error."}}),
            );
        };
        match req {
            Value::Array(calls) if !calls.is_empty() => {
                let out: Vec<Value> = calls.iter().map(|c| self.one(c).1).collect();
                (StatusCode::OK, Value::Array(out))
            }
            Value::Object(_) => self.one(&req),
            _ => (
                StatusCode::BAD_REQUEST,
                json!({"jsonrpc": "2.0", "id": null,
                       "error": {"code": -32600, "message": "Invalid Request."}}),
            ),
        }
    }

    fn one(&self, req: &Value) -> (StatusCode, Value) {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let params = match req.get("params") {
            Some(Value::Array(p)) => p.clone(),
            None => vec![],
            Some(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    json!({"jsonrpc": "2.0", "id": id,
                           "error": {"code": -32602, "message": "Invalid params."}}),
                );
            }
        };
        match self.dispatch(method, params) {
            Ok(result) => (
                StatusCode::OK,
                json!({"jsonrpc": "2.0", "id": id, "result": result}),
            ),
            Err(f) => (
                f.http,
                json!({"jsonrpc": "2.0", "id": id,
                       "error": {"code": f.code, "message": f.message}}),
            ),
        }
    }

    /// Takes the secret off the front of `params` and checks it.
    fn authorize(&self, params: &mut Vec<Value>) -> Result<(), Fault> {
        let given = params
            .first()
            .and_then(Value::as_str)
            .and_then(|t| t.strip_prefix("token:"))
            .map(str::to_string);
        match given {
            Some(t) if same(&t, &self.secret) => {
                params.remove(0);
                Ok(())
            }
            _ => Err(Fault {
                http: StatusCode::UNAUTHORIZED,
                code: 1,
                message: "Unauthorized".into(),
            }),
        }
    }

    fn dispatch(&self, method: &str, mut params: Vec<Value>) -> Result<Value, Fault> {
        match method {
            // aria2 answers these two without the secret.
            "system.listMethods" => return Ok(json!(METHODS)),
            "system.listNotifications" => return Ok(json!([])),
            "system.multicall" => return self.multicall(params),
            _ => {}
        }
        if !METHODS.contains(&method) {
            return Err(Fault {
                http: StatusCode::NOT_FOUND,
                code: 1,
                message: format!("No such method: {method}"),
            });
        }
        self.authorize(&mut params)?;
        let p = |i: usize| params.get(i);
        match method {
            "aria2.addUri" => self.add(&params),
            "aria2.remove" | "aria2.forceRemove" => {
                let id = id_of(p(0))?;
                self.find(id)?;
                self.host.remove(id).map_err(fault)?;
                Ok(json!(gid(id)))
            }
            "aria2.pause" | "aria2.forcePause" => {
                let id = id_of(p(0))?;
                self.find(id)?;
                self.host.pause(id).map_err(fault)?;
                Ok(json!(gid(id)))
            }
            "aria2.unpause" => {
                let id = id_of(p(0))?;
                self.find(id)?;
                self.host.resume(id).map_err(fault)?;
                Ok(json!(gid(id)))
            }
            "aria2.pauseAll" | "aria2.forcePauseAll" => {
                for j in self.host.jobs() {
                    if matches!(j.status, Status::Active | Status::Waiting) {
                        let _ = self.host.pause(j.id);
                    }
                }
                Ok(json!("OK"))
            }
            "aria2.unpauseAll" => {
                for j in self.host.jobs() {
                    if j.status == Status::Paused {
                        let _ = self.host.resume(j.id);
                    }
                }
                Ok(json!("OK"))
            }
            "aria2.tellStatus" => {
                let job = self.find(id_of(p(0))?)?;
                Ok(status(&job, keys(p(1))))
            }
            "aria2.getUris" => Ok(uris(&self.find(id_of(p(0))?)?)),
            "aria2.getFiles" => Ok(files(&self.find(id_of(p(0))?)?)),
            "aria2.getPeers" | "aria2.getServers" => {
                self.find(id_of(p(0))?)?;
                Ok(json!([]))
            }
            "aria2.tellActive" => {
                let k = keys(p(0));
                Ok(self.list(|s| s == Status::Active, 0, usize::MAX, k))
            }
            "aria2.tellWaiting" => {
                let (offset, num) = (int(p(0))?, count(p(1))?);
                Ok(self.list(
                    |s| matches!(s, Status::Waiting | Status::Paused),
                    offset,
                    num,
                    keys(p(2)),
                ))
            }
            "aria2.tellStopped" => {
                let (offset, num) = (int(p(0))?, count(p(1))?);
                Ok(self.list(Status::stopped, offset, num, keys(p(2))))
            }
            "aria2.getOption" => {
                let job = self.find(id_of(p(0))?)?;
                Ok(json!({"dir": job.dir, "out": job.name}))
            }
            // Per-download options aren't changed this way; saying OK keeps
            // clients that set them on every add working.
            "aria2.changeOption" => {
                self.find(id_of(p(0))?)?;
                Ok(json!("OK"))
            }
            "aria2.getGlobalOption" => Ok(json!({
                "dir": self.host.default_dir(),
                "max-concurrent-downloads": self.host.max_running().to_string(),
            })),
            "aria2.changeGlobalOption" => {
                if let Some(n) = p(0)
                    .and_then(|o| o.get("max-concurrent-downloads"))
                    .and_then(Value::as_str)
                {
                    let n = n
                        .parse::<usize>()
                        .map_err(|_| fault("max-concurrent-downloads must be a number."))?;
                    self.host.set_max_running(n).map_err(fault)?;
                }
                Ok(json!("OK"))
            }
            "aria2.getGlobalStat" => Ok(self.stat()),
            "aria2.purgeDownloadResult" => {
                for j in self.host.jobs().into_iter().filter(|j| j.status.stopped()) {
                    let _ = self.host.remove(j.id);
                }
                Ok(json!("OK"))
            }
            "aria2.removeDownloadResult" => {
                let job = self.find(id_of(p(0))?)?;
                if !job.status.stopped() {
                    return Err(fault(format!(
                        "Could not remove download result of GID#{}",
                        gid(job.id)
                    )));
                }
                self.host.remove(job.id).map_err(fault)?;
                Ok(json!("OK"))
            }
            "aria2.getVersion" => Ok(json!({
                "version": ARIA2_VERSION,
                "enabledFeatures": ["HTTPS"],
            })),
            "aria2.getSessionInfo" => Ok(json!({"sessionId": self.session})),
            "aria2.saveSession" => Ok(json!("OK")),
            _ => Err(fault(format!("No such method: {method}"))),
        }
    }

    fn multicall(&self, params: Vec<Value>) -> Result<Value, Fault> {
        let Some(Value::Array(calls)) = params.into_iter().next() else {
            return Err(fault("system.multicall needs a list of calls."));
        };
        let out = calls
            .into_iter()
            .map(|c| {
                let method = c.get("methodName").and_then(Value::as_str).unwrap_or("");
                if method == "system.multicall" {
                    return json!({"code": 1, "message": "Recursive system.multicall forbidden."});
                }
                let params = match c.get("params") {
                    Some(Value::Array(p)) => p.clone(),
                    _ => vec![],
                };
                match self.dispatch(method, params) {
                    Ok(v) => json!([v]),
                    Err(f) => json!({"code": f.code, "message": f.message}),
                }
            })
            .collect();
        Ok(Value::Array(out))
    }

    fn add(&self, params: &[Value]) -> Result<Value, Fault> {
        let uris: Vec<String> = params
            .first()
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        if uris.is_empty() {
            return Err(fault("addUri needs at least one link."));
        }
        let opts = params.get(1);
        let opt = |k: &str| {
            opts.and_then(|o| o.get(k))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        let add = Add {
            uris,
            dir: opt("dir"),
            out: opt("out"),
            paused: opt("pause").as_deref() == Some("true"),
        };
        let id = self.host.add(add).map_err(fault)?;
        Ok(json!(gid(id)))
    }

    fn find(&self, id: i64) -> Result<Job, Fault> {
        self.host
            .jobs()
            .into_iter()
            .find(|j| j.id == id)
            .ok_or_else(|| fault(format!("GID {} is not found", gid(id))))
    }

    fn list(
        &self,
        keep: impl Fn(Status) -> bool,
        offset: i64,
        num: usize,
        keys: Option<Vec<String>>,
    ) -> Value {
        let picked: Vec<Job> = self
            .host
            .jobs()
            .into_iter()
            .filter(|j| keep(j.status))
            .collect();
        // A negative offset counts from the end, newest first (as aria2 does).
        let items: Vec<&Job> = if offset < 0 {
            let start = usize::try_from(-(offset + 1)).unwrap_or(usize::MAX);
            picked.iter().rev().skip(start).take(num).collect()
        } else {
            let start = usize::try_from(offset).unwrap_or(usize::MAX);
            picked.iter().skip(start).take(num).collect()
        };
        Value::Array(items.into_iter().map(|j| status(j, keys.clone())).collect())
    }

    fn stat(&self) -> Value {
        let jobs = self.host.jobs();
        let count = |f: &dyn Fn(Status) -> bool| jobs.iter().filter(|j| f(j.status)).count();
        let speed: u64 = jobs
            .iter()
            .filter(|j| j.status == Status::Active)
            .map(|j| j.speed)
            .sum();
        let stopped = count(&Status::stopped);
        json!({
            "downloadSpeed": speed.to_string(),
            "uploadSpeed": "0",
            "numActive": count(&|s| s == Status::Active).to_string(),
            "numWaiting": count(&|s| matches!(s, Status::Waiting | Status::Paused)).to_string(),
            "numStopped": stopped.to_string(),
            "numStoppedTotal": stopped.to_string(),
        })
    }
}

fn int(v: Option<&Value>) -> Result<i64, Fault> {
    v.and_then(Value::as_i64)
        .ok_or_else(|| fault("A number is needed here."))
}

fn count(v: Option<&Value>) -> Result<usize, Fault> {
    usize::try_from(int(v)?).map_err(|_| fault("The count can't be negative."))
}

fn keys(v: Option<&Value>) -> Option<Vec<String>> {
    v.and_then(Value::as_array).map(|a| {
        a.iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    })
}

fn path_of(job: &Job) -> String {
    if job.name.is_empty() {
        String::new()
    } else if job.dir.is_empty() {
        job.name.clone()
    } else {
        format!("{}/{}", job.dir.trim_end_matches(['/', '\\']), job.name)
    }
}

fn uris(job: &Job) -> Value {
    Value::Array(
        std::iter::once(&job.url)
            .chain(&job.mirrors)
            .map(|u| json!({"uri": u, "status": "used"}))
            .collect(),
    )
}

fn files(job: &Job) -> Value {
    json!([{
        "index": "1",
        "path": path_of(job),
        "length": job.total.unwrap_or(0).to_string(),
        "completedLength": job.done.to_string(),
        "selected": "true",
        "uris": uris(job),
    }])
}

/// aria2's status struct: every number a string. `keys` picks fields.
fn status(job: &Job, keys: Option<Vec<String>>) -> Value {
    let mut all = json!({
        "gid": gid(job.id),
        "status": job.status.word(),
        "totalLength": job.total.unwrap_or(0).to_string(),
        "completedLength": job.done.to_string(),
        "uploadLength": "0",
        "downloadSpeed": if job.status == Status::Active { job.speed } else { 0 }.to_string(),
        "uploadSpeed": "0",
        "connections": if job.status == Status::Active { "1" } else { "0" },
        "numPieces": "1",
        "pieceLength": job.total.unwrap_or(0).to_string(),
        "dir": job.dir,
        "files": files(job),
    });
    if let (Status::Error, Some(obj)) = (job.status, all.as_object_mut()) {
        obj.insert("errorCode".into(), json!("1"));
        obj.insert(
            "errorMessage".into(),
            json!(job.error.clone().unwrap_or_default()),
        );
    }
    match (keys, all) {
        (Some(k), Value::Object(m)) if !k.is_empty() => Value::Object(
            m.into_iter()
                .filter(|(name, _)| k.iter().any(|x| x == name))
                .collect(),
        ),
        (_, v) => v,
    }
}

type Body = http_body_util::combinators::BoxBody<Bytes, std::convert::Infallible>;

fn reply(code: StatusCode, body: Vec<u8>, ty: &'static str) -> Response<Body> {
    let mut r = Response::new(Full::new(Bytes::from(body)).boxed());
    *r.status_mut() = code;
    let h = r.headers_mut();
    h.insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static(ty),
    );
    // Web front ends (AriaNg) run on another origin; the secret is the guard.
    h.insert(
        hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        hyper::header::HeaderValue::from_static("*"),
    );
    h.insert(
        hyper::header::ACCESS_CONTROL_ALLOW_HEADERS,
        hyper::header::HeaderValue::from_static("Content-Type"),
    );
    h.insert(
        hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
        hyper::header::HeaderValue::from_static("POST, OPTIONS"),
    );
    h.insert(
        hyper::header::CACHE_CONTROL,
        hyper::header::HeaderValue::from_static("no-store"),
    );
    r
}

fn text(code: StatusCode, msg: &str) -> Response<Body> {
    reply(code, msg.as_bytes().to_vec(), "text/plain; charset=utf-8")
}

/// `localhost` or an IP address: a web page that rebinds its own name to this
/// computer still sends its own name, and is refused.
fn host_ok(req: &Request<Incoming>) -> bool {
    let Some(h) = req
        .headers()
        .get(hyper::header::HOST)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let name = if let Some(rest) = h.strip_prefix('[') {
        rest.split(']').next().unwrap_or("")
    } else {
        h.rsplit_once(':').map_or(h, |(n, _)| n)
    };
    name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::IpAddr>().is_ok()
}

async fn serve<H: Host>(rpc: Arc<Rpc<H>>, req: Request<Incoming>) -> Response<Body> {
    if !host_ok(&req) {
        return text(
            StatusCode::FORBIDDEN,
            "Use this computer's address (127.0.0.1 or its IP), not a name.",
        );
    }
    if req.uri().path() != "/jsonrpc" {
        return text(
            StatusCode::NOT_FOUND,
            "Fuselane's remote control is at /jsonrpc.",
        );
    }
    match req.method().as_str() {
        "OPTIONS" => reply(StatusCode::NO_CONTENT, vec![], "text/plain"),
        "POST" => {
            let said = req
                .headers()
                .get(hyper::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<usize>().ok());
            if said.is_some_and(|n| n > MAX_BODY) {
                return text(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Requests up to 1 MB are accepted.",
                );
            }
            let Ok(body) = http_body_util::Limited::new(req.into_body(), MAX_BODY)
                .collect()
                .await
            else {
                return text(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "Requests up to 1 MB are accepted.",
                );
            };
            let body = body.to_bytes();
            // The work touches the download list; keep it off the network thread.
            let r = rpc.clone();
            let (code, value) = tokio::task::spawn_blocking(move || r.handle(&body))
                .await
                .unwrap_or((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"jsonrpc": "2.0", "id": null,
                           "error": {"code": 1, "message": "Fuselane hit a problem with that call."}}),
                ));
            if code == StatusCode::UNAUTHORIZED {
                // Slows down guessing; the secret is 128 bits anyway.
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            reply(
                code,
                serde_json::to_vec(&value).unwrap_or_default(),
                "application/json",
            )
        }
        _ => text(
            StatusCode::METHOD_NOT_ALLOWED,
            "Send JSON-RPC calls with POST (WebSocket isn't supported yet).",
        ),
    }
}

/// A running endpoint; stops when dropped.
pub struct Server {
    pub addr: SocketAddr,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

impl Server {
    /// Stops and waits until the port is free again.
    pub async fn stop(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
    }

    /// Listens on `bind` (127.0.0.1 for this computer only, 0.0.0.0 for the
    /// local network too).
    pub async fn start<H: Host>(
        host: Arc<H>,
        bind: SocketAddr,
        secret: String,
    ) -> std::io::Result<Server> {
        if secret.len() < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the secret must be at least 8 characters",
            ));
        }
        let listener = tokio::net::TcpListener::bind(bind).await?;
        let addr = listener.local_addr()?;
        let rpc = Arc::new(Rpc::new(host, secret));
        let task = tokio::spawn(async move {
            // Connections live in here, so stopping ends them too: one kept
            // open can't go on using a secret that has since been replaced.
            let mut conns = tokio::task::JoinSet::new();
            while let Ok((tcp, _)) = listener.accept().await {
                while conns.try_join_next().is_some() {}
                let rpc = rpc.clone();
                conns.spawn(async move {
                    let svc = hyper::service::service_fn(move |req| {
                        let rpc = rpc.clone();
                        async move { Ok::<_, std::convert::Infallible>(serve(rpc, req).await) }
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(hyper_util::rt::TokioIo::new(tcp), svc)
                        .await;
                });
            }
        });
        Ok(Server {
            addr,
            task: Some(task),
        })
    }
}
