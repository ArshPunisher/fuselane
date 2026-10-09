//! "Unpack it" when a download finishes (B8.7): zip and tar archives (plain or
//! gzip) go into a folder next to the archive, named after it. Archives come
//! from the internet, so every entry is checked: no absolute paths, no `..`, no
//! links or devices, and a cap on the unpacked size against zip bombs. It
//! unpacks into a hidden working folder and only takes the final name once
//! everything is out, so a failure never leaves half a folder behind.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Whether `name` is an archive this can unpack.
pub fn is_archive(name: &str) -> bool {
    kind(name).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Zip,
    Tar,
    TarGz,
}

fn kind(name: &str) -> Option<Kind> {
    let n = name.to_ascii_lowercase();
    if n.ends_with(".zip") {
        Some(Kind::Zip)
    } else if n.ends_with(".tar.gz") || n.ends_with(".tgz") {
        Some(Kind::TarGz)
    } else if n.ends_with(".tar") {
        Some(Kind::Tar)
    } else {
        None
    }
}

/// The archive's name without its archive extension ("photos.tar.gz" → "photos").
fn stem(name: &str) -> &str {
    let lower = name.to_ascii_lowercase();
    for ext in [".tar.gz", ".tgz", ".tar", ".zip"] {
        if lower.ends_with(ext) && name.len() > ext.len() {
            return &name[..name.len() - ext.len()];
        }
    }
    name
}

/// Limits for one archive.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Most bytes it may unpack to.
    pub max_bytes: u64,
    pub max_entries: usize,
}

impl Limits {
    /// Generous for real archives (100 times the archive, at least 4 GiB),
    /// small enough to stop a bomb filling the disk.
    pub fn for_archive(len: u64) -> Limits {
        Limits {
            max_bytes: len.saturating_mul(100).max(4 << 30),
            max_entries: 200_000,
        }
    }
}

/// A path inside an archive, made safe to join under the destination, or None.
fn safe_relative(p: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            // Absolute, a drive prefix, or `..`: refuse the entry.
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Unpacks `archive` next to itself and returns the new folder.
pub fn unpack(archive: &Path, limits: Limits) -> Result<PathBuf, String> {
    let name = archive
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the archive's name can't be read")?;
    let k = kind(name).ok_or("this isn't a zip or tar archive")?;
    let parent = archive.parent().ok_or("the archive has no folder")?;
    let work = parent.join(format!(".{}.unpacking", stem(name)));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir(&work).map_err(|e| format!("couldn't make a folder for it ({e})"))?;
    let result = match k {
        Kind::Zip => unzip(archive, &work, limits),
        Kind::Tar => {
            let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
            untar(f, &work, limits)
        }
        Kind::TarGz => {
            let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
            untar(flate2::read::GzDecoder::new(f), &work, limits)
        }
    };
    if let Err(e) = result {
        let _ = std::fs::remove_dir_all(&work);
        return Err(e);
    }
    let target = free_dir(parent, stem(name));
    std::fs::rename(&work, &target).map_err(|e| {
        let _ = std::fs::remove_dir_all(&work);
        format!("couldn't name the folder ({e})")
    })?;
    Ok(target)
}

/// `dir/name`, or `dir/name (2)`… when taken.
fn free_dir(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if first.symlink_metadata().is_err() {
        return first;
    }
    (2..10_000)
        .map(|n| dir.join(format!("{name} ({n})")))
        .find(|p| p.symlink_metadata().is_err())
        .unwrap_or(first)
}

struct Budget {
    limits: Limits,
    bytes: u64,
    entries: usize,
}

impl Budget {
    fn entry(&mut self) -> Result<(), String> {
        self.entries += 1;
        if self.entries > self.limits.max_entries {
            return Err(format!(
                "it has more than {} files, which looks unsafe",
                self.limits.max_entries
            ));
        }
        Ok(())
    }

    /// Copies `from` into a new file at `to`, counting bytes against the cap.
    fn write(&mut self, mut from: impl Read, to: &Path) -> Result<(), String> {
        if let Some(dir) = to.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        // create_new: two entries with one name can't overwrite each other.
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(to)
            .map_err(|e| format!("couldn't write {} ({e})", to.display()))?;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = from
                .read(&mut buf)
                .map_err(|e| format!("the archive is damaged ({e})"))?;
            if n == 0 {
                return Ok(());
            }
            self.bytes += n as u64;
            if self.bytes > self.limits.max_bytes {
                return Err(
                    "it unpacks to far more than its own size, which looks like a zip bomb".into(),
                );
            }
            std::io::Write::write_all(&mut out, &buf[..n]).map_err(|e| e.to_string())?;
        }
    }
}

fn unzip(archive: &Path, work: &Path, limits: Limits) -> Result<(), String> {
    let f = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("the zip is damaged ({e})"))?;
    let mut budget = Budget {
        limits,
        bytes: 0,
        entries: 0,
    };
    for i in 0..z.len() {
        budget.entry()?;
        let mut entry = z
            .by_index(i)
            .map_err(|e| format!("the zip is damaged ({e})"))?;
        if entry.encrypted() {
            return Err("it is password-protected; unpack it yourself".into());
        }
        let Some(rel) = entry.enclosed_name().as_deref().and_then(safe_relative) else {
            return Err(format!("it holds an unsafe path ({})", entry.name()));
        };
        // Links in zips are stored as files with a link mode; never recreate them.
        if entry.is_symlink() {
            continue;
        }
        let to = work.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
        } else {
            budget.write(&mut entry, &to)?;
        }
    }
    Ok(())
}

fn untar(from: impl Read, work: &Path, limits: Limits) -> Result<(), String> {
    let mut t = tar::Archive::new(from);
    let mut budget = Budget {
        limits,
        bytes: 0,
        entries: 0,
    };
    for entry in t
        .entries()
        .map_err(|e| format!("the archive is damaged ({e})"))?
    {
        budget.entry()?;
        let entry = entry.map_err(|e| format!("the archive is damaged ({e})"))?;
        let path = entry
            .path()
            .map_err(|e| format!("the archive is damaged ({e})"))?;
        let Some(rel) = safe_relative(&path) else {
            return Err(format!("it holds an unsafe path ({})", path.display()));
        };
        let to = work.join(&rel);
        match entry.header().entry_type() {
            tar::EntryType::Directory => {
                std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            }
            tar::EntryType::Regular | tar::EntryType::Continuous => budget.write(entry, &to)?,
            // Links, devices, fifos and metadata entries are skipped, never made.
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip_with(path: &Path, entries: &[(&str, &[u8])]) {
        let f = std::fs::File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            z.start_file(*name, opts).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }

    fn tar_gz_with(path: &Path, entries: &[(&str, &[u8])]) {
        let f = std::fs::File::create(path).unwrap();
        let gz = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
        let mut t = tar::Builder::new(gz);
        for (name, data) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_entry_type(tar::EntryType::Regular);
            // set_path refuses `..`; write the raw name like a hostile archive would.
            let raw = h.as_old_mut();
            raw.name[..name.len()].copy_from_slice(name.as_bytes());
            h.set_cksum();
            t.append(&h, *data).unwrap();
        }
        t.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn a_zip_unpacks_into_a_folder_named_after_it() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("Photos 2026.zip");
        zip_with(&a, &[("a.txt", b"one"), ("sub/b.txt", b"two")]);
        let out = unpack(&a, Limits::for_archive(1)).unwrap();
        assert_eq!(out, d.path().join("Photos 2026"));
        assert_eq!(std::fs::read(out.join("a.txt")).unwrap(), b"one");
        assert_eq!(std::fs::read(out.join("sub/b.txt")).unwrap(), b"two");
        assert!(a.exists(), "the archive stays");
        // Again: a second folder, never mixed into the first.
        let again = unpack(&a, Limits::for_archive(1)).unwrap();
        assert_eq!(again, d.path().join("Photos 2026 (2)"));
    }

    #[test]
    fn a_tar_gz_unpacks_and_its_stem_drops_both_extensions() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("site.tar.gz");
        tar_gz_with(
            &a,
            &[("index.html", b"<h1>hi</h1>"), ("css/s.css", b"body{}")],
        );
        let out = unpack(&a, Limits::for_archive(1)).unwrap();
        assert_eq!(out, d.path().join("site"));
        assert_eq!(std::fs::read(out.join("css/s.css")).unwrap(), b"body{}");
    }

    #[test]
    fn hostile_paths_are_refused_and_leave_nothing_behind() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("evil.tar.gz");
        tar_gz_with(&a, &[("ok.txt", b"fine"), ("../../escaped.txt", b"gotcha")]);
        let err = unpack(&a, Limits::for_archive(1)).unwrap_err();
        assert!(err.contains("unsafe path"), "{err}");
        assert!(!d.path().join("escaped.txt").exists());
        assert!(!d.path().parent().unwrap().join("escaped.txt").exists());
        assert!(!d.path().join("evil").exists(), "no half-unpacked folder");
        assert!(!d.path().join(".evil.unpacking").exists());

        let z = d.path().join("evil.zip");
        zip_with(&z, &[("/etc/passwd-copy", b"x")]);
        assert!(
            unpack(&z, Limits::for_archive(1))
                .unwrap_err()
                .contains("unsafe path")
        );
    }

    #[test]
    fn a_zip_bomb_stops_at_the_cap() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("bomb.zip");
        // 3 MB of zeros compresses to almost nothing.
        zip_with(&a, &[("zeros.bin", &vec![0u8; 3 << 20])]);
        let tight = Limits {
            max_bytes: 1 << 20,
            max_entries: 10,
        };
        assert!(unpack(&a, tight).unwrap_err().contains("zip bomb"));
        assert!(!d.path().join("bomb").exists());
        let many: Vec<(String, &[u8])> = (0..20).map(|i| (format!("f{i}"), &b"x"[..])).collect();
        let refs: Vec<(&str, &[u8])> = many.iter().map(|(n, b)| (n.as_str(), *b)).collect();
        let lots = d.path().join("lots.zip");
        zip_with(&lots, &refs);
        assert!(
            unpack(&lots, tight)
                .unwrap_err()
                .contains("more than 10 files")
        );
    }

    #[test]
    fn only_zip_and_tar_count_as_archives() {
        assert!(is_archive("a.ZIP") && is_archive("b.tar") && is_archive("c.tgz"));
        assert!(is_archive("d.tar.gz"));
        assert!(!is_archive("e.7z") && !is_archive("f.iso") && !is_archive("zip"));
        assert_eq!(stem("x.TAR.GZ"), "x");
        assert_eq!(stem(".zip"), ".zip");
    }
}
