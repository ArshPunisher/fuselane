//! Starting async work from anywhere in the app.
//!
//! `tokio::spawn` only works on a thread that is already inside a Tokio runtime.
//! Fuselane also runs work from plain OS threads (the 5-second ticker in
//! `main.rs`, the clipboard watcher) and from sync Tauri commands, which run on
//! the main thread. There `tokio::spawn` panics with "no reactor running"; a
//! panic in the ticker stopped retries, schedules and allowance counting for the
//! rest of the session and left a download marked running with nothing running.
//! This uses the current runtime when there is one (async code, tests) and
//! Tauri's own runtime otherwise.

use std::future::Future;

/// Spawns `task` on the current Tokio runtime, or on Tauri's when the calling
/// thread has none.
pub fn spawn<F>(task: F) -> tokio::task::JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(here) => here.spawn(task),
        Err(_) => tauri::async_runtime::handle().inner().spawn(task),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    #[test]
    fn work_started_from_a_plain_thread_runs() {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            super::spawn(async move { tx.send(42).unwrap() });
        })
        .join()
        .unwrap();
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
            42
        );
    }
}
