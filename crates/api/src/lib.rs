//! Local API for Fuselane.
//!
//! JSON-RPC over a Unix socket or named pipe for the CLI, the desktop app and
//! the browser extension's native-messaging host. Every payload is validated at
//! runtime. Design: `docs/03-architecture/ARCHITECTURE.md` §2. Rules: L-97, L-98.

pub mod client;
pub mod hosts;
pub mod native;
pub mod offer;
pub mod server;
