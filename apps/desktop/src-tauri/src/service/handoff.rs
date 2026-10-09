//! Hand off over Nearby (B9.9): a paused download goes to another Fuselane and
//! carries on there. Two files travel: the partial file (`name.fuselane`) and a
//! small manifest (`name.fuselane-handoff`) with the link and which bytes are
//! already secured. The receiver checks the manifest strictly and adds the
//! download paused; on Resume the engine's own checks (ETag, Last-Modified,
//! exact ranges) prove it's the same file before any byte is kept (L-108).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use fuselane_core::{Event, Status};

use super::{Service, UiError, not_found, store_error};

/// The manifest's file name ends with this.
pub const MANIFEST_SUFFIX: &str = ".fuselane-handoff";
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
}

/// Checks everything another computer sent before anything is trusted.
pub fn validate(m: &Manifest, part_len: u64) -> Result<(), String> {
    if m.v != 1 {
        return Err("it was made by a newer Fuselane".into());
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

impl Service {
    /// The two files to send for download `id`: its partial file and a manifest
    /// written under `out`. It must be paused (or stopped and resumable).
    pub fn handoff_files(&self, id: i64, out: &Path) -> Result<Vec<PathBuf>, UiError> {
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
        };
        std::fs::create_dir_all(out).map_err(store_error)?;
        let manifest = out.join(format!("{}{MANIFEST_SUFFIX}", m.filename));
        let json = serde_json::to_vec(&m).map_err(store_error)?;
        std::fs::write(&manifest, json).map_err(store_error)?;
        Ok(vec![resume.staging_path, manifest])
    }

    /// Received files from Nearby: if they hold a hand-off, it becomes a paused
    /// download here. Returns its id; anything wrong is reported, never trusted.
    pub fn import_handoff(&self, received: &[PathBuf]) -> Option<Result<i64, String>> {
        let manifest = received.iter().find(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(MANIFEST_SUFFIX))
        })?;
        Some(self.import_from(manifest, received))
    }

    fn import_from(&self, manifest: &Path, received: &[PathBuf]) -> Result<i64, String> {
        let len = std::fs::metadata(manifest)
            .map_err(|e| e.to_string())?
            .len();
        if len > MAX_MANIFEST {
            return Err("the hand-off is too big to be real".into());
        }
        let m: Manifest =
            serde_json::from_slice(&std::fs::read(manifest).map_err(|e| e.to_string())?)
                .map_err(|_| "the hand-off couldn't be read".to_string())?;
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
