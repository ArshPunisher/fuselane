//! Feeds (B10.8): follow an RSS or Atom feed and download what's new in it by
//! itself: podcast episodes, release files, nightly builds. Words filter the
//! titles. Torrent items are listed for the person to open, unless they let
//! that feed start torrents by themselves (with all their files).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Starts a torrent from a feed item's link (a magnet, or an http(s) link to a
/// .torrent file) with all its files; returns its name.
pub type StartTorrent = Arc<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<String, UiError>> + Send>> + Send + Sync,
>;

pub fn real_torrents(tor: Weak<crate::torrents::Torrents>) -> StartTorrent {
    Arc::new(move |link| {
        let tor = tor.clone();
        Box::pin(async move {
            let tor = tor
                .upgrade()
                .ok_or_else(|| err("closing", "Fuselane is closing.", None))?;
            let listing = if link.starts_with("magnet:") {
                tor.inspect_magnet(&link, None).await?
            } else {
                tor.inspect_bytes(torrent_file(&link).await?, None).await?
            };
            let all = listing.files.iter().map(|f| f.index).collect();
            tor.add(&listing.token, all).await?;
            Ok(listing.name)
        })
    })
}

/// A magnet, or a link to a .torrent file.
fn torrent_link(url: &str) -> bool {
    url.starts_with("magnet:") || url.ends_with(".torrent")
}

/// Reads a .torrent file from the web. Anything bigger than a torrent file can
/// be (the engine's limit) is refused before or while it arrives, so a
/// mislabelled link can't fill the disk.
async fn torrent_file(link: &str) -> Result<Vec<u8>, UiError> {
    let max = fuselane_engine_torrent::engine::MAX_TORRENT_FILE;
    let too_big = || {
        err(
            "torrent-too-big",
            format!(
                "That link is over {} MB, too big for a .torrent file.",
                max / (1024 * 1024)
            ),
            Some("Open it in Fuselane to see what it is."),
        )
    };
    let unreadable = |why: String| {
        err(
            "torrent-unreadable",
            format!("Couldn't fetch the .torrent file: {why}"),
            Some("Fuselane tries again only if you open it yourself."),
        )
    };
    let web = url::Url::parse(link)
        .ok()
        .is_some_and(|u| matches!(u.scheme(), "http" | "https"));
    if !web {
        return Err(err(
            "bad-link",
            "Only http and https links to .torrent files are fetched.",
            None,
        ));
    }
    let preview = fuselane_core::runner::preview(link)
        .await
        .map_err(unreadable)?;
    if preview.web_page {
        return Err(err(
            "torrent-unreadable",
            "That link opens a web page, not a .torrent file.",
            Some("Open it in Fuselane to pick the file yourself."),
        ));
    }
    if preview.total.is_some_and(|t| t > max as u64) {
        return Err(too_big());
    }
    let mut tag = [0u8; 8];
    let _ = getrandom::fill(&mut tag);
    let tag: String = tag.iter().map(|b| format!("{b:02x}")).collect();
    let dir = std::env::temp_dir().join(format!("fuselane-feed-{tag}"));
    std::fs::create_dir_all(&dir).map_err(|e| unreadable(e.to_string()))?;
    // A server that sent no size, or more than it said, is stopped at the limit.
    let cancel = fuselane_engine_http::download::Cancel::new();
    let over = Arc::new(AtomicBool::new(false));
    let progress = {
        let (cancel, over) = (cancel.clone(), over.clone());
        fuselane_engine_http::download::ProgressFn(Arc::new(move |done, _| {
            if done > max as u64 {
                over.store(true, Ordering::Release);
                cancel.cancel();
            }
        }))
    };
    let opts = fuselane_core::RunOptions {
        progress: Some(progress),
        cancel: Some(cancel),
        filename: Some("feed.torrent".into()),
        ..fuselane_core::RunOptions::default()
    };
    let fetched = fuselane_core::runner::fetch(link, dir.clone(), opts).await;
    let bytes = fetched
        .map_err(unreadable)
        .and_then(|r| std::fs::read(&r.path).map_err(|e| unreadable(e.to_string())));
    let _ = std::fs::remove_dir_all(&dir);
    if over.load(Ordering::Acquire) {
        return Err(too_big());
    }
    let bytes = bytes?;
    if bytes.len() > max {
        return Err(too_big());
    }
    Ok(bytes)
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
    /// Magnets and .torrent files start by themselves, with all their files.
    /// Off unless the person turns it on for this feed.
    #[serde(default)]
    start_torrents: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Recent {
    pub title: String,
    pub url: String,
    /// added | filtered | no-file | torrent | torrent-started | failed
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
    pub start_torrents: bool,
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
            start_torrents: s.start_torrents,
        }
    }
}

/// What a check did with one new item.
struct Outcome {
    item: Item,
    state: &'static str,
    note: Option<String>,
}

pub struct Feeds {
    svc: Weak<Service>,
    subs: Mutex<Vec<Sub>>,
    fetch: Fetch,
    torrents: StartTorrent,
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
    pub fn new(svc: &Arc<Service>, fetch: Fetch, torrents: StartTorrent) -> Arc<Feeds> {
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
            torrents,
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
        start_torrents: bool,
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
            start_torrents,
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
            self.apply(sub.id, feed).await;
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
        start_torrents: bool,
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
            s.start_torrents = start_torrents;
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
        let feed = {
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
                    feed
                }
                Err(why) => {
                    s.problem = Some(why);
                    return;
                }
            }
        };
        self.apply(id, feed).await;
    }

    /// Adds what's new in `feed` and remembers it. Decided under the lock,
    /// done without it: starting a torrent can wait on its peers.
    async fn apply(&self, id: u64, feed: feeds::Feed) {
        let (fresh, include, exclude, start_torrents) = {
            let subs = lock(&self.subs);
            let Some(s) = subs.iter().find(|s| s.id == id) else {
                return;
            };
            // Oldest new item first, so they download in order.
            let fresh: Vec<Item> = feed
                .items
                .iter()
                .rev()
                .filter(|i| !s.seen.contains(&i.id))
                .cloned()
                .collect();
            (
                fresh,
                s.include.clone(),
                s.exclude.clone(),
                s.start_torrents,
            )
        };
        let svc = self.svc.upgrade();
        let mut done: Vec<Outcome> = Vec::new();
        // Torrents start side by side: each may wait up to 90 s for peers.
        let mut starts = tokio::task::JoinSet::new();
        let mut added_now = 0;
        let mut added = 0;
        for item in fresh {
            let (state, note) = if item.url.is_empty() {
                ("no-file", None)
            } else if !feeds::matches(&item.title, &include, &exclude) {
                ("filtered", None)
            } else if torrent_link(&item.url) && !start_torrents {
                ("torrent", None)
            } else if added_now >= MAX_PER_CHECK {
                // Not remembered: it comes back next time.
                continue;
            } else if torrent_link(&item.url) {
                added_now += 1;
                let index = done.len();
                let start = (self.torrents)(item.url.clone());
                starts.spawn(async move { (index, start.await) });
                // Replaced when it answers.
                ("failed", Some("It couldn't be started.".into()))
            } else {
                let req = AddRequest {
                    name: feeds::suggested_name(&item),
                    ..AddRequest::default()
                };
                match svc.as_ref().map(|svc| svc.add_with(&item.url, None, &req)) {
                    Some(Ok(_)) => {
                        added_now += 1;
                        added += 1;
                        ("added", None)
                    }
                    Some(Err(e)) if e.code == "duplicate" => {
                        ("added", Some("Already in the list.".into()))
                    }
                    Some(Err(e)) => ("failed", Some(e.message)),
                    None => return,
                }
            };
            done.push(Outcome { item, state, note });
        }
        while let Some(joined) = starts.join_next().await {
            let Ok((index, result)) = joined else {
                continue;
            };
            let Some(o) = done.get_mut(index) else {
                continue;
            };
            (o.state, o.note) = match result {
                Ok(_) => {
                    added += 1;
                    ("torrent-started", None)
                }
                Err(e) if e.code == "already-added" => {
                    ("torrent-started", Some("Already in your torrents.".into()))
                }
                Err(e) => ("failed", Some(e.message)),
            };
        }
        let mut subs = lock(&self.subs);
        let Some(s) = subs.iter_mut().find(|s| s.id == id) else {
            return;
        };
        s.added += added;
        for o in done {
            if s.seen.contains(&o.item.id) {
                continue;
            }
            s.seen.push(o.item.id.clone());
            s.recent.insert(
                0,
                Recent {
                    title: o.item.title,
                    url: o.item.url,
                    state: o.state.into(),
                    note: o.note,
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

    /// Links handed to the torrent starter, in order.
    type Started = Arc<Mutex<Vec<String>>>;

    /// Starts nothing for real: remembers the link. "dead" in a link finds no
    /// peers, and "again" is a torrent already in the list.
    fn fake_torrents(started: Started) -> StartTorrent {
        Arc::new(move |link: String| {
            let started = started.clone();
            Box::pin(async move {
                if link.contains("dead") {
                    return Err(err(
                        "no-peers",
                        "Nobody sharing this torrent could be found yet.",
                        None,
                    ));
                }
                if link.contains("again") {
                    return Err(err("already-added", "It's already in your torrents.", None));
                }
                lock(&started).push(link.clone());
                Ok(format!("name of {link}"))
            })
        })
    }

    fn setup() -> (
        tempfile::TempDir,
        Arc<Service>,
        Arc<Mutex<String>>,
        Arc<Feeds>,
    ) {
        let (d, svc, xml, feeds, _) = setup_with_torrents();
        (d, svc, xml, feeds)
    }

    fn setup_with_torrents() -> (
        tempfile::TempDir,
        Arc<Service>,
        Arc<Mutex<String>>,
        Arc<Feeds>,
        Started,
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
        let started: Started = Arc::default();
        let feeds = Feeds::new(&svc, fetch, fake_torrents(started.clone()));
        (dir, svc, xml, feeds, started)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn new_items_download_once_with_titles_as_names() {
        let (_d, svc, xml, feeds) = setup();
        assert_eq!(
            feeds
                .add("ftp://x", "", "", 60, false, false)
                .await
                .unwrap_err()
                .code,
            "bad-link"
        );
        assert_eq!(
            feeds
                .add("https://pod.example/rss", "", "", 7, false, false)
                .await
                .unwrap_err()
                .code,
            "bad-value"
        );
        // Start with the latest episode only.
        let v = feeds
            .add("https://pod.example/rss", "", "", 60, true, false)
            .await
            .unwrap();
        assert_eq!((v[0].title.as_str(), v[0].added), ("Pod", 1));
        let jobs = svc.jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "Episode 2.mp3", "not default.mp3");
        assert_eq!(
            feeds
                .add("https://pod.example/rss", "", "", 60, false, false)
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
        feeds.update(id, "", "trailer", 60, false).unwrap();
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
        let again = Feeds::new(&svc, real_fetch(), fake_torrents(Arc::default()));
        assert_eq!(again.views()[0].added, 2);
        assert!(feeds.remove(id).unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn tick_checks_only_feeds_that_are_due() {
        let (_d, svc, xml, feeds) = setup();
        feeds
            .add("https://pod.example/rss", "", "", 15, false, false)
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

    fn torrent_feed(items: &[(&str, &str, &str)]) -> String {
        let body: String = items
            .iter()
            .map(|(id, title, link)| {
                format!(
                    r#"<item><title>{title}</title><guid>{id}</guid><link>{link}</link></item>"#
                )
            })
            .collect();
        format!(r#"<rss version="2.0"><channel><title>Shows</title>{body}</channel></rss>"#)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn torrents_start_by_themselves_only_when_the_feed_allows_it() {
        let (_d, svc, xml, feeds, started) = setup_with_torrents();
        *lock(&xml) = torrent_feed(&[("old", "Old show", "magnet:?xt=urn:btih:old")]);
        // Turned on in the add form; the latest item is a magnet and starts.
        let v = feeds
            .add("https://shows.example/rss", "", "", 60, true, true)
            .await
            .unwrap();
        let id = v[0].id;
        assert!(v[0].start_torrents);
        assert_eq!(v[0].recent[0].state, "torrent-started");
        assert_eq!(v[0].added, 1);
        assert_eq!(*lock(&started), ["magnet:?xt=urn:btih:old"]);

        // New items: a magnet, a .torrent link, one nobody shares, one already
        // in the list, and a plain file, oldest first in the feed's order.
        *lock(&xml) = torrent_feed(&[
            ("f", "Plain file", "https://cdn.example/f.mkv"),
            ("again", "Already there", "magnet:?xt=urn:btih:again"),
            ("dead", "Nobody shares", "magnet:?xt=urn:btih:dead"),
            ("t", "Torrent file", "https://shows.example/t.torrent"),
            ("m", "Magnet", "magnet:?xt=urn:btih:m"),
            ("old", "Old show", "magnet:?xt=urn:btih:old"),
        ]);
        let v = feeds.check_now(id).await.unwrap();
        let states: Vec<(&str, &str, Option<&str>)> = v[0]
            .recent
            .iter()
            .map(|r| (r.title.as_str(), r.state.as_str(), r.note.as_deref()))
            .collect();
        assert_eq!(
            states[..5],
            [
                ("Plain file", "added", None),
                (
                    "Already there",
                    "torrent-started",
                    Some("Already in your torrents.")
                ),
                (
                    "Nobody shares",
                    "failed",
                    Some("Nobody sharing this torrent could be found yet.")
                ),
                ("Torrent file", "torrent-started", None),
                ("Magnet", "torrent-started", None),
            ]
        );
        // They start side by side, so in any order.
        let mut new_ones = lock(&started)[1..].to_vec();
        new_ones.sort();
        assert_eq!(
            new_ones,
            ["https://shows.example/t.torrent", "magnet:?xt=urn:btih:m"]
        );
        assert_eq!(v[0].added, 4, "two torrents and a file, after the first");
        assert_eq!(
            svc.jobs().unwrap().len(),
            1,
            "torrents aren't HTTP downloads"
        );
        // Remembered: checking again starts nothing twice.
        feeds.check_now(id).await.unwrap();
        assert_eq!(lock(&started).len(), 3);

        // Turned off in Filters: torrents wait for the person again.
        feeds.update(id, "", "", 60, false).unwrap();
        *lock(&xml) = torrent_feed(&[("n", "Next", "magnet:?xt=urn:btih:n")]);
        let v = feeds.check_now(id).await.unwrap();
        assert!(!v[0].start_torrents);
        assert_eq!(v[0].recent[0].state, "torrent");
        assert_eq!(lock(&started).len(), 3);

        // Saved with the feed.
        feeds.update(id, "", "", 60, true).unwrap();
        let again = Feeds::new(&svc, real_fetch(), fake_torrents(Arc::default()));
        assert!(again.views()[0].start_torrents);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn torrent_starts_count_toward_the_limit_per_check() {
        let (_d, _svc, xml, feeds, started) = setup_with_torrents();
        *lock(&xml) = torrent_feed(&[]);
        let id = feeds
            .add("https://shows.example/rss", "", "", 60, false, true)
            .await
            .unwrap()[0]
            .id;
        let many: Vec<(String, String, String)> = (0..25)
            .rev()
            .map(|n| {
                (
                    format!("e{n}"),
                    format!("Episode {n}"),
                    format!("magnet:?xt=urn:btih:{n}"),
                )
            })
            .collect();
        let items: Vec<(&str, &str, &str)> = many
            .iter()
            .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
            .collect();
        *lock(&xml) = torrent_feed(&items);
        feeds.check_now(id).await.unwrap();
        // The oldest ones, started side by side.
        let mut first: Vec<String> = lock(&started).clone();
        first.sort();
        let mut oldest: Vec<String> = (0..MAX_PER_CHECK)
            .map(|n| format!("magnet:?xt=urn:btih:{n}"))
            .collect();
        oldest.sort();
        assert_eq!(first, oldest);
        // The rest come back next time.
        let v = feeds.check_now(id).await.unwrap();
        assert_eq!(lock(&started).len(), 25);
        assert_eq!(v[0].added, 25);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn feeds_saved_before_the_setting_existed_still_load() {
        let (_d, svc, _xml, _feeds) = setup();
        let old = r#"[{"id":3,"url":"https://pod.example/rss","title":"Pod","include":"","exclude":"","every":60,"seen":["e1"],"last_check":1700000000,"problem":null,"added":4,"recent":[]}]"#;
        svc.store().set_setting(SETTING, old).unwrap();
        let feeds = Feeds::new(&svc, real_fetch(), fake_torrents(Arc::default()));
        let v = feeds.views();
        assert_eq!((v[0].id, v[0].added, v[0].start_torrents), (3, 4, false));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_torrent_file_from_the_web_is_read_within_the_limit() {
        use fuselane_testkit::{Content, RangeServer};
        let server = RangeServer::start(Content::new(
            fuselane_engine_torrent::engine::MAX_TORRENT_FILE as u64 + 1,
            7,
        ))
        .await
        .unwrap();
        let small = b"d8:announce0:4:infod4:name1:xee".to_vec();
        server.serve_file("/show.torrent", small.clone());
        let base = format!("http://{}", server.addr());
        assert_eq!(
            torrent_file(&format!("{base}/show.torrent")).await.unwrap(),
            small
        );
        // Bigger than any .torrent file: refused, and nothing is left behind.
        let e = torrent_file(&format!("{base}{}", server.path()))
            .await
            .unwrap_err();
        assert_eq!(e.code, "torrent-too-big", "{}", e.message);
        assert_eq!(
            torrent_file("ftp://shows.example/a.torrent")
                .await
                .unwrap_err()
                .code,
            "bad-link"
        );
    }
}
