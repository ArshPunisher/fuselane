//! A librqbit session wired for Fuselane (ADR 0006): every outgoing peer and HTTP
//! tracker connection goes through our SOCKS5 proxy, so the balancer decides which
//! network carries it. uTP, UPnP and local peer discovery are off; DHT and
//! trackers are the caller's choice.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use librqbit::dht::DhtPersistenceConfig;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ConnectionOptions, DhtSessionConfig,
    ListenerMode, ListenerOptions, ManagedTorrent, Session, SessionOptions, TorrentStatsState,
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

const LSD_V4: std::net::SocketAddrV4 =
    std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(239, 192, 152, 143), 6771);
const LSD_V6: std::net::Ipv6Addr = std::net::Ipv6Addr::new(0xff15, 0, 0, 0, 0, 0, 0xefc0, 0x988f);

/// Asks the local networks who has a torrent. A peer on the same network that
/// shares it answers at once, instead of at its next announce, which can be five
/// minutes away (librqbit only announces, never asks).
#[derive(Debug, Clone)]
pub struct LocalSearch {
    interfaces: Vec<Interface>,
    info_hash: String,
    port: u16,
    cookie: u32,
}

impl LocalSearch {
    /// Sends the search out of every network. Best effort: failures are ignored.
    pub fn send(&self) {
        for iface in &self.interfaces {
            for ip in &iface.addrs {
                let _ = lsd_search(*ip, iface.index, &self.info_hash, self.port, self.cookie);
            }
        }
    }
}

/// One BEP 14 search for `info_hash`, sent out of the network with address `ip`.
fn lsd_search(
    ip: std::net::IpAddr,
    index: u32,
    info_hash: &str,
    port: u16,
    cookie: u32,
) -> std::io::Result<()> {
    use socket2::{Domain, Protocol, Socket, Type};
    let (socket, to, host) = match ip {
        std::net::IpAddr::V4(v4) => {
            let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
            s.set_multicast_if_v4(&v4)?;
            s.set_multicast_loop_v4(true)?;
            s.bind(&SocketAddr::from((v4, 0)).into())?;
            (s, SocketAddr::V4(LSD_V4), LSD_V4.to_string())
        }
        std::net::IpAddr::V6(v6) => {
            let s = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))?;
            s.set_multicast_if_v6(index)?;
            s.set_multicast_loop_v6(true)?;
            s.bind(&SocketAddr::from((v6, 0)).into())?;
            let to = std::net::SocketAddrV6::new(LSD_V6, 6771, 0, index);
            (s, SocketAddr::V6(to), format!("[{LSD_V6}]:6771"))
        }
    };
    let msg = format!(
        "BT-SEARCH * HTTP/1.1\r\nHost: {host}\r\nPort: {port}\r\nInfohash: {info_hash}\r\ncookie: {cookie}\r\n\r\n\r\n"
    );
    socket.send_to(msg.as_bytes(), &to.into())?;
    Ok(())
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
    /// Where the DHT keeps its node list. None: not saved. Never the library's
    /// default folder (L-70).
    pub state_dir: Option<PathBuf>,
    /// The app's speed limits and data allowances (shared with HTTP downloads).
    pub limiter: Option<Arc<fuselane_limits::Limiter>>,
    /// Ask the router to forward the listening port (UPnP), so peers can reach us.
    /// Off for ordinary torrents; Fuse Send turns it on for its own engine.
    pub upnp: bool,
    /// Find peers on the same network (BitTorrent local service discovery).
    pub local_discovery: bool,
}

/// How to start a torrent.
#[derive(Debug, Clone, Default)]
pub struct AddOptions {
    /// Files to download by index (None: all of them).
    pub only: Option<HashSet<usize>>,
    pub paused: bool,
    /// Continue into files Fuselane saved earlier (after a restart): librqbit
    /// rechecks every piece. Without it, an existing file is refused.
    pub resume: bool,
    /// Where the torrent's bytes live, when not plain files in the output folder
    /// (Fuse Send keeps shares encrypted on the wire, plain on disk).
    pub storage: Option<Storage>,
}

/// A librqbit storage backend for one torrent.
pub struct Storage(pub librqbit::storage::BoxStorageFactory);

impl Clone for Storage {
    fn clone(&self) -> Self {
        Storage(self.0.clone_box())
    }
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Storage(custom)")
    }
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
    /// The person paused it. librqbit can lose a pause asked for at the very end
    /// of the first check; until it's put back, the torrent still reads as paused.
    held: Arc<std::sync::atomic::AtomicBool>,
}

/// One connected peer, as the window shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// Its address (stable key while connected).
    pub addr: String,
    /// The app it says it is ("qBittorrent 5.0.1"), when it tells.
    pub client: Option<String>,
    /// Bytes received from it and sent to it so far.
    pub received: u64,
    pub sent: u64,
    /// Which of our networks the connection goes over (None: it connected to us).
    pub network: Option<String>,
    /// "tcp", "utp" or "socks".
    pub kind: &'static str,
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
            // A pause asked for during the check: stopped once the check ends.
            TorrentStatsState::Initializing { paused: true } => Phase::Paused,
            TorrentStatsState::Initializing { .. } => Phase::Checking,
            TorrentStatsState::Paused => Phase::Paused,
            TorrentStatsState::Error => Phase::Failed,
            TorrentStatsState::Live if self.held.load(std::sync::atomic::Ordering::SeqCst) => {
                Phase::Paused
            }
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
    /// Bytes of each file that have arrived and passed their check, in listing
    /// order (empty until the torrent is known).
    pub fn file_progress(&self) -> Vec<u64> {
        self.handle.stats().file_progress
    }

    /// Indices of the files being downloaded.
    pub fn selected(&self) -> HashSet<usize> {
        self.layout
            .selected
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    /// The peers connected right now.
    pub fn peers(&self) -> Vec<PeerInfo> {
        let Some(live) = self.handle.live() else {
            return Vec::new();
        };
        let mut out: Vec<PeerInfo> = live
            .per_peer_stats_snapshot(Default::default())
            .peers
            .into_iter()
            .map(|(addr, p)| PeerInfo {
                network: addr.parse().ok().and_then(|a| self.balancer.route_of(a)),
                addr,
                client: p.client_name.filter(|c| !c.trim().is_empty()),
                received: p.counters.fetched_bytes,
                sent: p.counters.uploaded_bytes,
                // librqbit doesn't export the type, only its name.
                kind: match p.conn_kind.map(|k| format!("{k:?}")).as_deref() {
                    Some("Utp") => "utp",
                    Some("Socks") => "socks",
                    _ => "tcp",
                },
            })
            .collect();
        out.sort_by(|a, b| b.received.cmp(&a.received).then(a.addr.cmp(&b.addr)));
        out
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
        let balancer = Arc::new(Balancer::new(opts.networks).with_limiter(opts.limiter.clone()));
        let socks = socks::start(balancer.clone()).await.map_err(engine)?;
        let download_dir = opts.download_dir;
        let session = Session::new_with_opts(
            download_dir.clone(),
            SessionOptions {
                dht: opts.dht.then(|| DhtSessionConfig {
                    persistence: opts.state_dir.as_ref().map(|d| DhtPersistenceConfig {
                        config_filename: Some(d.join("dht.json")),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                fastresume: false,
                persistence: None,
                listen: opts.listen.map(|listen_addr| ListenerOptions {
                    mode: ListenerMode::TcpOnly,
                    listen_addr,
                    enable_upnp_port_forwarding: opts.upnp,
                    ..Default::default()
                }),
                connect: Some(ConnectionOptions {
                    proxy_url: Some(socks.url()),
                    ..Default::default()
                }),
                disable_local_service_discovery: !opts.local_discovery,
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

    /// Networks that take no new peers until changed (the app avoids metered
    /// networks while torrents only seed).
    pub fn avoid_networks<I: IntoIterator<Item = String>>(&self, names: I) {
        self.balancer.set_avoid(names);
    }

    /// A BEP 14 search for `info_hash` on this engine's networks (see [`LocalSearch`]).
    pub fn local_search(&self, info_hash: &str) -> Option<LocalSearch> {
        let port = self.listen_addr()?.port();
        // Any number but the answering peer's own (it ignores its own messages).
        let cookie = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.subsec_nanos() ^ std::process::id());
        Some(LocalSearch {
            interfaces: self.interfaces(),
            info_hash: info_hash.to_string(),
            port,
            cookie,
        })
    }

    /// Follows network changes (plugged in, unplugged, new address).
    pub fn set_networks(&self, now: Vec<Interface>) {
        self.balancer.set_networks(now);
    }

    /// How complete each part of the torrent is, in `cells` equal slices of its
    /// pieces (0 to 100 each), for a map that fills in as pieces arrive.
    pub fn piece_map(&self, t: &Torrent, cells: usize) -> Vec<u8> {
        let api = librqbit::Api::new(self.session.clone(), None);
        let Ok((have, total)) = api.api_dump_haves(librqbit::api::TorrentIdOrHash::Id(t.id()))
        else {
            return Vec::new();
        };
        let total = total as usize;
        if total == 0 || cells == 0 {
            return Vec::new();
        }
        let cells = cells.min(total);
        (0..cells)
            .map(|c| {
                let (from, to) = (
                    c * total / cells,
                    ((c + 1) * total / cells).max(c * total / cells + 1),
                );
                let got = (from..to)
                    .filter(|&i| have.get(i).is_some_and(|b| *b))
                    .count();
                (got * 100 / (to - from)) as u8
            })
            .collect()
    }

    /// The networks present now.
    pub fn interfaces(&self) -> Vec<Interface> {
        self.balancer.networks()
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
        // Only a magnet needs peers to learn its files; a .torrent file has them.
        // Handing peers to a list-only add opens connections that are then dropped,
        // and the seeder can hold the real connection back until it times out.
        let needs_peers = matches!(source, Source::Magnet(_));
        let add = source.checked()?;
        let base = output_folder.unwrap_or_else(|| self.download_dir.clone());
        let listed = self
            .session
            .add_torrent(
                add,
                Some(AddTorrentOptions {
                    list_only: true,
                    output_folder: Some(base.to_string_lossy().into_owned()),
                    initial_peers: (needs_peers && !initial_peers.is_empty())
                        .then(|| initial_peers.clone()),
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
        opts: AddOptions,
    ) -> Result<Torrent, TorrentError> {
        let listing = self.inspect(source, output_folder, initial_peers).await?;
        self.add_listed(listing, opts).await
    }

    /// Adds a torrent already inspected (for example after the user picked files).
    pub async fn add_listed(
        &self,
        listing: Listing,
        add: AddOptions,
    ) -> Result<Torrent, TorrentError> {
        let selected = match add.only {
            Some(set) => check_selection(&listing.files, set)?,
            None => listing.wanted_indices(),
        };
        let held = Arc::new(std::sync::atomic::AtomicBool::new(add.paused));
        let opts = AddTorrentOptions {
            output_folder: Some(listing.folder.to_string_lossy().into_owned()),
            initial_peers: (!listing.peers.is_empty()).then(|| listing.peers.clone()),
            only_files: Some(selected.iter().copied().collect()),
            // Never write into a file the user already has (librqbit would reuse it),
            // unless it is Fuselane's own from before a restart.
            overwrite: add.resume,
            paused: add.paused,
            storage_factory: add.storage.map(|s| s.0),
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
                held,
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

    /// Reads one file in order as it arrives, for playing while downloading
    /// (B8.10). librqbit fetches the pieces just ahead of the reader first; the
    /// file must be selected.
    pub async fn stream(
        &self,
        t: &Torrent,
        file: usize,
    ) -> Result<Box<dyn FileReader>, TorrentError> {
        if t.layout.listing.files.get(file).is_none_or(|f| f.padding) {
            return Err(TorrentError::NoSuchFile(file));
        }
        let s = t.handle.clone().stream(file).await.map_err(engine)?;
        Ok(Box::new(s))
    }

    pub async fn pause(&self, t: &Torrent) -> Result<(), TorrentError> {
        t.held.store(true, std::sync::atomic::Ordering::SeqCst);
        self.session.pause(&t.handle).await.map_err(engine)?;
        // During the first check a pause is only a request, and librqbit starts the
        // torrent anyway if the check ends at that moment. Watch until the check is
        // over and pause again if it went live while still held.
        let (session, t) = (self.session.clone(), t.clone());
        tokio::spawn(async move {
            loop {
                match t.handle.stats().state {
                    TorrentStatsState::Initializing { .. } => {
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    TorrentStatsState::Live if t.held.load(std::sync::atomic::Ordering::SeqCst) => {
                        let _ = session.pause(&t.handle).await;
                        break;
                    }
                    _ => break,
                }
            }
        });
        Ok(())
    }

    /// Starts the torrent's peers afresh. librqbit waits 10 s, then 60 s, then 6
    /// minutes before retrying a peer that failed, so after every network was
    /// blocked (data allowances) a torrent could sit idle for minutes once one is
    /// back. Pausing and resuming rebuilds the peer list with fresh retry timers.
    pub async fn reconnect(&self, t: &Torrent) -> Result<(), TorrentError> {
        if t.progress().phase == Phase::Paused {
            return Ok(());
        }
        self.session.pause(&t.handle).await.map_err(engine)?;
        self.session.unpause(&t.handle).await.map_err(engine)
    }

    pub async fn resume(&self, t: &Torrent) -> Result<(), TorrentError> {
        t.held.store(false, std::sync::atomic::Ordering::SeqCst);
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

impl TorrentEngine {
    /// Deletes a released torrent's files with the same safe deleter as `remove`.
    pub fn delete_files(&self, listing: &Listing) -> Cleanup {
        let all: Vec<&Planned> = listing.files.iter().collect();
        paths::remove(&listing.folder, listing.own_folder, &all)
    }

    /// Like [`Self::delete_files`] with the same path checks, but each file goes
    /// through `remove_file` (for example to the Trash).
    pub fn delete_files_with(
        &self,
        listing: &Listing,
        remove_file: impl FnMut(&std::path::Path) -> std::io::Result<()>,
    ) -> Cleanup {
        let all: Vec<&Planned> = listing.files.iter().collect();
        paths::remove_with(&listing.folder, listing.own_folder, &all, remove_file)
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

/// A torrent file read in order as it arrives (librqbit's stream type isn't
/// exported, so callers get it behind this).
pub trait FileReader: tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send {}
impl<T: tokio::io::AsyncRead + tokio::io::AsyncSeek + Unpin + Send> FileReader for T {}

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
    /// The folder the user chose (the torrent's own folder sits inside it).
    pub fn base(&self) -> &std::path::Path {
        if self.own_folder {
            self.folder.parent().unwrap_or(&self.folder)
        } else {
            &self.folder
        }
    }
    /// The .torrent itself, to save and add again after a restart.
    pub fn torrent_bytes(&self) -> &[u8] {
        &self.torrent
    }
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
