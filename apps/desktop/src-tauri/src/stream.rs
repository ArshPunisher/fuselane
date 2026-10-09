//! Play while downloading (B8.10): a small HTTP server on 127.0.0.1 that serves
//! one torrent file as it arrives, with byte ranges so a player can seek. It
//! only listens on loopback, every link carries a random token, and the Host
//! header must be the loopback address, so a web page can't reach it through a
//! DNS trick. Each response closes its connection; players reconnect to seek.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use fuselane_engine_torrent::FileReader;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

/// Opens torrent `id`'s file `index`: the reader, its length and its name.
pub type OpenFn = Arc<
    dyn Fn(
            String,
            usize,
        )
            -> Pin<Box<dyn Future<Output = Option<(Box<dyn FileReader>, u64, String)>> + Send>>
        + Send
        + Sync,
>;

/// A running stream server.
#[derive(Debug, Clone)]
pub struct StreamServer {
    addr: SocketAddr,
    token: String,
}

impl StreamServer {
    /// Starts listening on a free loopback port.
    pub async fn start(open: OpenFn) -> std::io::Result<StreamServer> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        let mut raw = [0u8; 16];
        getrandom::fill(&mut raw).map_err(|e| std::io::Error::other(e.to_string()))?;
        let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        let me = StreamServer { addr, token };
        let server = me.clone();
        tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                let (server, open) = (server.clone(), open.clone());
                tokio::spawn(async move {
                    let _ = server.serve(sock, open).await;
                });
            }
        });
        Ok(me)
    }

    /// The link for one file; the name at the end is for players that show it.
    pub fn url(&self, id: &str, file: usize, name: &str) -> String {
        let safe: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || ".-_".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        format!("http://{}/{}/{id}/{file}/{safe}", self.addr, self.token)
    }

    async fn serve(&self, mut sock: tokio::net::TcpStream, open: OpenFn) -> std::io::Result<()> {
        let head = match tokio::time::timeout(Duration::from_secs(10), read_head(&mut sock)).await {
            Ok(Ok(h)) => h,
            _ => return Ok(()),
        };
        let Some(req) = parse_request(&head) else {
            return respond_empty(&mut sock, 400, "Bad Request").await;
        };
        let host_ok = req.host.as_deref().is_some_and(|h| {
            h == self.addr.to_string() || h == format!("localhost:{}", self.addr.port())
        });
        if !host_ok {
            return respond_empty(&mut sock, 403, "Forbidden").await;
        }
        if req.method != "GET" && req.method != "HEAD" {
            return respond_empty(&mut sock, 405, "Method Not Allowed").await;
        }
        let mut parts = req.path.trim_start_matches('/').split('/');
        let (Some(token), Some(id), Some(file)) = (parts.next(), parts.next(), parts.next()) else {
            return respond_empty(&mut sock, 404, "Not Found").await;
        };
        if !same(token, &self.token) {
            return respond_empty(&mut sock, 403, "Forbidden").await;
        }
        let Ok(file) = file.parse::<usize>() else {
            return respond_empty(&mut sock, 404, "Not Found").await;
        };
        let Some((mut reader, len, name)) = open(id.to_string(), file).await else {
            return respond_empty(&mut sock, 404, "Not Found").await;
        };
        let (status, first, last) = match req.range.as_deref().map(|r| parse_range(r, len)) {
            None => (200, 0, len.saturating_sub(1)),
            Some(Some((a, b))) => (206, a, b),
            Some(None) => {
                let head = format!(
                    "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{len}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                return sock.write_all(head.as_bytes()).await;
            }
        };
        let count = if len == 0 { 0 } else { last - first + 1 };
        let mut head = format!(
            "HTTP/1.1 {status} {}\r\nContent-Type: {}\r\nContent-Length: {count}\r\nAccept-Ranges: bytes\r\nCache-Control: no-store\r\nConnection: close\r\n",
            if status == 206 {
                "Partial Content"
            } else {
                "OK"
            },
            content_type(&name),
        );
        if status == 206 {
            head.push_str(&format!("Content-Range: bytes {first}-{last}/{len}\r\n"));
        }
        head.push_str("\r\n");
        sock.write_all(head.as_bytes()).await?;
        if req.method == "HEAD" || count == 0 {
            return Ok(());
        }
        reader.seek(std::io::SeekFrom::Start(first)).await?;
        let mut body = reader.take(count);
        tokio::io::copy(&mut body, &mut sock).await?;
        sock.flush().await
    }
}

async fn respond_empty(
    sock: &mut tokio::net::TcpStream,
    code: u16,
    why: &str,
) -> std::io::Result<()> {
    let head = format!("HTTP/1.1 {code} {why}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    sock.write_all(head.as_bytes()).await
}

/// Compares secrets without stopping at the first difference.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Reads up to the blank line ending the request head (at most 8 KB).
async fn read_head(sock: &mut tokio::net::TcpStream) -> std::io::Result<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 8192 {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[derive(Debug, PartialEq)]
struct Req {
    method: String,
    path: String,
    host: Option<String>,
    range: Option<String>,
}

fn parse_request(head: &str) -> Option<Req> {
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    if !first.next()?.starts_with("HTTP/1.") {
        return None;
    }
    let mut host = None;
    let mut range = None;
    for l in lines {
        let Some((k, v)) = l.split_once(':') else {
            continue;
        };
        match k.trim().to_ascii_lowercase().as_str() {
            "host" => host = Some(v.trim().to_string()),
            "range" => range = Some(v.trim().to_string()),
            _ => {}
        }
    }
    Some(Req {
        method,
        path,
        host,
        range,
    })
}

/// One byte range ("bytes=0-99", "bytes=100-", "bytes=-50") within `len`, as
/// first and last byte; None when it can't be served (416).
fn parse_range(h: &str, len: u64) -> Option<(u64, u64)> {
    let spec = h.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || len == 0 {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        return (n > 0).then(|| (len.saturating_sub(n), len - 1));
    }
    let first: u64 = a.parse().ok()?;
    let last = if b.is_empty() {
        len - 1
    } else {
        b.parse::<u64>().ok()?.min(len - 1)
    };
    (first <= last && first < len).then_some((first, last))
}

fn content_type(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "ts" => "video/mp2t",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        "ogg" | "opus" => "audio/ogg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// Media a player can usually start before the whole file is there.
pub fn playable(name: &str) -> bool {
    content_type(name) != "application/octet-stream"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_read_like_servers_read_them() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=900-", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=500-5000", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=5-1", 1000), None);
        assert_eq!(parse_range("bytes=0-1,5-6", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
    }

    #[test]
    fn requests_parse_and_media_is_recognised() {
        let r = parse_request(
            "GET /t/abc/2/x.mkv HTTP/1.1\r\nHost: 127.0.0.1:5\r\nRange: bytes=0-\r\n\r\n",
        )
        .unwrap();
        assert_eq!(
            r,
            Req {
                method: "GET".into(),
                path: "/t/abc/2/x.mkv".into(),
                host: Some("127.0.0.1:5".into()),
                range: Some("bytes=0-".into())
            }
        );
        assert!(parse_request("nonsense").is_none());
        assert!(playable("Movie.MKV") && playable("a.mp3") && !playable("a.iso"));
    }

    async fn get(
        addr: SocketAddr,
        path: &str,
        host: &str,
        range: Option<&str>,
    ) -> (u16, Vec<u8>, String) {
        let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
        let mut req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n");
        if let Some(r) = range {
            req.push_str(&format!("Range: {r}\r\n"));
        }
        req.push_str("\r\n");
        s.write_all(req.as_bytes()).await.unwrap();
        let mut out = vec![];
        s.read_to_end(&mut out).await.unwrap();
        let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&out[..split]).into_owned();
        let code = head[9..12].parse().unwrap();
        (code, out[split + 4..].to_vec(), head)
    }

    #[tokio::test]
    async fn it_serves_ranges_only_with_the_token_and_a_loopback_host() {
        let data: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let shared = Arc::new(data.clone());
        let open: OpenFn = Arc::new(move |id, file| {
            let d = shared.clone();
            Box::pin(async move {
                (id == "abc" && file == 1).then(|| {
                    let r: Box<dyn FileReader> = Box::new(std::io::Cursor::new(d.to_vec()));
                    (r, d.len() as u64, "clip.mp4".to_string())
                })
            })
        });
        let srv = StreamServer::start(open).await.unwrap();
        let url = srv.url("abc", 1, "clip (1).mp4");
        assert!(url.ends_with("/abc/1/clip__1_.mp4"));
        let path = url.trim_start_matches(&format!("http://{}", srv.addr));
        let host = srv.addr.to_string();
        let (code, body, head) = get(srv.addr, path, &host, None).await;
        assert_eq!((code, body.len()), (200, 10_000));
        assert!(head.contains("Content-Type: video/mp4"));
        let (code, body, head) = get(srv.addr, path, &host, Some("bytes=100-199")).await;
        assert_eq!((code, &body[..]), (206, &data[100..200]));
        assert!(head.contains("Content-Range: bytes 100-199/10000"));
        assert_eq!(
            get(srv.addr, path, &host, Some("bytes=20000-")).await.0,
            416
        );
        // A wrong token, an unknown file, and a non-loopback Host are refused.
        let bad = path.replacen(&srv.token, &"0".repeat(32), 1);
        assert_eq!(get(srv.addr, &bad, &host, None).await.0, 403);
        assert_eq!(
            get(srv.addr, &path.replace("/abc/1/", "/abc/9/"), &host, None)
                .await
                .0,
            404
        );
        assert_eq!(get(srv.addr, path, "evil.example:80", None).await.0, 403);
    }
}
