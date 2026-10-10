//! Talking to a running Fuselane's local API (the CLI, the native-messaging relay).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// Where the API listens: `<home>/api.sock`, or a per-user named pipe on Windows.
pub fn endpoint(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let _ = home;
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".into());
        let safe: String = user
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        // Debug builds have their own data folder (core::home), so their own pipe
        // too: a dev run never answers the installed app's CLI or extension.
        let dev = if cfg!(debug_assertions) { "-dev" } else { "" };
        PathBuf::from(format!(r"\\.\pipe\fuselane-api{dev}-{safe}"))
    }
    #[cfg(not(windows))]
    {
        let direct = home.join("api.sock");
        // Unix socket paths are limited (104 bytes on macOS, 108 on Linux). A long
        // home folder gets a short private folder instead, one per home.
        if direct.as_os_str().len() < MAX_SOCKET_PATH {
            return direct;
        }
        let hash = home
            .as_os_str()
            .as_encoded_bytes()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
            });
        // SAFETY: getuid has no preconditions and cannot fail.
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!(
            "/tmp/fuselane-{uid}-{:08x}/api.sock",
            hash & 0xffff_ffff
        ))
    }
}

/// Room left under the smallest limit (macOS: 104 bytes including the end).
#[cfg(not(windows))]
pub const MAX_SOCKET_PATH: usize = 100;

/// Sends one request and returns its `result`, or the error message.
pub async fn request(
    endpoint: &Path,
    method: &str,
    params: Value,
    wait: Duration,
) -> Result<Value, String> {
    let line = format!("{}\n", json!({"id": 1, "method": method, "params": params}));
    let reply = tokio::time::timeout(wait, async {
        #[cfg(unix)]
        let stream = tokio::net::UnixStream::connect(endpoint)
            .await
            .map_err(|e| format!("Fuselane isn't running ({e})"))?;
        #[cfg(windows)]
        let stream = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(endpoint.as_os_str())
            .map_err(|e| format!("Fuselane isn't running ({e})"))?;
        let (read, mut write) = tokio::io::split(stream);
        write
            .write_all(line.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut out = String::new();
        let mut r = BufReader::new(read).take(crate::server::MAX_LINE as u64);
        r.read_line(&mut out).await.map_err(|e| e.to_string())?;
        Ok::<_, String>(out)
    })
    .await
    .map_err(|_| "Fuselane didn't answer in time".to_string())??;
    let v: Value = serde_json::from_str(reply.trim())
        .map_err(|_| "Fuselane sent something unreadable".to_string())?;
    match (v.get("result"), v.get("error")) {
        (Some(r), _) => Ok(r.clone()),
        (None, Some(e)) => Err(e
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("error")
            .to_owned()),
        _ => Err("Fuselane sent an empty answer".into()),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::offer::Offer;
    use crate::server::{BoxFuture, Decline, Handler, start};
    use std::sync::Arc;

    struct Fake;
    impl Handler for Fake {
        fn offer(&self, _: Offer) -> BoxFuture<'_, Result<String, Decline>> {
            Box::pin(async { Ok("7".into()) })
        }
        fn version(&self) -> String {
            "1.2.3".into()
        }
    }

    #[test]
    fn long_homes_get_a_short_private_socket_path() {
        let short = Path::new("/Users/me/Library/Application Support/app.fuselane");
        assert_eq!(endpoint(short), short.join("api.sock"));
        let long = PathBuf::from(format!("/Users/{}/x", "a".repeat(120)));
        let ep = endpoint(&long);
        assert!(ep.as_os_str().len() < MAX_SOCKET_PATH, "{}", ep.display());
        assert!(ep.starts_with("/tmp"));
        let other = PathBuf::from(format!("/Users/{}/y", "a".repeat(120)));
        assert_ne!(endpoint(&other), ep, "different homes, different sockets");
        assert_eq!(endpoint(&long), ep, "stable");
    }

    #[tokio::test]
    async fn a_long_home_still_serves() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("a".repeat(90));
        std::fs::create_dir_all(&home).unwrap();
        let ep = endpoint(&home);
        start(&ep, Arc::new(Fake)).await.unwrap();
        assert_eq!(
            request(&ep, "ping", Value::Null, Duration::from_secs(2))
                .await
                .unwrap()["version"],
            "1.2.3"
        );
        let _ = std::fs::remove_file(&ep);
        let _ = std::fs::remove_dir(ep.parent().unwrap());
    }

    #[tokio::test]
    async fn a_request_reaches_the_server_and_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let ep = endpoint(dir.path());
        let wait = Duration::from_secs(2);
        assert!(
            request(&ep, "ping", Value::Null, wait)
                .await
                .unwrap_err()
                .contains("isn't running")
        );
        start(&ep, Arc::new(Fake)).await.unwrap();
        assert_eq!(
            request(&ep, "ping", Value::Null, wait).await.unwrap()["version"],
            "1.2.3"
        );
        let r = request(
            &ep,
            "download.offer",
            json!({"v":1,"type":"download.offer","url":"https://e.org/a"}),
            wait,
        )
        .await
        .unwrap();
        assert_eq!(r["jobId"], "7");
        assert_eq!(
            request(&ep, "nope", Value::Null, wait).await.unwrap_err(),
            "unknown method"
        );
    }
}
