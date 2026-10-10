//! The local API server: one JSON object per line over a Unix socket (macOS,
//! Linux) or a named pipe (Windows), for this user only (ARCHITECTURE.md §2).
//!
//! Request:  {"id": 1, "method": "download.offer", "params": {...}}
//! Response: {"id": 1, "result": ...} or {"id": 1, "error": {"code", "message"}}

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::offer::{Offer, check_offer};

/// A line longer than this closes the connection (nothing legitimate is close).
pub const MAX_LINE: usize = 1024 * 1024;

/// Why the app hands a download back to the browser (schema v1 reasons).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decline {
    IpLocked,
    Unsupported,
    UserCancelled,
    Invalid,
}

impl Decline {
    fn word(self) -> &'static str {
        match self {
            Decline::IpLocked => "ip_locked",
            Decline::Unsupported => "unsupported",
            Decline::UserCancelled => "user_cancelled",
            Decline::Invalid => "invalid",
        }
    }
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// What the app does with requests (the desktop service, or a test double).
pub trait Handler: Send + Sync + 'static {
    /// Takes a checked offer; Ok(job id) or a reason to hand it back.
    fn offer(&self, offer: Offer) -> BoxFuture<'_, Result<String, Decline>>;
    /// The app's version, for `ping`.
    fn version(&self) -> String;
    /// A page whose video the person wants (the extension's "Get the video"):
    /// the app opens its video picker with it. False when it can't.
    fn page_video(&self, _url: String) -> bool {
        false
    }
    /// A feed the person wants to follow (the extension's "Follow this site's
    /// feed"): the app opens Feeds with it. False when it can't.
    fn page_feed(&self, _url: String) -> bool {
        false
    }
}

/// A page link from the extension: http(s) only, and not absurdly long.
fn page_url(params: &Value) -> Option<String> {
    let url = params.get("url")?.as_str()?.trim();
    let lower = url.get(..8).unwrap_or("").to_ascii_lowercase();
    (url.len() <= 8192 && (lower.starts_with("https://") || lower.starts_with("http://")))
        .then(|| url.to_string())
}

fn accepted(job_id: &str) -> Value {
    json!({"v": 1, "type": "download.accepted", "jobId": job_id})
}

fn declined(reason: Decline) -> Value {
    json!({"v": 1, "type": "download.declined", "reason": reason.word(), "fallback": "browser"})
}

fn error(id: &Value, code: &str, message: &str) -> Value {
    json!({"id": id, "error": {"code": code, "message": message}})
}

/// Answers one request line.
pub async fn answer(handler: &dyn Handler, line: &str) -> Value {
    let Ok(req) = serde_json::from_str::<Value>(line) else {
        return error(&Value::Null, "parse", "not JSON");
    };
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    if !(id.is_null() || id.is_u64() || id.as_str().is_some_and(|s| s.len() <= 64)) {
        return error(
            &Value::Null,
            "bad-request",
            "id must be a number or short text",
        );
    }
    match req.get("method").and_then(Value::as_str) {
        Some("ping") => {
            json!({"id": id, "result": {"app": "fuselane", "version": handler.version(), "v": 1}})
        }
        Some("download.offer") => {
            let params = req.get("params").cloned().unwrap_or(Value::Null);
            let result = match check_offer(&params) {
                Ok(offer) => match handler.offer(offer).await {
                    Ok(job) => accepted(&job),
                    Err(reason) => declined(reason),
                },
                // Refused before reaching the app; the browser keeps the download.
                Err(_) => declined(Decline::Invalid),
            };
            json!({"id": id, "result": result})
        }
        Some("page.video") => {
            let params = req.get("params").cloned().unwrap_or(Value::Null);
            let result = match page_url(&params) {
                Some(url) => {
                    if handler.page_video(url) {
                        json!({"v": 1, "type": "page.video.opened"})
                    } else {
                        declined(Decline::Unsupported)
                    }
                }
                None => declined(Decline::Invalid),
            };
            json!({"id": id, "result": result})
        }
        Some("page.feed") => {
            let params = req.get("params").cloned().unwrap_or(Value::Null);
            let result = match page_url(&params) {
                Some(url) => {
                    if handler.page_feed(url) {
                        json!({"v": 1, "type": "page.feed.opened"})
                    } else {
                        declined(Decline::Unsupported)
                    }
                }
                None => declined(Decline::Invalid),
            };
            json!({"id": id, "result": result})
        }
        Some(_) => error(&id, "unknown-method", "unknown method"),
        None => error(&id, "bad-request", "method is missing"),
    }
}

/// Serves one connection until it closes or misbehaves.
pub async fn serve_conn<S: AsyncRead + AsyncWrite + Unpin>(stream: S, handler: Arc<dyn Handler>) {
    let (read, mut write) = tokio::io::split(stream);
    let mut lines = BufReader::new(read);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        // Read one line, refusing to buffer more than MAX_LINE.
        let mut limited = (&mut lines).take(MAX_LINE as u64 + 1);
        match limited.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if buf.len() > MAX_LINE {
            let _ = write
                .write_all(
                    format!("{}\n", error(&Value::Null, "too-long", "message too long")).as_bytes(),
                )
                .await;
            return;
        }
        let Ok(line) = std::str::from_utf8(&buf) else {
            let _ = write
                .write_all(format!("{}\n", error(&Value::Null, "parse", "not UTF-8")).as_bytes())
                .await;
            continue;
        };
        if line.trim().is_empty() {
            continue;
        }
        let reply = answer(handler.as_ref(), line.trim()).await;
        if write
            .write_all(format!("{reply}\n").as_bytes())
            .await
            .is_err()
        {
            return;
        }
    }
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tokio::net::{UnixListener, UnixStream};

    /// Binds the socket for this user only. A socket left by a crashed app is
    /// replaced; one that answers belongs to a running Fuselane and is kept.
    pub async fn bind(path: &Path) -> std::io::Result<UnixListener> {
        if path.exists() {
            if UnixStream::connect(path).await.is_ok() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "Fuselane is already running",
                ));
            }
            std::fs::remove_file(path)?;
        }
        if let Some(dir) = path.parent() {
            // Fuselane's own short folder in the shared /tmp (see client::endpoint).
            let ours = dir.parent() == Some(Path::new("/tmp"))
                && dir
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("fuselane-"));
            if ours {
                private_dir(dir)?;
            } else {
                std::fs::create_dir_all(dir)?;
            }
        }
        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(listener)
    }

    /// The short socket folder under /tmp: created 0700, or refused when it exists
    /// as a link, someone else's, or open to others (it could hold a planted socket).
    fn private_dir(dir: &Path) -> std::io::Result<()> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        match std::fs::DirBuilder::new().mode(0o700).create(dir) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let m = std::fs::symlink_metadata(dir)?;
        // SAFETY: getuid has no preconditions and cannot fail.
        let me = unsafe { libc::getuid() };
        if !m.is_dir() || m.uid() != me || m.mode() & 0o077 != 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{} isn't a private folder of this user", dir.display()),
            ));
        }
        Ok(())
    }

    /// Accepts this user's connections only (the OS reports the peer's uid).
    pub async fn run(listener: UnixListener, handler: Arc<dyn Handler>) {
        // SAFETY: getuid has no preconditions and cannot fail.
        let me = unsafe { libc::getuid() };
        while let Ok((stream, _)) = listener.accept().await {
            match stream.peer_cred() {
                Ok(c) if c.uid() == me => {
                    tokio::spawn(serve_conn(stream, handler.clone()));
                }
                _ => drop(stream),
            }
        }
    }
}

#[cfg(unix)]
pub use unix::{bind, run};

/// Starts serving on `path` (Unix) in the background; the error says why not.
#[cfg(unix)]
pub async fn start(path: &Path, handler: Arc<dyn Handler>) -> std::io::Result<()> {
    let listener = bind(path).await?;
    tokio::spawn(run(listener, handler));
    Ok(())
}

/// Starts serving on the named pipe `name` (Windows), local clients only.
#[cfg(windows)]
pub async fn start(name: &Path, handler: Arc<dyn Handler>) -> std::io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;
    let name = name.as_os_str().to_owned();
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(&name)?;
    tokio::spawn(async move {
        loop {
            if server.connect().await.is_err() {
                return;
            }
            let Ok(next) = ServerOptions::new()
                .reject_remote_clients(true)
                .create(&name)
            else {
                return;
            };
            let conn = std::mem::replace(&mut server, next);
            tokio::spawn(serve_conn(conn, handler.clone()));
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;
    impl Handler for Fake {
        fn offer(&self, offer: Offer) -> BoxFuture<'_, Result<String, Decline>> {
            Box::pin(async move {
                if offer.needs_session() {
                    Err(Decline::Unsupported)
                } else {
                    Ok("42".into())
                }
            })
        }
        fn version(&self) -> String {
            "9.9.9".into()
        }
        fn page_video(&self, url: String) -> bool {
            url.contains("youtube")
        }
        fn page_feed(&self, url: String) -> bool {
            url.contains("feed")
        }
    }

    #[tokio::test]
    async fn a_feed_opens_feeds_and_bad_links_are_refused() {
        let ok = answer(
            &Fake,
            r#"{"id":1,"method":"page.feed","params":{"url":"https://blog.example/feed.xml"}}"#,
        )
        .await;
        assert_eq!(ok["result"]["type"], "page.feed.opened");
        let no = answer(
            &Fake,
            r#"{"id":2,"method":"page.feed","params":{"url":"https://example.org/"}}"#,
        )
        .await;
        assert_eq!(no["result"]["reason"], "unsupported");
        let bad = answer(
            &Fake,
            r#"{"id":3,"method":"page.feed","params":{"url":"file:///etc/feed"}}"#,
        )
        .await;
        assert_eq!(bad["result"]["reason"], "invalid");
    }

    #[tokio::test]
    async fn a_video_page_opens_the_picker_and_bad_links_are_refused() {
        let ok = answer(&Fake, r#"{"id":1,"method":"page.video","params":{"url":"https://www.youtube.com/watch?v=x"}}"#).await;
        assert_eq!(ok["result"]["type"], "page.video.opened");
        let no = answer(
            &Fake,
            r#"{"id":2,"method":"page.video","params":{"url":"https://example.org/"}}"#,
        )
        .await;
        assert_eq!(no["result"]["reason"], "unsupported");
        let bad = answer(
            &Fake,
            r#"{"id":3,"method":"page.video","params":{"url":"javascript:alert(1)"}}"#,
        )
        .await;
        assert_eq!(bad["result"]["reason"], "invalid");
        let missing = answer(&Fake, r#"{"id":4,"method":"page.video"}"#).await;
        assert_eq!(missing["result"]["reason"], "invalid");
    }

    const OFFER: &str = r#"{"v":1,"type":"download.offer","url":"https://example.org/big.iso"}"#;

    #[tokio::test]
    async fn requests_get_answers_and_bad_ones_get_errors() {
        let ping = answer(&Fake, r#"{"id":1,"method":"ping"}"#).await;
        assert_eq!(ping["result"]["version"], "9.9.9");
        let ok = answer(
            &Fake,
            &format!(r#"{{"id":"a","method":"download.offer","params":{OFFER}}}"#),
        )
        .await;
        assert_eq!(ok, json!({"id": "a", "result": accepted("42")}));
        let session = answer(&Fake, r#"{"id":2,"method":"download.offer","params":{"v":1,"type":"download.offer","url":"https://e.org/a","cookies":"s=1"}}"#).await;
        assert_eq!(session["result"]["reason"], "unsupported");
        let hostile = answer(&Fake, r#"{"id":3,"method":"download.offer","params":{"v":1,"type":"download.offer","url":"file:///etc/passwd"}}"#).await;
        assert_eq!(hostile["result"], declined(Decline::Invalid));
        assert_eq!(answer(&Fake, "not json").await["error"]["code"], "parse");
        assert_eq!(
            answer(&Fake, r#"{"id":4,"method":"rm -rf"}"#).await["error"]["code"],
            "unknown-method"
        );
        assert_eq!(
            answer(&Fake, r#"{"id":5}"#).await["error"]["code"],
            "bad-request"
        );
        assert_eq!(
            answer(&Fake, r#"{"id":{"x":1},"method":"ping"}"#).await["error"]["code"],
            "bad-request"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_socket_is_private_and_speaks_one_json_per_line() {
        use std::os::unix::fs::PermissionsExt;
        use tokio::io::AsyncReadExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api.sock");
        start(&path, Arc::new(Fake)).await.unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "only this user can connect");

        let mut c = tokio::net::UnixStream::connect(&path).await.unwrap();
        c.write_all(b"{\"id\":1,\"method\":\"ping\"}\n\n{\"id\":2,\"method\":\"nope\"}\n")
            .await
            .unwrap();
        let mut r = BufReader::new(&mut c);
        let mut l = String::new();
        r.read_line(&mut l).await.unwrap();
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap()["id"], 1);
        l.clear();
        r.read_line(&mut l).await.unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&l).unwrap()["error"]["code"],
            "unknown-method"
        );

        // A second Fuselane can't take the socket over while this one answers.
        let again = start(&path, Arc::new(Fake)).await;
        assert_eq!(again.unwrap_err().kind(), std::io::ErrorKind::AddrInUse);

        // An endless line is cut off, not buffered.
        let mut c2 = tokio::net::UnixStream::connect(&path).await.unwrap();
        let _ = c2.write_all(&vec![b'a'; MAX_LINE + 10]).await;
        let mut out = Vec::new();
        let _ =
            tokio::time::timeout(std::time::Duration::from_secs(5), c2.read_to_end(&mut out)).await;
        assert!(String::from_utf8_lossy(&out).contains("too-long"));
    }

    #[cfg(unix)]
    #[test]
    fn a_shared_tmp_folder_that_isnt_private_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let name = format!("/tmp/fuselane-test-{}-{}", std::process::id(), line!());
        let dir = Path::new(&name);
        std::fs::create_dir(dir).unwrap();
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let r = rt.block_on(bind(&dir.join("api.sock")));
        assert_eq!(r.unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            rt.block_on(bind(&dir.join("api.sock"))).is_ok(),
            "private: fine"
        );
        let _ = std::fs::remove_file(dir.join("api.sock"));
        let _ = std::fs::remove_dir(dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_socket_left_by_a_crash_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api.sock");
        // A dead socket file: bound once, never served.
        drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
        assert!(path.exists());
        start(&path, Arc::new(Fake)).await.unwrap();
        assert!(tokio::net::UnixStream::connect(&path).await.is_ok());
    }
}
