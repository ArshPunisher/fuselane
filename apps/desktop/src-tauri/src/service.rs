//! The desktop app's backend: owns the store, runs a small queue of downloads and
//! pushes live events to the window. It has no Tauri types so it can be tested
//! directly against the hostile test server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use fuselane_core::runner::{self, parse_link, pick_networks};
use fuselane_core::{Event, Job, Outcome, RunOptions, StartError, Status, Store};
use fuselane_engine_http::download::{Cancel, Snapshot, SnapshotFn};
use fuselane_engine_http::headers::filename_from_path;
use serde::Serialize;

/// Downloads that run at once; the rest wait their turn (L-53).
pub const MAX_RUNNING: usize = 3;

/// One row in the transfers list.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: i64,
    pub url: String,
    pub name: String,
    pub dir: String,
    pub status: &'static str,
    pub resumable: bool,
    pub written: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    pub final_path: Option<String>,
    pub created_at: i64,
}

/// One network the app can see.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetView {
    pub name: String,
    pub label: String,
    pub kind: String,
    pub usable: bool,
    pub addrs: Vec<String>,
}

/// One network's part of a live update.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LiveNet {
    pub name: String,
    pub label: String,
    pub kind: String,
    pub bytes: u64,
    pub rate: f64,
    pub streams: u32,
    pub dead: bool,
}

/// What the Fuse Core draws for one running job, a few times a second.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Live {
    pub id: i64,
    pub written: u64,
    pub total: Option<u64>,
    pub rate: f64,
    pub networks: Vec<LiveNet>,
    /// Triples per tick: fill 0..=100, owner (index into `networks` + 1, 0 = none),
    /// in-flight network (same encoding). Flat so the IPC payload stays small.
    pub ticks: Vec<u16>,
    pub retries: u64,
    pub hedges: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiEvent {
    Jobs { jobs: Vec<JobView> },
    Live(Live),
}

/// An error the window can show as is: a stable code, what happened, what to do.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct UiError {
    pub code: &'static str,
    pub message: String,
    pub hint: Option<String>,
}

impl UiError {
    fn new(code: &'static str, message: impl Into<String>, hint: Option<&str>) -> UiError {
        UiError {
            code,
            message: message.into(),
            hint: hint.map(str::to_string),
        }
    }
}

pub type Emit = Arc<dyn Fn(UiEvent) + Send + Sync>;

#[derive(Debug)]
struct Running {
    cancel: Cancel,
    remove_after: bool,
}

/// The app's backend. Cheap to share: methods take `&Arc<Self>`.
pub struct Service {
    store: Arc<Store>,
    running: Mutex<HashMap<i64, Running>>,
    emit: Mutex<Emit>,
    default_dir: PathBuf,
    max_running: usize,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Service")
            .field("default_dir", &self.default_dir)
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn kind_word(k: fuselane_netif::Kind) -> String {
    format!("{k:?}").to_lowercase()
}

fn job_name(job: &Job) -> String {
    if let Some(f) = job.final_path.as_ref().and_then(|p| p.file_name()) {
        return f.to_string_lossy().into_owned();
    }
    if let Some(f) = &job.filename {
        return f.clone();
    }
    match parse_link(&job.url) {
        Ok((_, host, _, path)) => {
            let path = path.split('?').next().unwrap_or_default();
            filename_from_path(path).unwrap_or(host)
        }
        Err(_) => job.url.clone(),
    }
}

fn view(job: &Job) -> JobView {
    JobView {
        id: job.id,
        url: job.url.clone(),
        name: job_name(job),
        dir: job.dir.to_string_lossy().into_owned(),
        status: job.status.as_str(),
        resumable: matches!(
            job.status,
            Status::Paused | Status::Failed { resumable: true }
        ),
        written: if job.status == Status::Completed {
            job.total.unwrap_or_else(|| job.secured_bytes())
        } else {
            job.secured_bytes()
        },
        total: job.total,
        error: job.error.clone(),
        final_path: job
            .final_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        created_at: job.created_at,
    }
}

fn store_error(e: impl std::fmt::Display) -> UiError {
    UiError::new(
        "store",
        format!("Fuselane couldn't read or save its list of downloads: {e}"),
        Some("Check that your disk has free space, then try again."),
    )
}

fn not_found(id: i64) -> UiError {
    UiError::new(
        "not-found",
        format!("There's no download {id}."),
        Some("It may have been removed already."),
    )
}

/// Packs a snapshot for the window.
fn live(id: i64, s: &Snapshot, nets: &[(String, String, String)]) -> Live {
    let index = |net: Option<u32>| -> u16 {
        net.and_then(|n| s.networks.iter().position(|x| x.id == n))
            .map_or(0, |i| i as u16 + 1)
    };
    let mut ticks = Vec::with_capacity(s.ticks.len() * 3);
    for t in &s.ticks {
        ticks.push((t.fill.clamp(0.0, 1.0) * 100.0).round() as u16);
        ticks.push(index(t.owner));
        ticks.push(index(t.in_flight));
    }
    Live {
        id,
        written: s.written,
        total: s.total,
        rate: if s.rate.is_finite() { s.rate } else { 0.0 },
        networks: s
            .networks
            .iter()
            .map(|n| {
                let (name, label, kind) = nets
                    .get(n.id.saturating_sub(1) as usize)
                    .cloned()
                    .unwrap_or_else(|| (format!("net{}", n.id), String::new(), "other".into()));
                LiveNet {
                    name,
                    label,
                    kind,
                    bytes: n.bytes,
                    rate: if n.rate.is_finite() { n.rate } else { 0.0 },
                    streams: n.streams,
                    dead: n.dead,
                }
            })
            .collect(),
        ticks,
        retries: s.retries,
        hedges: s.hedges,
    }
}

impl Service {
    /// Opens the service. Jobs a crash left running come back paused (L-53).
    pub fn new(store: Store, default_dir: PathBuf) -> Result<Arc<Service>, UiError> {
        store.recover_interrupted().map_err(store_error)?;
        Ok(Arc::new(Service {
            store: Arc::new(store),
            running: Mutex::new(HashMap::new()),
            emit: Mutex::new(Arc::new(|_| {})),
            default_dir,
            max_running: MAX_RUNNING,
        }))
    }

    /// For tests: a different queue size.
    #[cfg(test)]
    pub fn with_max_running(
        store: Store,
        dir: PathBuf,
        max: usize,
    ) -> Result<Arc<Service>, UiError> {
        let s = Service::new(store, dir)?;
        let mut s = Arc::try_unwrap(s).map_err(|_| store_error("busy"))?;
        s.max_running = max.max(1);
        Ok(Arc::new(s))
    }

    /// Where events go (the window's channel). Replaces any earlier subscriber.
    pub fn subscribe(self: &Arc<Self>, emit: Emit) {
        *lock(&self.emit) = emit;
        self.publish_jobs();
    }

    pub fn default_dir(&self) -> &Path {
        &self.default_dir
    }

    fn send(&self, e: UiEvent) {
        let emit = lock(&self.emit).clone();
        emit(e);
    }

    fn publish_jobs(&self) {
        if let Ok(jobs) = self.jobs() {
            self.send(UiEvent::Jobs { jobs });
        }
    }

    pub fn jobs(&self) -> Result<Vec<JobView>, UiError> {
        Ok(self
            .store
            .list()
            .map_err(store_error)?
            .iter()
            .map(view)
            .collect())
    }

    pub fn networks(&self) -> Result<Vec<NetView>, UiError> {
        let all = fuselane_netif::list().map_err(|e| {
            UiError::new(
                "networks",
                format!("Couldn't list your networks: {e}"),
                None,
            )
        })?;
        Ok(all
            .iter()
            .map(|i| NetView {
                name: i.name.clone(),
                label: i.display_name.clone(),
                kind: kind_word(i.kind),
                usable: i.usable(),
                addrs: i.addrs.iter().map(ToString::to_string).collect(),
            })
            .collect())
    }

    /// Adds a download and starts it when a slot is free.
    pub fn add(self: &Arc<Self>, url: &str, dir: Option<&str>) -> Result<i64, UiError> {
        let url = url.trim();
        if url.is_empty() {
            return Err(UiError::new(
                "bad-link",
                "Paste a link to download.",
                Some("Links start with http:// or https://."),
            ));
        }
        if url.len() > 8192 {
            return Err(UiError::new(
                "bad-link",
                "That link is too long (over 8 KB).",
                Some("Copy the link again from its page."),
            ));
        }
        parse_link(url).map_err(|m| {
            UiError::new("bad-link", m, Some("Links start with http:// or https://."))
        })?;
        let dir = match dir.map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => PathBuf::from(d),
            None => self.default_dir.clone(),
        };
        if !dir.is_dir() {
            return Err(UiError::new(
                "folder-missing",
                format!("The folder \"{}\" doesn't exist.", dir.display()),
                Some("Pick another folder to save into."),
            ));
        }
        let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        let id = self.store.create(url, &dir).map_err(store_error)?;
        self.publish_jobs();
        self.pump();
        Ok(id)
    }

    pub fn pause(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        if let Some(r) = lock(&self.running).get(&id) {
            r.cancel.cancel(); // the task records the pause once progress is saved
            return Ok(());
        }
        let job = self.store.get(id).map_err(|_| not_found(id))?;
        match job.status {
            Status::Queued => {
                self.store
                    .apply(id, Event::Pause, None)
                    .map_err(store_error)?;
                self.publish_jobs();
                Ok(())
            }
            Status::Paused => Ok(()),
            _ => Err(UiError::new(
                "not-running",
                "This download isn't running, so there's nothing to pause.",
                None,
            )),
        }
    }

    pub fn resume(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        if lock(&self.running).contains_key(&id) {
            return Ok(());
        }
        let job = match runner::job_to_resume(&self.store, &id.to_string()) {
            Ok(j) => j,
            Err(StartError::BadInput(m) | StartError::Setup(m)) => {
                return Err(match self.store.get(id) {
                    Err(_) => not_found(id),
                    Ok(_) => UiError::new(
                        "not-resumable",
                        m,
                        Some("Add the link again to download it from the start."),
                    ),
                });
            }
        };
        if job.status != Status::Queued {
            self.store
                .apply(id, Event::Resume, None)
                .map_err(store_error)?;
        }
        self.publish_jobs();
        self.pump();
        Ok(())
    }

    /// Removes a download (and its partial file). A running one is stopped first.
    pub fn remove(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        if let Some(r) = lock(&self.running).get_mut(&id) {
            r.remove_after = true;
            r.cancel.cancel();
            return Ok(());
        }
        runner::remove(&self.store, id).map_err(|_| not_found(id))?;
        self.publish_jobs();
        Ok(())
    }

    /// Where a finished download's file is, checked to still exist. The window
    /// passes an id, never a path, so it can't open arbitrary files (L-98).
    pub fn finished_file(&self, id: i64) -> Result<PathBuf, UiError> {
        let job = self.store.get(id).map_err(|_| not_found(id))?;
        let path = match (job.status, job.final_path) {
            (Status::Completed, Some(p)) => p,
            _ => {
                return Err(UiError::new(
                    "not-finished",
                    "This download hasn't finished yet, so there's no file to show.",
                    None,
                ));
            }
        };
        if !path.is_file() {
            return Err(UiError::new(
                "file-missing",
                format!("The file is no longer at {}.", path.display()),
                Some("It may have been moved, renamed or deleted."),
            ));
        }
        Ok(path)
    }

    /// Pauses everything that's running (used when the app quits).
    pub fn pause_all(&self) {
        for r in lock(&self.running).values() {
            r.cancel.cancel();
        }
    }

    pub fn running(&self) -> usize {
        lock(&self.running).len()
    }

    /// Starts queued jobs, oldest first, while there are free slots.
    fn pump(self: &Arc<Self>) {
        let Ok(mut jobs) = self.store.list() else {
            return;
        };
        jobs.sort_by_key(|j| (j.created_at, j.id));
        let mut running = lock(&self.running);
        for job in jobs.into_iter().filter(|j| j.status == Status::Queued) {
            if running.len() >= self.max_running {
                break;
            }
            if running.contains_key(&job.id) {
                continue;
            }
            let cancel = Cancel::new();
            running.insert(
                job.id,
                Running {
                    cancel: cancel.clone(),
                    remove_after: false,
                },
            );
            let me = self.clone();
            tokio::spawn(async move { me.run(job, cancel).await });
        }
    }

    async fn run(self: Arc<Self>, job: Job, cancel: Cancel) {
        let id = job.id;
        let picked = pick_networks(&[]).unwrap_or_default();
        let nets: Vec<(String, String, String)> = picked
            .iter()
            .map(|i| (i.name.clone(), i.display_name.clone(), kind_word(i.kind)))
            .collect();
        let snapshot = {
            let me = self.clone();
            SnapshotFn(Arc::new(move |s: &Snapshot| {
                me.send(UiEvent::Live(live(id, s, &nets)));
            }))
        };
        let opts = RunOptions {
            networks: picked.iter().map(|i| i.name.clone()).collect(),
            snapshot: Some(snapshot),
            cancel: Some(cancel),
            ..RunOptions::default()
        };
        let resume = job.resume();
        let outcome = runner::run(
            self.store.clone(),
            id,
            &job.url,
            job.dir.clone(),
            resume,
            opts,
        )
        .await;
        match outcome {
            Err(e) => {
                // Setup failed before the engine started: record it, don't retry forever.
                let message = e.to_string();
                let _ = self.store.apply(id, Event::Start, None);
                let _ = self
                    .store
                    .apply(id, Event::Fail { resumable: true }, Some(&message));
            }
            Ok(Outcome::Completed { .. } | Outcome::Paused | Outcome::Failed { .. }) => {}
        }
        let removed = lock(&self.running)
            .remove(&id)
            .is_some_and(|r| r.remove_after);
        if removed {
            let _ = runner::remove(&self.store, id);
        }
        self.publish_jobs();
        self.pump();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuselane_testkit::{Content, Fault, RangeServer, Rule};
    use std::time::{Duration, Instant};

    const KB: u64 = 1024;

    struct Harness {
        svc: Arc<Service>,
        events: Arc<Mutex<Vec<UiEvent>>>,
        dir: tempfile::TempDir,
    }

    fn harness(max: usize) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("fuselane.db")).unwrap();
        let svc = Service::with_max_running(store, dir.path().to_path_buf(), max).unwrap();
        let events: Arc<Mutex<Vec<UiEvent>>> = Arc::default();
        svc.subscribe({
            let events = events.clone();
            Arc::new(move |e| lock(&events).push(e))
        });
        Harness { svc, events, dir }
    }

    impl Harness {
        fn job(&self, id: i64) -> JobView {
            self.svc
                .jobs()
                .unwrap()
                .into_iter()
                .find(|j| j.id == id)
                .unwrap()
        }

        async fn wait(&self, what: &str, f: impl Fn(&Harness) -> bool) {
            let start = Instant::now();
            while !f(self) {
                assert!(
                    start.elapsed() < Duration::from_secs(20),
                    "timed out waiting for {what}"
                );
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }

        fn lives(&self, id: i64) -> Vec<Live> {
            lock(&self.events)
                .iter()
                .filter_map(|e| match e {
                    UiEvent::Live(l) if l.id == id => Some(l.clone()),
                    _ => None,
                })
                .collect()
        }
    }

    fn link(server: &RangeServer) -> String {
        format!("http://{}{}", server.addr(), server.path())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_completes_and_streams_live_updates() {
        let content = Content::new(3 * 1024 * KB, 91);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(1500 * KB),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        let job = h.job(id);
        let path = PathBuf::from(job.final_path.unwrap());
        assert_eq!(
            fuselane_testkit::sha256_file(&path).unwrap(),
            content.sha256()
        );
        assert_eq!(job.written, content.size);
        let lives = h.lives(id);
        assert!(!lives.is_empty(), "no live updates");
        for l in &lives {
            assert_eq!(l.ticks.len() % 3, 0);
            assert!(
                l.ticks
                    .chunks(3)
                    .all(|t| t[0] <= 100 && (t[1] as usize) <= l.networks.len())
            );
            assert!(l.networks.iter().all(|n| !n.name.is_empty()));
        }
        // The window gets a JSON it can parse: camelCase, tagged.
        let json = serde_json::to_string(&UiEvent::Live(lives[0].clone())).unwrap();
        assert!(json.starts_with(r#"{"type":"live","id":"#), "{json}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn bad_links_and_folders_are_refused_before_anything_is_saved() {
        let h = harness(3);
        for bad in [
            "",
            "   ",
            "ftp://example.com/f",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "magnet:?xt=urn:btih:abc",
            "not a link",
            &format!("https://example.com/{}", "a".repeat(9000)),
        ] {
            let e = h.svc.add(bad, None).unwrap_err();
            assert_eq!(e.code, "bad-link", "{bad:?}");
            assert!(e.hint.is_some());
        }
        let e = h
            .svc
            .add("https://example.com/f", Some("/definitely/not/a/folder"))
            .unwrap_err();
        assert_eq!(e.code, "folder-missing");
        // A file is not a folder either.
        let file = h.dir.path().join("plain.txt");
        std::fs::write(&file, b"x").unwrap();
        let e = h
            .svc
            .add("https://example.com/f", file.to_str())
            .unwrap_err();
        assert_eq!(e.code, "folder-missing");
        assert!(h.svc.jobs().unwrap().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_then_resume_is_byte_exact() {
        let content = Content::new(4 * 1024 * KB, 92);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(600 * KB),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("some progress", |h| {
            h.lives(id).iter().any(|l| l.written > 256 * KB)
        })
        .await;
        h.svc.pause(id).unwrap();
        h.wait("paused", |h| {
            h.job(id).status == "paused" && h.svc.running() == 0
        })
        .await;
        assert!(h.job(id).resumable);
        // Pausing twice is harmless.
        h.svc.pause(id).unwrap();
        h.svc.resume(id).unwrap();
        h.wait("completion", |h| h.job(id).status == "completed")
            .await;
        let path = PathBuf::from(h.job(id).final_path.unwrap());
        assert_eq!(
            fuselane_testkit::sha256_file(&path).unwrap(),
            content.sha256()
        );
        // A finished download can't be resumed or paused.
        assert_eq!(h.svc.resume(id).unwrap_err().code, "not-resumable");
        assert_eq!(h.svc.pause(id).unwrap_err().code, "not-running");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn removing_a_running_download_deletes_its_partial_file() {
        let content = Content::new(4 * 1024 * KB, 93);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(300 * KB),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("some progress", |h| !h.lives(id).is_empty()).await;
        h.svc.remove(id).unwrap();
        h.wait("removal", |h| {
            h.svc.jobs().unwrap().is_empty() && h.svc.running() == 0
        })
        .await;
        let leftovers: Vec<_> = std::fs::read_dir(h.dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "fuselane"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "partial file left behind: {leftovers:?}"
        );
        assert_eq!(h.svc.remove(id).unwrap_err().code, "not-found");
        assert_eq!(h.svc.pause(999).unwrap_err().code, "not-found");
        assert_eq!(h.svc.resume(999).unwrap_err().code, "not-found");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_queue_runs_one_at_a_time_when_asked() {
        let a = Content::new(1024 * KB, 94);
        let b = Content::new(1024 * KB, 95);
        let sa = RangeServer::start(a).await.unwrap();
        let sb = RangeServer::start(b).await.unwrap();
        sa.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(800 * KB),
        });
        let h = harness(1);
        let ia = h.svc.add(&link(&sa), None).unwrap();
        let ib = h.svc.add(&link(&sb), None).unwrap();
        h.wait("first running", |h| h.job(ia).status == "running")
            .await;
        assert_eq!(h.job(ib).status, "queued");
        assert_eq!(h.svc.running(), 1);
        h.wait("both done", |h| {
            h.job(ia).status == "completed" && h.job(ib).status == "completed"
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unreachable_server_fails_with_a_plain_message_instead_of_hanging() {
        // Bind then drop a listener so nothing answers on the port.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let h = harness(3);
        let id = h
            .svc
            .add(&format!("http://127.0.0.1:{port}/f.bin"), None)
            .unwrap();
        h.wait("failure", |h| h.job(id).status.starts_with("failed"))
            .await;
        let job = h.job(id);
        assert_eq!(job.name, "f.bin");
        let msg = job.error.unwrap_or_default();
        assert!(msg.contains("Couldn't reach"), "{msg}");
        assert!(job.resumable);
        assert_eq!(h.svc.running(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_server_that_lies_about_the_file_is_reported_not_saved() {
        let content = Content::new(1024 * KB, 96);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::Status(404, None),
        });
        let h = harness(3);
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("failure", |h| h.job(id).status.starts_with("failed"))
            .await;
        assert!(h.job(id).error.unwrap_or_default().contains("404"));
        assert!(h.job(id).final_path.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_same_link_twice_at_once_gives_two_whole_files() {
        let content = Content::new(2 * 1024 * KB, 97);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        let a = h.svc.add(&link(&server), None).unwrap();
        let b = h.svc.add(&link(&server), None).unwrap();
        h.wait("both done", |h| {
            h.job(a).status == "completed" && h.job(b).status == "completed"
        })
        .await;
        let (pa, pb) = (h.job(a).final_path.unwrap(), h.job(b).final_path.unwrap());
        assert_ne!(pa, pb, "the second must not overwrite the first");
        for p in [pa, pb] {
            assert_eq!(
                fuselane_testkit::sha256_file(Path::new(&p)).unwrap(),
                content.sha256()
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn empty_files_and_servers_without_ranges_still_finish() {
        let empty = Content::new(0, 98);
        let se = RangeServer::start(empty).await.unwrap();
        let whole = Content::new(700 * KB, 99);
        let sw = RangeServer::start(whole).await.unwrap();
        sw.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::IgnoreRange,
        });
        let h = harness(3);
        let ie = h.svc.add(&link(&se), None).unwrap();
        let iw = h.svc.add(&link(&sw), None).unwrap();
        h.wait("both done", |h| {
            let (e, w) = (h.job(ie), h.job(iw));
            (e.status == "completed" || e.status.starts_with("failed"))
                && (w.status == "completed" || w.status.starts_with("failed"))
        })
        .await;
        let e = h.job(ie);
        assert_eq!(e.status, "completed", "{:?}", e.error);
        assert_eq!(std::fs::metadata(e.final_path.unwrap()).unwrap().len(), 0);
        let w = h.job(iw);
        assert_eq!(w.status, "completed", "{:?}", w.error);
        assert_eq!(
            fuselane_testkit::sha256_file(Path::new(&w.final_path.unwrap())).unwrap(),
            whole.sha256()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_flood_of_downloads_never_runs_more_than_the_queue_allows() {
        let content = Content::new(256 * KB, 100);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::Throttle(2048 * KB),
        });
        let h = harness(3);
        let ids: Vec<i64> = (0..30)
            .map(|_| h.svc.add(&link(&server), None).unwrap())
            .collect();
        let mut peak = 0;
        let start = Instant::now();
        loop {
            let jobs = h.svc.jobs().unwrap();
            let running = jobs.iter().filter(|j| j.status == "running").count();
            peak = peak.max(running).max(h.svc.running());
            if jobs.iter().all(|j| j.status == "completed") {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "flood didn't finish"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(peak <= 3, "ran {peak} at once");
        assert_eq!(ids.len(), 30);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn waiting_jobs_can_be_paused_removed_and_resumed() {
        let content = Content::new(2 * 1024 * KB, 101);
        let server = RangeServer::start(content).await.unwrap();
        server.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(1024 * KB),
        });
        let h = harness(1);
        let first = h.svc.add(&link(&server), None).unwrap();
        let waiting = h.svc.add(&link(&server), None).unwrap();
        let doomed = h.svc.add(&link(&server), None).unwrap();
        h.wait("first running", |h| h.job(first).status == "running")
            .await;
        h.svc.pause(waiting).unwrap();
        assert_eq!(h.job(waiting).status, "paused");
        h.svc.remove(doomed).unwrap();
        assert!(h.svc.jobs().unwrap().iter().all(|j| j.id != doomed));
        // Resuming a running job is a harmless no-op.
        h.svc.resume(first).unwrap();
        h.svc.resume(waiting).unwrap();
        h.wait("all done", |h| {
            h.job(first).status == "completed" && h.job(waiting).status == "completed"
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn only_finished_files_that_still_exist_can_be_revealed() {
        let content = Content::new(64 * KB, 102);
        let server = RangeServer::start(content).await.unwrap();
        let h = harness(3);
        assert_eq!(h.svc.finished_file(404).unwrap_err().code, "not-found");
        let id = h.svc.add(&link(&server), None).unwrap();
        h.wait("done", |h| h.job(id).status == "completed").await;
        let path = h.svc.finished_file(id).unwrap();
        assert!(path.starts_with(std::fs::canonicalize(h.dir.path()).unwrap()));
        std::fs::remove_file(&path).unwrap();
        let e = h.svc.finished_file(id).unwrap_err();
        assert_eq!(e.code, "file-missing");
        assert!(e.hint.is_some());
        // An unfinished download has no file to show.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let bad = h
            .svc
            .add(&format!("http://127.0.0.1:{port}/x"), None)
            .unwrap();
        h.wait("failed", |h| h.job(bad).status.starts_with("failed"))
            .await;
        assert_eq!(h.svc.finished_file(bad).unwrap_err().code, "not-finished");
    }

    /// The field names `apps/desktop/src/lib/types.ts` declares for an interface.
    fn ts_fields(src: &str, name: &str) -> Vec<String> {
        let start = src
            .find(&format!("export interface {name} {{"))
            .unwrap_or_else(|| panic!("types.ts has no interface {name}"));
        let body = &src[start..];
        let body = &body[body.find('{').unwrap() + 1..body.find("\n}").unwrap()];
        let mut out: Vec<String> = body
            .lines()
            .map(str::trim)
            .filter(|l| {
                !l.is_empty() && !l.starts_with("//") && !l.starts_with("/*") && !l.starts_with('*')
            })
            .filter_map(|l| l.split(':').next())
            .map(|f| f.trim_end_matches('?').to_string())
            .collect();
        out.sort();
        out
    }

    fn json_fields(v: &impl Serialize) -> Vec<String> {
        let mut keys: Vec<String> = serde_json::to_value(v)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn the_window_types_match_what_the_backend_sends() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/types.ts"))
                .unwrap();
        let job = JobView {
            id: 1,
            url: "u".into(),
            name: "n".into(),
            dir: "d".into(),
            status: "queued",
            resumable: false,
            written: 0,
            total: None,
            error: None,
            final_path: None,
            created_at: 0,
        };
        let net = NetView {
            name: "en0".into(),
            label: "Wi-Fi".into(),
            kind: "wifi".into(),
            usable: true,
            addrs: vec![],
        };
        let lnet = LiveNet {
            name: "en0".into(),
            label: "Wi-Fi".into(),
            kind: "wifi".into(),
            bytes: 0,
            rate: 0.0,
            streams: 0,
            dead: false,
        };
        let live = Live {
            id: 1,
            written: 0,
            total: None,
            rate: 0.0,
            networks: vec![],
            ticks: vec![],
            retries: 0,
            hedges: 0,
        };
        let err = UiError::new("c", "m", None);
        for (name, got) in [
            ("JobView", json_fields(&job)),
            ("NetView", json_fields(&net)),
            ("LiveNet", json_fields(&lnet)),
            ("Live", json_fields(&live)),
            ("UiError", json_fields(&err)),
        ] {
            assert_eq!(
                got,
                ts_fields(&src, name),
                "{name}: types.ts and service.rs disagree"
            );
        }
        // Events are tagged exactly as the window switches on them.
        let jobs = serde_json::to_value(UiEvent::Jobs { jobs: vec![] }).unwrap();
        assert_eq!(jobs["type"], "jobs");
        assert!(
            src.contains("{ type: 'jobs'; jobs: JobView[] }")
                && src.contains("{ type: 'live' } & Live")
        );
    }

    #[test]
    fn crashed_jobs_come_back_paused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fuselane.db");
        {
            let store = Store::open(&path).unwrap();
            let id = store.create("http://x/f", dir.path()).unwrap();
            store.apply(id, Event::Start, None).unwrap();
        }
        let svc = Service::new(Store::open(&path).unwrap(), dir.path().to_path_buf()).unwrap();
        assert_eq!(svc.jobs().unwrap()[0].status, "paused");
    }
}
