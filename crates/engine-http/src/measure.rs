//! Speed, peak and time-left, measured on a clock (ENGINE-DOWNLOAD.md §10, L-30–L-33).
//!
//! Bytes are counted into meters as they arrive; rates are read only on the
//! engine tick, over a rolling window with a 1 s minimum. Never from the gaps
//! between socket events (that's what made Plexo's early speeds jump around).

use std::collections::VecDeque;

pub const WINDOW_MS: u64 = 3_000;
pub const MIN_WINDOW_MS: u64 = 1_000;
pub const PEAK_WINDOW_MS: u64 = 5_000;

/// Rolling byte counter.
#[derive(Debug, Clone, Default)]
pub struct Meter {
    samples: VecDeque<(u64, u64)>, // (time ms, bytes)
    started_ms: Option<u64>,
}

impl Meter {
    pub fn add(&mut self, bytes: u64, now_ms: u64) {
        self.started_ms.get_or_insert(now_ms);
        match self.samples.back_mut() {
            Some((t, b)) if *t == now_ms => *b += bytes,
            _ => self.samples.push_back((now_ms, bytes)),
        }
        self.trim(now_ms);
    }

    fn trim(&mut self, now_ms: u64) {
        while self
            .samples
            .front()
            .is_some_and(|(t, _)| now_ms.saturating_sub(*t) > WINDOW_MS)
        {
            self.samples.pop_front();
        }
    }

    /// Bytes per second over the last `WINDOW_MS`, with at least `MIN_WINDOW_MS` as the
    /// denominator so a single burst can't read as a huge sustained speed (L-30).
    pub fn rate(&mut self, now_ms: u64) -> f64 {
        self.trim(now_ms);
        let bytes: u64 = self.samples.iter().map(|(_, b)| b).sum();
        let since_start = self.started_ms.map_or(0, |s| now_ms.saturating_sub(s));
        let window = since_start.clamp(MIN_WINDOW_MS, WINDOW_MS);
        bytes as f64 * 1000.0 / window as f64
    }

    /// Zero the meter, e.g. while the stream waits to retry (L-31).
    pub fn clear(&mut self) {
        self.samples.clear();
        self.started_ms = None;
    }
}

/// Average and peak over a whole download, computed from the same per-second bytes (L-33).
#[derive(Debug, Clone, Default)]
pub struct Totals {
    per_second: Vec<u64>,
}

impl Totals {
    /// Records the bytes of one active second (paused seconds aren't recorded).
    pub fn push_second(&mut self, bytes: u64) {
        self.per_second.push(bytes);
    }

    pub fn average(&self) -> Option<f64> {
        if self.per_second.is_empty() {
            return None;
        }
        Some(self.per_second.iter().sum::<u64>() as f64 / self.per_second.len() as f64)
    }

    /// Best 5-second average; `None` until 5 seconds have been held. Never below the average.
    pub fn peak(&self) -> Option<f64> {
        let w = (PEAK_WINDOW_MS / 1000) as usize;
        if self.per_second.len() < w {
            return None;
        }
        let best = self
            .per_second
            .windows(w)
            .map(|s| s.iter().sum::<u64>() as f64 / w as f64)
            .fold(0.0, f64::max);
        Some(best.max(self.average().unwrap_or(0.0)))
    }
}

/// Time left, smoothed asymmetrically (falls 30%/s, rises 10%/s), computed once in the core (L-32).
#[derive(Debug, Clone, Default)]
pub struct Eta {
    shown: Option<f64>,
}

impl Eta {
    /// Updates with `remaining` bytes at `rate` bytes/s after `dt_s` seconds; returns seconds left.
    pub fn update(&mut self, remaining: u64, rate: f64, dt_s: f64) -> Option<f64> {
        if remaining == 0 {
            self.shown = Some(0.0);
            return self.shown;
        }
        if !(rate.is_finite() && rate > 0.0) {
            return self.shown; // keep the last estimate while stalled
        }
        let target = remaining as f64 / rate;
        let next = match self.shown {
            None => target,
            // Count down between estimates, then move towards the new one.
            Some(prev) => {
                let counted = (prev - dt_s).max(0.0);
                if target <= counted * 0.5 {
                    target // much better news: accept immediately
                } else {
                    let k = if target < counted { 0.3 } else { 0.1 };
                    let k = 1.0 - (1.0_f64 - k).powf(dt_s.max(0.0));
                    counted + (target - counted) * k
                }
            }
        };
        self.shown = Some(next.max(0.0));
        self.shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn a_single_burst_is_not_a_huge_speed() {
        let mut m = Meter::default();
        m.add(1_000_000, 0);
        assert!(
            (m.rate(5) - 1_000_000.0).abs() < 1.0,
            "1 MB in 5 ms reads as 1 MB/s, not 200 MB/s"
        );
    }

    #[test]
    fn rate_uses_a_rolling_window() {
        let mut m = Meter::default();
        for t in (0..=6_000).step_by(100) {
            m.add(100_000, t);
        }
        let r = m.rate(6_000);
        assert!((r - 1_000_000.0).abs() < 40_000.0, "{r}");
        assert_eq!(m.rate(20_000), 0.0, "stale samples age out");
        m.clear();
        assert_eq!(m.rate(20_000), 0.0);
    }

    #[test]
    fn peak_waits_for_five_seconds_and_is_never_below_average() {
        let mut t = Totals::default();
        for b in [9, 9, 9, 9] {
            t.push_second(b);
        }
        assert_eq!(t.peak(), None);
        t.push_second(9);
        assert_eq!(t.peak(), Some(9.0));
        let mut spiky = Totals::default();
        for b in [100, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
            spiky.push_second(b);
        }
        assert!(spiky.peak().unwrap() >= spiky.average().unwrap());
    }

    #[test]
    fn eta_falls_fast_rises_slow_and_survives_stalls() {
        let mut e = Eta::default();
        assert_eq!(e.update(1_000, 10.0, 1.0), Some(100.0));
        let slower = e.update(1_000, 5.0, 1.0).unwrap();
        assert!(slower < 120.0, "rises slowly: {slower}");
        assert_eq!(
            e.update(1_000, 0.0, 1.0),
            Some(slower),
            "a stall keeps the estimate"
        );
        assert_eq!(e.update(1_000, f64::NAN, 1.0), Some(slower));
        let faster = e.update(10, 100.0, 1.0).unwrap();
        assert!(
            faster < 1.0,
            "big improvements are accepted at once: {faster}"
        );
        assert_eq!(e.update(0, 0.0, 1.0), Some(0.0));
    }

    proptest! {
        #[test]
        fn rates_are_finite_and_non_negative(events in proptest::collection::vec((0u64..10_000_000, 0u64..500), 0..300)) {
            let mut m = Meter::default();
            let mut now = 0;
            for (b, gap) in events {
                now += gap;
                m.add(b, now);
                let r = m.rate(now);
                prop_assert!(r.is_finite() && r >= 0.0);
            }
        }

        #[test]
        fn eta_is_never_negative_or_nan(steps in proptest::collection::vec((0u64..u64::MAX / 4, -1.0f64..1e9, 0.0f64..5.0), 1..100)) {
            let mut e = Eta::default();
            for (rem, rate, dt) in steps {
                if let Some(v) = e.update(rem, rate, dt) {
                    prop_assert!(v.is_finite() && v >= 0.0, "{v}");
                }
            }
        }
    }
}
