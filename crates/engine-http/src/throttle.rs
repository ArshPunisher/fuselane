//! Spotting a throttled network (STEPS 8.2; market research §3.2).
//!
//! Daily phone plans drop a tethered phone to 64 kbps once the day's data is
//! used, and links sometimes collapse to a sliver of their own speed while the
//! others keep going. A crawling network that keeps taking blocks holds them
//! until the end-of-download races rescue them, so it's better to stop giving
//! it work, hand what it holds to the other networks, and try it again now and
//! then, so a passing dip doesn't lose the network for the rest of the download.
//!
//! Pure, like the scheduler: the engine feeds one [`Sample`] per network on its
//! tick (bytes so far, attempts in flight) and applies the [`Change`]s. No clock
//! of its own, no I/O.
//!
//! A network is benched only when all of this holds over a whole window:
//! - it was busy the whole time (attempts in flight), so a low rate isn't idleness;
//! - it kept delivering (bytes on most ticks): a silent or stalled network is
//!   the retry policy's business, and a burst before a stall isn't a speed;
//! - it was slow: under `floor` overall, or under `collapse` of its own best both
//!   overall *and* per stream (fewer streams near the end isn't a collapse);
//! - another busy network reaching the same server was much faster, overall *and*
//!   per stream. When every network is slow it's the server; a server that caps
//!   each connection shows the same per-stream speed on every network;
//! - the download is big enough to matter, and no speed limit shapes the rates.
//!
//! A benched network gets a check after `probe_after_ms`: one stream works for
//! `probe_ms`. Fast again (several times its throttled speed) puts it back to
//! work; still slow doubles the wait, up to `probe_max_ms`.

use std::collections::{HashMap, VecDeque};

use crate::scheduler::NetId;

/// Thresholds. Starting values: tuned for daily-quota phone plans without
/// catching the ordinary ups and downs of a mobile link.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThrottlePolicy {
    /// Off: no network is ever benched.
    pub enabled: bool,
    /// How long a network must stay slow, busy all along, before it's benched.
    pub window_ms: u64,
    /// Under this many bytes per second a busy network is slow outright.
    /// 64 kbps is 8 KB/s; 32 KB/s also catches 128 and 256 kbps plans.
    pub floor: f64,
    /// Or under this share of its own best speed (overall and per stream).
    pub collapse: f64,
    /// Another network must be at least this many times faster, overall and per stream.
    pub ratio: f64,
    /// ...and at least this fast overall, so two crawling networks never judge each other.
    pub healthy: f64,
    /// Downloads smaller than this are left alone: the end-of-download races cover them.
    pub min_total: u64,
    /// Span of the windows that set a network's best speed.
    pub best_window_ms: u64,
    /// First check after benching; each check that finds it still slow doubles
    /// the wait, up to `probe_max_ms`.
    pub probe_after_ms: u64,
    pub probe_max_ms: u64,
    /// How long a check measures once its stream has work.
    pub probe_ms: u64,
    /// A check that gets no work in this long gives up until the next one.
    pub probe_wait_ms: u64,
    /// A check puts the network back when one stream reaches this many times its
    /// throttled speed (and at least twice `floor`): the cap is gone.
    pub recover: f64,
}

impl Default for ThrottlePolicy {
    fn default() -> Self {
        ThrottlePolicy {
            enabled: true,
            window_ms: 15_000,
            floor: 32.0 * 1024.0,
            collapse: 0.1,
            ratio: 4.0,
            healthy: 128.0 * 1024.0,
            min_total: 8 * 1024 * 1024,
            best_window_ms: 5_000,
            probe_after_ms: 60_000,
            probe_max_ms: 300_000,
            probe_ms: 8_000,
            probe_wait_ms: 20_000,
            recover: 4.0,
        }
    }
}

/// Share of a window's ticks with attempts in flight for it to count as busy.
const BUSY: f64 = 0.9;
/// Share of a window's ticks that must bring bytes for a slow network to count
/// as throttled rather than stalled. Even 2 KB/s brings a TCP segment most ticks.
const MOVING: f64 = 0.5;

/// One network on one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub net: NetId,
    /// Lanes reaching the same server share a group; only they are compared
    /// (a slow mirror is the mirror's fault, not the network's).
    pub group: u32,
    /// Every byte this network has received in this download so far, races included.
    pub bytes: u64,
    /// Attempts in flight right now.
    pub attempts: u32,
    /// A speed limit the person set applies to it, so its speed says nothing.
    pub limited: bool,
    /// Gone (dead) or standing aside (its data allowance is used up).
    pub out: bool,
}

/// What the engine should do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    /// Stop giving `net` work and hand back what it holds. It ran at `rate`
    /// bytes per second; its best was `best` (0 when never measured).
    Bench { net: NetId, rate: f64, best: f64 },
    /// Let one stream of `net` work, to measure it.
    Probe { net: NetId },
    /// The check found it fast again (`rate` on one stream): back to work.
    Restore { net: NetId, rate: f64 },
    /// The check found it still slow (`Some(rate)`) or got no work to measure
    /// with (`None`): it rests until the next check.
    Rest { net: NetId, rate: Option<f64> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    at: u64,
    bytes: u64,
    attempts: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum State {
    #[default]
    Normal,
    Benched {
        rate: f64,
        next_probe_ms: u64,
        interval_ms: u64,
    },
    Probing {
        started_ms: u64,
        interval_ms: u64,
        rate: f64,
        /// When the check's stream first had work, and the bytes then.
        from: Option<(u64, u64)>,
    },
}

#[derive(Debug, Clone, Default)]
struct Track {
    history: VecDeque<Point>,
    best_total: f64,
    best_per: f64,
    state: State,
}

impl Track {
    /// Adds a point, keeping just enough history to measure `keep_ms` back.
    fn push(&mut self, p: Point, keep_ms: u64) {
        if self.history.back().is_some_and(|b| b.at >= p.at) {
            self.history.pop_back(); // same tick twice: keep the newer
        }
        self.history.push_back(p);
        let edge = p.at.saturating_sub(keep_ms);
        while self.history.len() >= 2 && self.history[1].at <= edge {
            self.history.pop_front();
        }
    }
}

/// Speeds over a span of history.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Measure {
    /// Bytes per second overall.
    total: f64,
    /// Bytes per second per attempt in flight.
    per: f64,
    /// Share of ticks with attempts in flight.
    busy: f64,
    /// Share of ticks that brought bytes.
    moving: f64,
    bytes: u64,
}

/// Speeds over the last `span_ms`, or None until the history covers that span.
fn measure(h: &VecDeque<Point>, now_ms: u64, span_ms: u64) -> Option<Measure> {
    let since = now_ms.checked_sub(span_ms)?;
    let start = h.iter().rposition(|p| p.at <= since)?;
    let (first, last) = (h[start], *h.back()?);
    let dt = last.at.saturating_sub(first.at);
    if dt == 0 {
        return None;
    }
    let (mut attempt_ms, mut busy, mut moving, mut ticks) = (0.0, 0u32, 0u32, 0u32);
    for k in start + 1..h.len() {
        let (a, b) = (h[k - 1], h[k]);
        attempt_ms += b.at.saturating_sub(a.at) as f64 * f64::from(a.attempts + b.attempts) / 2.0;
        ticks += 1;
        if b.attempts > 0 {
            busy += 1;
        }
        if b.bytes > a.bytes {
            moving += 1;
        }
    }
    let bytes = last.bytes.saturating_sub(first.bytes);
    Some(Measure {
        total: bytes as f64 * 1000.0 / dt as f64,
        per: if attempt_ms > 0.0 {
            bytes as f64 * 1000.0 / attempt_ms
        } else {
            0.0
        },
        busy: f64::from(busy) / f64::from(ticks.max(1)),
        moving: f64::from(moving) / f64::from(ticks.max(1)),
        bytes,
    })
}

/// Per-download throttle detector. Feed it every engine tick.
#[derive(Debug, Clone)]
pub struct Detector {
    policy: ThrottlePolicy,
    tracks: HashMap<NetId, Track>,
}

impl Detector {
    pub fn new(policy: ThrottlePolicy) -> Detector {
        Detector {
            policy,
            tracks: HashMap::new(),
        }
    }

    /// Benched or being checked: it gets no ordinary work.
    pub fn benched(&self, net: NetId) -> bool {
        self.tracks
            .get(&net)
            .is_some_and(|t| t.state != State::Normal)
    }

    /// One tick. `total` is the download's size (None when unknown); `shaped`
    /// means an overall or per-download speed limit is in force, which makes
    /// every rate artificial, so nothing is judged.
    pub fn tick(
        &mut self,
        now_ms: u64,
        samples: &[Sample],
        total: Option<u64>,
        shaped: bool,
    ) -> Vec<Change> {
        let p = self.policy;
        let mut out = Vec::new();
        if !p.enabled {
            return out;
        }
        let keep = p.window_ms.max(p.best_window_ms);
        for s in samples {
            let point = Point {
                at: now_ms,
                bytes: s.bytes,
                attempts: s.attempts,
            };
            let t = self.tracks.entry(s.net).or_default();
            t.push(point, keep);
            // Best speeds come from busy stretches of ordinary work.
            if t.state == State::Normal
                && !s.limited
                && !shaped
                && let Some(m) = measure(&t.history, now_ms, p.best_window_ms)
                && m.busy >= BUSY
            {
                t.best_total = t.best_total.max(m.total);
                t.best_per = t.best_per.max(m.per);
            }
            if let Some(c) = advance_check(t, s, now_ms, &p) {
                out.push(c);
            }
        }
        if shaped || total.is_none_or(|t| t < p.min_total) {
            return out;
        }
        let measured: Vec<(Sample, Measure)> = samples
            .iter()
            .filter_map(|s| {
                let t = self.tracks.get(&s.net)?;
                if t.state != State::Normal || s.out {
                    return None;
                }
                let m = measure(&t.history, now_ms, p.window_ms)?;
                (m.busy >= BUSY).then_some((*s, m))
            })
            .collect();
        for (s, m) in &measured {
            if s.limited || m.bytes == 0 || m.moving < MOVING {
                continue;
            }
            let Some(t) = self.tracks.get_mut(&s.net) else {
                continue;
            };
            let slow = m.total < p.floor
                || (t.best_total > 0.0
                    && m.total < p.collapse * t.best_total
                    && m.per < p.collapse * t.best_per);
            if !slow {
                continue;
            }
            let others_fast = measured.iter().any(|(o, om)| {
                o.net != s.net
                    && o.group == s.group
                    && om.total >= (p.ratio * m.total).max(p.healthy)
                    && om.per >= p.ratio * m.per
            });
            if others_fast {
                // The speed it has now: the last few seconds, since the window
                // may still hold a little of its earlier speed.
                let rate = measure(&t.history, now_ms, p.best_window_ms)
                    .map_or(m.total, |r| r.total.min(m.total));
                t.state = State::Benched {
                    rate,
                    next_probe_ms: now_ms + p.probe_after_ms,
                    interval_ms: p.probe_after_ms,
                };
                out.push(Change::Bench {
                    net: s.net,
                    rate,
                    best: t.best_total,
                });
            }
        }
        out
    }
}

/// Starts a benched network's check on time, and judges it once measured.
fn advance_check(t: &mut Track, s: &Sample, now_ms: u64, p: &ThrottlePolicy) -> Option<Change> {
    match t.state {
        State::Normal => None,
        State::Benched {
            rate,
            next_probe_ms,
            interval_ms,
        } => {
            if now_ms < next_probe_ms {
                return None;
            }
            if s.out {
                // Nothing to check while it's gone or out of allowance.
                t.state = State::Benched {
                    rate,
                    next_probe_ms: now_ms + interval_ms,
                    interval_ms,
                };
                return None;
            }
            t.state = State::Probing {
                started_ms: now_ms,
                interval_ms,
                rate,
                from: None,
            };
            Some(Change::Probe { net: s.net })
        }
        State::Probing {
            started_ms,
            interval_ms,
            rate,
            from,
        } => match from {
            None if s.attempts > 0 => {
                t.state = State::Probing {
                    started_ms,
                    interval_ms,
                    rate,
                    from: Some((now_ms, s.bytes)),
                };
                None
            }
            None if now_ms.saturating_sub(started_ms) >= p.probe_wait_ms => {
                t.state = State::Benched {
                    rate,
                    next_probe_ms: now_ms + interval_ms,
                    interval_ms,
                };
                Some(Change::Rest {
                    net: s.net,
                    rate: None,
                })
            }
            Some((t0, b0)) if now_ms.saturating_sub(t0) >= p.probe_ms => {
                let got = s.bytes.saturating_sub(b0) as f64 * 1000.0 / (now_ms - t0) as f64;
                if got >= (p.recover * rate).max(2.0 * p.floor) {
                    t.state = State::Normal;
                    Some(Change::Restore {
                        net: s.net,
                        rate: got,
                    })
                } else {
                    let next = interval_ms.saturating_mul(2).min(p.probe_max_ms);
                    t.state = State::Benched {
                        rate,
                        next_probe_ms: now_ms + next,
                        interval_ms: next,
                    };
                    Some(Change::Rest {
                        net: s.net,
                        rate: Some(got),
                    })
                }
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const KB: f64 = 1024.0;
    const MB: u64 = 1024 * 1024;
    const TICK: u64 = 500;

    /// A simulated network: a speed per tick and attempts in flight.
    #[derive(Clone, Copy)]
    struct Net {
        id: NetId,
        group: u32,
        rate: f64,
        attempts: u32,
        limited: bool,
        out: bool,
    }

    fn net(id: NetId, rate: f64, attempts: u32) -> Net {
        Net {
            id,
            group: 0,
            rate,
            attempts,
            limited: false,
            out: false,
        }
    }

    /// Drives a detector tick by tick; keeps each network's byte count.
    struct Sim {
        d: Detector,
        now: u64,
        bytes: HashMap<NetId, f64>,
        total: Option<u64>,
        shaped: bool,
        log: Vec<(u64, Change)>,
    }

    impl Sim {
        fn new() -> Sim {
            Sim::with(ThrottlePolicy::default())
        }
        fn with(p: ThrottlePolicy) -> Sim {
            Sim {
                d: Detector::new(p),
                now: 0,
                bytes: HashMap::new(),
                total: Some(4 * 1024 * MB),
                shaped: false,
                log: vec![],
            }
        }
        /// Runs `ms` with these networks; returns the changes in that stretch.
        fn run(&mut self, ms: u64, nets: &[Net]) -> Vec<Change> {
            let mut got = vec![];
            for _ in 0..ms / TICK {
                self.now += TICK;
                let samples: Vec<Sample> = nets
                    .iter()
                    .map(|n| {
                        let b = self.bytes.entry(n.id).or_default();
                        if n.attempts > 0 && !n.out {
                            *b += n.rate * TICK as f64 / 1000.0;
                        }
                        Sample {
                            net: n.id,
                            group: n.group,
                            bytes: *b as u64,
                            attempts: if n.out { 0 } else { n.attempts },
                            limited: n.limited,
                            out: n.out,
                        }
                    })
                    .collect();
                for c in self.d.tick(self.now, &samples, self.total, self.shaped) {
                    self.log.push((self.now, c));
                    got.push(c);
                }
            }
            got
        }
    }

    fn benched(changes: &[Change], id: NetId) -> bool {
        changes
            .iter()
            .any(|c| matches!(c, Change::Bench { net, .. } if *net == id))
    }

    #[test]
    fn a_phone_dropped_to_64_kbps_is_benched_while_wifi_keeps_going() {
        let mut s = Sim::new();
        let wifi = net(1, 4_000.0 * KB, 8);
        assert!(s.run(30_000, &[wifi, net(2, 1_500.0 * KB, 8)]).is_empty());
        let got = s.run(20_000, &[wifi, net(2, 8.0 * KB, 8)]);
        let Some(Change::Bench {
            net: id,
            rate,
            best,
        }) = got.first().copied()
        else {
            panic!("expected the phone benched, got {got:?}");
        };
        assert_eq!(id, 2);
        assert!((rate - 8.0 * KB).abs() < KB, "rate {rate}");
        assert!(best > 1_000.0 * KB, "best {best}");
        assert!(s.d.benched(2) && !s.d.benched(1));
        // Only after most of a window of crawling, not at the first slow ticks.
        let at = s.log.first().map(|(t, _)| *t).unwrap_or(0);
        assert!(at >= 30_000 + 12_000, "benched too early at {at}");
    }

    #[test]
    fn a_link_that_collapses_to_a_sliver_of_its_own_speed_is_benched() {
        let mut s = Sim::new();
        let eth = net(1, 3_000.0 * KB, 8);
        s.run(30_000, &[eth, net(2, 2_000.0 * KB, 8)]);
        // 150 KB/s is well above the floor, but 7.5% of what it did.
        let got = s.run(20_000, &[eth, net(2, 150.0 * KB, 8)]);
        assert!(benched(&got, 2), "{got:?}");
    }

    #[test]
    fn ordinary_ups_and_downs_never_bench() {
        let mut s = Sim::new();
        let wifi = net(1, 4_000.0 * KB, 8);
        // A mobile link swinging between 30% and 100% of its speed, with dips.
        for i in 0..40 {
            let rate = if i % 3 == 0 { 400.0 } else { 1_400.0 };
            assert!(s.run(5_000, &[wifi, net(2, rate * KB, 8)]).is_empty());
        }
    }

    #[test]
    fn a_short_stall_is_not_throttling() {
        let mut s = Sim::new();
        let wifi = net(1, 4_000.0 * KB, 8);
        s.run(30_000, &[wifi, net(2, 1_500.0 * KB, 8)]);
        // Eight silent seconds, then back to speed.
        assert!(s.run(8_000, &[wifi, net(2, 0.0, 8)]).is_empty());
        assert!(s.run(30_000, &[wifi, net(2, 1_500.0 * KB, 8)]).is_empty());
    }

    #[test]
    fn a_silent_network_is_left_to_the_retry_policy() {
        let mut s = Sim::new();
        let wifi = net(1, 4_000.0 * KB, 8);
        s.run(10_000, &[wifi, net(2, 1_500.0 * KB, 8)]);
        assert!(s.run(40_000, &[wifi, net(2, 0.0, 8)]).is_empty());
    }

    #[test]
    fn when_every_network_is_slow_it_is_the_server() {
        let mut s = Sim::new();
        let got = s.run(60_000, &[net(1, 12.0 * KB, 8), net(2, 9.0 * KB, 8)]);
        assert!(got.is_empty(), "{got:?}");
        // Both collapsing together (the server slowed down) isn't throttling either.
        let mut s = Sim::new();
        s.run(30_000, &[net(1, 3_000.0 * KB, 8), net(2, 2_000.0 * KB, 8)]);
        let got = s.run(30_000, &[net(1, 90.0 * KB, 8), net(2, 60.0 * KB, 8)]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn a_server_capping_each_connection_is_not_a_throttled_network() {
        // 10 KB/s per connection: one stream here, sixteen there. Overall speeds
        // differ 16x, but per stream they're the same: the server, not the network.
        let mut s = Sim::new();
        let got = s.run(60_000, &[net(1, 160.0 * KB, 16), net(2, 10.0 * KB, 1)]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn fewer_streams_near_the_end_is_not_a_collapse() {
        let mut s = Sim::new();
        s.run(30_000, &[net(1, 3_000.0 * KB, 8), net(2, 2_400.0 * KB, 8)]);
        // One attempt left on 2 (a twelfth of its old total, more per stream) while 1 races with 8.
        let got = s.run(30_000, &[net(1, 3_000.0 * KB, 8), net(2, 200.0 * KB, 1)]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn an_idle_network_is_not_judged() {
        let mut s = Sim::new();
        let got = s.run(60_000, &[net(1, 4_000.0 * KB, 8), net(2, 0.0, 0)]);
        assert!(got.is_empty());
        // Busy only part of the time (work comes and goes): not judged either.
        let mut s = Sim::new();
        let mut got = vec![];
        for i in 0..60 {
            let attempts = u32::from(i % 4 != 0);
            got.extend(s.run(
                1_000,
                &[net(1, 4_000.0 * KB, 8), net(2, 8.0 * KB, attempts)],
            ));
        }
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn tiny_or_unknown_size_downloads_are_left_alone() {
        for total in [Some(2 * MB), None] {
            let mut s = Sim::new();
            s.total = total;
            let got = s.run(60_000, &[net(1, 4_000.0 * KB, 8), net(2, 8.0 * KB, 8)]);
            assert!(got.is_empty(), "{total:?}: {got:?}");
        }
    }

    #[test]
    fn speed_limits_make_rates_meaningless() {
        // The person capped the phone at 20 KB/s: that's their choice, not throttling.
        let mut s = Sim::new();
        let mut phone = net(2, 20.0 * KB, 8);
        phone.limited = true;
        assert!(s.run(60_000, &[net(1, 4_000.0 * KB, 8), phone]).is_empty());
        // An overall or per-download limit shapes every network.
        let mut s = Sim::new();
        s.shaped = true;
        assert!(
            s.run(60_000, &[net(1, 4_000.0 * KB, 8), net(2, 8.0 * KB, 8)])
                .is_empty()
        );
    }

    #[test]
    fn only_lanes_to_the_same_server_are_compared() {
        // A slow mirror reached over Wi-Fi isn't Wi-Fi's fault.
        let mut s = Sim::new();
        let mut mirror = net(101, 8.0 * KB, 8);
        mirror.group = 1;
        let got = s.run(60_000, &[net(1, 4_000.0 * KB, 8), mirror]);
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn the_only_network_is_never_benched() {
        let mut s = Sim::new();
        assert!(s.run(60_000, &[net(1, 8.0 * KB, 8)]).is_empty());
        // Nor when the fast one is gone.
        let mut s = Sim::new();
        let mut gone = net(1, 4_000.0 * KB, 8);
        gone.out = true;
        assert!(s.run(60_000, &[gone, net(2, 8.0 * KB, 8)]).is_empty());
    }

    #[test]
    fn a_benched_network_is_checked_and_comes_back_when_fast_again() {
        let mut s = Sim::new();
        let wifi = net(1, 4_000.0 * KB, 8);
        s.run(30_000, &[wifi, net(2, 1_500.0 * KB, 8)]);
        assert!(benched(&s.run(20_000, &[wifi, net(2, 8.0 * KB, 8)]), 2));
        let benched_at = s.log.last().map(|(t, _)| *t).unwrap_or(0);
        // Resting: no attempts, until the check after a minute.
        let got = s.run(60_000, &[wifi, net(2, 0.0, 0)]);
        assert_eq!(got, vec![Change::Probe { net: 2 }]);
        let probe_at = s.log.last().map(|(t, _)| *t).unwrap_or(0);
        assert!(probe_at >= benched_at + 60_000 - TICK);
        // The quota reset: one stream at 900 KB/s for the check.
        let got = s.run(10_000, &[wifi, net(2, 900.0 * KB, 1)]);
        let Some(Change::Restore { net: id, rate }) = got.first().copied() else {
            panic!("expected a restore, got {got:?}");
        };
        assert_eq!(id, 2);
        assert!((rate - 900.0 * KB).abs() < 50.0 * KB, "rate {rate}");
        assert!(!s.d.benched(2));
        // Back at work and fast: never benched again.
        assert!(s.run(60_000, &[wifi, net(2, 1_500.0 * KB, 8)]).is_empty());
    }

    #[test]
    fn a_network_still_slow_at_its_check_waits_longer_each_time() {
        let mut s = Sim::new();
        let wifi = net(1, 4_000.0 * KB, 8);
        s.run(30_000, &[wifi, net(2, 1_500.0 * KB, 8)]);
        assert!(benched(&s.run(20_000, &[wifi, net(2, 8.0 * KB, 8)]), 2));
        // Every check finds the same 8 KB/s; the waits grow 60, 120, 240, 300, 300 s.
        let mut probes = vec![];
        for _ in 0..1_500_000 / TICK {
            let resting =
                s.d.benched(2) && !matches!(s.log.last(), Some((_, Change::Probe { .. })));
            let phone = if resting {
                net(2, 0.0, 0)
            } else {
                net(2, 8.0 * KB, 1)
            };
            for c in s.run(TICK, &[wifi, phone]) {
                match c {
                    Change::Probe { .. } => probes.push(s.now),
                    Change::Rest { rate: Some(r), .. } => assert!(r < 10.0 * KB),
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        let gaps: Vec<u64> = probes.windows(2).map(|w| (w[1] - w[0]) / 1000).collect();
        assert!(gaps.len() >= 4, "{probes:?}");
        // Each gap is the wait plus the 8 s check.
        assert_eq!(&gaps[..4], &[128, 248, 308, 308], "{gaps:?}");
        assert!(s.d.benched(2));
    }

    #[test]
    fn a_check_with_nothing_to_do_tries_again_later_at_the_same_pace() {
        let mut s = Sim::new();
        let wifi = net(1, 4_000.0 * KB, 8);
        s.run(30_000, &[wifi, net(2, 1_500.0 * KB, 8)]);
        assert!(benched(&s.run(20_000, &[wifi, net(2, 8.0 * KB, 8)]), 2));
        assert_eq!(
            s.run(60_000, &[wifi, net(2, 0.0, 0)]),
            vec![Change::Probe { net: 2 }]
        );
        // No work comes its way for the whole check.
        let got = s.run(20_000, &[wifi, net(2, 0.0, 0)]);
        assert_eq!(got, vec![Change::Rest { net: 2, rate: None }]);
        // Next check one minute later, not two.
        let got = s.run(60_000, &[wifi, net(2, 0.0, 0)]);
        assert_eq!(got, vec![Change::Probe { net: 2 }]);
    }

    #[test]
    fn no_check_while_the_network_is_gone() {
        let mut s = Sim::new();
        let wifi = net(1, 4_000.0 * KB, 8);
        s.run(30_000, &[wifi, net(2, 1_500.0 * KB, 8)]);
        assert!(benched(&s.run(20_000, &[wifi, net(2, 8.0 * KB, 8)]), 2));
        let mut gone = net(2, 0.0, 0);
        gone.out = true;
        assert!(s.run(200_000, &[wifi, gone]).is_empty());
    }

    #[test]
    fn switched_off_it_never_benches() {
        let mut s = Sim::with(ThrottlePolicy {
            enabled: false,
            ..ThrottlePolicy::default()
        });
        s.run(30_000, &[net(1, 4_000.0 * KB, 8), net(2, 1_500.0 * KB, 8)]);
        assert!(
            s.run(60_000, &[net(1, 4_000.0 * KB, 8), net(2, 8.0 * KB, 8)])
                .is_empty()
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(300))]

        /// Networks whose per-stream speeds stay within a factor of 3 of each
        /// other (one server limit, ordinary variance) and above the floor are
        /// never benched, whatever their stream counts and swings.
        #[test]
        fn comparable_networks_are_never_benched(
            base in 40.0f64..5_000.0,
            spread in proptest::collection::vec((0.5f64..1.0, 1u32..32), 2..4),
            swings in proptest::collection::vec(0.6f64..1.0, 20),
        ) {
            let mut s = Sim::new();
            for (i, swing) in swings.iter().enumerate() {
                let nets: Vec<Net> = spread
                    .iter()
                    .enumerate()
                    .map(|(k, (share, streams))| {
                        let per = base * KB * share * if (i + k) % 2 == 0 { *swing } else { 1.0 };
                        let total = (per * f64::from(*streams)).max(33.0 * KB);
                        net(k as NetId + 1, total, *streams)
                    })
                    .collect();
                let got = s.run(3_000, &nets);
                prop_assert!(got.is_empty(), "{:?}", got);
            }
        }
    }
}
