//! Fuselane core: jobs, queue, persistence and events.
//!
//! Owns job state machines, the download queue, SQLite persistence with
//! migrations, and the event bus the UI and API subscribe to.
//! Design: `docs/03-architecture/ARCHITECTURE.md` §4–6. Rules: L-49–L-55, L-85.
