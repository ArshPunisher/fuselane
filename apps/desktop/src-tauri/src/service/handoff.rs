//! Hand off over Nearby (B9.9): a paused download goes to another Fuselane and
//! carries on there.
//!
//! Version 2 (REVIEW-2026-10-11 C1–C4): a small manifest goes first, with the
//! link and which bytes are already downloaded; the download appears on the
//! other computer at once and starts there, fetching what was never
//! downloaded from the internet. Meanwhile only the downloaded bytes follow
//! (`<id>.fuselane-pack`, read straight from the partial file) and are written
//! into the same file as they arrive. If that stops early, what arrived is
//! kept and the rest comes from the internet. Samples of what arrived are
//! checked against the server before the file is published.
//!
//! Version 1 (older Fuselane) sent the whole partial file after the manifest;
//! it's still read, and added paused.

use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use fuselane_core::{Event, Status};
use fuselane_engine_http::download::Incoming;

use super::{Service, UiError, lock, not_found, store_error};

/// The manifest's file name ends with this.
pub const MANIFEST_SUFFIX: &str = ".fuselane-handoff";
/// The downloaded bytes of a version 2 hand-off: `<id>.fuselane-pack`.
pub const PACK_SUFFIX: &str = ".fuselane-pack";
/// Arrived bytes are synced to disk (and counted in) at least this often.
const SYNC_EVERY: u64 = 8 * 1024 * 1024;
/// Biggest manifest read (the secured list is one number per block).
const MAX_MANIFEST: u64 = 4 * 1024 * 1024;
/// Most blocks a manifest may list.
const MAX_BLOCKS: usize = 1_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Format version; this build reads 1.
    pub v: u32,
    pub url: String,
    pub filename: String,
    pub chosen_name: Option<String>,
    pub total: u64,
    pub block_size: u64,
    /// Bytes secured from the start of each block.
    pub secured: Vec<u64>,
    pub raw_etag: Option<String>,
    pub last_modified: Option<String>,
    pub sha256: Option<String>,
    #[serde(default)]
    pub mirrors: Vec<String>,
    /// The partial file's name as sent.
    pub part: String,
    /// Version 2: this hand-off's id; the downloaded bytes follow as
    /// `<id>.fuselane-pack`, block by block, only what's secured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Version 2: the sending computer's name, for "Arriving from …".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

impl Manifest {
    /// The downloaded parts, in the order the pack carries them: (offset, length).
    pub fn ranges(&self) -> Vec<(u64, u64)> {
        self.secured
            .iter()
            .enumerate()
            .filter(|(_, s)| **s > 0)
            .map(|(i, s)| (i as u64 * self.block_size, *s))
            .collect()
    }
}

/// A download arriving from another computer (version 2), while its bytes come.
pub(super) struct HandoffIn {
    pub job: i64,
    pub from: String,
    staging: PathBuf,
    block_size: u64,
    ranges: Vec<(u64, u64)>,
    pub incoming: Arc<Incoming>,
    /// Bytes of the pack written so far, and its size.
    pub got: Arc<AtomicU64>,
    pub size: u64,
    /// waiting (pack not started), arriving, arrived, stopped.
    pub state: Arc<std::sync::Mutex<&'static str>>,
    /// The download was removed while its bytes were arriving: the pack stops
    /// and cleans up after itself.
    gone: Arc<std::sync::atomic::AtomicBool>,
}

/// Where a hand-off's bytes go as they arrive.
struct PackSink {
    file: std::fs::File,
    path: PathBuf,
    ranges: Vec<(u64, u64)>,
    block_size: u64,
    /// Which range is being written, and how far into it.
    at: usize,
    into: u64,
    unsynced: u64,
    incoming: Arc<Incoming>,
    got: Arc<AtomicU64>,
    state: Arc<std::sync::Mutex<&'static str>>,
    gone: Arc<std::sync::atomic::AtomicBool>,
    tell: Arc<dyn Fn() + Send + Sync>,
    /// Drops this hand-off from the list (after a removed download's pack ends).
    forget: Arc<dyn Fn() + Send + Sync>,
}

impl PackSink {
    /// Makes what's written durable, then counts it in.
    fn settle(&mut self) -> std::io::Result<()> {
        self.file.sync_data()?;
        self.unsynced = 0;
        if let Some(&(off, _)) = self.ranges.get(self.at)
            && self.into > 0
        {
            self.incoming
                .arrived((off / self.block_size) as usize, self.into);
        }
        (self.tell)();
        Ok(())
    }

    fn end(&mut self, how: &'static str) {
        if self.gone.load(Ordering::Relaxed) {
            // Its download is gone: nothing to keep.
            let _ = std::fs::remove_file(&self.path);
            (self.forget)();
            return;
        }
        let _ = self.settle();
        self.incoming.release();
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = how;
        (self.tell)();
    }
}

impl fuselane_nearby::Sink for PackSink {
    fn write(&mut self, mut data: &[u8]) -> std::io::Result<()> {
        if self.gone.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("the download was removed"));
        }
        while !data.is_empty() {
            let Some(&(off, len)) = self.ranges.get(self.at) else {
                return Err(std::io::Error::other("more bytes than the hand-off said"));
            };
            let n = ((len - self.into) as usize).min(data.len());
            self.file.seek(SeekFrom::Start(off + self.into))?;
            self.file.write_all(&data[..n])?;
            self.into += n as u64;
            self.unsynced += n as u64;
            self.got.fetch_add(n as u64, Ordering::Relaxed);
            data = &data[n..];
            if self.into == len {
                self.settle()?;
                self.at += 1;
                self.into = 0;
            } else if self.unsynced >= SYNC_EVERY {
                self.settle()?;
            }
        }
        Ok(())
    }

    fn finish(mut self: Box<Self>) -> Result<PathBuf, String> {
        self.end("arrived");
        Ok(self.path.clone())
    }

    fn abort(mut self: Box<Self>) {
        self.end("stopped");
    }
}

/// The partial file's name for `filename` in `dir`, not taken yet.
fn free_staging(dir: &Path, filename: &str) -> PathBuf {
    let suffix = fuselane_storage::staging::STAGING_SUFFIX;
    let first = dir.join(format!("{filename}{suffix}"));
    if !first.exists() && !dir.join(filename).exists() {
        return first;
    }
    let (stem, ext) = match filename.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (filename, String::new()),
    };
    (2..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|name| !dir.join(name).exists() && !dir.join(format!("{name}{suffix}")).exists())
        .map_or(first, |name| dir.join(format!("{name}{suffix}")))
}

/// A random hand-off id: 32 hex digits.
fn new_id() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn is_id(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Checks everything another computer sent before anything is trusted.
pub fn validate(m: &Manifest, part_len: u64) -> Result<(), String> {
    if m.v != 1 && m.v != 2 {
        return Err("it was made by a newer Fuselane".into());
    }
    if m.v == 2 && !m.id.as_deref().is_some_and(is_id) {
        return Err("its id isn't usable".into());
    }
    fuselane_core::runner::parse_link(&m.url)
        .map_err(|e| format!("its link isn't usable ({e})"))?;
    for l in &m.mirrors {
        fuselane_core::runner::parse_link(l)
            .map_err(|_| "a mirror link isn't usable".to_string())?;
    }
    if m.total == 0 || m.block_size == 0 {
        return Err("its size is missing".into());
    }
    let blocks = m.total.div_ceil(m.block_size);
    if blocks as usize != m.secured.len() || m.secured.len() > MAX_BLOCKS {
        return Err("its progress doesn't match its size".into());
    }
    for (i, s) in m.secured.iter().enumerate() {
        let start = i as u64 * m.block_size;
        if *s > m.block_size.min(m.total - start) {
            return Err("its progress doesn't match its size".into());
        }
    }
    if part_len > m.total {
        return Err("the partial file is bigger than the download".into());
    }
    if let Some(h) = &m.sha256 {
        fuselane_core::runner::parse_sha256(h)
            .map_err(|e| format!("its checksum isn't usable ({e})"))?;
    }
    let bad_name =
        |n: &str| n.is_empty() || n.contains(['/', '\\']) || n.chars().any(char::is_control);
    if bad_name(&m.filename) || bad_name(&m.part) || m.chosen_name.as_deref().is_some_and(bad_name)
    {
        return Err("its file name isn't usable".into());
    }
    Ok(())
}

/// What to send for a hand-off (version 2): the manifest, then the pack.
pub struct HandoffOut {
    pub manifest: fuselane_nearby::Outgoing,
    pub pack: fuselane_nearby::Outgoing,
    /// The download's name, for the transfer's row.
    pub name: String,
}

impl Service {
    /// A hand-off of download `id` (version 2): a manifest written under `out`,
    /// and the downloaded parts of its partial file, read in place. It must be
    /// paused (or stopped and resumable).
    pub fn handoff_out(&self, id: i64, out: &Path, from: &str) -> Result<HandoffOut, UiError> {
        let (mut m, staging) = self.handoff_manifest(id)?;
        m.v = 2;
        m.id = Some(new_id());
        m.from = Some(from.chars().take(64).collect());
        std::fs::create_dir_all(out).map_err(store_error)?;
        let manifest = out.join(format!("{}{MANIFEST_SUFFIX}", m.filename));
        let json = serde_json::to_vec(&m).map_err(store_error)?;
        std::fs::write(&manifest, &json).map_err(store_error)?;
        let ranges = m.ranges();
        Ok(HandoffOut {
            manifest: fuselane_nearby::Outgoing {
                name: format!("{}{MANIFEST_SUFFIX}", m.filename),
                size: json.len() as u64,
                path: manifest,
                mime: "application/json".into(),
                ranges: vec![],
            },
            pack: fuselane_nearby::Outgoing {
                name: format!("{}{PACK_SUFFIX}", m.id.clone().unwrap_or_default()),
                size: ranges.iter().map(|r| r.1).sum(),
                path: staging,
                mime: "application/octet-stream".into(),
                ranges,
            },
            name: m.chosen_name.clone().unwrap_or_else(|| m.filename.clone()),
        })
    }

    /// The two files of a version 1 hand-off (older Fuselane): the whole
    /// partial file and a manifest written under `out`.
    #[cfg(test)]
    pub fn handoff_files(&self, id: i64, out: &Path) -> Result<Vec<PathBuf>, UiError> {
        let (m, staging) = self.handoff_manifest(id)?;
        std::fs::create_dir_all(out).map_err(store_error)?;
        let manifest = out.join(format!("{}{MANIFEST_SUFFIX}", m.filename));
        let json = serde_json::to_vec(&m).map_err(store_error)?;
        std::fs::write(&manifest, json).map_err(store_error)?;
        Ok(vec![staging, manifest])
    }

    /// The manifest (version 1) of a paused download, and its partial file.
    fn handoff_manifest(&self, id: i64) -> Result<(Manifest, PathBuf), UiError> {
        let job = self.store.get(id).map_err(|_| not_found(id))?;
        if !matches!(
            job.status,
            Status::Paused | Status::Failed { resumable: true }
        ) {
            return Err(UiError::new(
                "not-paused",
                "Pause the download first, then send it.",
                None,
            ));
        }
        let resume = job.resume().ok_or_else(|| {
            UiError::new(
                "not-resumable",
                "This download hasn't started yet, so there's nothing to hand over.",
                Some("Send the link instead, or start it for a moment first."),
            )
        })?;
        let part = resume
            .staging_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let m = Manifest {
            v: 1,
            url: job.url.clone(),
            filename: job
                .filename
                .clone()
                .unwrap_or_else(|| super::job_name(&job)),
            chosen_name: job.chosen_name.clone(),
            total: resume.total,
            block_size: resume.block_size,
            secured: resume.secured.clone(),
            raw_etag: resume.raw_etag.clone(),
            last_modified: resume.last_modified.clone(),
            sha256: job.expected_sha256.clone(),
            mirrors: job.mirrors.clone(),
            part,
            id: None,
            from: None,
        };
        Ok((m, resume.staging_path))
    }

    /// Received files from Nearby: if they hold a hand-off, it becomes a
    /// download here. Returns its id; anything wrong is reported, never trusted.
    pub fn import_handoff(self: &Arc<Self>, received: &[PathBuf]) -> Option<Result<i64, String>> {
        let manifest = received.iter().find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(MANIFEST_SUFFIX))
        })?;
        Some(self.import_from(manifest, received))
    }

    fn import_from(self: &Arc<Self>, manifest: &Path, received: &[PathBuf]) -> Result<i64, String> {
        let len = std::fs::metadata(manifest)
            .map_err(|e| e.to_string())?
            .len();
        if len > MAX_MANIFEST {
            return Err("the hand-off is too big to be real".into());
        }
        let m: Manifest =
            serde_json::from_slice(&std::fs::read(manifest).map_err(|e| e.to_string())?)
                .map_err(|_| "the hand-off couldn't be read".to_string())?;
        if m.v == 2 {
            return self.arriving(&m, manifest);
        }
        // The partial file is the other one received (it may have been renamed on arrival).
        let part = received
            .iter()
            .find(|p| *p != manifest && p.is_file())
            .ok_or_else(|| "the partial file didn't arrive".to_string())?;
        let part_len = std::fs::metadata(part).map_err(|e| e.to_string())?.len();
        validate(&m, part_len)?;
        let dir = part.parent().ok_or("no folder")?.to_path_buf();
        let new = fuselane_core::store::NewJob {
            name: m.chosen_name.clone(),
            sha256: m.sha256.clone(),
        };
        let id = self
            .store
            .create_with(&m.url, &dir, &new)
            .map_err(|e| e.to_string())?;
        let cp = fuselane_engine_http::download::Checkpoint {
            staging_path: part.clone(),
            filename: m.filename.clone(),
            total: Some(m.total),
            block_size: m.block_size,
            secured: m.secured.clone(),
            raw_etag: m.raw_etag.clone(),
            last_modified: m.last_modified.clone(),
        };
        let saved = self
            .store
            .save_checkpoint(id, &cp)
            .and_then(|()| self.store.apply(id, Event::Pause, None).map(|_| ()))
            .and_then(|()| self.store.set_mirrors(id, &m.mirrors));
        if let Err(e) = saved {
            let _ = self.store.delete(id);
            return Err(e.to_string());
        }
        let _ = std::fs::remove_file(manifest);
        let _ = self.store.set_error(
            id,
            "Handed over from another computer. Resume to carry on from where it was.",
            "handoff",
        );
        self.publish_jobs();
        Ok(id)
    }
}

impl Service {
    /// A version 2 hand-off's manifest arrived: the download is made here at
    /// once and starts (C2, C3); its downloaded bytes are on their way.
    fn arriving(self: &Arc<Self>, m: &Manifest, manifest: &Path) -> Result<i64, String> {
        validate(m, 0)?;
        let hid = m.id.clone().unwrap_or_default();
        if lock(&self.handoffs_in).contains_key(&hid) {
            return Err("this hand-off arrived already".into());
        }
        let dir = manifest.parent().ok_or("no folder")?.to_path_buf();
        let staging = free_staging(&dir, &m.filename);
        std::fs::File::create_new(&staging)
            .and_then(|f| f.set_len(m.total))
            .map_err(|e| format!("couldn't make the file ({e})"))?;
        let new = fuselane_core::store::NewJob {
            name: m.chosen_name.clone(),
            sha256: m.sha256.clone(),
        };
        let id = self
            .store
            .create_with(&m.url, &dir, &new)
            .map_err(|e| e.to_string())?;
        let cp = fuselane_engine_http::download::Checkpoint {
            staging_path: staging.clone(),
            filename: m.filename.clone(),
            total: Some(m.total),
            block_size: m.block_size,
            secured: vec![0; m.secured.len()],
            raw_etag: m.raw_etag.clone(),
            last_modified: m.last_modified.clone(),
        };
        let saved = self
            .store
            .save_checkpoint(id, &cp)
            .and_then(|()| self.store.set_mirrors(id, &m.mirrors));
        if let Err(e) = saved {
            let _ = self.store.delete(id);
            let _ = std::fs::remove_file(&staging);
            return Err(e.to_string());
        }
        let _ = std::fs::remove_file(manifest);
        let ranges = m.ranges();
        lock(&self.handoffs_in).insert(
            hid,
            HandoffIn {
                job: id,
                from: m
                    .from
                    .clone()
                    .filter(|f| !f.trim().is_empty() && !f.chars().any(char::is_control))
                    .unwrap_or_else(|| "another computer".into()),
                staging,
                block_size: m.block_size,
                size: ranges.iter().map(|r| r.1).sum(),
                ranges,
                incoming: Incoming::new(m.secured.clone()),
                got: Arc::default(),
                state: Arc::new(std::sync::Mutex::new("waiting")),
                gone: Arc::default(),
            },
        );
        // It starts now: what was never downloaded comes from the internet.
        self.publish_jobs();
        self.pump();
        Ok(id)
    }

    /// Whether a file Nearby is offered is the pack of a hand-off that's
    /// expected here (its id, random and known only to the two computers, is
    /// the proof: it's taken without asking).
    pub fn expects_pack(&self, file_name: &str) -> bool {
        file_name.strip_suffix(PACK_SUFFIX).is_some_and(|id| {
            lock(&self.handoffs_in)
                .get(id)
                .is_some_and(|h| h.state_is("waiting"))
        })
    }

    /// Where an expected pack's bytes go: straight into the download's file.
    pub fn pack_sink(self: &Arc<Self>, file_name: &str) -> Option<Box<dyn fuselane_nearby::Sink>> {
        let id = file_name.strip_suffix(PACK_SUFFIX)?;
        let handoffs = lock(&self.handoffs_in);
        let h = handoffs.get(id).filter(|h| h.state_is("waiting"))?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&h.staging)
            .ok()?;
        *h.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = "arriving";
        let me = Arc::downgrade(self);
        let (me2, hid) = (Arc::downgrade(self), id.to_string());
        Some(Box::new(PackSink {
            file,
            path: h.staging.clone(),
            ranges: h.ranges.clone(),
            block_size: h.block_size,
            at: 0,
            into: 0,
            unsynced: 0,
            incoming: h.incoming.clone(),
            got: h.got.clone(),
            state: h.state.clone(),
            gone: h.gone.clone(),
            tell: Arc::new(move || {
                if let Some(s) = me.upgrade() {
                    s.publish_jobs();
                }
            }),
            forget: Arc::new(move || {
                if let Some(s) = me2.upgrade() {
                    lock(&s.handoffs_in).remove(&hid);
                }
            }),
        }))
    }

    /// The bytes arriving for download `job`, for its run.
    pub(super) fn incoming_for(&self, job: i64) -> Option<Arc<Incoming>> {
        lock(&self.handoffs_in)
            .values()
            .find(|h| h.job == job)
            .map(|h| h.incoming.clone())
    }

    /// How a download arriving from another computer is getting on, for its view.
    pub(super) fn arriving_view(&self) -> HashMap<i64, ArrivingView> {
        lock(&self.handoffs_in)
            .values()
            .map(|h| {
                (
                    h.job,
                    ArrivingView {
                        from: h.from.clone(),
                        done: h.got.load(Ordering::Relaxed),
                        total: h.size,
                        state: h.state(),
                    },
                )
            })
            .collect()
    }

    /// Forgets the hand-off of a download that's gone or finished. One whose
    /// bytes are still arriving is told to stop; it cleans up when it ends.
    pub(super) fn forget_handoff(&self, job: i64) {
        lock(&self.handoffs_in).retain(|_, h| {
            if h.job != job {
                return true;
            }
            if h.state_is("arriving") {
                h.gone.store(true, Ordering::Relaxed);
                return true;
            }
            false
        });
    }
}

impl HandoffIn {
    fn state(&self) -> &'static str {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn state_is(&self, s: &str) -> bool {
        self.state() == s
    }
}

/// A download arriving from another computer, as the window shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArrivingView {
    pub from: String,
    /// Bytes of what was downloaded there that have arrived, of `total`.
    pub done: u64,
    pub total: u64,
    /// waiting, arriving, arrived or stopped.
    pub state: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good() -> Manifest {
        Manifest {
            v: 1,
            url: "https://e.org/os.iso".into(),
            filename: "os.iso".into(),
            chosen_name: None,
            total: 2500,
            block_size: 1000,
            secured: vec![1000, 300, 0],
            raw_etag: Some("\"v1\"".into()),
            last_modified: None,
            sha256: None,
            mirrors: vec![],
            part: "os.iso.fuselane".into(),
            id: None,
            from: None,
        }
    }

    #[test]
    fn a_handoff_is_checked_before_it_is_trusted() {
        assert!(validate(&good(), 2500).is_ok());
        let bad = |f: fn(&mut Manifest)| {
            let mut m = good();
            f(&mut m);
            validate(&m, 2500).is_err()
        };
        assert!(bad(|m| m.v = 2));
        assert!(bad(|m| m.url = "file:///etc/passwd".into()));
        assert!(bad(|m| m.mirrors = vec!["ftp://x/y".into()]));
        assert!(bad(|m| m.secured = vec![1000, 300]), "wrong block count");
        assert!(
            bad(|m| m.secured = vec![1000, 1000, 600]),
            "last block is 500"
        );
        assert!(bad(|m| m.block_size = 0));
        assert!(bad(|m| m.filename = "../../.ssh/key".into()));
        assert!(bad(|m| m.part = "a/b".into()));
        assert!(bad(|m| m.sha256 = Some("xyz".into())));
        assert!(
            validate(&good(), 2501).is_err(),
            "part longer than the file"
        );
    }
}
