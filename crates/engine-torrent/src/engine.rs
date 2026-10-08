//! A librqbit session wired for Fuselane (ADR 0006): every outgoing peer and HTTP
//! tracker connection goes through our SOCKS5 proxy, so the balancer decides which
//! network carries it. uTP, UPnP and local peer discovery are off; DHT and
//! trackers are the caller's choice.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ConnectionOptions, ListenerMode,
    ListenerOptions, ManagedTorrent, Session, SessionOptions, TorrentStatsState,
};

use crate::balancer::{Balancer, NetShare, NetStat};
use crate::paths::{self, Cleanup, Planned, Rules};
use crate::socks::{self, SocksServer};
use fuselane_netif::Interface;

/// Torrent files bigger than this are refused before parsing.
pub const MAX_TORRENT_FILE: usize = 8 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {
    #[error("That isn't a magnet link. It should start with \"magnet:?xt=urn:btih:\".")]
    NotAMagnet,
    #[error(
        "That torrent file is {0} bytes; the limit is 8 MiB, so it is probably not a torrent file."
    )]
    FileTooBig(usize),
    #[error("No network is available for torrents. Turn on at least one network.")]
    NoNetworks,
    #[error("{0} already exists in the download folder. Move it or pick another folder.")]
    FileExists(String),
    #[error("This torrent isn't safe to save: {0}")]
    UnsafePath(String),
    #[error("Pick at least one file to download.")]
    NothingSelected,
    #[error("This torrent has no file number {0}.")]
    NoSuchFile(usize),
    #[error("This torrent is already in your list.")]
    AlreadyAdded,
    #[error("Couldn't read the torrent: {0}")]
    Invalid(String),
    #[error("The torrent engine failed: {0}")]
    Engine(String),
}

fn engine(e: impl std::fmt::Display) -> TorrentError {
    TorrentError::Engine(e.to_string())
}

fn add_error(e: anyhow::Error) -> TorrentError {
    let full = format!("{e:#}");
    // librqbit 9 reports this as text only: `... (because allow_overwrite = false) "<path>": File exists`.
    if full.contains("allow_overwrite = false") {
        let path = full.split('"').nth(1).unwrap_or_default();
        let name = std::path::Path::new(path)
            .file_name()
            .map_or("A file".into(), |n| n.to_string_lossy().into_owned());
        return TorrentError::FileExists(name);
    }
    TorrentError::Invalid(full)
}

/// What to add: a magnet link or the bytes of a .torrent file.
#[derive(Debug, Clone)]
pub enum Source {
    Magnet(String),
    File(Vec<u8>),
}

impl Source {
    fn checked(self) -> Result<AddTorrent<'static>, TorrentError> {
        match self {
            Self::Magnet(m) => {
                let m = m.trim().to_owned();
                let lower = m.to_ascii_lowercase();
                if !lower.starts_with("magnet:?") || !lower.contains("xt=urn:btih:") {
                    return Err(TorrentError::NotAMagnet);
                }
                Ok(AddTorrent::Url(m.into()))
            }
            Self::File(b) if b.len() > MAX_TORRENT_FILE => Err(TorrentError::FileTooBig(b.len())),
            Self::File(b) => Ok(AddTorrent::TorrentFileBytes(b.into())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub download_dir: PathBuf,
    pub networks: Vec<Interface>,
    /// Find peers through the DHT (UDP; not bonded, see ADR 0006).
    pub dht: bool,
    /// Accept incoming peers on this address (None: outgoing only).
    pub listen: Option<SocketAddr>,
}

pub struct TorrentEngine {
    session: Arc<Session>,
    download_dir: PathBuf,
    balancer: Arc<Balancer>,
    socks: SocksServer,
}

impl std::fmt::Debug for TorrentEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TorrentEngine")
            .field("proxy", &self.socks.addr)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Checking,
    Downloading,
    Seeding,
    Paused,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Progress {
    pub phase: Phase,
    pub done: u64,
    pub total: u64,
    pub uploaded: u64,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Torrent {
    handle: Arc<ManagedTorrent>,
    layout: Arc<Layout>,
    balancer: Arc<Balancer>,
}

impl std::fmt::Debug for Torrent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Torrent")
            .field("id", &self.id())
            .field("name", &self.name())
            .finish()
    }
}

impl Torrent {
    pub fn id(&self) -> usize {
        self.handle.id()
    }
    pub fn name(&self) -> Option<String> {
        self.handle.name()
    }
    pub fn info_hash(&self) -> String {
        self.handle.info_hash().as_string()
    }
    pub fn progress(&self) -> Progress {
        let s = self.handle.stats();
        let phase = match s.state {
            TorrentStatsState::Initializing { .. } => Phase::Checking,
            TorrentStatsState::Paused => Phase::Paused,
            TorrentStatsState::Error => Phase::Failed,
            TorrentStatsState::Live if s.finished => Phase::Seeding,
            TorrentStatsState::Live => Phase::Downloading,
        };
        Progress {
            phase,
            done: s.progress_bytes,
            total: s.total_bytes,
            uploaded: s.uploaded_bytes,
            error: s.error,
        }
    }
    /// Each network's part in this torrent, credited with verified bytes only.
    pub fn networks(&self) -> Vec<NetShare> {
        self.balancer
            .torrent_shares(&self.info_hash(), self.progress().done)
    }
    pub fn listing(&self) -> &Listing {
        &self.layout.listing
    }
    /// Indices of the files being downloaded.
    pub fn selected(&self) -> HashSet<usize> {
        self.layout
            .selected
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    /// Resolves when every selected file is complete and verified.
    pub async fn finished(&self) -> Result<(), TorrentError> {
        self.handle.wait_until_completed().await.map_err(engine)
    }
}

impl TorrentEngine {
    pub async fn start(opts: EngineOptions) -> Result<Self, TorrentError> {
        if opts.networks.is_empty() {
            return Err(TorrentError::NoNetworks);
        }
        let balancer = Arc::new(Balancer::new(opts.networks));
        let socks = socks::start(balancer.clone()).await.map_err(engine)?;
        let download_dir = opts.download_dir;
        let session = Session::new_with_opts(
            download_dir.clone(),
            SessionOptions {
                dht: if opts.dht {
                    Some(Default::default())
                } else {
                    None
                },
                fastresume: false,
                persistence: None,
                listen: opts.listen.map(|listen_addr| ListenerOptions {
                    mode: ListenerMode::TcpOnly,
                    listen_addr,
                    enable_upnp_port_forwarding: false,
                    ..Default::default()
                }),
                connect: Some(ConnectionOptions {
                    proxy_url: Some(socks.url()),
                    ..Default::default()
                }),
                disable_local_service_discovery: true,
                ..Default::default()
            },
        )
        .await
        .map_err(engine)?;
        Ok(Self {
            session,
            download_dir,
            balancer,
            socks,
        })
    }

    /// Where the proxy listens (for diagnostics).
    pub fn proxy_addr(&self) -> SocketAddr {
        self.socks.addr
    }

    pub fn listen_addr(&self) -> Option<SocketAddr> {
        self.session.listen_addr()
    }

    pub fn networks(&self) -> Vec<NetStat> {
        self.balancer.snapshot()
    }

    /// Reads a torrent's file list without downloading (a magnet's list comes from
    /// peers) and checks every path (L-68). Nothing is written to disk.
    pub async fn inspect(
        &self,
        source: Source,
        output_folder: Option<PathBuf>,
        initial_peers: Vec<SocketAddr>,
    ) -> Result<Listing, TorrentError> {
        let add = source.checked()?;
        let base = output_folder.unwrap_or_else(|| self.download_dir.clone());
        let listed = self
            .session
            .add_torrent(
                add,
                Some(AddTorrentOptions {
                    list_only: true,
                    output_folder: Some(base.to_string_lossy().into_owned()),
                    initial_peers: (!initial_peers.is_empty()).then(|| initial_peers.clone()),
                    ..Default::default()
                }),
            )
            .await
            .map_err(add_error)?;
        let listed = match listed {
            AddTorrentResponse::ListOnly(l) => l,
            AddTorrentResponse::AlreadyManaged(..) => return Err(TorrentError::AlreadyAdded),
            AddTorrentResponse::Added(..) => {
                return Err(engine("list-only add started a download"));
            }
        };
        let name = listed
            .info
            .name()
            .map_or_else(|| listed.info_hash.as_string(), |n| n.into_owned());
        // librqbit writes straight into `output_folder`, so a multi-file torrent gets
        // its own folder, named after the torrent (checked like any other name, L-121).
        let folder_parts = if listed.info.info().files.is_some() {
            vec![name.clone()]
        } else {
            Vec::new()
        };
        let files: Vec<Planned> = listed
            .info
            .iter_file_details()
            .map(|fd| Planned {
                parts: fd
                    .filename
                    .iter_components()
                    .map(|c| c.into_owned())
                    .collect(),
                len: fd.len,
                padding: fd.attrs().padding,
            })
            .collect();
        paths::check(&base, &folder_parts, &files, Rules::native())
            .map_err(TorrentError::UnsafePath)?;
        let mut peers = initial_peers;
        for p in listed.seen_peers.iter().copied() {
            if !peers.contains(&p) {
                peers.push(p);
            }
        }
        Ok(Listing {
            name,
            info_hash: listed.info_hash.as_string(),
            folder: folder_parts.iter().fold(base, |p, c| p.join(c)),
            own_folder: !folder_parts.is_empty(),
            files,
            torrent: listed.torrent_bytes.to_vec(),
            peers,
        })
    }

    /// Adds a torrent: inspects it first, so every path is checked before librqbit
    /// may create anything. `only` picks files by index (None: all of them).
    pub async fn add(
        &self,
        source: Source,
        output_folder: Option<PathBuf>,
        initial_peers: Vec<SocketAddr>,
        only: Option<HashSet<usize>>,
    ) -> Result<Torrent, TorrentError> {
        let listing = self.inspect(source, output_folder, initial_peers).await?;
        self.add_listed(listing, only).await
    }

    /// Adds a torrent already inspected (for example after the user picked files).
    pub async fn add_listed(
        &self,
        listing: Listing,
        only: Option<HashSet<usize>>,
    ) -> Result<Torrent, TorrentError> {
        let selected = match only {
            Some(set) => check_selection(&listing.files, set)?,
            None => listing.wanted_indices(),
        };
        let opts = AddTorrentOptions {
            output_folder: Some(listing.folder.to_string_lossy().into_owned()),
            initial_peers: (!listing.peers.is_empty()).then(|| listing.peers.clone()),
            only_files: Some(selected.iter().copied().collect()),
            // Never write into a file the user already has (librqbit would reuse it).
            overwrite: false,
            ..Default::default()
        };
        let resp = self
            .session
            .add_torrent(
                AddTorrent::TorrentFileBytes(listing.torrent.clone().into()),
                Some(opts),
            )
            .await
            .map_err(add_error)?;
        match resp {
            AddTorrentResponse::Added(_, handle) => Ok(Torrent {
                handle,
                layout: Arc::new(Layout {
                    listing,
                    selected: Mutex::new(selected),
                }),
                balancer: self.balancer.clone(),
            }),
            AddTorrentResponse::AlreadyManaged(..) => Err(TorrentError::AlreadyAdded),
            AddTorrentResponse::ListOnly(_) => Err(engine("download did not start")),
        }
    }

    /// Changes which files to download while the torrent runs.
    pub async fn select(&self, t: &Torrent, files: HashSet<usize>) -> Result<(), TorrentError> {
        let set = check_selection(&t.layout.listing.files, files)?;
        self.session
            .update_only_files(&t.handle, &set)
            .await
            .map_err(engine)?;
        *t.layout.selected.lock().unwrap_or_else(|p| p.into_inner()) = set;
        Ok(())
    }

    pub async fn pause(&self, t: &Torrent) -> Result<(), TorrentError> {
        self.session.pause(&t.handle).await.map_err(engine)
    }

    pub async fn resume(&self, t: &Torrent) -> Result<(), TorrentError> {
        self.session.unpause(&t.handle).await.map_err(engine)
    }

    /// Stops the torrent and keeps the chosen files. Unchosen files that librqbit
    /// created (empty, or holding edge-piece bytes) are deleted (L-69).
    pub async fn release(&self, t: Torrent) -> Result<Cleanup, TorrentError> {
        let keep = t.selected();
        self.session
            .delete(t.handle.id().into(), false)
            .await
            .map_err(engine)?;
        let drop: Vec<&Planned> = t
            .layout
            .listing
            .files
            .iter()
            .enumerate()
            .filter(|(i, _)| !keep.contains(i))
            .map(|(_, f)| f)
            .collect();
        Ok(paths::remove(
            &t.layout.listing.folder,
            t.layout.listing.own_folder,
            &drop,
        ))
    }

    /// Removes the torrent; with `delete_files`, also every file it wrote. Deletion
    /// is Fuselane's own: it refuses symlinks and swapped folders (L-69).
    pub async fn remove(&self, t: Torrent, delete_files: bool) -> Result<Cleanup, TorrentError> {
        self.session
            .delete(t.handle.id().into(), false)
            .await
            .map_err(engine)?;
        if !delete_files {
            return Ok(Cleanup::default());
        }
        let all: Vec<&Planned> = t.layout.listing.files.iter().collect();
        Ok(paths::remove(
            &t.layout.listing.folder,
            t.layout.listing.own_folder,
            &all,
        ))
    }
}

fn check_selection(files: &[Planned], set: HashSet<usize>) -> Result<HashSet<usize>, TorrentError> {
    if let Some(bad) = set.iter().find(|i| **i >= files.len()) {
        return Err(TorrentError::NoSuchFile(*bad));
    }
    let set: HashSet<usize> = set.into_iter().filter(|i| !files[*i].padding).collect();
    if set.is_empty() {
        return Err(TorrentError::NothingSelected);
    }
    Ok(set)
}

/// What a torrent contains and where it will be saved; from [`TorrentEngine::inspect`].
#[derive(Debug, Clone)]
pub struct Listing {
    pub name: String,
    pub info_hash: String,
    /// Where files go: the download folder, or a folder named after the torrent.
    pub folder: PathBuf,
    own_folder: bool,
    pub files: Vec<Planned>,
    torrent: Vec<u8>,
    peers: Vec<SocketAddr>,
}

impl Listing {
    pub fn total(&self) -> u64 {
        self.files
            .iter()
            .filter(|f| !f.padding)
            .map(|f| f.len)
            .sum()
    }
    fn wanted_indices(&self) -> HashSet<usize> {
        (0..self.files.len())
            .filter(|i| !self.files[*i].padding)
            .collect()
    }
}

#[derive(Debug)]
struct Layout {
    listing: Listing,
    selected: Mutex<HashSet<usize>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_magnets_and_small_files_are_accepted() {
        assert!(matches!(
            Source::Magnet("https://example.com/x.torrent".into()).checked(),
            Err(TorrentError::NotAMagnet)
        ));
        assert!(matches!(
            Source::Magnet("magnet:?dn=nohash".into()).checked(),
            Err(TorrentError::NotAMagnet)
        ));
        assert!(
            Source::Magnet(
                "  MAGNET:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567 ".into()
            )
            .checked()
            .is_ok()
        );
        assert!(matches!(
            Source::File(vec![0; MAX_TORRENT_FILE + 1]).checked(),
            Err(TorrentError::FileTooBig(_))
        ));
    }
}
