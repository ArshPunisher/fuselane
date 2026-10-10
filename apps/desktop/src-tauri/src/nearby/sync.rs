//! Folders kept in sync (B10.3): a folder here is mirrored into a trusted
//! computer's downloads folder over Nearby. New and changed files go across
//! whenever both are on the network; nothing is deleted on the other side.
//! No cloud, no account: it only works between your own computers.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};

use super::{Nearby, err, folders, lock};
use crate::service::UiError;
use fuselane_nearby::client::{Outgoing, Target};

/// Most sent in one round; the rest follow in the next.
const ROUND_FILES: usize = 300;
const ROUND_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncJob {
    pub id: u64,
    pub folder: PathBuf,
    pub device: String,
    pub alias: String,
    /// What was sent last, per relative name: size and time changed.
    #[serde(default)]
    pub sent: HashMap<String, (u64, i64)>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncView {
    pub id: u64,
    pub folder: String,
    pub name: String,
    pub device: String,
    /// up-to-date | sending | waiting | problem
    pub state: &'static str,
    pub files: usize,
    /// Files still to send.
    pub pending: usize,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Status {
    pub state: &'static str,
    pub files: usize,
    pub pending: usize,
    pub note: Option<String>,
}

impl Nearby {
    fn sync_file(&self) -> PathBuf {
        self.state_dir.join("folder-sync.json")
    }

    pub(super) fn load_syncs(&self) {
        let jobs: Vec<SyncJob> = std::fs::read(self.sync_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        *lock(&self.syncs) = jobs;
    }

    fn save_syncs(&self) {
        if let Ok(json) = serde_json::to_vec(&*lock(&self.syncs)) {
            let _ = std::fs::write(self.sync_file(), json);
        }
    }

    pub fn sync_views(&self) -> Vec<SyncView> {
        let status = lock(&self.sync_status).clone();
        lock(&self.syncs)
            .iter()
            .map(|j| {
                let s = status.get(&j.id).cloned().unwrap_or_default();
                SyncView {
                    id: j.id,
                    folder: j.folder.display().to_string(),
                    name: j
                        .folder
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    device: j.alias.clone(),
                    state: if s.state.is_empty() {
                        "waiting"
                    } else {
                        s.state
                    },
                    files: s.files,
                    pending: s.pending,
                    note: s.note,
                }
            })
            .collect()
    }

    /// Keeps `folder` in sync with a trusted device (files from here go there).
    pub fn add_sync(self: &Arc<Self>, folder: &str, fingerprint: &str) -> Result<(), UiError> {
        let folder = PathBuf::from(folder.trim());
        if !folder.is_dir() {
            return Err(err("sync-folder", "That folder isn't there anymore.", None));
        }
        if !self.is_trusted(fingerprint) {
            return Err(err(
                "sync-trust",
                "Folders are kept in sync only with trusted computers.",
                Some("Send that computer a file and tick Trust when it asks, then try again."),
            ));
        }
        let alias = lock(&self.trusted)
            .iter()
            .find(|t| t.fingerprint == fingerprint)
            .map(|t| t.alias.clone())
            .unwrap_or_default();
        {
            let mut jobs = lock(&self.syncs);
            if jobs
                .iter()
                .any(|j| j.folder == folder && j.device == fingerprint)
            {
                return Ok(());
            }
            let id = jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;
            jobs.push(SyncJob {
                id,
                folder,
                device: fingerprint.into(),
                alias,
                sent: HashMap::new(),
            });
        }
        self.save_syncs();
        self.publish();
        let me = self.clone();
        tokio::spawn(async move { me.sync_round(true).await });
        Ok(())
    }

    pub fn remove_sync(&self, id: u64) {
        lock(&self.syncs).retain(|j| j.id != id);
        lock(&self.sync_status).remove(&id);
        self.save_syncs();
        self.publish();
    }

    /// One pass over every synced folder; `now` skips the 30-second spacing.
    pub async fn sync_round(self: &Arc<Self>, now: bool) {
        if self.sync_busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let due = now
            || lock(&self.sync_last)
                .is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(30));
        if due {
            *lock(&self.sync_last) = Some(std::time::Instant::now());
            let jobs = lock(&self.syncs).clone();
            for job in jobs {
                self.sync_one(job).await;
            }
        }
        self.sync_busy.store(false, Ordering::SeqCst);
    }

    fn set_status(&self, id: u64, s: Status) {
        lock(&self.sync_status).insert(id, s);
        self.publish();
    }

    async fn sync_one(self: &Arc<Self>, job: SyncJob) {
        let id = job.id;
        let folder = job.folder.clone();
        let entries = tokio::task::spawn_blocking(move || folders::walk(&folder))
            .await
            .unwrap_or_default();
        let todo: Vec<folders::Entry> = folders::changed(&entries, &job.sent)
            .into_iter()
            .cloned()
            .collect();
        let files = entries.len();
        if !job.folder.is_dir() {
            return self.set_status(
                id,
                Status {
                    state: "problem",
                    files,
                    pending: 0,
                    note: Some("The folder isn't there anymore.".into()),
                },
            );
        }
        if todo.is_empty() {
            return self.set_status(
                id,
                Status {
                    state: "up-to-date",
                    files,
                    ..Status::default()
                },
            );
        }
        let seen = lock(&self.devices)
            .get(&job.device)
            .map(|s| (s.info.clone(), s.addr));
        let Some((_, addr)) = seen.filter(|_| self.is_trusted(&job.device)) else {
            return self.set_status(
                id,
                Status {
                    state: "waiting",
                    files,
                    pending: todo.len(),
                    note: Some(format!("Waiting for {} to be on the network.", job.alias)),
                },
            );
        };
        let mut batch = vec![];
        let mut bytes = 0;
        for e in &todo {
            if batch.len() >= ROUND_FILES || (bytes > 0 && bytes + e.size > ROUND_BYTES) {
                break;
            }
            bytes += e.size;
            batch.push(e.clone());
        }
        self.set_status(
            id,
            Status {
                state: "sending",
                files,
                pending: todo.len(),
                note: None,
            },
        );
        let out: Vec<Outgoing> = batch
            .iter()
            .map(|e| Outgoing {
                path: e.path.clone(),
                name: e.rel.clone(),
                size: e.size,
                mime: "application/octet-stream".into(),
            })
            .collect();
        let target = Target {
            addr,
            fingerprint: Some(job.device.clone()),
        };
        let me = self.info();
        let result = fuselane_nearby::client::send(
            &me,
            &target,
            &out,
            Arc::default(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .await;
        match result {
            Ok(_) => {
                if let Some(j) = lock(&self.syncs).iter_mut().find(|j| j.id == id) {
                    for e in &batch {
                        j.sent.insert(e.rel.clone(), (e.size, e.modified));
                    }
                }
                self.save_syncs();
                let left = todo.len() - batch.len();
                self.set_status(
                    id,
                    Status {
                        state: if left == 0 { "up-to-date" } else { "sending" },
                        files,
                        pending: left,
                        note: None,
                    },
                );
            }
            Err(e) => self.set_status(
                id,
                Status {
                    state: "problem",
                    files,
                    pending: todo.len(),
                    note: Some(format!("Couldn't send: {e}. Tried again in a moment.")),
                },
            ),
        }
    }
}
