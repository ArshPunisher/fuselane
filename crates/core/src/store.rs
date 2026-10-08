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
];

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

/// The job database. Safe to share across threads.
#[derive(Debug)]
pub struct Store {
    conn: Mutex<Connection>,
    /// Set when the previous database couldn't be read and was moved here.
    pub recovered_from: Option<PathBuf>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
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
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        match Self::open_inner(path) {
            Ok(conn) => Ok(Store {
                conn: Mutex::new(conn),
                recovered_from: None,
            }),
            Err(StoreError::TooNew { found, known }) => Err(StoreError::TooNew { found, known }),
            Err(_) if path.exists() => {
                // Unreadable: move it aside, never delete or overwrite it (L-50).
                let aside = path.with_extension(format!("corrupt-{}", now()));
                std::fs::rename(path, &aside)?;
                for ext in ["db-wal", "db-shm"] {
                    let _ = std::fs::remove_file(path.with_extension(ext));
                }
                let conn = Self::open_inner(path)?;
                Ok(Store {
                    conn: Mutex::new(conn),
                    recovered_from: Some(aside),
                })
            }
            Err(e) => Err(e),
        }
    }

    fn open_inner(path: &Path) -> Result<Connection, StoreError> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
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
        let c = self.lock();
        let t = now();
        c.execute(
            "INSERT INTO jobs (url, dir, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![url, dir.to_string_lossy(), Status::Queued.as_str(), t],
        )?;
        Ok(c.last_insert_rowid())
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
