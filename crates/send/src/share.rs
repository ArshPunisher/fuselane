//! Sending and receiving a share through Fuselane's torrent engine, so both sides
//! use every network they have (STEPS 6.4, 6.5).

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use fuselane_engine_torrent::{AddOptions, Source, Storage, Torrent, TorrentEngine, TorrentError};
use librqbit::storage::StorageFactoryExt;

use crate::crypt::{CryptError, HEADER_BLOCK, Header, Keys, check_name};
use crate::link::{Flags, Link};
use crate::storage::ShareStorageFactory;
use crate::torrent;
use crate::view::{Bytes, ReadOnly, View};

/// Why sending or receiving stopped, worded for the person (ERRORS.md).
#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error(transparent)]
    Crypt(#[from] CryptError),
    #[error(
        "Fuselane couldn't read \"{name}\" ({source}). Check the file is still there and you can open it."
    )]
    Read { name: String, source: io::Error },
    #[error("Only single files can be sent for now. Zip the folder and send the zip.")]
    Folder,
    #[error(
        "\"{0}\" changed while Fuselane was preparing it. Wait until it's finished saving, then share it again."
    )]
    Changed(String),
    #[error("This link doesn't lead to a Fuse Send share. Ask the sender for the link again.")]
    NotAShare,
    #[error(
        "Fuselane couldn't reach the sender. Ask them to open Fuselane and keep it open until the file arrives, then try the link again. On different networks, their router may block incoming connections: turning on UPnP there, or using the same Wi-Fi, fixes it."
    )]
    SenderOffline,
    #[error(
        "Fuselane couldn't save into {dir} ({source}). Check the folder exists and you can write to it."
    )]
    Write { dir: String, source: io::Error },
    #[error(
        "The received file didn't match what the sender shared, so it was deleted. Ask the sender for a new link."
    )]
    Mismatch,
    #[error(
        "Not enough free space in {dir}: this file needs {needed} MB and {free} MB is free. Free up space or pick another folder."
    )]
    NoSpace { dir: String, needed: u64, free: u64 },
    #[error(transparent)]
    Torrent(#[from] TorrentError),
}

/// A share ready to seed. Keep `head` and `torrent` (they decide the info-hash),
/// so the same link keeps working after a restart.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub link: Link,
    pub torrent: Vec<u8>,
    pub head: Vec<u8>,
    pub size: u64,
    pub name: String,
}

const CHUNK: usize = 1 << 20;

fn read_err(path: &Path) -> impl FnOnce(io::Error) -> ShareError + '_ {
    move |source| ShareError::Read {
        name: display_name(path),
        source,
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Reads the file twice (its BLAKE3, then the torrent's piece hashes over the
/// encrypted view). `progress(done, total)` covers both passes.
pub fn prepare(path: &Path, mut progress: impl FnMut(u64, u64)) -> Result<Prepared, ShareError> {
    let meta = std::fs::metadata(path).map_err(read_err(path))?;
    if meta.is_dir() {
        return Err(ShareError::Folder);
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| CryptError::BadName(display_name(path)))?
        .to_string();
    check_name(&name)?;
    let size = meta.len();
    let total = 2 * size;

    let mut file = File::open(path).map_err(read_err(path))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(read_err(path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        done += n as u64;
        progress(done.min(size), total);
    }
    if done != size {
        return Err(ShareError::Changed(name));
    }

    let mut key = [0u8; 32];
    getrandom::fill(&mut key).map_err(|e| read_err(path)(io::Error::other(e)))?;
    let keys = Keys::derive(&key);
    let header = Header {
        name: name.clone(),
        size,
        hash: *hasher.finalize().as_bytes(),
    };
    let head = keys.seal(&header)?;
    let view = View::new(
        keys,
        Box::new(Bytes(head.clone())),
        Box::new(ReadOnly(File::open(path).map_err(read_err(path))?)),
        size,
    );
    let built = torrent::build(&view, |at| {
        progress(
            size + at.saturating_sub(HEADER_BLOCK as u64).min(size),
            total,
        )
    })
    .map_err(read_err(path))?;
    if std::fs::metadata(path).map_err(read_err(path))?.len() != size {
        return Err(ShareError::Changed(name));
    }
    Ok(Prepared {
        link: Link {
            info_hash: built.info_hash,
            key,
            flags: Flags::default(),
        },
        torrent: built.torrent(),
        head,
        size,
        name,
    })
}

fn storage(view: View) -> Storage {
    Storage(ShareStorageFactory(Arc::new(view)).boxed())
}

/// Starts seeding a prepared share. The file is opened read-only.
pub async fn seed(
    engine: &TorrentEngine,
    share: &Prepared,
    path: &Path,
) -> Result<Torrent, ShareError> {
    let view = View::new(
        Keys::derive(&share.link.key),
        Box::new(Bytes(share.head.clone())),
        Box::new(ReadOnly(File::open(path).map_err(read_err(path))?)),
        share.size,
    );
    let folder = path.parent().map(Path::to_path_buf);
    Ok(engine
        .add(
            Source::File(share.torrent.clone()),
            folder,
            vec![],
            AddOptions {
                // The data is already there: librqbit checks it through the view.
                resume: true,
                storage: Some(storage(view)),
                ..Default::default()
            },
        )
        .await?)
}

/// A share being received into `dir`, in two hidden files until it checks out.
#[derive(Debug)]
pub struct Receiving {
    pub torrent: Torrent,
    pub size: u64,
    link: Link,
    dir: PathBuf,
    head: PathBuf,
    part: PathBuf,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// How long `receive` looks for the sender before saying they seem offline.
pub const LOOKUP: Duration = Duration::from_secs(60);

/// Finds the share by its info-hash, checks it has a share's shape, and starts it.
/// `peers`: addresses to try first (tests, or the same network). `lookup`: how long
/// to look for the sender (normally `LOOKUP`).
pub async fn receive(
    engine: &TorrentEngine,
    link: &Link,
    dir: &Path,
    peers: Vec<SocketAddr>,
    lookup: Duration,
) -> Result<Receiving, ShareError> {
    let id = hex(&link.info_hash);
    // While looking, ask the local networks every few seconds: a sender in the same
    // building answers at once (DHT finds senders elsewhere).
    let asking = engine.local_search(&id).map(|search| {
        tokio::spawn(async move {
            loop {
                search.send();
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        })
    });
    let listing = tokio::time::timeout(
        lookup,
        engine.inspect(
            Source::Magnet(format!("magnet:?xt=urn:btih:{id}")),
            Some(dir.to_path_buf()),
            peers,
        ),
    )
    .await;
    if let Some(a) = asking {
        a.abort();
    }
    let listing = listing
        .map_err(|_| ShareError::SenderOffline)?
        // The magnet is ours, so an engine failure here means nobody had the share:
        // every address tried, none answered.
        .map_err(|e| match e {
            TorrentError::Engine(_) | TorrentError::Invalid(_) => ShareError::SenderOffline,
            e => e.into(),
        })?;
    // Anything else under this info-hash is not ours to open.
    let size = match listing.files.as_slice() {
        [f] if !f.padding && f.parts == [torrent::NAME] && f.len >= HEADER_BLOCK as u64 => {
            f.len - HEADER_BLOCK as u64
        }
        _ => return Err(ShareError::NotAShare),
    };
    let head = dir.join(format!(".fuselane-{id}.head"));
    let part = dir.join(format!(".fuselane-{id}.part"));
    // What already arrived (a restart) needn't fit again.
    let have = std::fs::metadata(&part).map_or(0, |m| m.len());
    if let Ok(free) = fuselane_storage::free::free_space(dir)
        && free < size.saturating_sub(have)
    {
        let mb = |b: u64| b.div_ceil(1024 * 1024);
        return Err(ShareError::NoSpace {
            dir: dir.display().to_string(),
            needed: mb(size.saturating_sub(have)),
            free: mb(free),
        });
    }
    let open = |p: &Path| {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false) // keep what arrived before a restart
            .open(p)
            .map_err(|source| ShareError::Write {
                dir: dir.display().to_string(),
                source,
            })
    };
    let view = View::new(
        Keys::derive(&link.key),
        Box::new(open(&head)?),
        Box::new(open(&part)?),
        size,
    );
    let torrent = engine
        .add_listed(
            listing,
            AddOptions {
                resume: true,
                storage: Some(storage(view)),
                ..Default::default()
            },
        )
        .await?;
    Ok(Receiving {
        torrent,
        size,
        link: link.clone(),
        dir: dir.to_path_buf(),
        head,
        part,
    })
}

impl Receiving {
    /// After every piece has arrived: decrypts the header, checks the whole file
    /// against the sender's BLAKE3, and gives it its real name (never replacing a
    /// file that is already there). A mismatch deletes what arrived.
    pub fn finish(self) -> Result<PathBuf, ShareError> {
        let fail = |e: ShareError| {
            let _ = std::fs::remove_file(&self.part);
            let _ = std::fs::remove_file(&self.head);
            e
        };
        let head = std::fs::read(&self.head).map_err(read_err(&self.head))?;
        let header = Keys::derive(&self.link.key)
            .open(&head)
            .map_err(|e| fail(e.into()))?;
        if header.size != self.size {
            return Err(fail(ShareError::Mismatch));
        }
        let mut file = File::open(&self.part).map_err(read_err(&self.part))?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = file.read(&mut buf).map_err(read_err(&self.part))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        drop(file);
        if hasher.finalize().as_bytes() != &header.hash {
            return Err(fail(ShareError::Mismatch));
        }
        let target = free_name(&self.dir, &header.name);
        std::fs::rename(&self.part, &target).map_err(|source| ShareError::Write {
            dir: self.dir.display().to_string(),
            source,
        })?;
        let _ = std::fs::remove_file(&self.head);
        Ok(target)
    }
}

/// `name`, or `name (2)`, `name (3)`… whichever is free in `dir`.
fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(fuselane_storage::names::numbered(name, n)))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preparing_reports_progress_and_makes_a_stable_link() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("notes.txt");
        std::fs::write(&f, vec![3u8; 300_000]).unwrap();
        let mut last = (0, 0);
        let p = prepare(&f, |done, total| last = (done, total)).unwrap();
        assert_eq!(last, (600_000, 600_000), "both passes reach the end");
        assert_eq!(p.size, 300_000);
        assert_eq!(p.name, "notes.txt");
        assert_eq!(Link::parse(&p.link.url()).unwrap(), p.link);
        let t = librqbit::torrent_from_bytes(&p.torrent).unwrap();
        assert_eq!(t.info_hash.0, p.link.info_hash);
        assert_eq!(
            Keys::derive(&p.link.key).open(&p.head).unwrap().size,
            300_000
        );
    }

    #[test]
    fn folders_missing_files_and_unsafe_names_are_refused_clearly() {
        let d = tempfile::tempdir().unwrap();
        assert!(matches!(
            prepare(d.path(), |_, _| {}),
            Err(ShareError::Folder)
        ));
        let gone = d.path().join("gone.bin");
        let e = prepare(&gone, |_, _| {}).unwrap_err();
        assert!(matches!(e, ShareError::Read { .. }));
        assert!(e.to_string().contains("gone.bin"), "{e}");
        let bad = d.path().join("CON.txt");
        std::fs::write(&bad, b"x").unwrap();
        assert!(matches!(
            prepare(&bad, |_, _| {}),
            Err(ShareError::Crypt(CryptError::BadName(_)))
        ));
    }

    #[test]
    fn an_empty_file_can_be_shared() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("empty.txt");
        std::fs::write(&f, b"").unwrap();
        let p = prepare(&f, |_, _| {}).unwrap();
        assert_eq!(p.size, 0);
    }

    #[test]
    fn a_taken_name_gets_a_number_never_overwritten() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(free_name(d.path(), "a.pdf"), d.path().join("a.pdf"));
        std::fs::write(d.path().join("a.pdf"), b"mine").unwrap();
        std::fs::write(d.path().join("a (2).pdf"), b"mine too").unwrap();
        assert_eq!(free_name(d.path(), "a.pdf"), d.path().join("a (3).pdf"));
    }
}
