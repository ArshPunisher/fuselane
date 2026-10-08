//! Disk storage for Fuselane transfers.
//!
//! Staging files at fixed offsets, preallocation and sparse files, the writer
//! pool, crash-safe publish, and file names that are valid on every OS.
//! Design: `docs/03-architecture/ENGINE-DOWNLOAD.md` §8. Rules: L-36–L-48.

pub mod names;
pub mod staging;
