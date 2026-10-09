//! Nearby: send files to devices on the same network, speaking the LocalSend v2
//! protocol (ADR 0012, docs/03-architecture/NEARBY.md).

pub mod client;
pub mod discovery;
pub mod identity;
pub mod proto;
pub mod server;
pub mod tls;
pub mod web;
pub mod words;

pub use client::{Outgoing, SendError, Target};
pub use discovery::Discovery;
pub use identity::Identity;
pub use proto::{DeviceInfo, PrepareUpload};
pub use server::{Decision, Ended, Host, Server};
pub use words::check_words;
