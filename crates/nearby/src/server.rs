//! The receiving side: an HTTPS server speaking LocalSend v2. The app decides
//! who may send (the [`Host`]); this checks every message, writes accepted
//! files into the chosen folder under free names, and refuses anything that
//! doesn't match what was announced.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use sha2::Digest;
use tokio::io::AsyncWriteExt;

use crate::proto::{self, DeviceInfo, FileMeta, PrepareUpload, PrepareUploadReply};

/// What the app answers to a request to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Save these files (all when None) into `dir`.
    Accept {
        dir: PathBuf,
        only: Option<Vec<String>>,
    },
    Decline,
    /// Something else is arriving (409).
    Busy,
}

/// How an incoming session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    /// Every accepted file arrived and checked out.
    Done,
    /// The sender cancelled or vanished; partial files were removed.
    Cancelled,
    /// A file didn't match its announced size or checksum.
    Failed(String),
}

type Fut<'a, T> = Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

/// The app's side of the receiving server.
pub trait Host: Send + Sync + 'static {
    /// This device, as told to others.
    fn me(&self) -> DeviceInfo;
    /// A device introduced itself (register). Its fingerprint is verified by
    /// connecting back to it when possible.
    fn seen(&self, from: SocketAddr, info: DeviceInfo, verified: Option<String>);
    /// Someone wants to send files. `verified` is the sender's real fingerprint
    /// when its own server could be reached (LocalSend and Fuselane both run one).
    fn decide<'a>(
        &'a self,
        from: SocketAddr,
        verified: Option<String>,
        req: &'a PrepareUpload,
    ) -> Fut<'a, Decision>;
    /// An accepted session begins: who from, which files, how many bytes in all.
    fn started(&self, session: &str, from: &DeviceInfo, names: &[String], total: u64);
    fn progress(&self, session: &str, file: &str, written: u64);
    fn file_done(&self, session: &str, file: &str, path: &Path);
    fn ended(&self, session: &str, how: Ended);
    /// An accepted text message (no file is uploaded for it).
    fn message(&self, _from: &DeviceInfo, _text: &str) {}
}

struct Incoming1 {
    meta: FileMeta,
    token: String,
    done: bool,
}

struct Session {
    id: String,
    from: std::net::IpAddr,
    dir: PathBuf,
    files: HashMap<String, Incoming1>,
}

struct State<H> {
    host: Arc<H>,
    session: Mutex<Option<Session>>,
    /// Sessions the person on this side cancelled; their uploads stop.
    cancelled: Arc<Mutex<std::collections::HashSet<String>>>,
}

/// The reason a receive stopped because this side cancelled it.
const CANCELLED_HERE: &str = "cancelled here";

fn random_id() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn text(code: StatusCode, msg: &str) -> Response<Full<Bytes>> {
    let mut r = Response::new(Full::new(Bytes::from(msg.to_string())));
    *r.status_mut() = code;
    r
}

fn json<T: serde::Serialize>(v: &T) -> Response<Full<Bytes>> {
    let body = serde_json::to_vec(v).unwrap_or_default();
    let mut r = Response::new(Full::new(Bytes::from(body)));
    r.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/json"),
    );
    r
}

fn query(req: &Request<Incoming>) -> HashMap<String, String> {
    url::form_urlencoded::parse(req.uri().query().unwrap_or("").as_bytes())
        .into_owned()
        .collect()
}

async fn read_json(req: Request<Incoming>) -> Option<Bytes> {
    Limited::new(req.into_body(), proto::MAX_JSON)
        .collect()
        .await
        .ok()
        .map(http_body_util::Collected::to_bytes)
}

/// The sender's real fingerprint: connect back to its HTTPS server and read
/// its certificate (the TLS handshake proves it holds the key).
pub async fn fingerprint_of_peer(ip: std::net::IpAddr, port: u16) -> Option<String> {
    let verifier = crate::tls::Pinned::new(None);
    let cfg = crate::tls::client_config(verifier.clone()).ok()?;
    let tcp = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect((ip, port)),
    )
    .await
    .ok()?
    .ok()?;
    let name = rustls::pki_types::ServerName::IpAddress(ip.into());
    let _tls = tokio::time::timeout(
        Duration::from_secs(3),
        tokio_rustls::TlsConnector::from(cfg).connect(name, tcp),
    )
    .await
    .ok()?
    .ok()?;
    verifier
        .seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// A free name in `dir` (never overwriting): "a.jpg", "a (2).jpg", ...
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if first.symlink_metadata().is_err() {
        return first;
    }
    (2..100_000)
        .map(|n| dir.join(fuselane_storage::names::numbered(name, n)))
        .find(|p| p.symlink_metadata().is_err())
        .unwrap_or(first)
}

impl<H: Host> State<H> {
    async fn handle(
        self: Arc<Self>,
        req: Request<Incoming>,
        from: SocketAddr,
    ) -> Response<Full<Bytes>> {
        let path = req.uri().path().to_string();
        let method = req.method().clone();
        match (method.as_str(), path.as_str()) {
            ("GET", "/api/localsend/v2/info") => json(&self.host.me()),
            ("POST", "/api/localsend/v2/register") => {
                let Some(body) = read_json(req).await else {
                    return text(StatusCode::BAD_REQUEST, "Invalid body");
                };
                match proto::parse_info(&body) {
                    Ok(info) => {
                        let verified = fingerprint_of_peer(from.ip(), info.port).await;
                        self.host.seen(from, info, verified);
                        json(&self.host.me())
                    }
                    Err(e) => text(StatusCode::BAD_REQUEST, &e.to_string()),
                }
            }
            ("POST", "/api/localsend/v2/prepare-upload") => self.prepare(req, from).await,
            ("POST", "/api/localsend/v2/upload") => self.upload(req, from).await,
            ("POST", "/api/localsend/v2/cancel") => {
                let q = query(&req);
                let mut s = self
                    .session
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let mine = s
                    .as_ref()
                    .is_some_and(|x| Some(&x.id) == q.get("sessionId") && x.from == from.ip());
                if mine && let Some(old) = s.take() {
                    drop(s);
                    self.host.ended(&old.id, Ended::Cancelled);
                }
                text(StatusCode::OK, "")
            }
            _ => text(StatusCode::NOT_FOUND, "Not found"),
        }
    }

    async fn prepare(&self, req: Request<Incoming>, from: SocketAddr) -> Response<Full<Bytes>> {
        let Some(body) = read_json(req).await else {
            return text(StatusCode::BAD_REQUEST, "Invalid body");
        };
        let p = match proto::parse_prepare(&body) {
            Ok(p) => p,
            Err(e) => return text(StatusCode::BAD_REQUEST, &e.to_string()),
        };
        if self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        {
            return text(StatusCode::CONFLICT, "Blocked by another session");
        }
        let verified = fingerprint_of_peer(from.ip(), p.info.port).await;
        let decision = tokio::time::timeout(
            Duration::from_secs(120),
            self.host.decide(from, verified, &p),
        )
        .await
        .unwrap_or(Decision::Decline);
        let (dir, only) = match decision {
            Decision::Accept { dir, only } => (dir, only),
            Decision::Decline => return text(StatusCode::FORBIDDEN, "Rejected"),
            Decision::Busy => return text(StatusCode::CONFLICT, "Blocked by another session"),
        };
        let mut s = self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if s.is_some() {
            return text(StatusCode::CONFLICT, "Blocked by another session");
        }
        let sender = p.info.clone();
        // A lone text message is delivered here and needs no upload (LocalSend).
        if p.files.len() == 1
            && let Some(msg) = p.files.values().next().and_then(FileMeta::message)
        {
            drop(s);
            self.host.message(&sender, msg);
            return text(StatusCode::NO_CONTENT, "");
        }
        let mut files = HashMap::new();
        let mut reply = PrepareUploadReply {
            session_id: random_id(),
            files: Default::default(),
        };
        for (id, meta) in p.files {
            if only.as_ref().is_some_and(|o| !o.contains(&id)) {
                continue;
            }
            let token = random_id();
            reply.files.insert(id.clone(), token.clone());
            files.insert(
                id,
                Incoming1 {
                    meta,
                    token,
                    done: false,
                },
            );
        }
        if files.is_empty() {
            return text(StatusCode::NO_CONTENT, "");
        }
        let names: Vec<String> = files.values().map(|f| f.meta.file_name.clone()).collect();
        let total = files.values().map(|f| f.meta.size).sum();
        *s = Some(Session {
            id: reply.session_id.clone(),
            from: from.ip(),
            dir,
            files,
        });
        drop(s);
        self.host.started(&reply.session_id, &sender, &names, total);
        json(&reply)
    }

    async fn upload(&self, req: Request<Incoming>, from: SocketAddr) -> Response<Full<Bytes>> {
        let q = query(&req);
        let (Some(sid), Some(fid), Some(token)) =
            (q.get("sessionId"), q.get("fileId"), q.get("token"))
        else {
            return text(StatusCode::BAD_REQUEST, "Missing parameters");
        };
        // Look the file up, then work without holding the lock.
        if self
            .cancelled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(sid.as_str())
        {
            return text(StatusCode::FORBIDDEN, "Cancelled by the receiver");
        }
        let (meta, dir) = {
            let s = self
                .session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(s) = s.as_ref().filter(|s| &s.id == sid) else {
                return text(StatusCode::CONFLICT, "Blocked by another session");
            };
            let Some(f) = s.files.get(fid).filter(|f| &f.token == token && !f.done) else {
                return text(StatusCode::FORBIDDEN, "Invalid token or IP address");
            };
            if s.from != from.ip() {
                return text(StatusCode::FORBIDDEN, "Invalid token or IP address");
            }
            (f.meta.clone(), s.dir.clone())
        };
        let part = dir.join(format!(".{}.{}.fuselane-part", meta.file_name, &token[..8]));
        let result = self.receive(req, &part, &meta, sid, fid).await;
        let failed = |why: String| {
            let _ = std::fs::remove_file(&part);
            why
        };
        let outcome = match result {
            Ok(()) => {
                let target = free_name(&dir, &meta.file_name);
                match std::fs::rename(&part, &target) {
                    Ok(()) => Ok(target),
                    Err(e) => Err(failed(format!("couldn't save it ({e})"))),
                }
            }
            Err(why) => Err(failed(why)),
        };
        let mut s = self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match outcome {
            Ok(target) => {
                let mut all_done = false;
                if let Some(sess) = s.as_mut().filter(|x| &x.id == sid) {
                    if let Some(f) = sess.files.get_mut(fid) {
                        f.done = true;
                    }
                    all_done = sess.files.values().all(|f| f.done);
                }
                if all_done {
                    s.take();
                }
                drop(s);
                self.host.file_done(sid, fid, &target);
                if all_done {
                    self.host.ended(sid, Ended::Done);
                }
                text(StatusCode::OK, "")
            }
            Err(why) if why == CANCELLED_HERE => {
                s.take();
                drop(s);
                // The id stays in `cancelled`: the sender may have lost the
                // connection before reading this reply, and asks again.
                self.host.ended(sid, Ended::Cancelled);
                text(StatusCode::FORBIDDEN, "Cancelled by the receiver")
            }
            Err(why) => {
                let checksum = why.contains("checksum");
                s.take();
                drop(s);
                self.host.ended(sid, Ended::Failed(why.clone()));
                if checksum {
                    text(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "Checksum mismatch (sha256)",
                    )
                } else {
                    text(StatusCode::BAD_REQUEST, &why)
                }
            }
        }
    }

    /// Streams one file's body into `part`, exactly `meta.size` bytes, checking
    /// the announced SHA-256 when there is one.
    async fn receive(
        &self,
        req: Request<Incoming>,
        part: &Path,
        meta: &FileMeta,
        sid: &str,
        fid: &str,
    ) -> Result<(), String> {
        let mut out = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(part)
            .await
            .map_err(|e| format!("couldn't write it ({e})"))?;
        let mut body = req.into_body();
        let mut written = 0u64;
        let mut hash = sha2::Sha256::new();
        let mut last_report = 0u64;
        while let Some(frame) = tokio::time::timeout(Duration::from_secs(60), body.frame())
            .await
            .map_err(|_| "the sender stopped sending".to_string())?
        {
            let frame = frame.map_err(|_| "the connection dropped".to_string())?;
            let Ok(data) = frame.into_data() else {
                continue;
            };
            written += data.len() as u64;
            if written > meta.size {
                return Err("it was bigger than announced".into());
            }
            hash.update(&data);
            out.write_all(&data)
                .await
                .map_err(|e| format!("couldn't write it ({e})"))?;
            if self
                .cancelled
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains(sid)
            {
                return Err(CANCELLED_HERE.into());
            }
            if written - last_report >= 256 * 1024 {
                last_report = written;
                self.host.progress(sid, fid, written);
            }
        }
        if written != meta.size {
            return Err("it was smaller than announced".into());
        }
        out.flush().await.map_err(|e| e.to_string())?;
        out.sync_all().await.map_err(|e| e.to_string())?;
        if let Some(want) = meta.sha256.as_deref().filter(|s| !s.is_empty()) {
            let got: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
            if !got.eq_ignore_ascii_case(want) {
                return Err("its checksum didn't match".into());
            }
        }
        self.host.progress(sid, fid, written);
        Ok(())
    }
}

/// A running receiver.
#[derive(Debug)]
pub struct Server {
    pub addr: SocketAddr,
    task: tokio::task::JoinHandle<()>,
    cancelled: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    /// Stops an incoming session from this side: the file being received
    /// stops, its partial file is removed, and the sender is told.
    pub fn cancel(&self, session: &str) {
        let mut c = self
            .cancelled
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Remembered so a sender asking again hears why; old ones are dropped.
        if c.len() >= 64 {
            c.clear();
        }
        c.insert(session.to_string());
    }

    /// Listens on `port` on every IPv4 address (the next free port if taken).
    pub async fn start<H: Host>(
        id: &crate::identity::Identity,
        host: Arc<H>,
        port: u16,
    ) -> std::io::Result<Server> {
        let tls =
            crate::tls::server_config(id).map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut listener = None;
        for p in [port, 0] {
            if let Ok(l) = tokio::net::TcpListener::bind(("0.0.0.0", p)).await {
                listener = Some(l);
                break;
            }
        }
        let listener = listener.ok_or_else(|| std::io::Error::other("no port"))?;
        let addr = listener.local_addr()?;
        let cancelled: Arc<Mutex<std::collections::HashSet<String>>> = Arc::default();
        let state = Arc::new(State {
            host,
            session: Mutex::new(None),
            cancelled: cancelled.clone(),
        });
        let acceptor = tokio_rustls::TlsAcceptor::from(tls);
        let task = tokio::spawn(async move {
            while let Ok((tcp, from)) = listener.accept().await {
                let (acceptor, state) = (acceptor.clone(), state.clone());
                tokio::spawn(async move {
                    let Ok(Ok(tls)) =
                        tokio::time::timeout(Duration::from_secs(10), acceptor.accept(tcp)).await
                    else {
                        return;
                    };
                    let svc = hyper::service::service_fn(move |req| {
                        let state = state.clone();
                        async move { Ok::<_, std::convert::Infallible>(state.handle(req, from).await) }
                    });
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(hyper_util::rt::TokioIo::new(tls), svc)
                        .await;
                });
            }
        });
        Ok(Server {
            addr,
            task,
            cancelled,
        })
    }
}
