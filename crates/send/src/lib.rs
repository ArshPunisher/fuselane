//! Fuse Send: sharing a file directly, computer to computer, over every network
//! (docs/03-architecture/FUSE-SEND.md, ADR 0011). No server stores anything.

pub mod crypt;
pub mod link;
pub mod storage;
pub mod torrent;
pub mod view;
