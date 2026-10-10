//! Multi-network HTTP download engine for Fuselane.
//!
//! Probe, plan, pure scheduler and concurrency controller, streams, response
//! checks and retry policy. Design: `docs/03-architecture/ENGINE-DOWNLOAD.md`.
//! Rules: L-01–L-35.

pub mod concurrency;
pub mod download;
pub mod headers;
pub mod measure;
pub mod plan;
pub mod retry;
pub mod scheduler;
pub mod throttle;
