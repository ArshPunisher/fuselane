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
            // The engine can't send the browser's cookies or credentials yet, so a
            // download that needs them stays in the browser rather than failing here.
            if offer.needs_session() {
                return Err(Decline::Unsupported);
            }
            let link = offer.final_url.as_deref().unwrap_or(&offer.url);
            // Without the browser's cookies a server may answer with its login page.
            // Take the download only when Fuselane's own look at the link finds exactly
            // the size the browser saw; otherwise the browser carries on with it.
            let Some(size) = offer.size else {
                return Err(Decline::Unsupported);
            };
            match crate::service::preview(link).await {
                Ok(p) if p.total == Some(size) => {}
                _ => return Err(Decline::Unsupported),
            }
            self.svc
                .add(link, None)
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
        // A different size (a login page, say), an unknown size, or a dead link: the browser keeps it.
        assert_eq!(
            b.offer(ask(Some(299_999), Some(&url))).await,
            Err(Decline::Unsupported)
        );
        assert_eq!(
            b.offer(ask(None, Some(&url))).await,
            Err(Decline::Unsupported)
        );
        assert_eq!(
            b.offer(ask(Some(300_000), None)).await,
            Err(Decline::Unsupported)
        );
        assert_eq!(b.svc.jobs().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn downloads_needing_the_browsers_session_stay_in_the_browser() {
        let (b, _events, _dir) = bridge();
        for extra in [
            serde_json::json!({"cookies": "session=1"}),
            serde_json::json!({"headers": {"Authorization": "Bearer x"}}),
        ] {
            let mut v = serde_json::json!({"v": 1, "type": "download.offer", "url": "https://example.org/private.zip"});
            v.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            assert_eq!(b.offer(offer(v)).await, Err(Decline::Unsupported));
        }
        assert!(b.svc.jobs().unwrap().is_empty(), "nothing was added");
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
