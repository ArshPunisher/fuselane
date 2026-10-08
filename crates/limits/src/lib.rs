//! Speed limits and data-usage accounting for Fuselane.
//!
//! Token buckets (global, per network, per job) and day/week/month usage
//! periods. Design: `docs/03-architecture/ENGINE-DOWNLOAD.md` §12. Pure: time
//! is always passed in, so behaviour is exact and testable.

/// A token bucket holding at most one second of tokens, starting empty, that may
/// go into debt: the debt is how long the caller must wait (no burst after idle).
#[derive(Debug, Clone)]
pub struct Bucket {
    rate: u64,
    tokens: f64,
    last_ms: u64,
}

impl Bucket {
    /// `rate` bytes per second; 0 means unlimited.
    pub fn new(rate: u64, now_ms: u64) -> Bucket {
        Bucket {
            rate,
            tokens: 0.0,
            last_ms: now_ms,
        }
    }

    pub fn rate(&self) -> u64 {
        self.rate
    }

    /// Changes the rate in place; existing debt carries over (scaled to the new rate).
    pub fn set_rate(&mut self, rate: u64, now_ms: u64) {
        self.refill(now_ms);
        if self.rate > 0 && rate > 0 && self.tokens < 0.0 {
            self.tokens *= rate as f64 / self.rate as f64;
        }
        if rate == 0 {
            self.tokens = 0.0;
        }
        self.rate = rate;
        self.tokens = self.tokens.min(rate as f64);
    }

    fn refill(&mut self, now_ms: u64) {
        let dt = now_ms.saturating_sub(self.last_ms) as f64 / 1000.0;
        self.last_ms = self.last_ms.max(now_ms);
        if self.rate > 0 {
            self.tokens = (self.tokens + dt * self.rate as f64).min(self.rate as f64);
        }
    }

    /// Takes `bytes`; returns how many ms to wait before reading more (0 = go on).
    pub fn take(&mut self, bytes: u64, now_ms: u64) -> u64 {
        if self.rate == 0 {
            return 0;
        }
        self.refill(now_ms);
        self.tokens -= bytes as f64;
        if self.tokens >= 0.0 {
            0
        } else {
            (-self.tokens / self.rate as f64 * 1000.0).ceil() as u64
        }
    }
}

/// The largest wait among several buckets (global + network + job): all limits apply.
pub fn take_all(buckets: &mut [&mut Bucket], bytes: u64, now_ms: u64) -> u64 {
    buckets
        .iter_mut()
        .map(|b| b.take(bytes, now_ms))
        .max()
        .unwrap_or(0)
}

/// Live speed limits shared by every download: one overall limit plus one per
/// network (by device name, so it applies across downloads). Rates can change
/// while downloads run; 0 means unlimited. Thread-safe.
#[derive(Debug)]
pub struct Limiter {
    epoch: std::time::Instant,
    state: std::sync::Mutex<LimiterState>,
}

#[derive(Debug, Default)]
struct LimiterState {
    global: Option<Bucket>,
    nets: std::collections::HashMap<String, Bucket>,
    /// Bytes per network since the last `drain_usage` (for data allowances).
    used: std::collections::HashMap<String, u64>,
    /// Networks that reached their data allowance: their streams stand aside.
    blocked: std::collections::HashSet<String>,
}

/// The limits currently set, for showing and saving.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LimitSettings {
    /// Bytes per second over all networks; 0 = unlimited.
    pub global: u64,
    /// Bytes per second per network device name (only limited networks listed).
    pub networks: Vec<(String, u64)>,
}

impl Default for Limiter {
    fn default() -> Self {
        Limiter {
            epoch: std::time::Instant::now(),
            state: std::sync::Mutex::default(),
        }
    }
}

impl Limiter {
    fn now_ms(&self) -> u64 {
        self.epoch.elapsed().as_millis() as u64
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, LimiterState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Replaces every limit at once.
    pub fn apply(&self, s: &LimitSettings) {
        self.apply_at(s, self.now_ms());
    }

    pub fn apply_at(&self, s: &LimitSettings, now_ms: u64) {
        let mut st = self.lock();
        match (&mut st.global, s.global) {
            (_, 0) => st.global = None,
            (Some(b), r) => b.set_rate(r, now_ms),
            (None, r) => st.global = Some(Bucket::new(r, now_ms)),
        }
        st.nets
            .retain(|name, _| s.networks.iter().any(|(n, r)| n == name && *r > 0));
        for (name, rate) in s.networks.iter().filter(|(_, r)| *r > 0) {
            match st.nets.get_mut(name) {
                Some(b) => b.set_rate(*rate, now_ms),
                None => {
                    st.nets.insert(name.clone(), Bucket::new(*rate, now_ms));
                }
            }
        }
    }

    /// Bytes each network received since the last call; the counters restart.
    pub fn drain_usage(&self) -> Vec<(String, u64)> {
        let mut v: Vec<(String, u64)> = self.lock().used.drain().collect();
        v.sort();
        v
    }

    /// Replaces the set of networks that must stand aside (allowance reached).
    pub fn set_blocked<I: IntoIterator<Item = String>>(&self, nets: I) {
        self.lock().blocked = nets.into_iter().collect();
    }

    pub fn blocked(&self, net: &str) -> bool {
        self.lock().blocked.contains(net)
    }

    pub fn settings(&self) -> LimitSettings {
        let st = self.lock();
        let mut networks: Vec<(String, u64)> =
            st.nets.iter().map(|(n, b)| (n.clone(), b.rate())).collect();
        networks.sort();
        LimitSettings {
            global: st.global.as_ref().map_or(0, Bucket::rate),
            networks,
        }
    }

    /// Records `bytes` received on network `net`; returns how long to wait.
    pub fn take(&self, net: &str, bytes: u64) -> std::time::Duration {
        std::time::Duration::from_millis(self.take_at(net, bytes, self.now_ms()))
    }

    pub fn take_at(&self, net: &str, bytes: u64, now_ms: u64) -> u64 {
        let mut st = self.lock();
        let st = &mut *st;
        *st.used.entry(net.to_string()).or_default() += bytes;
        let g = st.global.as_mut().map_or(0, |b| b.take(bytes, now_ms));
        let n = st.nets.get_mut(net).map_or(0, |b| b.take(bytes, now_ms));
        g.max(n)
    }
}

/// Calendar period a data allowance applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day,
    Week,
    Month,
    /// A month that starts on this day (1 to 28), like a phone plan's billing day.
    MonthFrom(u8),
}

/// A local calendar date (the caller converts the clock using the user's time zone).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    /// Days since 1970-01-01 (proleptic Gregorian, Howard Hinnant's algorithm).
    fn days(self) -> i64 {
        let (y, m) = if self.month <= 2 {
            (i64::from(self.year) - 1, i64::from(self.month) + 9)
        } else {
            (i64::from(self.year), i64::from(self.month) - 3)
        };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let doy = (153 * m + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    fn from_days(z: i64) -> Date {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Date { year, month, day }
    }

    /// Monday = 0 … Sunday = 6.
    fn weekday(self) -> i64 {
        (self.days() + 3).rem_euclid(7) // 1970-01-01 was a Thursday
    }

    pub fn is_valid(self) -> bool {
        (1..=12).contains(&self.month) && self.day >= 1 && Date::from_days(self.days()) == self
    }
}

/// The key of the period containing `d`: its first day (weeks start Monday).
pub fn period_start(period: Period, d: Date) -> Date {
    match period {
        Period::Day => d,
        Period::Week => Date::from_days(d.days() - d.weekday()),
        Period::Month => Date { day: 1, ..d },
        Period::MonthFrom(reset) => {
            let reset = reset.clamp(1, 28);
            if d.day >= reset {
                Date { day: reset, ..d }
            } else if d.month == 1 {
                Date {
                    year: d.year - 1,
                    month: 12,
                    day: reset,
                }
            } else {
                Date {
                    month: d.month - 1,
                    day: reset,
                    ..d
                }
            }
        }
    }
}

/// The first day of the next period: when usage resets (local midnight).
pub fn next_reset(period: Period, d: Date) -> Date {
    match period {
        Period::Day => Date::from_days(d.days() + 1),
        Period::Week => Date::from_days(period_start(Period::Week, d).days() + 7),
        Period::MonthFrom(reset) => {
            let start = period_start(Period::MonthFrom(reset), d);
            if start.month == 12 {
                Date {
                    year: start.year + 1,
                    month: 1,
                    day: start.day,
                }
            } else {
                Date {
                    month: start.month + 1,
                    ..start
                }
            }
        }
        Period::Month => {
            if d.month == 12 {
                Date {
                    year: d.year + 1,
                    month: 1,
                    day: 1,
                }
            } else {
                Date {
                    year: d.year,
                    month: d.month + 1,
                    day: 1,
                }
            }
        }
    }
}

/// Bytes used on one network in its current period; rolls over automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub period: Period,
    pub start: Date,
    pub bytes: u64,
}

impl Usage {
    pub fn new(period: Period, today: Date) -> Usage {
        Usage {
            period,
            start: period_start(period, today),
            bytes: 0,
        }
    }

    /// Adds bytes received `today`, resetting first if a new period began.
    pub fn add(&mut self, bytes: u64, today: Date) {
        self.roll(today);
        self.bytes = self.bytes.saturating_add(bytes);
    }

    /// Resets when `today` is in a later period. A clock that jumps backwards
    /// (time-zone change, manual clock) keeps the current count rather than resetting.
    pub fn roll(&mut self, today: Date) {
        let start = period_start(self.period, today);
        if start > self.start {
            *self = Usage {
                period: self.period,
                start,
                bytes: 0,
            };
        }
    }

    pub fn reached(&self, allowance: Option<u64>) -> bool {
        allowance.is_some_and(|a| self.bytes >= a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const fn d(year: i32, month: u8, day: u8) -> Date {
        Date { year, month, day }
    }

    #[test]
    fn a_billing_month_starts_on_its_reset_day() {
        let p = Period::MonthFrom(15);
        assert_eq!(period_start(p, d(2026, 10, 15)), d(2026, 10, 15));
        assert_eq!(period_start(p, d(2026, 10, 14)), d(2026, 9, 15));
        assert_eq!(
            period_start(p, d(2026, 1, 3)),
            d(2025, 12, 15),
            "across the new year"
        );
        assert_eq!(next_reset(p, d(2026, 10, 20)), d(2026, 11, 15));
        assert_eq!(next_reset(p, d(2026, 12, 20)), d(2027, 1, 15));
        // Days past 28 are clamped so every month has the day.
        assert_eq!(
            period_start(Period::MonthFrom(31), d(2026, 2, 28)),
            d(2026, 2, 28)
        );
        assert_eq!(
            period_start(Period::MonthFrom(0), d(2026, 2, 5)),
            d(2026, 2, 1)
        );
        // Usage rolls over on the reset day, not on the 1st.
        let mut u = Usage::new(p, d(2026, 10, 20));
        u.add(500, d(2026, 11, 1));
        assert_eq!(u.bytes, 500);
        u.add(1, d(2026, 11, 15));
        assert_eq!(u.bytes, 1);
    }

    #[test]
    fn limiter_counts_usage_per_network_and_blocks_on_request() {
        let l = Limiter::default();
        l.take_at("en0", 100, 0);
        l.take_at("en0", 50, 0);
        l.take_at("en7", 7, 0);
        assert_eq!(
            l.drain_usage(),
            vec![("en0".into(), 150), ("en7".into(), 7)]
        );
        assert!(l.drain_usage().is_empty(), "counters restart");
        assert!(!l.blocked("en7"));
        l.set_blocked(["en7".to_string()]);
        assert!(l.blocked("en7") && !l.blocked("en0"));
        l.set_blocked(Vec::new());
        assert!(!l.blocked("en7"));
    }

    #[test]
    fn limiter_applies_global_and_per_network_limits_and_changes_live() {
        let l = Limiter::default();
        assert_eq!(l.take_at("en0", 10_000_000, 0), 0, "unlimited by default");
        l.apply_at(
            &LimitSettings {
                global: 1000,
                networks: vec![("en5".into(), 100)],
            },
            0,
        );
        // 500 bytes against a 1000 B/s overall limit: half a second of debt.
        assert_eq!(l.take_at("en0", 500, 0), 500);
        // en5 has its own, tighter limit; the larger wait wins.
        assert_eq!(l.take_at("en5", 100, 0), 1000);
        assert_eq!(
            l.settings(),
            LimitSettings {
                global: 1000,
                networks: vec![("en5".into(), 100)]
            }
        );
        // Removing limits takes effect at once.
        l.apply_at(&LimitSettings::default(), 10);
        assert_eq!(l.take_at("en5", 1_000_000, 10), 0);
        assert_eq!(l.settings(), LimitSettings::default());
    }

    #[test]
    fn limited_throughput_converges_to_the_limit() {
        // Simulate a reader taking 16 KiB chunks and sleeping as told for 10 s.
        let l = Limiter::default();
        l.apply_at(
            &LimitSettings {
                global: 200_000,
                networks: vec![],
            },
            0,
        );
        let (mut now, mut got) = (0u64, 0u64);
        while now < 10_000 {
            got += 16_384;
            now += l.take_at("en0", 16_384, now).max(1);
        }
        let rate = got as f64 / (now as f64 / 1000.0);
        assert!((rate - 200_000.0).abs() / 200_000.0 < 0.05, "rate {rate}");
    }

    #[test]
    fn bucket_limits_to_its_rate_without_bursting_after_idle() {
        let mut b = Bucket::new(1_000, 0);
        assert_eq!(
            b.take(1_000, 0),
            1_000,
            "starts empty: 1 KB at 1 KB/s waits 1 s"
        );
        // After an hour idle the bucket holds at most one second.
        assert_eq!(b.take(1_000, 3_600_000), 0);
        assert_eq!(b.take(500, 3_600_000), 500);
    }

    #[test]
    fn unlimited_bucket_never_waits() {
        let mut b = Bucket::new(0, 0);
        assert_eq!(b.take(u64::MAX, 0), 0);
    }

    #[test]
    fn rate_change_keeps_debt_proportionally() {
        let mut b = Bucket::new(1_000, 0);
        assert_eq!(b.take(2_000, 0), 2_000);
        b.set_rate(2_000, 0);
        assert_eq!(
            b.take(0, 0),
            2_000,
            "debt of 2 s at the old rate stays 2 s at the new rate"
        );
        b.set_rate(0, 0);
        assert_eq!(b.take(10_000, 0), 0);
    }

    #[test]
    fn a_clock_going_backwards_doesnt_mint_tokens() {
        let mut b = Bucket::new(1_000, 10_000);
        b.take(1_000, 10_000);
        assert!(b.take(0, 5_000) > 0, "earlier timestamp must not refill");
    }

    #[test]
    fn the_strictest_limit_wins() {
        let (mut g, mut n) = (Bucket::new(10_000, 0), Bucket::new(1_000, 0));
        assert_eq!(take_all(&mut [&mut g, &mut n], 1_000, 0), 1_000);
    }

    #[test]
    fn periods_and_resets() {
        // 2026-10-08 is a Thursday.
        assert_eq!(period_start(Period::Week, d(2026, 10, 8)), d(2026, 10, 5));
        assert_eq!(period_start(Period::Week, d(2026, 10, 5)), d(2026, 10, 5));
        assert_eq!(period_start(Period::Week, d(2026, 10, 11)), d(2026, 10, 5));
        assert_eq!(next_reset(Period::Week, d(2026, 10, 11)), d(2026, 10, 12));
        assert_eq!(next_reset(Period::Day, d(2026, 12, 31)), d(2027, 1, 1));
        assert_eq!(next_reset(Period::Month, d(2026, 12, 15)), d(2027, 1, 1));
        assert_eq!(
            next_reset(Period::Day, d(2028, 2, 28)),
            d(2028, 2, 29),
            "leap year"
        );
        assert_eq!(
            next_reset(Period::Day, d(2100, 2, 28)),
            d(2100, 3, 1),
            "2100 isn't a leap year"
        );
        assert_eq!(
            period_start(Period::Week, d(2027, 1, 1)),
            d(2026, 12, 28),
            "weeks cross years"
        );
    }

    #[test]
    fn usage_rolls_over_but_not_backwards() {
        let mut u = Usage::new(Period::Day, d(2026, 10, 8));
        u.add(5, d(2026, 10, 8));
        u.add(5, d(2026, 10, 8));
        assert_eq!(u.bytes, 10);
        u.add(1, d(2026, 10, 9));
        assert_eq!(u.bytes, 1, "new day resets");
        u.add(1, d(2026, 10, 8));
        assert_eq!(u.bytes, 2, "a clock moved back doesn't reset or rewind");
        assert!(u.reached(Some(2)) && !u.reached(Some(3)) && !u.reached(None));
        u.add(u64::MAX, d(2026, 10, 9));
        assert_eq!(u.bytes, u64::MAX, "saturates");
    }

    #[test]
    fn date_validation() {
        assert!(d(2026, 2, 28).is_valid());
        assert!(!d(2026, 2, 29).is_valid());
        assert!(!d(2026, 13, 1).is_valid());
        assert!(!d(2026, 4, 31).is_valid());
        assert!(!d(2026, 1, 0).is_valid());
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(5000))]

        #[test]
        fn calendar_round_trips(days in -200_000i64..200_000) {
            let date = Date::from_days(days);
            prop_assert!(date.is_valid());
            prop_assert_eq!(date.days(), days);
        }

        #[test]
        fn period_start_is_in_the_period_and_reset_is_after(days in -100_000i64..100_000, p in prop_oneof![Just(Period::Day), Just(Period::Week), Just(Period::Month)]) {
            let date = Date::from_days(days);
            let s = period_start(p, date);
            let r = next_reset(p, date);
            prop_assert!(s <= date && date < r);
            prop_assert_eq!(period_start(p, s), s);
            prop_assert_eq!(period_start(p, r), r);
            if p == Period::Week { prop_assert_eq!(s.weekday(), 0); }
        }

        /// Over any schedule of takes, throughput never exceeds the rate by more
        /// than one second's worth plus the final request.
        #[test]
        fn bucket_enforces_its_rate(rate in 1u64..100_000_000, takes in proptest::collection::vec((1u64..10_000_000, 0u64..2_000), 1..200)) {
            let mut b = Bucket::new(rate, 0);
            let mut now = 0u64;
            let mut sent = 0u128;
            for (bytes, gap) in takes {
                now += gap;
                let wait = b.take(bytes, now);
                now += wait;
                sent += u128::from(bytes);
            }
            let allowed = u128::from(rate) * u128::from(now) / 1000 + u128::from(rate) + 1;
            prop_assert!(sent <= allowed, "sent {} > allowed {}", sent, allowed);
        }
    }
}
