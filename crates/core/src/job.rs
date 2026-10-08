//! The job state machine as a pure transition table (ARCHITECTURE.md §4, L-29).

/// Where a job is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    Queued,
    Running,
    Paused,
    /// Stopped with an error; `resumable` says whether Resume can continue it.
    Failed {
        resumable: bool,
    },
    Completed,
    Cancelled,
}

/// Something that happens to a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Start,
    Pause,
    Resume,
    Fail { resumable: bool },
    Complete,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("can't {event:?} a job that is {from:?}")]
pub struct InvalidTransition {
    pub from: Status,
    pub event: Event,
}

impl Status {
    /// The only way a job's status changes. Every pair is listed: adding a status or
    /// event without deciding its transitions is a compile error (exhaustive match).
    pub fn apply(self, event: Event) -> Result<Status, InvalidTransition> {
        use Event as E;
        use Status as S;
        let next = match (self, event) {
            (S::Queued, E::Start) => S::Running,
            (S::Queued, E::Pause) => S::Paused,
            (S::Queued, E::Cancel) => S::Cancelled,
            (S::Running, E::Pause) => S::Paused,
            (S::Running, E::Fail { resumable }) => S::Failed { resumable },
            (S::Running, E::Complete) => S::Completed,
            (S::Running, E::Cancel) => S::Cancelled,
            (S::Paused, E::Resume) => S::Queued,
            (S::Paused, E::Start) => S::Running,
            (S::Paused, E::Cancel) => S::Cancelled,
            (S::Failed { resumable: true }, E::Resume) => S::Queued,
            (S::Failed { resumable: true }, E::Start) => S::Running,
            (S::Failed { .. }, E::Cancel) => S::Cancelled,
            (S::Queued, E::Resume | E::Fail { .. } | E::Complete)
            | (S::Running, E::Start | E::Resume)
            | (S::Paused, E::Pause | E::Fail { .. } | E::Complete)
            | (S::Failed { resumable: false }, E::Resume | E::Start)
            | (S::Failed { .. }, E::Pause | E::Fail { .. } | E::Complete)
            | (S::Completed, _)
            | (S::Cancelled, _) => return Err(InvalidTransition { from: self, event }),
        };
        Ok(next)
    }

    /// Stable text for the database.
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Queued => "queued",
            Status::Running => "running",
            Status::Paused => "paused",
            Status::Failed { resumable: true } => "failed",
            Status::Failed { resumable: false } => "failed-final",
            Status::Completed => "completed",
            Status::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        Some(match s {
            "queued" => Status::Queued,
            "running" => Status::Running,
            "paused" => Status::Paused,
            "failed" => Status::Failed { resumable: true },
            "failed-final" => Status::Failed { resumable: false },
            "completed" => Status::Completed,
            "cancelled" => Status::Cancelled,
            _ => return None,
        })
    }

    pub fn finished(self) -> bool {
        matches!(self, Status::Completed | Status::Cancelled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Status; 7] = [
        Status::Queued,
        Status::Running,
        Status::Paused,
        Status::Failed { resumable: true },
        Status::Failed { resumable: false },
        Status::Completed,
        Status::Cancelled,
    ];
    const EVENTS: [Event; 7] = [
        Event::Start,
        Event::Pause,
        Event::Resume,
        Event::Fail { resumable: true },
        Event::Fail { resumable: false },
        Event::Complete,
        Event::Cancel,
    ];

    #[test]
    fn finished_jobs_never_change() {
        for e in EVENTS {
            assert!(Status::Completed.apply(e).is_err());
            assert!(Status::Cancelled.apply(e).is_err());
        }
    }

    #[test]
    fn the_normal_life_of_a_job() {
        let s = Status::Queued.apply(Event::Start).unwrap();
        let s = s.apply(Event::Pause).unwrap();
        let s = s.apply(Event::Resume).unwrap();
        assert_eq!(s, Status::Queued);
        let s = s
            .apply(Event::Start)
            .unwrap()
            .apply(Event::Complete)
            .unwrap();
        assert_eq!(s, Status::Completed);
    }

    #[test]
    fn only_resumable_failures_can_resume() {
        assert_eq!(
            Status::Failed { resumable: true }.apply(Event::Resume),
            Ok(Status::Queued)
        );
        assert!(
            Status::Failed { resumable: false }
                .apply(Event::Resume)
                .is_err()
        );
        assert_eq!(
            Status::Failed { resumable: false }.apply(Event::Cancel),
            Ok(Status::Cancelled)
        );
    }

    #[test]
    fn every_pair_is_decided_and_text_round_trips() {
        let mut allowed = 0;
        for s in ALL {
            assert_eq!(Status::parse(s.as_str()), Some(s));
            for e in EVENTS {
                if let Ok(next) = s.apply(e) {
                    allowed += 1;
                    assert_ne!(next, s, "{s:?} --{e:?}--> itself is pointless");
                }
            }
        }
        assert_eq!(
            allowed, 15,
            "transition count changed: update the docs and this test deliberately"
        );
        assert_eq!(Status::parse("garbage"), None);
    }
}
