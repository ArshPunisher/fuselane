//! How many streams each network runs (ENGINE-DOWNLOAD.md §5, L-18, L-26, L-27).
//!
//! Deliberately simple and pure (no clock of its own, no speed comparisons),
//! because clever measured controllers failed on noisy mobile links:
//! - start at the user's pick (fixed) or `START` (auto);
//! - auto doubles while **every** stream has received data, up to `MAX`;
//! - a server refusing some streams *while serving others* lowers that network's
//!   ceiling to what it accepted; one stream comes back per `RECOVER_MS` without
//!   a refusal;
//! - a disk that stays behind halves every network's streams; keeping up at the
//!   cap for `DISK_RECOVER_MS` doubles it back.

use std::collections::HashMap;

use crate::scheduler::NetId;

pub const START: u32 = 8;
pub const MAX: u32 = 32;
pub const RECOVER_MS: u64 = 60_000;
pub const DISK_PATIENCE_MS: u64 = 500;
pub const DISK_RECOVER_MS: u64 = 10_000;

/// One network's streams since the last tick.
#[derive(Debug, Clone, Copy)]
pub struct NetTick {
    pub id: NetId,
    /// Live streams (not counting retiring ones).
    pub streams: u32,
    /// Streams that have received at least one byte, ever.
    pub answered: u32,
    /// Streams the server refused (403/429/503 or accepted-but-silent) since the last tick.
    pub refused: u32,
    /// Streams that received data since the last tick.
    pub served: u32,
}

/// What the disk writer reported for this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disk {
    KeepingUp,
    Behind,
    /// Nothing to judge (no writes, or writes stopped entirely).
    Unknown,
}

/// A change the transfer should make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Add { net: NetId, count: u32 },
    Retire { net: NetId, count: u32 },
}

#[derive(Debug, Clone, Copy)]
struct Ceiling {
    value: u32,
    since_ms: u64,
}

/// Per-download controller. Kept across pause/resume.
#[derive(Debug, Clone)]
pub struct Controller {
    max: u32,
    grows: bool,
    ceilings: HashMap<NetId, Ceiling>,
    disk_cap: Option<u32>,
    disk_state: (Disk, u64),
}

impl Controller {
    /// `pick = None` is Auto; `Some(n)` keeps exactly n (except refusals and disk).
    pub fn new(pick: Option<u32>) -> Controller {
        match pick {
            Some(n) => Controller {
                max: n.clamp(1, MAX),
                grows: false,
                ceilings: HashMap::new(),
                disk_cap: None,
                disk_state: (Disk::Unknown, 0),
            },
            None => Controller {
                max: MAX,
                grows: true,
                ceilings: HashMap::new(),
                disk_cap: None,
                disk_state: (Disk::Unknown, 0),
            },
        }
    }

    /// Streams a network starts with.
    pub fn starting(&self, net: NetId) -> u32 {
        let base = if self.grows { START } else { self.max };
        base.min(self.limit(net))
    }

    /// Most streams `net` may run right now (always ≥ 1).
    pub fn limit(&self, net: NetId) -> u32 {
        let server = self.ceilings.get(&net).map_or(self.max, |c| c.value);
        server
            .min(self.disk_cap.unwrap_or(u32::MAX))
            .min(self.max)
            .max(1)
    }

    pub fn disk_limited(&self) -> bool {
        self.disk_cap.is_some()
    }

    /// One controller tick. `spare_work` = pending blocks that more streams could take.
    pub fn tick(
        &mut self,
        now_ms: u64,
        nets: &[NetTick],
        spare_work: u32,
        disk: Disk,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        self.judge_disk(now_ms, nets, disk);

        let mut spare = spare_work;
        for n in nets {
            // Refusals: lower the ceiling only when the server serves others at the same time.
            if n.refused > 0 && n.served > 0 {
                let accepted = n.streams.saturating_sub(n.refused).max(1);
                let value = self
                    .ceilings
                    .get(&n.id)
                    .map_or(accepted, |c| c.value.min(accepted));
                self.ceilings.insert(
                    n.id,
                    Ceiling {
                        value,
                        since_ms: now_ms,
                    },
                );
            } else if let Some(c) = self.ceilings.get_mut(&n.id)
                && now_ms.saturating_sub(c.since_ms) >= RECOVER_MS
            {
                c.value += 1;
                c.since_ms = now_ms;
                if c.value >= self.max {
                    self.ceilings.remove(&n.id);
                }
            }

            let limit = self.limit(n.id);
            if n.streams > limit {
                actions.push(Action::Retire {
                    net: n.id,
                    count: n.streams - limit,
                });
                continue;
            }
            let refusing = n.refused > 0;
            let all_answered = n.streams > 0 && n.answered >= n.streams;
            if self.grows && !refusing && all_answered && self.disk_state.0 != Disk::Behind {
                let add = n.streams.min(limit - n.streams).min(spare);
                if add > 0 {
                    spare -= add;
                    actions.push(Action::Add {
                        net: n.id,
                        count: add,
                    });
                }
            }
        }
        actions
    }

    fn judge_disk(&mut self, now_ms: u64, nets: &[NetTick], disk: Disk) {
        if disk != self.disk_state.0 {
            self.disk_state = (disk, now_ms);
        }
        if !self.grows {
            return; // the disk is judged in Auto only; a user's pick is kept
        }
        let held_for = now_ms.saturating_sub(self.disk_state.1);
        let busiest = nets.iter().map(|n| n.streams).max().unwrap_or(0);
        match self.disk_state.0 {
            Disk::Behind if held_for >= DISK_PATIENCE_MS && busiest > 1 => {
                let cap = busiest.div_ceil(2).max(1);
                if self.disk_cap.is_none_or(|c| cap < c) {
                    self.disk_cap = Some(cap);
                    self.disk_state.1 = now_ms; // judge the new cap afresh
                }
            }
            Disk::KeepingUp if held_for >= DISK_RECOVER_MS => {
                if let Some(cap) = self.disk_cap {
                    let at_cap = nets.iter().any(|n| n.streams >= cap);
                    if at_cap {
                        let next = cap * 2;
                        self.disk_cap = if next >= self.max { None } else { Some(next) };
                        self.disk_state.1 = now_ms;
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn net(id: NetId, streams: u32, answered: u32, refused: u32, served: u32) -> NetTick {
        NetTick {
            id,
            streams,
            answered,
            refused,
            served,
        }
    }

    #[test]
    fn auto_doubles_while_everyone_is_served() {
        let mut c = Controller::new(None);
        assert_eq!(c.starting(1), 8);
        assert_eq!(
            c.tick(0, &[net(1, 8, 8, 0, 8)], 100, Disk::KeepingUp),
            vec![Action::Add { net: 1, count: 8 }]
        );
        assert_eq!(
            c.tick(1, &[net(1, 16, 16, 0, 16)], 100, Disk::KeepingUp),
            vec![Action::Add { net: 1, count: 16 }]
        );
        assert_eq!(
            c.tick(2, &[net(1, 32, 32, 0, 32)], 100, Disk::KeepingUp),
            vec![]
        );
    }

    #[test]
    fn no_growth_until_every_stream_answered_or_without_spare_work() {
        let mut c = Controller::new(None);
        assert_eq!(
            c.tick(0, &[net(1, 8, 7, 0, 7)], 100, Disk::KeepingUp),
            vec![]
        );
        assert_eq!(
            c.tick(0, &[net(1, 8, 8, 0, 8)], 3, Disk::KeepingUp),
            vec![Action::Add { net: 1, count: 3 }]
        );
        assert_eq!(c.tick(0, &[net(1, 8, 8, 0, 8)], 0, Disk::KeepingUp), vec![]);
    }

    #[test]
    fn spare_work_is_shared_across_networks() {
        let mut c = Controller::new(None);
        let a = c.tick(
            0,
            &[net(1, 8, 8, 0, 8), net(2, 8, 8, 0, 8)],
            10,
            Disk::KeepingUp,
        );
        let total: u32 = a
            .iter()
            .map(|x| {
                if let Action::Add { count, .. } = x {
                    *count
                } else {
                    0
                }
            })
            .sum();
        assert_eq!(total, 10);
    }

    #[test]
    fn refusals_lower_the_ceiling_then_recover_one_a_minute() {
        let mut c = Controller::new(None);
        let a = c.tick(0, &[net(1, 16, 16, 4, 12)], 100, Disk::KeepingUp);
        assert_eq!(a, vec![Action::Retire { net: 1, count: 4 }]);
        assert_eq!(c.limit(1), 12);
        assert_eq!(
            c.tick(30_000, &[net(1, 12, 12, 0, 12)], 100, Disk::KeepingUp),
            vec![]
        );
        c.tick(60_000, &[net(1, 12, 12, 0, 12)], 100, Disk::KeepingUp);
        assert_eq!(c.limit(1), 13);
        assert_eq!(c.limit(2), MAX, "other networks are unaffected");
    }

    #[test]
    fn refusal_while_serving_nothing_is_not_a_connection_limit() {
        let mut c = Controller::new(None);
        assert_eq!(
            c.tick(0, &[net(1, 8, 0, 8, 0)], 100, Disk::KeepingUp),
            vec![]
        );
        assert_eq!(c.limit(1), MAX);
    }

    #[test]
    fn a_slow_disk_halves_streams_and_recovers() {
        let mut c = Controller::new(None);
        c.tick(0, &[net(1, 32, 32, 0, 32)], 100, Disk::Behind);
        assert!(!c.disk_limited(), "not before the patience window");
        let a = c.tick(600, &[net(1, 32, 32, 0, 32)], 100, Disk::Behind);
        assert_eq!(c.limit(1), 16);
        assert!(a.contains(&Action::Retire { net: 1, count: 16 }));
        c.tick(700, &[net(1, 16, 16, 0, 16)], 100, Disk::KeepingUp);
        c.tick(10_800, &[net(1, 16, 16, 0, 16)], 100, Disk::KeepingUp);
        assert!(!c.disk_limited(), "32 ≥ MAX clears the cap");
    }

    #[test]
    fn a_users_pick_is_kept_except_for_refusals() {
        let mut c = Controller::new(Some(4));
        assert_eq!(c.starting(1), 4);
        assert_eq!(c.tick(0, &[net(1, 4, 4, 0, 4)], 100, Disk::Behind), vec![]);
        assert_eq!(
            c.tick(900, &[net(1, 4, 4, 0, 4)], 100, Disk::Behind),
            vec![],
            "disk ignored for a user pick"
        );
        assert_eq!(
            c.tick(1_000, &[net(1, 4, 4, 2, 2)], 100, Disk::KeepingUp),
            vec![Action::Retire { net: 1, count: 2 }]
        );
        assert_eq!(Controller::new(Some(0)).starting(1), 1);
        assert_eq!(Controller::new(Some(999)).starting(1), MAX);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(3000))]

        /// Over any sequence of ticks: limits stay in [1, max], adds never exceed spare work,
        /// never exceed the limit, and nothing is added to a network that just refused.
        #[test]
        fn controller_invariants(
            pick in proptest::option::of(0u32..40),
            ticks in proptest::collection::vec((
                proptest::collection::vec((0u32..3, 0u32..40, 0u32..40, 0u32..10, 0u32..40), 1..4),
                0u32..64,
                prop_oneof![Just(Disk::KeepingUp), Just(Disk::Behind), Just(Disk::Unknown)],
                0u64..20_000,
            ), 1..60),
        ) {
            let mut c = Controller::new(pick);
            let max = pick.map_or(MAX, |p| p.clamp(1, MAX));
            let mut now = 0u64;
            for (nets, spare, disk, dt) in ticks {
                now += dt;
                let mut seen = std::collections::HashSet::new();
                let nets: Vec<NetTick> = nets.into_iter()
                    .filter(|(id, ..)| seen.insert(*id))
                    .map(|(id, s, a, r, sv)| NetTick { id, streams: s, answered: a.min(s), refused: r.min(s), served: sv.min(s) })
                    .collect();
                let actions = c.tick(now, &nets, spare, disk);
                let mut added = 0;
                for a in &actions {
                    match *a {
                        Action::Add { net, count } => {
                            added += count;
                            let n = nets.iter().find(|n| n.id == net).unwrap();
                            prop_assert!(n.refused == 0, "added to a refusing network");
                            prop_assert!(n.streams + count <= c.limit(net));
                        }
                        Action::Retire { net, count } => {
                            let n = nets.iter().find(|n| n.id == net).unwrap();
                            prop_assert!(count <= n.streams);
                            prop_assert_eq!(n.streams - count, c.limit(net));
                        }
                    }
                }
                prop_assert!(added <= spare);
                for n in &nets {
                    let l = c.limit(n.id);
                    prop_assert!((1..=max).contains(&l));
                }
            }
        }
    }
}
