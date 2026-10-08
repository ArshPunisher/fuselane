//! A librqbit session wired for Fuselane (ADR 0006): every outgoing peer and HTTP
//! tracker connection goes through our SOCKS5 proxy, so the balancer decides which
//! network carries it. uTP, UPnP and local peer discovery are off; DHT and
//! trackers are the caller's choice.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use librqbit::{
    AddTorrent, AddTorrentOptions, ConnectionOptions, ListenerMode, ListenerOptions,
    ManagedTorrent, Session, SessionOptions, TorrentStatsState,
};

use crate::balancer::{Balancer, NetStat};
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
        let session = Session::new_with_opts(
            opts.download_dir,
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

    pub async fn add(
        &self,
        source: Source,
        output_folder: Option<PathBuf>,
        initial_peers: Vec<SocketAddr>,
    ) -> Result<Torrent, TorrentError> {
        let add = source.checked()?;
        let opts = AddTorrentOptions {
            output_folder: output_folder.map(|p| p.to_string_lossy().into_owned()),
            initial_peers: (!initial_peers.is_empty()).then_some(initial_peers),
            // Never write into a file the user already has (librqbit would reuse it).
            overwrite: false,
            ..Default::default()
        };
        let resp = self
            .session
            .add_torrent(add, Some(opts))
            .await
            .map_err(add_error)?;
        let handle = resp
            .into_handle()
            .ok_or_else(|| TorrentError::Invalid("no torrent was added".into()))?;
        Ok(Torrent { handle })
    }

    pub async fn pause(&self, t: &Torrent) -> Result<(), TorrentError> {
        self.session.pause(&t.handle).await.map_err(engine)
    }

    pub async fn resume(&self, t: &Torrent) -> Result<(), TorrentError> {
        self.session.unpause(&t.handle).await.map_err(engine)
    }

    pub async fn remove(&self, t: Torrent, delete_files: bool) -> Result<(), TorrentError> {
        self.session
            .delete(t.handle.id().into(), delete_files)
            .await
            .map_err(engine)
    }
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
