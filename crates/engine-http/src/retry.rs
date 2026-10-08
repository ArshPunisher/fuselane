//! What to do when an attempt fails (ENGINE-DOWNLOAD.md §7, L-14–L-21, L-46).
//!
//! Every failure has an owner: the network, the server, the disk, or the file
//! itself. Each is handled differently, and the decision is pure (time and
//! randomness are passed in) so the whole policy is testable.

use crate::headers::RangeError;

pub const MAX_STRIKES: u32 = 5;
pub const BASE_DELAY_MS: u64 = 1_000;
pub const MAX_DELAY_MS: u64 = 15_000;
pub const UNREACHABLE_RETRY_MS: u64 = 5_000;
pub const BUSY_WINDOW_MS: u64 = 300_000;
pub const RETRY_AFTER_CAP_MS: u64 = 120_000;

/// Statuses that mean "busy, try later" (curl's transient set).
pub fn is_busy(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}
/// Statuses that mean "this link no longer works" (signed URLs expire this way).
pub fn is_link_refusal(status: u16) -> bool {
    matches!(status, 401 | 403 | 404 | 410)
}
/// Statuses that suggest the server limits connections from this address.
pub fn is_connection_limit(status: u16) -> bool {
    matches!(status, 403 | 429 | 503)
}

/// Disk failures: never retried as strikes; the job pauses with a clear message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiskFailure {
    NoSpace,
    QuotaExceeded,
    ReadOnly,
    PermissionDenied,
    Io,
    DriveMissing,
}

/// How an attempt failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Couldn't connect, reset, timed out, or went silent: the network's fault.
    Connection,
    /// The selected network has no address in the host's IP family.
    NoRoute,
    /// The server answered with a status we can't use.
    Status {
        code: u16,
        retry_after_ms: Option<u64>,
    },
    /// The server answered wrongly for the range (wrong start, overrun, short body...).
    BadResponse(RangeError),
    /// The file's validators changed. `size_changed` is proof of a new file.
    VersionChanged {
        size_changed: bool,
    },
    Disk(DiskFailure),
}

/// What the stream should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Wait, then try again. `hold_network_ms`: no stream on this network may send
    /// a request before then (a server's Retry-After applies to the whole address).
    Retry {
        delay_ms: u64,
        hold_network_ms: Option<u64>,
    },
    /// Mark the network unreachable; keep one probe stream retrying at this interval.
    Unreachable { retry_ms: u64 },
    /// Retire this stream (too many strikes); its block goes back to the queue.
    Retire,
    /// The network can't reach this host at all.
    FailNetwork,
    /// Compare sampled bytes already on disk before deciding (L-05).
    ConfirmBytes,
    /// The file on the server is a different file: fail and discard.
    FailAndDiscard,
    /// Pause the whole job with this disk message (L-46).
    PauseJob(DiskFailure),
}

/// Side signals for the job and the concurrency controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Signals {
    /// Counts as a refusal for the concurrency controller (L-18).
    pub refused: bool,
    /// The link was refused with this status: if every network sees it, offer "Fix link".
    pub link_refused: Option<u16>,
}

/// Retry state of one stream.
#[derive(Debug, Clone, Copy, Default)]
pub struct StreamRetry {
    failures: u32,
    strikes: u32,
    busy_since_ms: Option<u64>,
}

impl StreamRetry {
    /// An attempt delivered bytes: the stream is healthy again.
    pub fn progressed(&mut self) {
        *self = StreamRetry::default();
    }

    pub fn strikes(&self) -> u32 {
        self.strikes
    }

    /// `jitter` in [0, 1) spreads retries (±20%) so streams don't stampede.
    /// `network_silent`: nothing has arrived on this network for a while.
    pub fn decide(
        &mut self,
        failure: &Failure,
        now_ms: u64,
        network_silent: bool,
        jitter: f64,
    ) -> (Decision, Signals) {
        let jitter = jitter.clamp(0.0, 1.0);
        match failure {
            Failure::Disk(d) => (Decision::PauseJob(*d), Signals::default()),
            Failure::NoRoute => (Decision::FailNetwork, Signals::default()),
            Failure::VersionChanged { size_changed: true } => {
                (Decision::FailAndDiscard, Signals::default())
            }
            Failure::VersionChanged {
                size_changed: false,
            } => (Decision::ConfirmBytes, Signals::default()),
            Failure::Connection => {
                // Never uses up retries while the network exists (L-16).
                self.failures += 1;
                if network_silent {
                    (
                        Decision::Unreachable {
                            retry_ms: UNREACHABLE_RETRY_MS,
                        },
                        Signals::default(),
                    )
                } else {
                    (
                        Decision::Retry {
                            delay_ms: self.backoff(jitter),
                            hold_network_ms: None,
                        },
                        Signals::default(),
                    )
                }
            }
            Failure::Status {
                code,
                retry_after_ms,
            } => {
                self.failures += 1;
                let signals = Signals {
                    refused: is_connection_limit(*code),
                    link_refused: is_link_refusal(*code).then_some(*code),
                };
                if is_busy(*code) {
                    let since = *self.busy_since_ms.get_or_insert(now_ms);
                    if now_ms.saturating_sub(since) < BUSY_WINDOW_MS {
                        let hold = retry_after_ms.map(|r| r.min(RETRY_AFTER_CAP_MS));
                        let delay = self.backoff(jitter).max(hold.unwrap_or(0));
                        return (
                            Decision::Retry {
                                delay_ms: delay,
                                hold_network_ms: hold,
                            },
                            signals,
                        );
                    }
                }
                self.strike(jitter, signals)
            }
            Failure::BadResponse(_) => {
                self.failures += 1;
                self.strike(jitter, Signals::default())
            }
        }
    }

    fn strike(&mut self, jitter: f64, signals: Signals) -> (Decision, Signals) {
        self.strikes += 1;
        if self.strikes > MAX_STRIKES {
            (Decision::Retire, signals)
        } else {
            (
                Decision::Retry {
                    delay_ms: self.backoff(jitter),
                    hold_network_ms: None,
                },
                signals,
            )
        }
    }

    /// `min(1 s · 2^(n−1), 15 s) × (0.8–1.2)`.
    fn backoff(&self, jitter: f64) -> u64 {
        let exp = self.failures.saturating_sub(1).min(16);
        let base = BASE_DELAY_MS.saturating_mul(1u64 << exp).min(MAX_DELAY_MS);
        (base as f64 * (0.8 + 0.4 * jitter)) as u64
    }
}

/// Parses `Retry-After` as seconds or an HTTP date (RFC 9110), relative to `now_unix_s`.
pub fn parse_retry_after(value: &str, now_unix_s: u64) -> Option<u64> {
    let v = value.trim();
    if let Ok(secs) = v.parse::<u64>() {
        return Some(secs.saturating_mul(1000));
    }
    let at = parse_http_date(v)?;
    Some(at.saturating_sub(now_unix_s).saturating_mul(1000))
}

/// IMF-fixdate only (`Sun, 06 Nov 1994 08:49:37 GMT`), the form RFC 9110 says to send.
fn parse_http_date(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() != 6 || parts[5] != "GMT" {
        return None;
    }
    let day: u64 = parts[1].parse().ok()?;
    let month = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .iter()
    .position(|m| *m == parts[2])? as u64
        + 1;
    let year: u64 = parts[3].parse().ok()?;
    let hms: Vec<u64> = parts[4]
        .split(':')
        .map(|x| x.parse().ok())
        .collect::<Option<_>>()?;
    let [h, m, sec] = hms[..] else { return None };
    if !(1..=31).contains(&day) || h > 23 || m > 59 || sec > 60 || year < 1970 {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let (y, mo) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = y / 400;
    let yoe = y - era * 400;
    let doy = (153 * mo + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + m * 60 + sec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn connection_errors_never_retire_a_stream() {
        let mut r = StreamRetry::default();
        for i in 0..1000 {
            let (d, _) = r.decide(&Failure::Connection, i * 1000, false, 0.5);
            assert!(matches!(d, Decision::Retry { .. }), "attempt {i}: {d:?}");
        }
    }

    #[test]
    fn silent_network_becomes_unreachable() {
        let mut r = StreamRetry::default();
        assert_eq!(
            r.decide(&Failure::Connection, 0, true, 0.0).0,
            Decision::Unreachable { retry_ms: 5_000 }
        );
    }

    #[test]
    fn backoff_grows_and_caps() {
        let mut r = StreamRetry::default();
        let delays: Vec<u64> = (0..8)
            .map(|_| match r.decide(&Failure::Connection, 0, false, 0.5).0 {
                Decision::Retry { delay_ms, .. } => delay_ms,
                d => panic!("{d:?}"),
            })
            .collect();
        assert_eq!(
            delays,
            vec![1_000, 2_000, 4_000, 8_000, 15_000, 15_000, 15_000, 15_000]
        );
    }

    #[test]
    fn busy_servers_are_waited_out_for_five_minutes_honouring_retry_after() {
        let mut r = StreamRetry::default();
        let (d, s) = r.decide(
            &Failure::Status {
                code: 429,
                retry_after_ms: Some(30_000),
            },
            0,
            false,
            0.5,
        );
        assert_eq!(
            d,
            Decision::Retry {
                delay_ms: 30_000,
                hold_network_ms: Some(30_000)
            }
        );
        assert!(s.refused);
        let (d, _) = r.decide(
            &Failure::Status {
                code: 503,
                retry_after_ms: Some(10_000_000),
            },
            1_000,
            false,
            0.5,
        );
        assert_eq!(
            d,
            Decision::Retry {
                delay_ms: RETRY_AFTER_CAP_MS,
                hold_network_ms: Some(RETRY_AFTER_CAP_MS)
            },
            "Retry-After capped at 2 min"
        );
        for t in [60_000, 120_000, 299_000] {
            assert!(matches!(
                r.decide(
                    &Failure::Status {
                        code: 503,
                        retry_after_ms: None
                    },
                    t,
                    false,
                    0.5
                )
                .0,
                Decision::Retry { .. }
            ));
        }
        assert_eq!(r.strikes(), 0, "waiting out a busy server isn't a strike");
        // After the window, busy statuses count as strikes.
        let mut last = Decision::FailNetwork;
        for i in 0..=MAX_STRIKES {
            last = r
                .decide(
                    &Failure::Status {
                        code: 503,
                        retry_after_ms: None,
                    },
                    300_000 + u64::from(i),
                    false,
                    0.5,
                )
                .0;
        }
        assert_eq!(last, Decision::Retire);
    }

    #[test]
    fn link_refusals_strike_and_signal_fix_link() {
        let mut r = StreamRetry::default();
        let (d, s) = r.decide(
            &Failure::Status {
                code: 410,
                retry_after_ms: None,
            },
            0,
            false,
            0.0,
        );
        assert!(matches!(d, Decision::Retry { .. }));
        assert_eq!(s.link_refused, Some(410));
        assert!(!s.refused, "410 isn't a connection limit");
        assert_eq!(r.strikes(), 1);
    }

    #[test]
    fn six_bad_answers_in_a_row_retire_but_progress_forgives() {
        let mut r = StreamRetry::default();
        let bad = Failure::BadResponse(RangeError::MissingContentRange);
        for _ in 0..MAX_STRIKES {
            assert!(matches!(
                r.decide(&bad, 0, false, 0.0).0,
                Decision::Retry { .. }
            ));
        }
        r.progressed();
        assert_eq!(r.strikes(), 0);
        for _ in 0..MAX_STRIKES {
            r.decide(&bad, 0, false, 0.0);
        }
        assert_eq!(r.decide(&bad, 0, false, 0.0).0, Decision::Retire);
    }

    #[test]
    fn disk_and_version_failures_are_never_retried_blindly() {
        let mut r = StreamRetry::default();
        assert_eq!(
            r.decide(&Failure::Disk(DiskFailure::NoSpace), 0, false, 0.0)
                .0,
            Decision::PauseJob(DiskFailure::NoSpace)
        );
        assert_eq!(
            r.decide(
                &Failure::VersionChanged { size_changed: true },
                0,
                false,
                0.0
            )
            .0,
            Decision::FailAndDiscard
        );
        assert_eq!(
            r.decide(
                &Failure::VersionChanged {
                    size_changed: false
                },
                0,
                false,
                0.0
            )
            .0,
            Decision::ConfirmBytes
        );
        assert_eq!(
            r.decide(&Failure::NoRoute, 0, false, 0.0).0,
            Decision::FailNetwork
        );
        assert_eq!(r.strikes(), 0);
    }

    #[test]
    fn retry_after_formats() {
        let now = 784_111_777; // Sun, 06 Nov 1994 08:49:37 GMT
        assert_eq!(parse_retry_after("120", now), Some(120_000));
        assert_eq!(
            parse_retry_after("Sun, 06 Nov 1994 08:49:37 GMT", now),
            Some(0)
        );
        assert_eq!(
            parse_retry_after("Sun, 06 Nov 1994 08:50:37 GMT", now),
            Some(60_000)
        );
        assert_eq!(
            parse_retry_after("Sun, 06 Nov 1994 08:48:37 GMT", now),
            Some(0),
            "dates in the past mean now"
        );
        for bad in [
            "",
            "soon",
            "-5",
            "1.5",
            "Sun, 06 Nov 1994 08:49:37 PST",
            "Sun, 32 Nov 1994 08:49:37 GMT",
            "Sun, 06 Foo 1994 08:49:37 GMT",
            "Sun, 06 Nov 1994 25:49:37 GMT",
            "Sunday, 06-Nov-94 08:49:37 GMT",
        ] {
            assert_eq!(parse_retry_after(bad, now), None, "{bad:?}");
        }
        assert_eq!(parse_retry_after("99999999999999999999", now), None);
        assert_eq!(
            parse_retry_after("18446744073709551615", now),
            Some(u64::MAX),
            "saturates instead of overflowing"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(3000))]

        #[test]
        fn delays_are_bounded_and_retries_never_panic(
            events in proptest::collection::vec((0u8..6, 100u16..600, proptest::option::of(0u64..u64::MAX), any::<bool>(), 0.0f64..1.0), 1..80),
        ) {
            let mut r = StreamRetry::default();
            let mut now = 0u64;
            for (kind, code, ra, silent, jitter) in events {
                now += 997;
                let f = match kind {
                    0 => Failure::Connection,
                    1 => Failure::Status { code, retry_after_ms: ra },
                    2 => Failure::BadResponse(RangeError::RangeIgnored),
                    3 => Failure::Disk(DiskFailure::Io),
                    4 => Failure::VersionChanged { size_changed: silent },
                    _ => Failure::NoRoute,
                };
                let (d, _) = r.decide(&f, now, silent, jitter);
                if let Decision::Retry { delay_ms, hold_network_ms } = d {
                    prop_assert!(delay_ms <= RETRY_AFTER_CAP_MS.max((MAX_DELAY_MS as f64 * 1.2) as u64));
                    prop_assert!(hold_network_ms.is_none_or(|h| h <= RETRY_AFTER_CAP_MS));
                }
                if matches!(f, Failure::Connection) {
                    prop_assert!(!matches!(d, Decision::Retire), "connection errors must never retire");
                }
            }
        }

        #[test]
        fn retry_after_never_panics(s in "\\PC{0,60}", now in any::<u64>()) {
            let _ = parse_retry_after(&s, now);
        }
    }
}
