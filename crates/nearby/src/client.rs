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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub mime: String,
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

/// Introduces this device to `t` and returns who it says it is.
pub async fn register(me: &DeviceInfo, t: &Target) -> Result<DeviceInfo, SendError> {
    let body = serde_json::to_vec(me).map_err(|e| SendError::Failed(e.to_string()))?;
    let (status, reply) = call(t, "/api/localsend/v2/register", full(body), None).await?;
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
        let file = tokio::fs::File::open(&f.path)
            .await
            .map_err(|e| SendError::Failed(format!("couldn't read {} ({e})", f.name)))?;
        let body = FileBody {
            file,
            left: f.size,
            sent: sent.clone(),
            cancel: cancel.clone(),
            buf: Vec::new(),
        };
        let q: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("sessionId", &accepted.session_id)
            .append_pair("fileId", &id)
            .append_pair("token", token)
            .finish();
        let path = format!("/api/localsend/v2/upload?{q}");
        let res = call(t, &path, body.boxed(), Some(f.size)).await;
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
