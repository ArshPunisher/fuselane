//! The app's side of the local API: what happens to a download the browser
//! extension (or another local tool) hands over.

use std::sync::Arc;

use fuselane_api::offer::Offer;
use fuselane_api::server::{BoxFuture, Decline, Handler, Handoff, JobAction, Seen};

use crate::service::Service;

pub struct ApiBridge {
    pub svc: Arc<Service>,
}

/// Drafts starting with this open New download on its video picker (B10.4).
pub const VIDEO_DRAFT: &str = "fuselane-video:";
/// Drafts starting with this open Feeds with the feed's address (B10.8).
pub const FEED_DRAFT: &str = "fuselane-feed:";

impl Handler for ApiBridge {
    fn page_feed(&self, url: String) -> bool {
        self.svc.open_request(format!("{FEED_DRAFT}{url}"));
        self.svc.bring_to_front();
        true
    }

    fn page_peek(&self, url: String) {
        // The answer is kept a few minutes, so "Get the video" finds it ready.
        let svc = self.svc.clone();
        tokio::spawn(async move {
            let _ = svc.media_info(&url).await;
        });
    }

    fn page_video(&self, url: String) -> bool {
        self.svc.open_request(format!("{VIDEO_DRAFT}{url}"));
        self.svc.bring_to_front();
        true
    }

    fn offer(&self, offer: Offer) -> BoxFuture<'_, Result<String, Decline>> {
        Box::pin(async move {
            if offer
                .url
                .get(..7)
                .is_some_and(|s| s.eq_ignore_ascii_case("magnet:"))
            {
                // A magnet needs the user to pick files: open the dialog with it.
                self.svc.open_request(offer.url);
                self.svc.bring_to_front();
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
                // The same click offered twice comes back as the one download.
                .add_offer(
                    link,
                    &crate::service::AddRequest {
                        headers,
                        ..Default::default()
                    },
                )
                // It carries on in the background; the extension's popup shows it
                // starting. The window comes up only when the person has to choose.
                .map(|id| id.to_string())
                .map_err(|_| Decline::Unsupported)
        })
    }

    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn extension_seen(&self, seen: Seen) {
        self.svc.extension_seen(seen);
    }

    fn handoff(&self) -> Option<Handoff> {
        Some(self.svc.handoff())
    }

    fn set_handoff(&self, handoff: Handoff) -> Option<Handoff> {
        self.svc.set_handoff(handoff).ok()
    }

    fn jobs(&self) -> Option<serde_json::Value> {
        self.svc.extension_jobs().ok()
    }

    fn speedtest_start(&self) -> Result<serde_json::Value, String> {
        self.svc
            .start_netcheck(crate::service::netcheck::CheckTargets::default())
            .map_err(|e| e.message)?;
        Ok(self.svc.speedtest_for_extension())
    }

    fn speedtest_state(&self) -> Option<serde_json::Value> {
        Some(self.svc.speedtest_for_extension())
    }

    fn job_action(&self, id: i64, action: JobAction) -> Result<(), String> {
        self.svc.extension_action(id, action).map_err(|e| {
            [e.message, e.hint.unwrap_or_default()]
                .join(" ")
                .trim()
                .to_string()
        })
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
        // With no size (a right-click), a server that sends a file: taken. The
        // link is already downloading, so it's that same download (one click,
        // one job).
        assert_eq!(b.offer(ask(None, Some(&url))).await, Ok(id));
        assert_eq!(b.svc.jobs().unwrap().len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_same_link_offered_twice_at_once_is_one_download() {
        use fuselane_testkit::{Content, RangeServer};
        let server = RangeServer::start(Content::new(300_000, 3)).await.unwrap();
        let url = format!("http://{}{}", server.addr(), server.path());
        let (b, _events, _dir) = bridge();
        let ask = || {
            offer(
                serde_json::json!({"v": 1, "type": "download.offer", "url": url, "size": 300_000}),
            )
        };
        let (one, two) = tokio::join!(b.offer(ask()), b.offer(ask()));
        assert_eq!(one.unwrap(), two.unwrap());
        assert_eq!(b.svc.jobs().unwrap().len(), 1);
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

    #[test]
    fn what_the_extension_asks_for_brings_the_window_to_the_front() {
        let (b, _events, _dir) = bridge();
        let shown = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = shown.clone();
        b.svc.set_to_front(Arc::new(move || {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        assert!(b.page_video("https://www.youtube.com/watch?v=x".into()));
        assert!(b.page_feed("https://blog.example/feed.xml".into()));
        assert_eq!(shown.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_popup_sees_downloads_and_can_cancel_them() {
        let (b, _events, _dir) = bridge();
        let id = b.svc.add("https://example.org/big.iso", None).unwrap();
        let list = b.jobs().unwrap();
        let first = &list["jobs"][0];
        assert_eq!(first["id"], id);
        assert_eq!(first["name"], "big.iso");
        assert_eq!(first["file"], false);
        assert_eq!(list["rate"], 0.0);
        b.job_action(id, JobAction::Cancel).unwrap();
        // A running download stops first, then goes (its connections may take a
        // while to close when the machine is busy).
        for _ in 0..400 {
            if b.jobs().unwrap()["jobs"].as_array().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert_eq!(b.jobs().unwrap()["jobs"].as_array().unwrap().len(), 0);
        let err = b.job_action(9999, JobAction::Pause).unwrap_err();
        assert!(!err.is_empty());
    }
}
