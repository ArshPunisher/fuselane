//! Ready by (B9.4): a time a download should be finished. Downloads with one
//! go first, earliest deadline first; one that would miss its deadline by
//! waiting for the schedule runs outside it; and each says On track or At risk
//! from the networks' recent speeds.

use std::sync::Arc;

use fuselane_core::Status;

use super::{Service, UiError, lock, not_found, store_error};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyState {
    OnTrack,
    AtRisk,
    Missed,
}

impl ReadyState {
    pub fn word(self) -> &'static str {
        match self {
            ReadyState::OnTrack => "on-track",
            ReadyState::AtRisk => "at-risk",
            ReadyState::Missed => "missed",
        }
    }
}

/// Without a speed to go on, a deadline closer than this counts as at risk.
const UNKNOWN_MARGIN_SECS: i64 = 3600;

/// Whether `remaining` bytes, starting `wait` seconds from `now`, at `rate`
/// bytes/s, finish by `deadline`. Unknown size or speed: at risk only when
/// the deadline is near.
pub fn state(now: i64, deadline: i64, remaining: Option<u64>, rate: f64, wait: i64) -> ReadyState {
    if now >= deadline {
        return ReadyState::Missed;
    }
    let need = match remaining {
        Some(r) if rate.is_finite() && rate > 0.0 => (r as f64 / rate).ceil() as i64,
        _ => {
            return if deadline - now <= UNKNOWN_MARGIN_SECS.max(wait) {
                ReadyState::AtRisk
            } else {
                ReadyState::OnTrack
            };
        }
    };
    if now + wait + need > deadline {
        ReadyState::AtRisk
    } else {
        ReadyState::OnTrack
    }
}

pub(super) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

impl Service {
    /// Sets (or clears) the time a download should be finished by.
    pub fn set_ready_by(self: &Arc<Self>, id: i64, at: Option<i64>) -> Result<(), UiError> {
        if let Some(t) = at
            && !(unix_now()..unix_now() + 366 * 86_400).contains(&t)
        {
            return Err(UiError::new(
                "bad-value",
                "Pick a time in the next year.",
                None,
            ));
        }
        self.store.set_ready_by(id, at).map_err(|e| match e {
            fuselane_core::StoreError::NotFound(id) => not_found(id),
            e => store_error(e),
        })?;
        self.publish_jobs();
        self.pump();
        Ok(())
    }

    /// The combined recent speed of every network, the best guess for a download
    /// that hasn't started.
    pub(super) fn expected_rate(&self) -> f64 {
        lock(&self.net_rates).values().sum()
    }

    /// Seconds until the schedule lets downloads run (0 when it does now).
    pub(super) fn schedule_wait(&self) -> i64 {
        if self.schedule_allows() {
            return 0;
        }
        let s = lock(&self.automation).schedule.clone();
        crate::automation::minutes_until_change(&s, self.now())
            .map_or(i64::MAX / 4, |m| i64::from(m) * 60)
    }

    /// How a download with a deadline is doing; None without one or once done.
    pub(super) fn ready_state(&self, job: &fuselane_core::Job) -> Option<ReadyState> {
        let running = lock(&self.running).contains_key(&job.id);
        self.ready_state_at(job, running)
    }

    /// `ready_state` for callers already holding the running list's lock.
    fn ready_state_at(&self, job: &fuselane_core::Job, running: bool) -> Option<ReadyState> {
        let deadline = job.ready_by?;
        if job.status == Status::Completed {
            return None;
        }
        let remaining = job.total.map(|t| t.saturating_sub(job.secured_bytes()));
        let wait = if running { 0 } else { self.schedule_wait() };
        Some(state(
            unix_now(),
            deadline,
            remaining,
            self.expected_rate(),
            wait,
        ))
    }

    /// Runs even outside the schedule: waiting for it would miss the deadline.
    /// Never takes the running list's lock (pump and the schedule hold it).
    pub(super) fn deadline_needs_now(&self, job: &fuselane_core::Job, running: bool) -> bool {
        matches!(
            self.ready_state_at(job, running),
            Some(ReadyState::AtRisk | ReadyState::Missed)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1_000_000;

    #[test]
    fn a_deadline_is_on_track_at_risk_or_missed() {
        // 600 MB at 1 MB/s takes 10 min.
        assert_eq!(
            state(0, 3600, Some(600 * MB), MB as f64, 0),
            ReadyState::OnTrack
        );
        assert_eq!(
            state(0, 500, Some(600 * MB), MB as f64, 0),
            ReadyState::AtRisk
        );
        // Waiting 55 min for the schedule would miss an hour's deadline.
        assert_eq!(
            state(0, 3600, Some(600 * MB), MB as f64, 3300),
            ReadyState::AtRisk
        );
        assert_eq!(state(4000, 3600, Some(1), 1.0, 0), ReadyState::Missed);
    }

    #[test]
    fn without_a_speed_only_a_near_deadline_is_at_risk() {
        assert_eq!(state(0, 7200, None, 0.0, 0), ReadyState::OnTrack);
        assert_eq!(state(0, 1800, Some(MB), 0.0, 0), ReadyState::AtRisk);
        // A long schedule wait counts too.
        assert_eq!(state(0, 7200, None, 0.0, 9000), ReadyState::AtRisk);
    }
}
