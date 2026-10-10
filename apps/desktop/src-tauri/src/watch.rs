//! Watch folder (B10.9): files put in a chosen folder are added by themselves.
//! `.torrent` files start with all their files, Metalinks add what they list,
//! and `.txt` or `.links` files add every link in them. This is how media
//! tools hand over work ("torrent blackhole"). A handled file is renamed with
//! `.added` (or `.failed`), so it's never taken twice, and nothing is deleted.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};

use serde::{Deserialize, Serialize};

use crate::service::{Service, UiError};

const SETTING: &str = "watch_folder";
/// Largest link list or Metalink read.
const MAX_TEXT: u64 = 4 * 1024 * 1024;
/// Files handled per look, so a folder of thousands doesn't stall the app.
const PER_LOOK: usize = 20;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Starts a .torrent file with all its files; returns its name.
pub type AddTorrent = Arc<
    dyn Fn(PathBuf) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send + Sync,
>;

pub fn real_torrents(tor: Weak<crate::torrents::Torrents>) -> AddTorrent {
    Arc::new(move |path| {
        let tor = tor.clone();
        Box::pin(async move {
            let tor = tor.upgrade().ok_or("Fuselane is closing.")?;
            let listing = tor.inspect_file(&path, None).await.map_err(|e| e.message)?;
            let all = listing.files.iter().map(|f| f.index).collect();
            tor.add(&listing.token, all).await.map_err(|e| e.message)?;
            Ok(listing.name)
        })
    })
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
struct Saved {
    on: bool,
    path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Handled {
    pub name: String,
    pub ok: bool,
    /// What was added, or why not.
    pub note: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WatchView {
    pub on: bool,
    pub path: String,
    /// The folder is gone or can't be read.
    pub problem: Option<String>,
    /// The latest files handled, newest first.
    pub recent: Vec<Handled>,
}

pub struct Watch {
    svc: Weak<Service>,
    torrents: AddTorrent,
    saved: Mutex<Saved>,
    problem: Mutex<Option<String>>,
    recent: Mutex<Vec<Handled>>,
    /// Size last seen per file: a file is taken once it stops growing.
    sizes: Mutex<HashMap<PathBuf, u64>>,
    busy: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for Watch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watch").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Torrent,
    Metalink,
    Links,
}

fn kind_of(path: &Path) -> Option<Kind> {
    let name = path.file_name()?.to_str()?;
    if name.starts_with('.') {
        return None;
    }
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "torrent" => Some(Kind::Torrent),
        "meta4" | "metalink" => Some(Kind::Metalink),
        "txt" | "links" => Some(Kind::Links),
        _ => None,
    }
}

fn mark(path: &Path, suffix: &str) {
    let mut target = path.as_os_str().to_owned();
    target.push(suffix);
    let mut target = PathBuf::from(target);
    let mut n = 2;
    while target.exists() {
        let mut t = path.as_os_str().to_owned();
        t.push(format!(".{n}{suffix}"));
        target = PathBuf::from(t);
        n += 1;
    }
    let _ = std::fs::rename(path, target);
}

fn read_small(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_TEXT {
        return Err("it's over 4 MB".into());
    }
    std::fs::read_to_string(path).map_err(|_| "it isn't text".into())
}

impl Watch {
    pub fn new(svc: &Arc<Service>, torrents: AddTorrent) -> Arc<Watch> {
        let saved = svc
            .store()
            .setting(SETTING)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<Saved>(&s).ok())
            .unwrap_or_default();
        Arc::new(Watch {
            svc: Arc::downgrade(svc),
            torrents,
            saved: Mutex::new(saved),
            problem: Mutex::new(None),
            recent: Mutex::new(Vec::new()),
            sizes: Mutex::new(HashMap::new()),
            busy: tokio::sync::Mutex::new(()),
        })
    }

    pub fn view(&self) -> WatchView {
        let s = lock(&self.saved).clone();
        WatchView {
            on: s.on,
            path: s.path,
            problem: lock(&self.problem).clone(),
            recent: lock(&self.recent).clone(),
        }
    }

    /// Turns watching on or off, for `path`.
    pub fn set(&self, on: bool, path: &str) -> Result<WatchView, UiError> {
        let path = path.trim();
        if on {
            let p = Path::new(path);
            if path.is_empty() || !p.is_absolute() || !p.is_dir() {
                return Err(UiError::new_public(
                    "watch-folder",
                    "Pick a folder to watch.",
                    Some(
                        "Choose an existing folder, such as one your media tool drops files into.",
                    ),
                ));
            }
            let svc = self.svc.upgrade();
            if svc.is_some_and(|s| s.default_dir() == p) {
                return Err(UiError::new_public(
                    "watch-folder",
                    "That's your downloads folder, where finished .torrent and .txt files land.",
                    Some("Pick a separate folder, so downloads aren't taken as new work."),
                ));
            }
        }
        {
            let mut s = lock(&self.saved);
            s.on = on;
            if !path.is_empty() {
                s.path = path.to_string();
            }
        }
        lock(&self.sizes).clear();
        *lock(&self.problem) = None;
        let svc = self
            .svc
            .upgrade()
            .ok_or_else(|| UiError::new_public("closing", "Fuselane is closing.", None))?;
        let json = serde_json::to_string(&*lock(&self.saved)).unwrap_or_default();
        svc.store().set_setting(SETTING, &json).map_err(|e| {
            UiError::new_public(
                "store",
                format!("Fuselane couldn't save the setting: {e}"),
                Some("Check that your disk has free space, then try again."),
            )
        })?;
        Ok(self.view())
    }

    /// Called every few seconds: takes the files that have stopped growing.
    pub async fn tick(&self) {
        let s = lock(&self.saved).clone();
        if !s.on {
            return;
        }
        let Ok(_one) = self.busy.try_lock() else {
            return;
        };
        let dir = PathBuf::from(&s.path);
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                *lock(&self.problem) = Some(format!("The folder can't be read ({e})."));
                return;
            }
        };
        *lock(&self.problem) = None;
        let mut ready = Vec::new();
        {
            let mut sizes = lock(&self.sizes);
            let mut present = Vec::new();
            for e in entries.flatten() {
                let path = e.path();
                let Ok(meta) = e.metadata() else { continue };
                if !meta.is_file() || kind_of(&path).is_none() {
                    continue;
                }
                present.push(path.clone());
                // Taken on the second look at the same size: it's done being written.
                if sizes.insert(path.clone(), meta.len()) == Some(meta.len()) {
                    ready.push(path);
                }
            }
            sizes.retain(|p, _| present.contains(p));
        }
        ready.sort();
        for path in ready.into_iter().take(PER_LOOK) {
            let result = self.take(&path).await;
            lock(&self.sizes).remove(&path);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ok = result.is_ok();
            mark(&path, if ok { ".added" } else { ".failed" });
            let mut recent = lock(&self.recent);
            recent.insert(
                0,
                Handled {
                    name,
                    ok,
                    note: result.unwrap_or_else(|why| format!("Not added: {why}.")),
                },
            );
            recent.truncate(10);
        }
    }

    async fn take(&self, path: &Path) -> Result<String, String> {
        let svc = self.svc.upgrade().ok_or("Fuselane is closing")?;
        match kind_of(path) {
            Some(Kind::Torrent) => {
                let name = (self.torrents)(path.to_path_buf())
                    .await
                    .map_err(|e| e.trim_end_matches('.').to_string())?;
                Ok(format!("Torrent started: {name}."))
            }
            Some(Kind::Metalink) => {
                let text = read_small(path)?;
                let r = svc
                    .add_metalink_text(&text, None, false)
                    .map_err(|e| e.message.trim_end_matches('.').to_string())?;
                if r.added.is_empty() {
                    return Err(r
                        .skipped
                        .first()
                        .map(|s| s.reason.trim_end_matches('.').to_string())
                        .unwrap_or_else(|| "nothing in it could be added".into()));
                }
                Ok(format!("{} added from the Metalink.", files(r.added.len())))
            }
            Some(Kind::Links) => {
                let text = read_small(path)?;
                let r = svc
                    .add_batch(&text, None, false, Some(""))
                    .map_err(|e| e.message.trim_end_matches('.').to_string())?;
                if r.added.is_empty() {
                    return Err(r
                        .skipped
                        .first()
                        .map(|s| s.reason.trim_end_matches('.').to_string())
                        .unwrap_or_else(|| "nothing in it could be added".into()));
                }
                Ok(format!("{} added.", files(r.added.len())))
            }
            None => Err("Fuselane doesn't take this kind of file".into()),
        }
    }
}

fn files(n: usize) -> String {
    if n == 1 {
        "1 download".into()
    } else {
        format!("{n} downloads")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn files_are_taken_once_they_stop_growing_and_marked() {
        let home = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&home.path().join("db")).unwrap();
        std::fs::create_dir(home.path().join("Downloads")).unwrap();
        let svc = Service::new(store, home.path().join("Downloads")).unwrap();
        let started: Arc<Mutex<Vec<PathBuf>>> = Arc::default();
        let torrents: AddTorrent = {
            let started = started.clone();
            Arc::new(move |p| {
                let started = started.clone();
                Box::pin(async move {
                    if std::fs::read(&p).unwrap() == b"broken" {
                        return Err("That isn't a valid .torrent file.".into());
                    }
                    lock(&started).push(p);
                    Ok("Debian 13".into())
                })
            })
        };
        let watch = Watch::new(&svc, torrents);
        let inbox = tempfile::tempdir().unwrap();
        assert_eq!(
            watch.set(true, "relative/dir").unwrap_err().code,
            "watch-folder"
        );
        watch
            .set(true, &inbox.path().display().to_string())
            .unwrap();

        let dir = inbox.path();
        std::fs::write(dir.join("debian.torrent"), b"d8:announce...e").unwrap();
        std::fs::write(dir.join("bad.torrent"), b"broken").unwrap();
        std::fs::write(
            dir.join("list.txt"),
            "https://a.example/one.iso\nhttps://a.example/two.iso\n",
        )
        .unwrap();
        std::fs::write(dir.join("notes.pdf"), b"%PDF").unwrap();
        std::fs::write(dir.join(".hidden.torrent"), b"x").unwrap();

        // First look: sizes noted, nothing taken (they may still be arriving).
        watch.tick().await;
        assert!(lock(&started).is_empty());
        // Second look: taken.
        watch.tick().await;
        assert_eq!(lock(&started).len(), 1);
        assert_eq!(
            svc.jobs().unwrap().len(),
            2,
            "two links from the list: {:?}",
            watch.view()
        );
        assert!(dir.join("debian.torrent.added").exists());
        assert!(dir.join("bad.torrent.failed").exists());
        assert!(dir.join("list.txt.added").exists());
        assert!(dir.join("notes.pdf").exists(), "other files are left alone");
        assert!(dir.join(".hidden.torrent").exists());
        let v = watch.view();
        assert_eq!(v.recent.len(), 3);
        let bad = v.recent.iter().find(|h| h.name == "bad.torrent").unwrap();
        assert!(!bad.ok && bad.note.contains("isn't a valid .torrent"));
        // Nothing is taken twice.
        watch.tick().await;
        watch.tick().await;
        assert_eq!(lock(&started).len(), 1);

        // A list where every link is already there fails, with why.
        std::fs::write(dir.join("again.txt"), "https://a.example/one.iso").unwrap();
        watch.tick().await;
        watch.tick().await;
        assert!(dir.join("again.txt.failed").exists());
        let note = &watch.view().recent[0].note;
        assert!(note.contains("already"), "{note}");

        // Off: nothing more is taken; the setting survives a restart.
        watch.set(false, "").unwrap();
        std::fs::write(dir.join("later.txt"), "https://a.example/three.iso").unwrap();
        watch.tick().await;
        watch.tick().await;
        assert!(dir.join("later.txt").exists());
        let again = Watch::new(&svc, real_torrents(Weak::new()));
        assert_eq!(again.view().path, dir.display().to_string());
        assert!(!again.view().on);
    }
}
