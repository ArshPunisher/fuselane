//! Fuselane core: jobs, queue, persistence and events.
//!
//! Owns job state machines, the download queue, SQLite persistence with
//! migrations, and the event bus the UI and API subscribe to.
//! Design: `docs/03-architecture/ARCHITECTURE.md` §4–6. Rules: L-49–L-55, L-85.

pub mod batch;
pub mod checksums;
pub mod clip;
pub mod grab;
pub mod job;
pub mod runner;
pub mod store;

pub use job::{Event, InvalidTransition, Status};
pub mod home;
pub use home::open_default;
pub use runner::{Outcome, RunOptions, StartError};
pub use store::{Job, NewJob, Store, StoreError};
