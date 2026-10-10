//! Feeds (B10.8): follow an RSS or Atom feed and download what's new in it by
//! itself: podcast episodes, release files, nightly builds. Words filter the
//! titles. Torrent items are listed for the person to open; they don't start
//! by themselves.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};

use serde::{Deserialize, Serialize};

use crate::service::{AddRequest, Service, UiError};
use fuselane_core::feeds::{self, Item};

const SETTING: &str = "feeds";
/// Largest feed read (some podcast feeds are 20 MB of show notes).
const MAX_FEED: usize = 32 * 1024 * 1024;
/// Items remembered per feed, so old ones never come back as new.
const MAX_SEEN: usize = 2000;
/// Most downloads one check adds; the rest wait for the next check.
const MAX_PER_CHECK: usize = 20;
const MAX_FEEDS: usize = 100;
/// How often a feed may be checked, in minutes.
pub const EVERY: [u32; 4] = [15, 60, 360, 1440];

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn err(code: &'static str, message: impl Into<String>, hint: Option<&str>) -> UiError {
    UiError::new_public(code, message, hint)
}

/// Reads a feed: its final address and text.
pub type Fetch = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<(String, String), String>> + Send>>
        + Send
        + Sync,
>;

pub fn real_fetch() -> Fetch {
    Arc::new(|url| Box::pin(async move { fuselane_core::runner::fetch_text(&url, MAX_FEED).await }))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Sub {
    id: u64,
    url: String,
    title: String,
    #[serde(default)]
    include: String,
    #[serde(default)]
    exclude: String,
    every: u32,
    /// Item ids already handled, oldest first.
    #[serde(default)]
    seen: Vec<String>,
    #[serde(default)]
    last_check: Option<i64>,
    #[serde(default)]
    problem: Option<String>,
    /// Downloads this feed has added in all.
    #[serde(default)]
    added: u32,
    /// The latest items and what happened to each, newest first.
    #[serde(default)]
    recent: Vec<Recent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub title: String,
    pub url: String,
    /// added | filtered | no-file | torrent | waiting | failed
    pub state: String,
    pub note: Option<String>,
}

/// One feed as the window shows it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeedView {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub include: String,
    pub exclude: String,
    pub every: u32,
    pub last_check: Option<i64>,
    pub problem: Option<String>,
    pub added: u32,
    pub recent: Vec<Recent>,
}

impl From<&Sub> for FeedView {
    fn from(s: &Sub) -> Self {
        FeedView {
            id: s.id,
            url: s.url.clone(),
            title: s.title.clone(),
            include: s.include.clone(),
            exclude: s.exclude.clone(),
            every: s.every,
            last_check: s.last_check,
            problem: s.problem.clone(),
            added: s.added,
            recent: s.recent.clone(),
        }
    }
}

pub struct Feeds {
    svc: Weak<Service>,
    subs: Mutex<Vec<Sub>>,
    fetch: Fetch,
    /// One check at a time.
    busy: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for Feeds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Feeds").finish_non_exhaustive()
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl Feeds {
    pub fn new(svc: &Arc<Service>, fetch: Fetch) -> Arc<Feeds> {
        let subs = svc
            .store()
            .setting(SETTING)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<Vec<Sub>>(&s).ok())
            .unwrap_or_default();
        Arc::new(Feeds {
            svc: Arc::downgrade(svc),
            subs: Mutex::new(subs),
            fetch,
            busy: tokio::sync::Mutex::new(()),
        })
    }

    pub fn views(&self) -> Vec<FeedView> {
        lock(&self.subs).iter().map(FeedView::from).collect()
    }

    fn save(&self) -> Result<(), UiError> {
        let svc = self
            .svc
            .upgrade()
            .ok_or_else(|| err("closing", "Fuselane is closing.", None))?;
        let json = serde_json::to_string(&*lock(&self.subs)).unwrap_or_default();
        svc.store().set_setting(SETTING, &json).map_err(|e| {
            err(
                "store",
                format!("Fuselane couldn't save the feeds: {e}"),
                Some("Check that your disk has free space, then try again."),
            )
        })
    }

    fn check_every(every: u32) -> Result<(), UiError> {
        if EVERY.contains(&every) {
            Ok(())
        } else {
            Err(err(
                "bad-value",
                "Check every 15 minutes, hour, 6 hours or day.",
                None,
            ))
        }
    }

    async fn read(&self, url: &str) -> Result<(String, feeds::Feed), String> {
        let (place, text) = (self.fetch)(url.to_string()).await?;
        let feed = feeds::parse(&text, &place)?;
        Ok((place, feed))
    }

    /// Follows a feed. What's in it now counts as seen, except the newest
    /// matching item when `latest` (to start with the current episode).
    pub async fn add(
        &self,
        url: &str,
        include: &str,
        exclude: &str,
        every: u32,
        latest: bool,
    ) -> Result<Vec<FeedView>, UiError> {
        let url = url.trim();
        let parsed = url::Url::parse(url)
            .ok()
            .filter(|u| matches!(u.scheme(), "http" | "https"));
        if parsed.is_none() {
            return Err(err(
                "bad-link",
                "Paste the feed's address.",
                Some(
                    "Feed addresses start with http:// or https:// and often end in /feed, .rss or .xml.",
                ),
            ));
        }
        Self::check_every(every)?;
        {
            let subs = lock(&self.subs);
            if subs.iter().any(|s| s.url == url) {
                return Err(err("feed-duplicate", "You already follow this feed.", None));
            }
            if subs.len() >= MAX_FEEDS {
                return Err(err(
                    "feed-limit",
                    format!("Up to {MAX_FEEDS} feeds can be followed."),
                    Some("Remove one you no longer need."),
                ));
            }
        }
        let (_, feed) = self.read(url).await.map_err(|why| {
            err(
                "feed-unreadable",
                why,
                Some("Check the address: it should open as a feed (RSS or Atom), not a web page."),
            )
        })?;
        let mut sub = Sub {
            id: 0,
            url: url.to_string(),
            title: if feed.title.is_empty() {
                parsed
                    .and_then(|u| u.host_str().map(str::to_string))
                    .unwrap_or_default()
            } else {
                feed.title.clone()
            },
            include: include.trim().to_string(),
            exclude: exclude.trim().to_string(),
            every,
            seen: vec![],
            last_check: Some(now()),
            problem: None,
            added: 0,
            recent: vec![],
        };
        let first = if latest {
            feed.items
                .iter()
                .find(|i| !i.url.is_empty() && feeds::matches(&i.title, &sub.include, &sub.exclude))
                .map(|i| i.id.clone())
        } else {
            None
        };
        // Newest first in the feed; remembered oldest first.
        sub.seen = feed
            .items
            .iter()
            .rev()
            .filter(|i| Some(&i.id) != first.as_ref())
            .map(|i| i.id.clone())
            .collect();
        {
            let mut subs = lock(&self.subs);
            sub.id = subs.iter().map(|s| s.id).max().unwrap_or(0) + 1;
            subs.push(sub.clone());
        }
        if first.is_some() {
            self.apply(sub.id, feed);
        }
        self.save()?;
        Ok(self.views())
    }

    pub fn update(
        &self,
        id: u64,
        include: &str,
        exclude: &str,
        every: u32,
    ) -> Result<Vec<FeedView>, UiError> {
        Self::check_every(every)?;
        {
            let mut subs = lock(&self.subs);
            let s = subs
                .iter_mut()
                .find(|s| s.id == id)
                .ok_or_else(|| err("feed-missing", "That feed isn't followed anymore.", None))?;
            s.include = include.trim().to_string();
            s.exclude = exclude.trim().to_string();
            s.every = every;
        }
        self.save()?;
        Ok(self.views())
    }

    pub fn remove(&self, id: u64) -> Result<Vec<FeedView>, UiError> {
        lock(&self.subs).retain(|s| s.id != id);
        self.save()?;
        Ok(self.views())
    }

    /// Checks one feed now.
    pub async fn check_now(&self, id: u64) -> Result<Vec<FeedView>, UiError> {
        let url = lock(&self.subs)
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.url.clone())
            .ok_or_else(|| err("feed-missing", "That feed isn't followed anymore.", None))?;
        let _one = self.busy.lock().await;
        self.check(id, &url).await;
        self.save()?;
        Ok(self.views())
    }

    /// Called every minute: checks the feeds that are due.
    pub async fn tick(&self) {
        let due: Vec<(u64, String)> = {
            let t = now();
            lock(&self.subs)
                .iter()
                .filter(|s| {
                    s.last_check
                        .is_none_or(|at| t - at >= i64::from(s.every) * 60)
                })
                .map(|s| (s.id, s.url.clone()))
                .collect()
        };
        if due.is_empty() {
            return;
        }
        let _one = self.busy.lock().await;
        for (id, url) in due {
            self.check(id, &url).await;
        }
        let _ = self.save();
    }

    async fn check(&self, id: u64, url: &str) {
        let result = self.read(url).await;
        let mut subs = lock(&self.subs);
        let Some(s) = subs.iter_mut().find(|s| s.id == id) else {
            return;
        };
        s.last_check = Some(now());
        match result {
            Ok((_, feed)) => {
                s.problem = None;
                if s.title.is_empty() && !feed.title.is_empty() {
                    s.title = feed.title.clone();
                }
                drop(subs);
                self.apply(id, feed);
            }
            Err(why) => s.problem = Some(why),
        }
    }

    /// Adds what's new in `feed` and remembers it.
    fn apply(&self, id: u64, feed: feeds::Feed) {
        let svc = self.svc.upgrade();
        let mut subs = lock(&self.subs);
        let Some(s) = subs.iter_mut().find(|s| s.id == id) else {
            return;
        };
        // Oldest new item first, so they download in order.
        let fresh: Vec<&Item> = feed
            .items
            .iter()
            .filter(|i| !s.seen.contains(&i.id))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let mut added_now = 0;
        for item in fresh {
            let (state, note) = if item.url.is_empty() {
                ("no-file", None)
            } else if !feeds::matches(&item.title, &s.include, &s.exclude) {
                ("filtered", None)
            } else if item.url.starts_with("magnet:") || item.url.ends_with(".torrent") {
                ("torrent", None)
            } else if added_now >= MAX_PER_CHECK {
                // Not remembered: it comes back next time.
                continue;
            } else {
                let req = AddRequest {
                    name: feeds::suggested_name(item),
                    ..AddRequest::default()
                };
                match svc.as_ref().map(|svc| svc.add_with(&item.url, None, &req)) {
                    Some(Ok(_)) => {
                        added_now += 1;
                        s.added += 1;
                        ("added", None)
                    }
                    Some(Err(e)) if e.code == "duplicate" => {
                        ("added", Some("Already in the list.".into()))
                    }
                    Some(Err(e)) => ("failed", Some(e.message)),
                    None => return,
                }
            };
            s.seen.push(item.id.clone());
            s.recent.insert(
                0,
                Recent {
                    title: item.title.clone(),
                    url: item.url.clone(),
                    state: state.into(),
                    note,
                },
            );
        }
        s.recent.truncate(8);
        let extra = s.seen.len().saturating_sub(MAX_SEEN);
        s.seen.drain(..extra);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn feed_xml(items: &[(&str, &str)]) -> String {
        let body: String = items
            .iter()
            .map(|(id, title)| {
                format!(
                    r#"<item><title>{title}</title><guid>{id}</guid><enclosure url="https://cdn.example/{id}/default.mp3" length="1000"/></item>"#
                )
            })
            .collect();
        format!(r#"<rss version="2.0"><channel><title>Pod</title>{body}</channel></rss>"#)
    }

    fn setup() -> (
        tempfile::TempDir,
        Arc<Service>,
        Arc<Mutex<String>>,
        Arc<Feeds>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("db")).unwrap();
        let svc = Service::new(store, dir.path().to_path_buf()).unwrap();
        let xml = Arc::new(Mutex::new(feed_xml(&[
            ("e2", "Episode 2"),
            ("e1", "Episode 1"),
        ])));
        let fetch: Fetch = {
            let xml = xml.clone();
            Arc::new(move |url| {
                let x = lock(&xml).clone();
                Box::pin(async move {
                    if x.is_empty() {
                        Err("It didn't answer.".into())
                    } else {
                        Ok((url, x))
                    }
                })
            })
        };
        let feeds = Feeds::new(&svc, fetch);
        (dir, svc, xml, feeds)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn new_items_download_once_with_titles_as_names() {
        let (_d, svc, xml, feeds) = setup();
        assert_eq!(
            feeds
                .add("ftp://x", "", "", 60, false)
                .await
                .unwrap_err()
                .code,
            "bad-link"
        );
        assert_eq!(
            feeds
                .add("https://pod.example/rss", "", "", 7, false)
                .await
                .unwrap_err()
                .code,
            "bad-value"
        );
        // Start with the latest episode only.
        let v = feeds
            .add("https://pod.example/rss", "", "", 60, true)
            .await
            .unwrap();
        assert_eq!((v[0].title.as_str(), v[0].added), ("Pod", 1));
        let jobs = svc.jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "Episode 2.mp3", "not default.mp3");
        assert_eq!(
            feeds
                .add("https://pod.example/rss", "", "", 60, false)
                .await
                .unwrap_err()
                .code,
            "feed-duplicate"
        );

        // A new episode, an unwanted one, and a torrent appear.
        *lock(&xml) = r#"<rss version="2.0"><channel><title>Pod</title>
            <item><title>Episode 4 (trailer)</title><guid>e4</guid><enclosure url="https://cdn.example/e4.mp3"/></item>
            <item><title>Episode 3</title><guid>e3</guid><enclosure url="https://cdn.example/e3.mp3"/></item>
            <item><title>Episode 3 torrent</title><guid>t3</guid><link>magnet:?xt=urn:btih:abc</link></item>
            <item><title>Episode 2</title><guid>e2</guid><enclosure url="https://cdn.example/e2/default.mp3"/></item>
            </channel></rss>"#
            .to_string();
        let id = v[0].id;
        feeds.update(id, "", "trailer", 60).unwrap();
        let v = feeds.check_now(id).await.unwrap();
        let states: Vec<(&str, &str)> = v[0]
            .recent
            .iter()
            .map(|r| (r.title.as_str(), r.state.as_str()))
            .collect();
        assert_eq!(
            states[..3],
            [
                ("Episode 4 (trailer)", "filtered"),
                ("Episode 3", "added"),
                ("Episode 3 torrent", "torrent"),
            ]
        );
        assert_eq!(svc.jobs().unwrap().len(), 2);
        // Checking again adds nothing: everything is remembered.
        feeds.check_now(id).await.unwrap();
        assert_eq!(svc.jobs().unwrap().len(), 2);

        // A feed that stops answering says so and keeps its place.
        lock(&xml).clear();
        let v = feeds.check_now(id).await.unwrap();
        assert_eq!(v[0].problem.as_deref(), Some("It didn't answer."));

        // Saved: a restart remembers the feed and what it has seen.
        let again = Feeds::new(&svc, real_fetch());
        assert_eq!(again.views()[0].added, 2);
        assert!(feeds.remove(id).unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tick_checks_only_feeds_that_are_due() {
        let (_d, svc, xml, feeds) = setup();
        feeds
            .add("https://pod.example/rss", "", "", 15, false)
            .await
            .unwrap();
        *lock(&xml) = feed_xml(&[
            ("e3", "Episode 3"),
            ("e2", "Episode 2"),
            ("e1", "Episode 1"),
        ]);
        feeds.tick().await;
        assert!(svc.jobs().unwrap().is_empty(), "checked a moment ago");
        lock(&feeds.subs)[0].last_check = Some(now() - 16 * 60);
        feeds.tick().await;
        assert_eq!(svc.jobs().unwrap().len(), 1);
    }
}
