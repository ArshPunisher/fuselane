//! Test harness for Fuselane.
//!
//! Fault-injecting range server, fake networks, invariant and leak checks.
//! Only test code depends on this crate (L-100).
//! Design: `docs/05-quality/TESTING.md` §2.

pub mod content;
pub mod server;

pub use content::{Content, sha256_file};
pub use server::{Fault, RangeServer, RequestLog, Rule};
