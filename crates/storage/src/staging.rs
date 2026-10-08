//! The staging file: one file at fixed offsets beside the destination, published
//! by rename (ENGINE-DOWNLOAD.md §8, L-36–L-45).

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use crate::names::{numbered, sanitize};

/// Suffix of a file being downloaded.
pub const STAGING_SUFFIX: &str = ".fuselane";
/// Collision attempts before giving up (`name (1)` … `name (9999)`).
pub const MAX_COLLISIONS: u32 = 9_999;

#[derive(Debug, thiserror::Error)]
pub enum StagingError {
    #[error("the folder {0} doesn't exist")]
    MissingFolder(PathBuf),
    #[error("every name from {0} to {0} (9999) is taken")]
    NamesExhausted(String),
    #[error("refusing to publish: {written} of {expected} bytes are on disk")]
    Incomplete { written: u64, expected: u64 },
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// A claimed staging file. Dropping it without publishing leaves the file for resume.
#[derive(Debug)]
pub struct Staging {
    file: File,
    dir: PathBuf,
    name: String,
    staging_path: PathBuf,
    size: Option<u64>,
}

impl Staging {
    /// Claims `<name>.fuselane` in `dir` with an exclusive create (L-37). The name is
    /// sanitized first, and skipped while either it or its staging file is taken.
    pub fn create(dir: &Path, suggested: &str, size: Option<u64>) -> Result<Staging, StagingError> {
        if !dir.is_dir() {
            return Err(StagingError::MissingFolder(dir.to_path_buf()));
        }
        let base = sanitize(suggested);
        for n in 0..=MAX_COLLISIONS {
            let name = if n == 0 {
                base.clone()
            } else {
                numbered(&base, n)
            };
            if dir.join(&name).symlink_metadata().is_ok() {
                continue;
            }
            let staging_path = dir.join(format!("{name}{STAGING_SUFFIX}"));
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&staging_path)
            {
                Ok(file) => {
                    if let Some(len) = size {
                        file.set_len(len)?; // sparse on APFS/ext4; NTFS sparse is set by the Windows path
                    }
                    return Ok(Staging {
                        file,
                        dir: dir.to_path_buf(),
                        name,
                        staging_path,
                        size,
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(StagingError::NamesExhausted(base))
    }

    /// Reopens an existing staging file for resume.
    pub fn reopen(staging_path: &Path, size: Option<u64>) -> Result<Staging, StagingError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(staging_path)?;
        let file_name = staging_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let name = file_name
            .strip_suffix(STAGING_SUFFIX)
            .unwrap_or(file_name)
            .to_string();
        let dir = staging_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        Ok(Staging {
            file,
            dir,
            name,
            staging_path: staging_path.to_path_buf(),
            size,
        })
    }

    pub fn path(&self) -> &Path {
        &self.staging_path
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// A second handle for a writer thread.
    pub fn handle(&self) -> io::Result<File> {
        self.file.try_clone()
    }

    /// Writes `buf` at `offset` (positional; never appends).
    pub fn write_at(file: &File, offset: u64, buf: &[u8]) -> io::Result<()> {
        write_all_at(file, offset, buf)
    }

    /// Makes written data durable (call before recording progress as secured, L-55).
    pub fn sync(&self) -> io::Result<()> {
        self.file.sync_data()
    }

    /// Checks completeness, fsyncs, and renames to the final name without replacing
    /// anything; returns the published path (L-43, L-45).
    pub fn publish(self, written: u64) -> Result<PathBuf, StagingError> {
        let expected = self.size.unwrap_or(written);
        let on_disk = self.file.metadata()?.len();
        if written != expected || (self.size.is_some() && on_disk != expected) {
            return Err(StagingError::Incomplete { written, expected });
        }
        self.file.sync_all()?;
        drop(self.file); // Windows can't rename a file with an open handle
        for n in 0..=MAX_COLLISIONS {
            let name = if n == 0 {
                self.name.clone()
            } else {
                numbered(&self.name, n)
            };
            let target = self.dir.join(&name);
            if target.symlink_metadata().is_ok() {
                continue; // something appeared under our name meanwhile: never overwrite
            }
            rename_with_retry(&self.staging_path, &target)?;
            sync_dir(&self.dir);
            return Ok(target);
        }
        Err(StagingError::NamesExhausted(self.name))
    }

    /// Deletes the staging file (cancel).
    pub fn discard(self) -> io::Result<()> {
        drop(self.file);
        std::fs::remove_file(&self.staging_path)
    }
}

#[cfg(unix)]
fn write_all_at(file: &File, offset: u64, buf: &[u8]) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.write_all_at(buf, offset)
}

#[cfg(windows)]
fn write_all_at(file: &File, mut offset: u64, mut buf: &[u8]) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    while !buf.is_empty() {
        let n = file.seek_write(buf, offset)?;
        if n == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        buf = &buf[n..];
        offset += n as u64;
    }
    Ok(())
}

/// Antivirus, indexers and sync clients briefly hold files on Windows (L-45).
fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    let mut last = None;
    for attempt in 0..5u64 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::PermissionDenied | io::ErrorKind::ResourceBusy
                ) =>
            {
                last = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("rename failed")))
}

fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn writes_at_offsets_in_any_order_and_publishes() {
        let d = tmp();
        let s = Staging::create(d.path(), "data.bin", Some(10)).unwrap();
        let f = s.handle().unwrap();
        Staging::write_at(&f, 5, b"WORLD").unwrap();
        Staging::write_at(&f, 0, b"HELLO").unwrap();
        let out = s.publish(10).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"HELLOWORLD");
        assert_eq!(out.file_name().unwrap(), "data.bin");
        assert!(!d.path().join("data.bin.fuselane").exists());
    }

    #[test]
    fn refuses_to_publish_an_incomplete_file() {
        let d = tmp();
        let s = Staging::create(d.path(), "x.iso", Some(100)).unwrap();
        let err = s.publish(60).unwrap_err();
        assert!(matches!(
            err,
            StagingError::Incomplete {
                written: 60,
                expected: 100
            }
        ));
        assert!(
            d.path().join("x.iso.fuselane").exists(),
            "partial data is kept for resume"
        );
        assert!(!d.path().join("x.iso").exists());
    }

    #[test]
    fn claims_free_names_and_never_overwrites() {
        let d = tmp();
        std::fs::write(d.path().join("report.pdf"), b"user's file").unwrap();
        let a = Staging::create(d.path(), "report.pdf", Some(1)).unwrap();
        let b = Staging::create(d.path(), "report.pdf", Some(1)).unwrap();
        assert_eq!(a.name(), "report (1).pdf");
        assert_eq!(
            b.name(),
            "report (2).pdf",
            "two downloads with one name get different files"
        );
        // Someone creates our final name while we download: publish picks the next one.
        std::fs::write(d.path().join("report (1).pdf"), b"appeared meanwhile").unwrap();
        Staging::write_at(&a.handle().unwrap(), 0, b"A").unwrap();
        let out = a.publish(1).unwrap();
        assert_ne!(out.file_name().unwrap(), "report (1).pdf");
        assert_eq!(
            std::fs::read(d.path().join("report.pdf")).unwrap(),
            b"user's file"
        );
        assert_eq!(
            std::fs::read(d.path().join("report (1).pdf")).unwrap(),
            b"appeared meanwhile"
        );
    }

    #[test]
    fn hostile_names_stay_inside_the_folder() {
        let d = tmp();
        for evil in [
            "../../escape.txt",
            "/etc/passwd",
            "..\\..\\win.ini",
            "CON",
            "",
        ] {
            let s = Staging::create(d.path(), evil, Some(0)).unwrap();
            assert_eq!(s.path().parent().unwrap(), d.path(), "{evil:?} escaped");
            let out = s.publish(0).unwrap();
            assert_eq!(out.parent().unwrap(), d.path());
        }
    }

    #[test]
    fn missing_folder_is_a_clear_error() {
        let d = tmp();
        let err = Staging::create(&d.path().join("nope"), "a", None).unwrap_err();
        assert!(matches!(err, StagingError::MissingFolder(_)));
    }

    #[test]
    fn unknown_size_publishes_what_was_written() {
        let d = tmp();
        let s = Staging::create(d.path(), "stream.log", None).unwrap();
        Staging::write_at(&s.handle().unwrap(), 0, b"abc").unwrap();
        let out = s.publish(3).unwrap();
        assert_eq!(std::fs::read(out).unwrap(), b"abc");
    }

    #[test]
    fn reopen_resumes_and_discard_deletes() {
        let d = tmp();
        let s = Staging::create(d.path(), "big.bin", Some(4)).unwrap();
        Staging::write_at(&s.handle().unwrap(), 0, b"ab").unwrap();
        let p = s.path().to_path_buf();
        drop(s);
        let r = Staging::reopen(&p, Some(4)).unwrap();
        assert_eq!(r.name(), "big.bin");
        Staging::write_at(&r.handle().unwrap(), 2, b"cd").unwrap();
        assert_eq!(std::fs::read(r.publish(4).unwrap()).unwrap(), b"abcd");
        let s2 = Staging::create(d.path(), "gone.bin", Some(4)).unwrap();
        let p2 = s2.path().to_path_buf();
        s2.discard().unwrap();
        assert!(!p2.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_under_our_name_is_never_followed_or_replaced() {
        let d = tmp();
        let outside = tmp();
        std::os::unix::fs::symlink(outside.path().join("victim"), d.path().join("link.txt"))
            .unwrap();
        let s = Staging::create(d.path(), "link.txt", Some(1)).unwrap();
        assert_eq!(s.name(), "link (1).txt");
        Staging::write_at(&s.handle().unwrap(), 0, b"x").unwrap();
        s.publish(1).unwrap();
        assert!(!outside.path().join("victim").exists());
    }
}
