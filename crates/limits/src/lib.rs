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

/// Calendar period a data allowance applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day,
    Week,
    Month,
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
    }
}

/// The first day of the next period: when usage resets (local midnight).
pub fn next_reset(period: Period, d: Date) -> Date {
    match period {
        Period::Day => Date::from_days(d.days() + 1),
        Period::Week => Date::from_days(period_start(Period::Week, d).days() + 7),
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
