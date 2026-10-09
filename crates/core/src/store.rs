//! SQLite persistence for jobs (ADR 0008, L-49–L-53).
//!
//! WAL mode, numbered migrations in one transaction, and corruption handled
//! loudly: a database that can't be read is moved aside (never overwritten) and
//! the caller is told, so the user can be told (L-50).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use fuselane_engine_http::download::{Checkpoint, Resume};
use rusqlite::{Connection, OptionalExtension, params};

use crate::job::{Event, InvalidTransition, Status};

/// Each entry upgrades the schema by one version. Never edit a released entry;
/// append a new one (L-51).
const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE jobs (
        id            INTEGER PRIMARY KEY,
        url           TEXT NOT NULL,
        dir           TEXT NOT NULL,
        filename      TEXT,
        staging_path  TEXT,
        total         INTEGER,
        block_size    INTEGER,
        secured       BLOB,
        raw_etag      TEXT,
        last_modified TEXT,
        status        TEXT NOT NULL,
        error         TEXT,
        final_path    TEXT,
        created_at    INTEGER NOT NULL,
        updated_at    INTEGER NOT NULL
    );",
    // v2: what the user can do about a failure (runner::action_for).
    "ALTER TABLE jobs ADD COLUMN error_code TEXT;",
    // v3: never reuse an id. Without AUTOINCREMENT, removing the newest job let the
    // next one take its id, inheriting the UI's live view and notices (L-116).
    "CREATE TABLE jobs_v3 (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        url           TEXT NOT NULL,
        dir           TEXT NOT NULL,
        filename      TEXT,
        staging_path  TEXT,
        total         INTEGER,
        block_size    INTEGER,
        secured       BLOB,
        raw_etag      TEXT,
        last_modified TEXT,
        status        TEXT NOT NULL,
        error         TEXT,
        final_path    TEXT,
        created_at    INTEGER NOT NULL,
        updated_at    INTEGER NOT NULL,
        error_code    TEXT
    );
    INSERT INTO jobs_v3 (id, url, dir, filename, staging_path, total, block_size, secured,
        raw_etag, last_modified, status, error, final_path, created_at, updated_at, error_code)
    SELECT id, url, dir, filename, staging_path, total, block_size, secured,
        raw_etag, last_modified, status, error, final_path, created_at, updated_at, error_code FROM jobs;
    DROP TABLE jobs;
    ALTER TABLE jobs_v3 RENAME TO jobs;",
    // v4: app settings (speed limits and later preferences), as small JSON values.
    "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    // v5: queue order, a name chosen before starting, and a checksum to verify.
    "ALTER TABLE jobs ADD COLUMN position INTEGER;
     ALTER TABLE jobs ADD COLUMN chosen_name TEXT;
     ALTER TABLE jobs ADD COLUMN expected_sha256 TEXT;",
    // v6: a download's own speed limit (bytes per second; 0 = none).
    "ALTER TABLE jobs ADD COLUMN speed_limit INTEGER NOT NULL DEFAULT 0;",
    // v7: start at a set time (unix seconds); the job waits paused until then.
    "ALTER TABLE jobs ADD COLUMN start_at INTEGER;",
    // v8: replace a file of the same name once this one is complete (else keep both).
    "ALTER TABLE jobs ADD COLUMN replace_existing INTEGER NOT NULL DEFAULT 0;",
    // v9: other links to the same file (one per line), checked before they help.
    "ALTER TABLE jobs ADD COLUMN mirrors TEXT NOT NULL DEFAULT '';",
    // v10: where a checksum found by itself came from ('' = looked, none found).
    "ALTER TABLE jobs ADD COLUMN sha256_from TEXT;",
];

/// What a person can set when adding a download, beyond the link and folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewJob {
    /// Save under this name instead of the server's.
    pub name: Option<String>,
    /// SHA-256 to verify, as 64 hex digits.
    pub sha256: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Transition(#[from] InvalidTransition),
    #[error("no job {0}")]
    NotFound(i64),
    #[error(
        "the database was made by a newer Fuselane (schema v{found}, this build knows v{known})"
    )]
    TooNew { found: i64, known: i64 },
}

/// A job as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: i64,
    pub url: String,
    pub dir: PathBuf,
    pub filename: Option<String>,
    pub staging_path: Option<PathBuf>,
    pub total: Option<u64>,
    pub block_size: Option<u64>,
    pub secured: Vec<u64>,
    pub raw_etag: Option<String>,
    pub last_modified: Option<String>,
    pub status: Status,
    pub error: Option<String>,
    /// The fix the UI offers for `error`: fix-link, retry, start-over, free-space.
    pub error_code: Option<String>,
    pub final_path: Option<PathBuf>,
    pub created_at: i64,
    pub updated_at: i64,
    /// Queue order: lower starts first. Jobs from before v5 sort by id.
    pub position: i64,
    pub chosen_name: Option<String>,
    pub expected_sha256: Option<String>,
    /// This download's own speed limit in bytes per second; 0 = none.
    pub speed_limit: u64,
    /// When it starts by itself (unix seconds); it waits paused until then.
    pub start_at: Option<i64>,
    /// Replace a file already there under the same name (moved to the Trash).
    pub replace_existing: bool,
    /// Other links to the same file.
    pub mirrors: Vec<String>,
    /// Where Fuselane found its checksum by itself ("SHA256SUMS"); Some("") when
    /// it looked and found none; None when it hasn't looked (B9.7).
    pub sha256_from: Option<String>,
}

impl Job {
    pub fn secured_bytes(&self) -> u64 {
        self.secured.iter().sum()
    }

    /// What the engine needs to continue, if this job can be continued.
    pub fn resume(&self) -> Option<Resume> {
        Some(Resume {
            staging_path: self.staging_path.clone()?,
            total: self.total?,
            block_size: self.block_size?,
            secured: self.secured.clone(),
            raw_etag: self.raw_etag.clone(),
            last_modified: self.last_modified.clone(),
        })
    }
}

/// Held while a download runs; the OS releases it when this drops or the process ends.
#[derive(Debug)]
pub struct JobLock {
    _file: Option<std::fs::File>,
}

/// The job database. Safe to share across threads.
#[derive(Debug)]
pub struct Store {
    conn: Mutex<Connection>,
    /// The folder holding the database (per-download locks live beside it).
    dir: Option<PathBuf>,
    /// Set when the previous database couldn't be read and was moved here.
    pub recovered_from: Option<PathBuf>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// Only damage moves a database aside. Busy or locked (another process
/// mid-write), a full disk or a permission problem is reported as is: the file is
/// fine, and moving it would orphan every job in it.
fn is_corrupt(e: &StoreError) -> bool {
    use rusqlite::ErrorCode::{DatabaseCorrupt, NotADatabase};
    match e {
        // open_inner's marker for a failed quick_check.
        StoreError::Sql(rusqlite::Error::InvalidQuery) => true,
        StoreError::Sql(e) => matches!(e.sqlite_error_code(), Some(DatabaseCorrupt | NotADatabase)),
        _ => false,
    }
}

fn encode(secured: &[u64]) -> Vec<u8> {
    secured.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Untrusted bytes back into numbers; a malformed blob is treated as no progress (L-53).
fn decode(blob: Option<Vec<u8>>) -> Vec<u64> {
    match blob {
        Some(b) if b.len() % 8 == 0 => b
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| u64::from_le_bytes(*c))
            .collect(),
        _ => Vec::new(),
    }
}

impl Store {
    /// Opens (or creates) the database at `path`.
    /// Claims download `id` for this process until the guard drops, so the
    /// command line and the app never run the same download at once (2.42).
    /// None when another process (or another run here) already has it.
    pub fn lock_job(&self, id: i64) -> std::io::Result<Option<JobLock>> {
        let Some(dir) = &self.dir else {
            return Ok(Some(JobLock { _file: None }));
        };
        let locks = dir.join("locks");
        std::fs::create_dir_all(&locks)?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(locks.join(format!("job-{id}.lock")))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(JobLock { _file: Some(file) })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }

    pub fn open(path: &Path) -> Result<Store, StoreError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        match Self::open_inner(path) {
            Ok(conn) => Ok(Store {
                conn: Mutex::new(conn),
                dir: path.parent().map(Path::to_path_buf),
                recovered_from: None,
            }),
            Err(e) if path.exists() && is_corrupt(&e) => {
                // Unreadable: move it aside, never delete or overwrite it (L-50).
                let aside = path.with_extension(format!("corrupt-{}", now()));
                std::fs::rename(path, &aside)?;
                for ext in ["db-wal", "db-shm"] {
                    let _ = std::fs::remove_file(path.with_extension(ext));
                }
                let conn = Self::open_inner(path)?;
                Ok(Store {
                    conn: Mutex::new(conn),
                    dir: path.parent().map(Path::to_path_buf),
                    recovered_from: Some(aside),
                })
            }
            Err(e) => Err(e),
        }
    }

    fn open_inner(path: &Path) -> Result<Connection, StoreError> {
        let mut conn = Connection::open(path)?;
        // Set before anything else touches the file, so every statement here
        // (including the WAL switch) waits for another process instead of failing.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        // Integrity first: a garbage file must fail here, not later.
        let ok: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if ok != "ok" {
            return Err(StoreError::Sql(rusqlite::Error::InvalidQuery));
        }
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let known = MIGRATIONS.len() as i64;
        if version > known {
            return Err(StoreError::TooNew {
                found: version,
                known,
            });
        }
        let tx = conn.transaction()?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
        }
        tx.commit()?;
        Ok(conn)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn create(&self, url: &str, dir: &Path) -> Result<i64, StoreError> {
        self.create_with(url, dir, &NewJob::default())
    }

    /// A new queued job at the end of the queue.
    pub fn create_with(&self, url: &str, dir: &Path, new: &NewJob) -> Result<i64, StoreError> {
        let c = self.lock();
        let t = now();
        c.execute(
            "INSERT INTO jobs (url, dir, status, created_at, updated_at, position, chosen_name, expected_sha256)
             VALUES (?1, ?2, ?3, ?4, ?4,
                     (SELECT COALESCE(MAX(COALESCE(position, id)), 0) + 1 FROM jobs), ?5, ?6)",
            params![
                url,
                dir.to_string_lossy(),
                Status::Queued.as_str(),
                t,
                new.name,
                new.sha256
            ],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// Puts `ids` first in the queue, in that order; everyone else keeps their order after.
    pub fn reorder(&self, ids: &[i64]) -> Result<(), StoreError> {
        let mut c = self.lock();
        let tx = c.transaction()?;
        let rest: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT id FROM jobs ORDER BY COALESCE(position, id), id")?;
            stmt.query_map([], |r| r.get(0))?
                .collect::<Result<Vec<i64>, _>>()?
                .into_iter()
                .filter(|id| !ids.contains(id))
                .collect()
        };
        for (pos, id) in ids.iter().chain(rest.iter()).enumerate() {
            tx.execute(
                "UPDATE jobs SET position = ?2 WHERE id = ?1",
                params![id, pos as i64 + 1],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: i64) -> Result<Job, StoreError> {
        self.lock()
            .query_row("SELECT * FROM jobs WHERE id = ?1", params![id], row_to_job)
            .optional()?
            .ok_or(StoreError::NotFound(id))
    }

    /// Newest first.
    pub fn list(&self) -> Result<Vec<Job>, StoreError> {
        let c = self.lock();
        let mut stmt = c.prepare("SELECT * FROM jobs ORDER BY id DESC")?;
        let rows = stmt
            .query_map([], row_to_job)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Records a checkpoint (the engine has already fsynced the staging file, L-55).
    pub fn save_checkpoint(&self, id: i64, cp: &Checkpoint) -> Result<(), StoreError> {
        let n = self.lock().execute(
            "UPDATE jobs SET filename = ?2, staging_path = ?3, total = ?4, block_size = ?5, secured = ?6,
                 raw_etag = ?7, last_modified = ?8, updated_at = ?9 WHERE id = ?1",
            params![
                id,
                cp.filename,
                cp.staging_path.to_string_lossy(),
                cp.total.map(|t| t as i64),
                cp.block_size as i64,
                encode(&cp.secured),
                cp.raw_etag,
                cp.last_modified,
                now()
            ],
        )?;
        if n == 0 {
            Err(StoreError::NotFound(id))
        } else {
            Ok(())
        }
    }

    /// Applies an event through the state machine; invalid transitions are refused.
    pub fn apply(&self, id: i64, event: Event, error: Option<&str>) -> Result<Status, StoreError> {
        let current = self.get(id)?.status;
        let next = current.apply(event)?;
        self.lock().execute(
            "UPDATE jobs SET status = ?2, error = ?3, error_code = NULL, updated_at = ?4 WHERE id = ?1",
            params![id, next.as_str(), error, now()],
        )?;
        Ok(next)
    }

    /// Records where the finished file is and its size (a fast download may finish
    /// before its first checkpoint, so the size isn't known any other way).
    /// A stored setting, if set.
    pub fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .lock()
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.lock().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Explains why a stopped job stopped (for example, an allowance pause).
    pub fn set_error(&self, id: i64, message: &str, code: &str) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE jobs SET error = ?2, error_code = ?3 WHERE id = ?1",
            params![id, message, code],
        )?;
        Ok(())
    }

    /// Records which fix the UI should offer for the current failure.
    pub fn set_error_code(&self, id: i64, code: &str) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE jobs SET error_code = ?2 WHERE id = ?1",
            params![id, code],
        )?;
        Ok(())
    }

    /// Points a stopped job at a new link for the same file (an expired signed URL).
    /// The engine's resume checks still prove it's the same file (L-108).
    pub fn set_url(&self, id: i64, url: &str) -> Result<(), StoreError> {
        let job = self.get(id)?;
        if matches!(
            job.status,
            Status::Running | Status::Completed | Status::Cancelled
        ) {
            return Err(StoreError::Transition(InvalidTransition {
                from: job.status,
                event: Event::Resume,
            }));
        }
        self.lock().execute(
            "UPDATE jobs SET url = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, url, now()],
        )?;
        Ok(())
    }

    /// Sets a download's own speed limit (0 removes it); any state.
    pub fn set_speed_limit(&self, id: i64, rate: u64) -> Result<(), StoreError> {
        let n = self.lock().execute(
            "UPDATE jobs SET speed_limit = ?2 WHERE id = ?1",
            params![id, i64::try_from(rate).unwrap_or(i64::MAX)],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id));
        }
        Ok(())
    }

    /// Records a checksum looked up next to the file, or that none was found
    /// (`None`), so the lookup isn't repeated on every resume.
    pub fn set_found_sha256(&self, id: i64, found: Option<(&str, &str)>) -> Result<(), StoreError> {
        let n = match found {
            Some((hash, from)) => self.lock().execute(
                "UPDATE jobs SET expected_sha256 = ?2, sha256_from = ?3
                 WHERE id = ?1 AND expected_sha256 IS NULL",
                params![id, hash, from],
            )?,
            None => self.lock().execute(
                "UPDATE jobs SET sha256_from = '' WHERE id = ?1",
                params![id],
            )?,
        };
        if n == 0 && self.get(id).is_err() {
            return Err(StoreError::NotFound(id));
        }
        Ok(())
    }

    /// Other links to the same file (each one checked before it helps).
    pub fn set_mirrors(&self, id: i64, mirrors: &[String]) -> Result<(), StoreError> {
        let n = self.lock().execute(
            "UPDATE jobs SET mirrors = ?2 WHERE id = ?1",
            params![id, mirrors.join("\n")],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id));
        }
        Ok(())
    }

    /// Whether a file already there under the same name is replaced when this finishes.
    pub fn set_replace_existing(&self, id: i64, on: bool) -> Result<(), StoreError> {
        let n = self.lock().execute(
            "UPDATE jobs SET replace_existing = ?2 WHERE id = ?1",
            params![id, i64::from(on)],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id));
        }
        Ok(())
    }

    /// Sets (or clears) when a download starts by itself.
    pub fn set_start_at(&self, id: i64, at: Option<i64>) -> Result<(), StoreError> {
        let n = self.lock().execute(
            "UPDATE jobs SET start_at = ?2 WHERE id = ?1",
            params![id, at],
        )?;
        if n == 0 {
            return Err(StoreError::NotFound(id));
        }
        Ok(())
    }

    pub fn set_finished(&self, id: i64, path: &Path, total: u64) -> Result<(), StoreError> {
        self.lock().execute(
            "UPDATE jobs SET final_path = ?2, staging_path = NULL, total = ?3 WHERE id = ?1",
            params![id, path.to_string_lossy(), total as i64],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<(), StoreError> {
        let n = self
            .lock()
            .execute("DELETE FROM jobs WHERE id = ?1", params![id])?;
        if n == 0 {
            Err(StoreError::NotFound(id))
        } else {
            Ok(())
        }
    }

    /// After a crash, jobs still marked running didn't stop cleanly: they come back
    /// paused, never auto-started (L-53).
    pub fn recover_interrupted(&self) -> Result<usize, StoreError> {
        Ok(self.lock().execute(
            "UPDATE jobs SET status = 'paused', updated_at = ?1 WHERE status IN ('running', 'queued')",
            params![now()],
        )?)
    }
}

fn row_to_job(r: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    let path = |s: Option<String>| s.map(PathBuf::from);
    Ok(Job {
        id: r.get("id")?,
        url: r.get("url")?,
        dir: PathBuf::from(r.get::<_, String>("dir")?),
        filename: r.get("filename")?,
        staging_path: path(r.get("staging_path")?),
        total: r
            .get::<_, Option<i64>>("total")?
            .and_then(|v| u64::try_from(v).ok()),
        block_size: r
            .get::<_, Option<i64>>("block_size")?
            .and_then(|v| u64::try_from(v).ok())
            .filter(|b| *b > 0),
        secured: decode(r.get("secured")?),
        raw_etag: r.get("raw_etag")?,
        last_modified: r.get("last_modified")?,
        status: Status::parse(&r.get::<_, String>("status")?)
            .unwrap_or(Status::Failed { resumable: false }),
        error: r.get("error")?,
        error_code: r.get("error_code")?,
        final_path: path(r.get("final_path")?),
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        position: r
            .get::<_, Option<i64>>("position")?
            .unwrap_or(r.get::<_, i64>("id")?),
        chosen_name: r.get("chosen_name")?,
        expected_sha256: r.get("expected_sha256")?,
        speed_limit: u64::try_from(r.get::<_, i64>("speed_limit")?).unwrap_or(0),
        start_at: r.get("start_at")?,
        replace_existing: r.get::<_, i64>("replace_existing")? != 0,
        mirrors: r
            .get::<_, String>("mirrors")?
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
        sha256_from: r.get("sha256_from")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp(dir: &Path) -> Checkpoint {
        Checkpoint {
            staging_path: dir.join("f.bin.fuselane"),
            filename: "f.bin".into(),
            total: Some(10_000),
            block_size: 4_096,
            secured: vec![4_096, 1_000, 0],
            raw_etag: Some("\"v1\"".into()),
            last_modified: None,
        }
    }

    #[test]
    fn a_download_can_be_claimed_by_one_owner_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let a = Store::open(&dir.path().join("jobs.db")).unwrap();
        // A second Store on the same file stands in for the other front end.
        let b = Store::open(&dir.path().join("jobs.db")).unwrap();
        let held = a.lock_job(7).unwrap().expect("first claim");
        assert!(b.lock_job(7).unwrap().is_none(), "already claimed");
        assert!(a.lock_job(7).unwrap().is_none(), "even by the same store");
        assert!(b.lock_job(8).unwrap().is_some(), "other downloads are free");
        drop(held);
        assert!(b.lock_job(7).unwrap().is_some(), "free again once released");
    }

    #[test]
    fn a_start_time_is_kept_and_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("jobs.db")).unwrap();
        let a = s.create("https://example.com/a", dir.path()).unwrap();
        assert_eq!(s.get(a).unwrap().start_at, None);
        s.set_start_at(a, Some(1_791_590_400)).unwrap();
        assert_eq!(s.get(a).unwrap().start_at, Some(1_791_590_400));
        // Survives reopening (it is a column, not memory).
        drop(s);
        let s = Store::open(&dir.path().join("jobs.db")).unwrap();
        assert_eq!(s.get(a).unwrap().start_at, Some(1_791_590_400));
        s.set_start_at(a, None).unwrap();
        assert_eq!(s.get(a).unwrap().start_at, None);
        assert!(!s.get(a).unwrap().replace_existing);
        s.set_replace_existing(a, true).unwrap();
        assert!(s.get(a).unwrap().replace_existing);
        assert!(s.get(a).unwrap().mirrors.is_empty());
        let m = vec![
            "https://m1.example/f".to_string(),
            "https://m2.example/f".to_string(),
        ];
        s.set_mirrors(a, &m).unwrap();
        assert_eq!(s.get(a).unwrap().mirrors, m);
        assert!(matches!(
            s.set_start_at(999, None),
            Err(StoreError::NotFound(999))
        ));
    }

    #[test]
    fn a_speed_limit_is_kept_per_download() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("db")).unwrap();
        let a = s.create("https://e.x/a", Path::new("/tmp")).unwrap();
        let b = s.create("https://e.x/b", Path::new("/tmp")).unwrap();
        assert_eq!(s.get(a).unwrap().speed_limit, 0);
        s.set_speed_limit(a, 500_000).unwrap();
        assert_eq!(s.get(a).unwrap().speed_limit, 500_000);
        assert_eq!(s.get(b).unwrap().speed_limit, 0);
        s.set_speed_limit(a, 0).unwrap();
        assert_eq!(s.get(a).unwrap().speed_limit, 0);
        assert!(matches!(
            s.set_speed_limit(999, 1),
            Err(StoreError::NotFound(999))
        ));
    }

    #[test]
    fn settings_round_trip_and_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("db")).unwrap();
        assert_eq!(s.setting("limits").unwrap(), None);
        s.set_setting("limits", "{\"global\":1}").unwrap();
        s.set_setting("limits", "{\"global\":2}").unwrap();
        assert_eq!(
            s.setting("limits").unwrap().as_deref(),
            Some("{\"global\":2}")
        );
    }

    #[test]
    fn ids_are_never_reused_after_removing_the_newest_job() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("db")).unwrap();
        let a = s.create("http://x/a", dir.path()).unwrap();
        let b = s.create("http://x/b", dir.path()).unwrap();
        s.delete(b).unwrap();
        let c = s.create("http://x/c", dir.path()).unwrap();
        assert!(c > b && b > a, "{a} {b} {c}");
        // And across a reopen.
        s.delete(c).unwrap();
        drop(s);
        let s = Store::open(&dir.path().join("db")).unwrap();
        assert!(s.create("http://x/d", dir.path()).unwrap() > c);
    }

    #[test]
    fn a_v1_database_upgrades_and_keeps_its_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("old.db");
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch(MIGRATIONS[0]).unwrap();
            c.pragma_update(None, "user_version", 1).unwrap();
            c.execute(
                "INSERT INTO jobs (url, dir, status, error, created_at, updated_at) VALUES ('http://x/f', '/tmp', 'failed', 'old', 1, 1)",
                [],
            )
            .unwrap();
        }
        let s = Store::open(&p).unwrap();
        let j = &s.list().unwrap()[0];
        assert_eq!(j.id, 1, "ids survive the v3 table rebuild");
        assert_eq!(j.error.as_deref(), Some("old"));
        assert!(s.create("http://x/g", Path::new("/tmp")).unwrap() > j.id);
        assert_eq!(j.error_code, None);
        s.set_error_code(j.id, "fix-link").unwrap();
        assert_eq!(s.get(j.id).unwrap().error_code.as_deref(), Some("fix-link"));
        // Any later transition clears the stale code with the message.
        s.apply(j.id, Event::Resume, None).unwrap();
        assert_eq!(s.get(j.id).unwrap().error_code, None);
    }

    #[test]
    fn only_stopped_jobs_can_change_their_link() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(&dir.path().join("db")).unwrap();
        let id = s.create("http://a/f", dir.path()).unwrap();
        s.set_url(id, "http://b/f").unwrap(); // queued
        assert_eq!(s.get(id).unwrap().url, "http://b/f");
        s.apply(id, Event::Start, None).unwrap();
        assert!(s.set_url(id, "http://c/f").is_err(), "running");
        s.apply(id, Event::Fail { resumable: true }, Some("expired"))
            .unwrap();
        s.set_url(id, "http://c/f").unwrap();
        s.apply(id, Event::Cancel, None).unwrap();
        assert!(s.set_url(id, "http://d/f").is_err(), "cancelled");
        assert!(s.set_url(999, "http://d/f").is_err());
    }

    #[test]
    fn jobs_round_trip_through_checkpoints_and_transitions() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("jobs.db")).unwrap();
        let id = s.create("https://example.com/f.bin", d.path()).unwrap();
        s.save_checkpoint(id, &cp(d.path())).unwrap();
        s.apply(id, Event::Start, None).unwrap();
        s.apply(id, Event::Pause, None).unwrap();
        let j = s.get(id).unwrap();
        assert_eq!(j.status, Status::Paused);
        assert_eq!(j.secured, vec![4_096, 1_000, 0]);
        assert_eq!(j.secured_bytes(), 5_096);
        let r = j.resume().unwrap();
        assert_eq!((r.total, r.block_size), (10_000, 4_096));
        assert!(
            matches!(
                s.apply(id, Event::Complete, None),
                Err(StoreError::Transition(_))
            ),
            "paused jobs can't complete"
        );
        assert_eq!(s.list().unwrap().len(), 1);
        s.delete(id).unwrap();
        assert!(matches!(s.get(id), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn data_survives_reopening_and_migrations_are_idempotent() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("jobs.db");
        let id = {
            let s = Store::open(&p).unwrap();
            s.create("https://x/y", d.path()).unwrap()
        };
        let s = Store::open(&p).unwrap();
        assert_eq!(s.get(id).unwrap().url, "https://x/y");
        assert!(s.recovered_from.is_none());
    }

    #[test]
    fn a_corrupt_database_is_moved_aside_and_reported_not_overwritten() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("jobs.db");
        std::fs::write(
            &p,
            b"this is not a sqlite database, it's garbage from a power cut",
        )
        .unwrap();
        let s = Store::open(&p).unwrap();
        let aside = s.recovered_from.clone().expect("corruption reported");
        assert_eq!(
            std::fs::read(&aside).unwrap(),
            b"this is not a sqlite database, it's garbage from a power cut",
            "original kept"
        );
        assert!(s.list().unwrap().is_empty());
    }

    #[test]
    fn new_jobs_join_the_end_of_the_queue_with_their_choices() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("jobs.db")).unwrap();
        let a = s.create("http://x/a", Path::new("/tmp")).unwrap();
        let b = s
            .create_with(
                "http://x/b",
                Path::new("/tmp"),
                &NewJob {
                    name: Some("mine.iso".into()),
                    sha256: Some("ab".repeat(32)),
                },
            )
            .unwrap();
        let (ja, jb) = (s.get(a).unwrap(), s.get(b).unwrap());
        assert!(ja.position < jb.position);
        assert_eq!(jb.chosen_name.as_deref(), Some("mine.iso"));
        assert_eq!(jb.expected_sha256, Some("ab".repeat(32)));
        assert_eq!(ja.chosen_name, None);
    }

    #[test]
    fn the_queue_can_be_reordered_and_unknown_ids_are_ignored() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("jobs.db")).unwrap();
        let ids: Vec<i64> = (0..4)
            .map(|i| {
                s.create(&format!("http://x/{i}"), Path::new("/tmp"))
                    .unwrap()
            })
            .collect();
        s.reorder(&[ids[2], 999, ids[0]]).unwrap();
        let mut jobs = s.list().unwrap();
        jobs.sort_by_key(|j| j.position);
        let order: Vec<i64> = jobs.iter().map(|j| j.id).collect();
        assert_eq!(order, vec![ids[2], ids[0], ids[1], ids[3]]);
    }

    #[test]
    fn jobs_from_before_queue_order_sort_by_when_they_were_added() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("jobs.db");
        let s = Store::open(&p).unwrap();
        let a = s.create("http://x/a", Path::new("/tmp")).unwrap();
        let b = s.create("http://x/b", Path::new("/tmp")).unwrap();
        // As a v4 database would have them: no position.
        s.lock()
            .execute("UPDATE jobs SET position = NULL", [])
            .unwrap();
        let c = s.create("http://x/c", Path::new("/tmp")).unwrap();
        let pos = |id| s.get(id).unwrap().position;
        assert!(
            pos(a) < pos(b) && pos(b) < pos(c),
            "{} {} {}",
            pos(a),
            pos(b),
            pos(c)
        );
    }

    #[test]
    fn a_busy_database_is_an_error_never_moved_aside() {
        // Another process mid-write (the CLI while the app opens the store) is not
        // corruption: moving the file aside would orphan every job in it.
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("jobs.db");
        Store::open(&p)
            .unwrap()
            .create("http://x/f", Path::new("/tmp"))
            .unwrap();
        let other = Connection::open(&p).unwrap();
        other
            .execute_batch("PRAGMA journal_mode=DELETE; BEGIN EXCLUSIVE;")
            .unwrap();
        let Err(err) = Store::open(&p) else {
            panic!("busy must fail")
        };
        assert!(
            matches!(&err, StoreError::Sql(e) if e.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy)),
            "{err}"
        );
        other.execute_batch("COMMIT").unwrap();
        drop(other);
        let names: Vec<_> = std::fs::read_dir(d.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(!names.iter().any(|n| n.contains("corrupt")), "{names:?}");
        let s = Store::open(&p).unwrap();
        assert!(s.recovered_from.is_none());
        assert_eq!(s.list().unwrap().len(), 1, "the job is still there");
    }

    #[test]
    fn a_database_from_a_newer_version_is_refused_not_downgraded() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("jobs.db");
        {
            let c = Connection::open(&p).unwrap();
            c.pragma_update(None, "user_version", 99).unwrap();
        }
        assert!(matches!(
            Store::open(&p),
            Err(StoreError::TooNew { found: 99, .. })
        ));
        assert!(p.exists(), "never touched");
    }

    #[test]
    fn malformed_rows_degrade_safely() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("jobs.db")).unwrap();
        let id = s.create("https://x/y", d.path()).unwrap();
        s.lock()
            .execute("UPDATE jobs SET secured = X'0102', total = -5, block_size = 0, status = 'nonsense' WHERE id = ?1", params![id])
            .unwrap();
        let j = s.get(id).unwrap();
        assert!(j.secured.is_empty(), "odd-length blob is no progress");
        assert_eq!(j.total, None, "negative size rejected");
        assert_eq!(j.block_size, None, "zero block size rejected");
        assert_eq!(
            j.status,
            Status::Failed { resumable: false },
            "unknown status is never resumed"
        );
        assert!(j.resume().is_none());
    }

    #[test]
    fn interrupted_jobs_come_back_paused() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(&d.path().join("jobs.db")).unwrap();
        let a = s.create("https://x/a", d.path()).unwrap();
        let b = s.create("https://x/b", d.path()).unwrap();
        s.apply(a, Event::Start, None).unwrap();
        s.apply(b, Event::Start, None).unwrap();
        s.apply(b, Event::Complete, None).unwrap();
        assert_eq!(s.recover_interrupted().unwrap(), 1);
        assert_eq!(s.get(a).unwrap().status, Status::Paused);
        assert_eq!(s.get(b).unwrap().status, Status::Completed);
    }

    #[test]
    fn two_handles_can_write_concurrently() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("jobs.db");
        let s1 = Store::open(&p).unwrap();
        let s2 = Store::open(&p).unwrap();
        let handles: Vec<_> = [s1, s2]
            .into_iter()
            .map(|s| {
                std::thread::spawn(move || {
                    (0..50).for_each(|i| {
                        let _ = s
                            .create(&format!("https://x/{i}"), Path::new("/tmp"))
                            .unwrap();
                    })
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(Store::open(&p).unwrap().list().unwrap().len(), 100);
    }
}
