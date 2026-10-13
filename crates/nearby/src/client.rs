//! The sending side: introduce ourselves, ask to send, then stream each file,
//! over TLS pinned to the receiver's announced fingerprint.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Body, Frame};
use hyper::{Request, StatusCode};
use tokio::io::{AsyncRead, ReadBuf};

use crate::proto::{self, DeviceInfo, FileMeta, PrepareUpload, PrepareUploadReply};
use crate::tls::Pinned;

/// Why sending didn't work, in words the window can show.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    #[error("the device didn't answer")]
    Unreachable,
    #[error("the device answered with a different identity than it announced")]
    NotTheDevice,
    #[error("the other person declined")]
    Declined,
    #[error("the device is busy receiving something else")]
    Busy,
    #[error("the device refused it ({0})")]
    Refused(u16),
    #[error("{0}")]
    Failed(String),
    #[error("cancelled")]
    Cancelled,
    #[error("the other person cancelled it")]
    CancelledByThem,
}

/// A device to send to: where it listens, and the fingerprint it announced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub addr: SocketAddr,
    pub fingerprint: Option<String>,
}

/// A file to send.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outgoing {
    pub path: PathBuf,
    pub name: String,
    /// Bytes sent: the file's size, or the sum of `ranges`.
    pub size: u64,
    pub mime: String,
    /// Only these (offset, length) parts of the file, one after another; empty
    /// sends all of it. A download handed over sends just the bytes it has.
    pub ranges: Vec<(u64, u64)>,
}

type Conn = hyper::client::conn::http1::SendRequest<BoxedBody>;
type BoxedBody = http_body_util::combinators::BoxBody<Bytes, std::io::Error>;

async fn connect(t: &Target) -> Result<Conn, SendError> {
    let pinned = Pinned::new(t.fingerprint.as_deref());
    let cfg =
        crate::tls::client_config(pinned.clone()).map_err(|e| SendError::Failed(e.to_string()))?;
    let tcp = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect(t.addr),
    )
    .await
    .map_err(|_| SendError::Unreachable)?
    .map_err(|_| SendError::Unreachable)?;
    let name = rustls::pki_types::ServerName::IpAddress(t.addr.ip().into());
    let tls = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_rustls::TlsConnector::from(cfg).connect(name, tcp),
    )
    .await
    .map_err(|_| SendError::Unreachable)?
    .map_err(|_| {
        // A certificate other than the announced one is the interesting failure.
        if pinned
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        {
            SendError::NotTheDevice
        } else {
            SendError::Unreachable
        }
    })?;
    let (send, conn) = hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(tls))
        .await
        .map_err(|_| SendError::Unreachable)?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    Ok(send)
}

fn full(b: Vec<u8>) -> BoxedBody {
    Full::new(Bytes::from(b)).map_err(|n| match n {}).boxed()
}

async fn call(
    t: &Target,
    path: &str,
    body: BoxedBody,
    len: Option<u64>,
) -> Result<(StatusCode, Bytes), SendError> {
    let mut c = connect(t).await?;
    let mut req = Request::post(path)
        .header(hyper::header::HOST, t.addr.to_string())
        .header(hyper::header::CONTENT_TYPE, "application/json");
    if let Some(l) = len {
        req = req.header(hyper::header::CONTENT_LENGTH, l);
    }
    let req = req
        .body(body)
        .map_err(|e| SendError::Failed(e.to_string()))?;
    let res = c
        .send_request(req)
        .await
        .map_err(|_| SendError::Unreachable)?;
    let status = res.status();
    let body = Limited::new(res.into_body(), proto::MAX_JSON)
        .collect()
        .await
        .map(http_body_util::Collected::to_bytes)
        .unwrap_or_default();
    Ok((status, body))
}

/// How long an introduction may take in all. The other side checks who we are
/// by connecting back (up to 6 s), so this leaves room for that, but a device
/// that takes the connection and never answers can't hold anyone up.
const REGISTER_FOR: Duration = Duration::from_secs(15);

/// Introduces this device to `t` and returns who it says it is.
pub async fn register(me: &DeviceInfo, t: &Target) -> Result<DeviceInfo, SendError> {
    let body = serde_json::to_vec(me).map_err(|e| SendError::Failed(e.to_string()))?;
    let (status, reply) = tokio::time::timeout(
        REGISTER_FOR,
        call(t, "/api/localsend/v2/register", full(body), None),
    )
    .await
    .map_err(|_| SendError::Unreachable)??;
    if !status.is_success() {
        return Err(SendError::Refused(status.as_u16()));
    }
    proto::parse_info(&reply).map_err(|e| SendError::Failed(e.to_string()))
}

/// A file read as a request body, counting bytes as they go and stopping on cancel.
struct FileBody {
    file: tokio::fs::File,
    left: u64,
    sent: Arc<std::sync::atomic::AtomicU64>,
    cancel: Arc<AtomicBool>,
    buf: Vec<u8>,
}

impl Body for FileBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, std::io::Error>>> {
        if self.cancel.load(Ordering::Relaxed) {
            return Poll::Ready(Some(Err(std::io::Error::other("cancelled"))));
        }
        if self.left == 0 {
            return Poll::Ready(None);
        }
        let want = self.left.min(256 * 1024) as usize;
        let me = &mut *self;
        me.buf.resize(want, 0);
        let mut rb = ReadBuf::new(&mut me.buf);
        match Pin::new(&mut me.file).poll_read(cx, &mut rb) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
            Poll::Ready(Ok(())) => {
                let n = rb.filled().len();
                if n == 0 {
                    return Poll::Ready(Some(Err(std::io::Error::other("the file got shorter"))));
                }
                me.left -= n as u64;
                me.sent.fetch_add(n as u64, Ordering::Relaxed);
                Poll::Ready(Some(Ok(Frame::data(Bytes::copy_from_slice(&me.buf[..n])))))
            }
        }
    }
}

/// Streams chosen parts of a file, read in order by a task of its own.
struct RangedBody {
    rx: tokio::sync::mpsc::Receiver<std::io::Result<Bytes>>,
    sent: Arc<std::sync::atomic::AtomicU64>,
    cancel: Arc<AtomicBool>,
}

impl RangedBody {
    fn new(
        path: PathBuf,
        ranges: Vec<(u64, u64)>,
        sent: Arc<std::sync::atomic::AtomicU64>,
        cancel: Arc<AtomicBool>,
    ) -> RangedBody {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let stop = cancel.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncSeekExt};
            let mut file = match tokio::fs::File::open(&path).await {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            };
            for (offset, len) in ranges {
                if let Err(e) = file.seek(std::io::SeekFrom::Start(offset)).await {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
                let mut left = len;
                while left > 0 {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let mut buf = vec![0u8; left.min(256 * 1024) as usize];
                    let got = match file.read(&mut buf).await {
                        Ok(0) => Err(std::io::Error::other("the file got shorter")),
                        Ok(n) => Ok(n),
                        Err(e) => Err(e),
                    };
                    match got {
                        Ok(n) => {
                            buf.truncate(n);
                            left -= n as u64;
                            if tx.send(Ok(Bytes::from(buf))).await.is_err() {
                                return; // the upload ended
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                            return;
                        }
                    }
                }
            }
        });
        RangedBody { rx, sent, cancel }
    }
}

impl Body for RangedBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, std::io::Error>>> {
        if self.cancel.load(Ordering::Relaxed) {
            return Poll::Ready(Some(Err(std::io::Error::other("cancelled"))));
        }
        match self.rx.poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(Some(Ok(b))) => {
                self.sent.fetch_add(b.len() as u64, Ordering::Relaxed);
                Poll::Ready(Some(Ok(Frame::data(b))))
            }
        }
    }
}

/// Sends `files` to `t`: asks first, then streams each accepted file.
/// `sent` counts bytes across all files; `cancel` stops it.
pub async fn send(
    me: &DeviceInfo,
    t: &Target,
    files: &[Outgoing],
    sent: Arc<std::sync::atomic::AtomicU64>,
    cancel: Arc<AtomicBool>,
) -> Result<usize, SendError> {
    let mut offer = PrepareUpload {
        info: me.clone(),
        files: Default::default(),
    };
    for (i, f) in files.iter().enumerate() {
        let id = format!("f{i}");
        offer.files.insert(
            id.clone(),
            FileMeta {
                id,
                file_name: f.name.clone(),
                size: f.size,
                file_type: f.mime.clone(),
                sha256: None,
                preview: None,
                folder: None,
            },
        );
    }
    let body = serde_json::to_vec(&offer).map_err(|e| SendError::Failed(e.to_string()))?;
    let (status, reply) = call(t, "/api/localsend/v2/prepare-upload", full(body), None).await?;
    match status.as_u16() {
        200 => {}
        204 => return Ok(0),
        403 => return Err(SendError::Declined),
        409 => return Err(SendError::Busy),
        c => return Err(SendError::Refused(c)),
    }
    let accepted: PrepareUploadReply = serde_json::from_slice(&reply)
        .map_err(|_| SendError::Failed("the device's answer couldn't be read".into()))?;
    let mut count = 0;
    for (i, f) in files.iter().enumerate() {
        let id = format!("f{i}");
        let Some(token) = accepted.files.get(&id) else {
            continue;
        };
        if cancel.load(Ordering::Relaxed) {
            let _ = cancel_session(t, &accepted.session_id).await;
            return Err(SendError::Cancelled);
        }
        let body = if f.ranges.is_empty() {
            let file = tokio::fs::File::open(&f.path)
                .await
                .map_err(|e| SendError::Failed(format!("couldn't read {} ({e})", f.name)))?;
            FileBody {
                file,
                left: f.size,
                sent: sent.clone(),
                cancel: cancel.clone(),
                buf: Vec::new(),
            }
            .boxed()
        } else {
            RangedBody::new(
                f.path.clone(),
                f.ranges.clone(),
                sent.clone(),
                cancel.clone(),
            )
            .boxed()
        };
        let q: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("sessionId", &accepted.session_id)
            .append_pair("fileId", &id)
            .append_pair("token", token)
            .finish();
        let path = format!("/api/localsend/v2/upload?{q}");
        let res = call(t, &path, body, Some(f.size)).await;
        if cancel.load(Ordering::Relaxed) {
            let _ = cancel_session(t, &accepted.session_id).await;
            return Err(SendError::Cancelled);
        }
        // The receiver cancelling closes the connection mid-file, which can
        // look like a network failure here. Asking once more tells them apart.
        let (status, body) = match res {
            Err(SendError::Unreachable) => match call(t, &path, full(vec![]), Some(0)).await {
                Ok((s, b)) if s == StatusCode::FORBIDDEN && b.starts_with(b"Cancelled") => (s, b),
                _ => return Err(SendError::Unreachable),
            },
            other => other?,
        };
        if status == StatusCode::FORBIDDEN && body.starts_with(b"Cancelled") {
            return Err(SendError::CancelledByThem);
        }
        if !status.is_success() {
            return Err(SendError::Refused(status.as_u16()));
        }
        count += 1;
    }
    Ok(count)
}

/// Sends a text message (LocalSend-style: the text rides in the offer and the
/// receiver shows or copies it). A receiver that asks for it as a file gets it
/// uploaded as `message.txt`.
pub async fn send_text(me: &DeviceInfo, t: &Target, text: &str) -> Result<(), SendError> {
    if text.is_empty() || text.len() > crate::proto::MAX_TEXT {
        return Err(SendError::Failed("text must be 1 byte to 64 KB".into()));
    }
    let mut offer = PrepareUpload {
        info: me.clone(),
        files: Default::default(),
    };
    offer.files.insert(
        "t0".into(),
        FileMeta {
            id: "t0".into(),
            file_name: "message.txt".into(),
            size: text.len() as u64,
            file_type: "text/plain".into(),
            sha256: None,
            preview: Some(text.to_string()),
            folder: None,
        },
    );
    let body = serde_json::to_vec(&offer).map_err(|e| SendError::Failed(e.to_string()))?;
    let (status, reply) = call(t, "/api/localsend/v2/prepare-upload", full(body), None).await?;
    match status.as_u16() {
        204 => return Ok(()),
        200 => {}
        403 => return Err(SendError::Declined),
        409 => return Err(SendError::Busy),
        c => return Err(SendError::Refused(c)),
    }
    let accepted: PrepareUploadReply = serde_json::from_slice(&reply)
        .map_err(|_| SendError::Failed("the device's answer couldn't be read".into()))?;
    let Some(token) = accepted.files.get("t0") else {
        return Ok(());
    };
    let q: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("sessionId", &accepted.session_id)
        .append_pair("fileId", "t0")
        .append_pair("token", token)
        .finish();
    let bytes = text.as_bytes().to_vec();
    let len = bytes.len() as u64;
    let (status, _) = call(
        t,
        &format!("/api/localsend/v2/upload?{q}"),
        full(bytes),
        Some(len),
    )
    .await?;
    if status.is_success() {
        Ok(())
    } else {
        Err(SendError::Refused(status.as_u16()))
    }
}

async fn cancel_session(t: &Target, session: &str) -> Result<(), SendError> {
    let q: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("sessionId", session)
        .finish();
    call(
        t,
        &format!("/api/localsend/v2/cancel?{q}"),
        full(vec![]),
        None,
    )
    .await
    .map(|_| ())
}
