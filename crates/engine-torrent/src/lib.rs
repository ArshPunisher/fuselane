//! Torrent engine (ADR 0006, TORRENT.md): librqbit for BitTorrent itself, with
//! every outgoing peer connection sent through an in-process SOCKS5 server that
//! pins it to a network chosen by the balancer.

pub mod balancer;
pub mod engine;
pub mod paths;
pub mod socks;

pub use balancer::{NetShare, NetStat};
pub use engine::{
    AddOptions, EngineOptions, Listing, Phase, Progress, Source, Torrent, TorrentEngine,
    TorrentError,
};
pub use paths::{Cleanup, Planned};
