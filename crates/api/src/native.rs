//! The browser extension's native-messaging host: Fuselane's CLI started by the
//! browser, relaying messages to the app's local API (BROWSER-EXTENSION.md §2).
//! Messages are 32-bit native-endian length + JSON, both ways.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Nothing the extension sends is anywhere near this (an offer is a few KiB).
pub const MAX_IN: usize = 1024 * 1024;
/// Chrome refuses host messages over 1 MB.
pub const MAX_OUT: usize = 1024 * 1024;

/// Was this process started by a browser as its native-messaging host?
/// Chrome passes the caller's origin (`chrome-extension://<id>/`); Firefox passes
/// the manifest's path and the extension's id. `--native-messaging` forces it.
pub fn is_host_invocation(args: &[String]) -> bool {
    args.iter()
        .skip(1)
        .any(|a| a == "--native-messaging" || a.starts_with("chrome-extension://"))
        || (args.len() >= 3 && args[1].ends_with(".json") && args[2].contains('@'))
}

fn decline(reason: &str) -> Value {
    json!({"v": 1, "type": "download.declined", "reason": reason, "fallback": "browser"})
}

async fn read_msg<R: AsyncRead + Unpin>(r: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let n = u32::from_ne_bytes(len) as usize;
    if n > MAX_IN {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

async fn write_msg<W: AsyncWrite + Unpin>(w: &mut W, v: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(v).unwrap_or_default();
    let body = if body.len() > MAX_OUT {
        serde_json::to_vec(&decline("unsupported")).unwrap_or_default()
    } else {
        body
    };
    let len = u32::try_from(body.len()).unwrap_or(0).to_ne_bytes();
    w.write_all(&len).await?;
    w.write_all(&body).await?;
    w.flush().await
}

/// What to tell the extension for one of its messages.
async fn handle(endpoint: &Path, msg: &[u8]) -> Value {
    let Ok(v) = serde_json::from_slice::<Value>(msg) else {
        return decline("invalid");
    };
    let wait = Duration::from_secs(3);
    match v.get("type").and_then(Value::as_str) {
        Some("download.offer") => {
            match crate::client::request(endpoint, "download.offer", v, wait).await {
                Ok(reply) => reply,
                // The app isn't running or didn't answer: the browser keeps the download.
                Err(_) => decline("unsupported"),
            }
        }
        Some("ping") => match crate::client::request(endpoint, "ping", Value::Null, wait).await {
            Ok(r) => json!({"v": 1, "type": "pong", "app": r}),
            Err(e) => json!({"v": 1, "type": "pong", "error": e}),
        },
        _ => decline("invalid"),
    }
}

/// Relays until the browser closes the pipe. An oversized or broken frame ends it.
pub async fn relay<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    endpoint: &Path,
    mut input: R,
    mut output: W,
) -> std::io::Result<()> {
    while let Some(msg) = read_msg(&mut input).await? {
        let reply = handle(endpoint, &msg).await;
        write_msg(&mut output, &reply).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(v: &[u8]) -> Vec<u8> {
        let mut f = u32::try_from(v.len()).unwrap().to_ne_bytes().to_vec();
        f.extend_from_slice(v);
        f
    }

    fn unframe(mut b: &[u8]) -> Vec<Value> {
        let mut out = Vec::new();
        while b.len() >= 4 {
            let n = u32::from_ne_bytes(b[..4].try_into().unwrap()) as usize;
            out.push(serde_json::from_slice(&b[4..4 + n]).unwrap());
            b = &b[4 + n..];
        }
        out
    }

    #[test]
    fn browsers_are_recognised_by_how_they_start_the_host() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(is_host_invocation(&a(&[
            "fuselane",
            "chrome-extension://abcdef/"
        ])));
        assert!(is_host_invocation(&a(&[
            "fuselane",
            "chrome-extension://abcdef/",
            "--parent-window=123"
        ])));
        assert!(is_host_invocation(&a(&[
            "fuselane",
            "/home/u/.mozilla/native-messaging-hosts/app.fuselane.host.json",
            "fuselane@fuselane.app"
        ])));
        assert!(is_host_invocation(&a(&["fuselane", "--native-messaging"])));
        assert!(!is_host_invocation(&a(&[
            "fuselane",
            "get",
            "https://e.org/a"
        ])));
        assert!(!is_host_invocation(&a(&["fuselane"])));
    }

    #[tokio::test]
    async fn with_no_app_running_every_offer_goes_back_to_the_browser() {
        let dir = tempfile::tempdir().unwrap();
        let mut input = frame(br#"{"v":1,"type":"download.offer","url":"https://e.org/a"}"#);
        input.extend(frame(b"not json"));
        input.extend(frame(br#"{"type":"something else"}"#));
        let mut out = Vec::new();
        relay(&crate::client::endpoint(dir.path()), &input[..], &mut out)
            .await
            .unwrap();
        let replies = unframe(&out);
        assert_eq!(replies.len(), 3);
        assert_eq!(replies[0]["reason"], "unsupported");
        assert_eq!(replies[1]["reason"], "invalid");
        assert_eq!(replies[2]["reason"], "invalid");
        assert!(replies.iter().all(|r| r["fallback"] == "browser"));
    }

    #[tokio::test]
    async fn an_oversized_frame_ends_the_relay_without_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let input = u32::try_from(MAX_IN + 1).unwrap().to_ne_bytes().to_vec();
        let mut out = Vec::new();
        let r = relay(&crate::client::endpoint(dir.path()), &input[..], &mut out).await;
        assert_eq!(r.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        assert!(out.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn offers_reach_the_app_through_the_relay() {
        use crate::offer::Offer;
        use crate::server::{BoxFuture, Decline, Handler, start};
        struct App;
        impl Handler for App {
            fn offer(&self, o: Offer) -> BoxFuture<'_, Result<String, Decline>> {
                Box::pin(async move {
                    if o.url.ends_with(".iso") {
                        Ok("11".into())
                    } else {
                        Err(Decline::Unsupported)
                    }
                })
            }
            fn version(&self) -> String {
                "0.1.0".into()
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let ep = crate::client::endpoint(dir.path());
        start(&ep, std::sync::Arc::new(App)).await.unwrap();
        let mut input = frame(br#"{"v":1,"type":"download.offer","url":"https://e.org/big.iso"}"#);
        input.extend(frame(
            br#"{"v":1,"type":"download.offer","url":"https://e.org/page"}"#,
        ));
        input.extend(frame(
            br#"{"v":1,"type":"download.offer","url":"javascript:alert(1)"}"#,
        ));
        input.extend(frame(br#"{"type":"ping"}"#));
        let mut out = Vec::new();
        relay(&ep, &input[..], &mut out).await.unwrap();
        let r = unframe(&out);
        assert_eq!(r[0], json!({"v":1,"type":"download.accepted","jobId":"11"}));
        assert_eq!(r[1]["reason"], "unsupported");
        assert_eq!(r[2]["reason"], "invalid");
        assert_eq!(r[3]["app"]["version"], "0.1.0");
    }
}
