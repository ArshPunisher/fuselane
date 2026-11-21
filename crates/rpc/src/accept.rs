//! What an accept loop does when `accept` fails (L-126).
//!
//! `while let Ok(..) = listener.accept().await` ends the loop for good on the first
//! error, and `accept` fails for passing reasons: out of file descriptors (`EMFILE`),
//! a client that reset before it was accepted (`ECONNABORTED`). The listener then
//! stays "on" in the settings while nobody answers. These loops back off and go on.

use std::future::Future;
use std::io;
use std::time::Duration;

/// First wait after a failed accept; doubles up to [`MAX_WAIT`].
const FIRST_WAIT: Duration = Duration::from_millis(50);
const MAX_WAIT: Duration = Duration::from_secs(2);

/// Counts failures in a row so the wait grows and the log has one line per run.
#[derive(Debug, Default)]
pub(crate) struct Backoff {
    failures: u32,
}

impl Backoff {
    /// An accept worked: the next failure is a new run.
    pub(crate) fn ok(&mut self) {
        self.failures = 0;
    }

    /// Notes a failed accept and returns how long to wait before the next try.
    /// Only the first failure of a run is logged, so a flood of errors can't flood the log.
    pub(crate) fn failed(&mut self, what: &str, e: &io::Error) -> Duration {
        if self.failures == 0 {
            tracing::warn!(error = %e, "{what}: accept failed, retrying with back-off");
        }
        self.failures = self.failures.saturating_add(1);
        let doublings = (self.failures - 1).min(6);
        FIRST_WAIT.saturating_mul(1 << doublings).min(MAX_WAIT)
    }
}

/// Calls `accept` forever and hands each connection to `on_conn`; an error is
/// logged once per run and retried after a growing pause.
pub(crate) async fn accept_forever<C, F, Fut>(what: &str, mut accept: F, mut on_conn: impl FnMut(C))
where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<C>>,
{
    let mut backoff = Backoff::default();
    loop {
        match accept().await {
            Ok(conn) => {
                backoff.ok();
                on_conn(conn);
            }
            Err(e) => tokio::time::sleep(backoff.failed(what, &e)).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn the_wait_doubles_and_is_capped() {
        let mut b = Backoff::default();
        let e = io::Error::other("x");
        let waits: Vec<u64> = (0..10)
            .map(|_| b.failed("t", &e).as_millis() as u64)
            .collect();
        assert_eq!(
            waits,
            [50, 100, 200, 400, 800, 1600, 2000, 2000, 2000, 2000]
        );
        b.ok();
        assert_eq!(b.failed("t", &e), FIRST_WAIT, "a success starts a new run");
    }

    #[tokio::test(start_paused = true)]
    async fn a_listener_that_fails_is_tried_again_and_the_next_connection_is_served() {
        let calls = Arc::new(AtomicU32::new(0));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let c = calls.clone();
        let task = tokio::spawn(accept_forever(
            "test",
            move || {
                let n = c.fetch_add(1, Ordering::SeqCst);
                async move {
                    match n {
                        // Out of descriptors, twice, then a connection, then an abort.
                        0 | 1 => Err(io::Error::from_raw_os_error(24)),
                        2 => Ok("first"),
                        3 => Err(io::Error::from(io::ErrorKind::ConnectionAborted)),
                        4 => Ok("second"),
                        // Then nobody connects: wait, as a real listener does.
                        _ => std::future::pending().await,
                    }
                }
            },
            move |conn| {
                let _ = tx.send(conn);
            },
        ));
        assert_eq!(rx.recv().await, Some("first"));
        assert_eq!(rx.recv().await, Some("second"));
        assert!(!task.is_finished(), "the loop never ends on an error");
        task.abort();
    }
}
