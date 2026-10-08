//! The app's side of the local API: what happens to a download the browser
//! extension (or another local tool) hands over.

use std::sync::Arc;

use fuselane_api::offer::Offer;
use fuselane_api::server::{BoxFuture, Decline, Handler};

use crate::service::Service;

pub struct ApiBridge {
    pub svc: Arc<Service>,
}

impl Handler for ApiBridge {
    fn offer(&self, offer: Offer) -> BoxFuture<'_, Result<String, Decline>> {
        Box::pin(async move {
            if offer
                .url
                .get(..7)
                .is_some_and(|s| s.eq_ignore_ascii_case("magnet:"))
            {
                // A magnet needs the user to pick files: open the dialog with it.
                self.svc.open_request(offer.url);
                return Ok("open".into());
            }
            let link = offer.final_url.as_deref().unwrap_or(&offer.url);
            // The browser's session (cookies, referrer, User-Agent) goes with the
            // download, so files behind a sign-in work too. Anything the engine won't
            // send means the browser keeps the download.
            let headers = offer.session_headers();
            let Ok(session) = fuselane_engine_http::download::Headers::checked(&headers) else {
                return Err(Decline::Unsupported);
            };
            // Fuselane looks at the link itself first. Take it when the size matches
            // what the browser saw; with no size (a right-click), when the server
            // sends a file rather than a web page (often its sign-in page).
            let Ok((p, web_page)) = crate::service::preview_with(link, session).await else {
                return Err(Decline::Unsupported);
            };
            let same_file = match offer.size {
                Some(size) => p.total == Some(size),
                None => !web_page,
            };
            if !same_file {
                return Err(Decline::Unsupported);
            }
            self.svc
                // Asked for in the browser just now: a repeat is deliberate.
                .add_with(
                    link,
                    None,
                    &crate::service::AddRequest {
                        allow_duplicate: true,
                        headers,
                        ..Default::default()
                    },
                )
                .map(|id| id.to_string())
                .map_err(|_| Decline::Unsupported)
        })
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::UiEvent;
    use fuselane_api::offer::check_offer;
    use std::sync::Mutex;

    fn bridge() -> (ApiBridge, Arc<Mutex<Vec<UiEvent>>>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("jobs.db")).unwrap();
        let svc = Service::with_max_running(store, dir.path().to_path_buf(), 0).unwrap();
        let events: Arc<Mutex<Vec<UiEvent>>> = Arc::default();
        let sink = events.clone();
        svc.subscribe(Arc::new(move |e| sink.lock().unwrap().push(e)));
        (ApiBridge { svc }, events, dir)
    }

    fn offer(v: serde_json::Value) -> Offer {
        check_offer(&v).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_web_link_is_taken_only_when_fuselane_sees_the_same_file() {
        use fuselane_testkit::{Content, RangeServer};
        let server = RangeServer::start(Content::new(300_000, 3)).await.unwrap();
        let url = format!("http://{}{}", server.addr(), server.path());
        let (b, _events, _dir) = bridge();
        let ask = |size: Option<u64>, final_url: Option<&str>| {
            let mut v = serde_json::json!({"v": 1, "type": "download.offer", "url": "http://127.0.0.1:9/landing"});
            let o = v.as_object_mut().unwrap();
            o.insert(
                "finalUrl".into(),
                final_url.map_or(serde_json::Value::Null, |u| u.into()),
            );
            o.insert(
                "size".into(),
                size.map_or(serde_json::Value::Null, |s| s.into()),
            );
            offer(v)
        };
        // Same size from the final URL (after redirects): taken, from that URL.
        let id = b.offer(ask(Some(300_000), Some(&url))).await.unwrap();
        let job = b
            .svc
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id.to_string() == id)
            .unwrap();
        assert_eq!(job.url, url);
        // A different size (a login page, say) or a dead link: the browser keeps it.
        assert_eq!(
            b.offer(ask(Some(299_999), Some(&url))).await,
            Err(Decline::Unsupported)
        );
        assert_eq!(
            b.offer(ask(Some(300_000), None)).await,
            Err(Decline::Unsupported)
        );
        assert_eq!(b.svc.jobs().unwrap().len(), 1);
        // With no size (a right-click), a server that sends a file: taken.
        assert!(b.offer(ask(None, Some(&url))).await.is_ok());
        assert_eq!(b.svc.jobs().unwrap().len(), 2);
    }

    /// Answers every request with a small HTML page, like a sign-in screen.
    async fn web_page_server() -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let _ = s.read(&mut buf).await;
                    let body = "<html><body>Please sign in</body></html>";
                    let _ = s
                        .write_all(
                            format!(
                                "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\n\r\n{body}",
                                body.len()
                            )
                            .as_bytes(),
                        )
                        .await;
                });
            }
        });
        format!("http://{addr}/private.zip")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_right_click_on_a_page_that_wants_a_sign_in_stays_in_the_browser() {
        let (b, _events, _dir) = bridge();
        let url = web_page_server().await;
        let v = serde_json::json!({"v": 1, "type": "download.offer", "url": url, "source": "contextMenu"});
        assert_eq!(b.offer(offer(v)).await, Err(Decline::Unsupported));
        assert!(b.svc.jobs().unwrap().is_empty(), "nothing was added");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_browsers_session_goes_with_the_download() {
        use fuselane_testkit::{Content, RangeServer};
        let content = Content::new(200_000, 4);
        let server = RangeServer::start(content).await.unwrap();
        let url = format!("http://{}{}", server.addr(), server.path());
        let (b, _events, _dir) = bridge();
        let v = serde_json::json!({
            "v": 1, "type": "download.offer", "url": url, "size": 200_000,
            "cookies": "session=s3cret", "referrer": "https://example.org/files",
            "userAgent": "Mozilla/5.0 (Test)",
        });
        let id = b.offer(offer(v)).await.unwrap();
        let t = std::time::Instant::now();
        while b
            .svc
            .jobs()
            .unwrap()
            .iter()
            .find(|j| j.id.to_string() == id)
            .is_some_and(|j| j.status != "completed")
        {
            assert!(
                t.elapsed() < std::time::Duration::from_secs(20),
                "never finished"
            );
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let log = server.requests();
        assert!(!log.is_empty());
        for r in &log {
            assert_eq!(
                r.cookie.as_deref(),
                Some("session=s3cret"),
                "every request, preview included"
            );
            assert_eq!(r.referer.as_deref(), Some("https://example.org/files"));
            assert_eq!(r.user_agent.as_deref(), Some("Mozilla/5.0 (Test)"));
        }
    }

    #[tokio::test]
    async fn magnets_open_the_dialog() {
        let (b, events, _dir) = bridge();
        let m = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            b.offer(offer(
                serde_json::json!({"v": 1, "type": "download.offer", "url": m})
            ))
            .await
            .unwrap(),
            "open"
        );
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, UiEvent::Open { target } if target == m))
        );
        assert!(!b.version().is_empty());
    }
}
