//! Who fetches what next (ENGINE-DOWNLOAD.md §4, L-24, L-25, L-28).
//!
//! A pure function of a snapshot: no clock, no I/O, so every rule can be
//! property-tested. Streams pull work; nothing is pushed.
//!
//! 1. **Primary:** the lowest block nobody is fetching. A block whose last
//!    attempt on *my* network delivered nothing goes to another network first,
//!    as long as one has an idle stream ("avoid"), but is never stranded.
//! 2. **Hedge:** when nothing is left to start (and the disk keeps up), race the
//!    slowest in-flight block from its frontier, preferring another network.
//!    The first attempt to finish wins; the bytes are identical, written at the
//!    same offsets, so racing is always safe.

/// Network identifier (stable id from `netif`).
pub type NetId = u32;
/// Stream identifier.
pub type StreamId = u32;

/// Racing rules. Values are starting points (Plexo's tuned ones, for reference).
#[derive(Debug, Clone, Copy)]
pub struct HedgePolicy {
    /// Every attempt on a block must have run this long before it can be raced.
    pub after_ms: u64,
    /// At most this many extra attempts per block.
    pub max_per_block: u8,
    /// Fixed cost of starting an attempt (connect + first byte), in ms.
    pub startup_ms: u64,
}

impl Default for HedgePolicy {
    fn default() -> Self {
        HedgePolicy {
            after_ms: 2_000,
            max_per_block: 2,
            startup_ms: 1_000,
        }
    }
}

/// One attempt at a block.
#[derive(Debug, Clone)]
pub struct Attempt {
    pub stream: StreamId,
    pub network: NetId,
    pub started_ms: u64,
    /// Bytes into the block this attempt has written.
    pub position: u64,
    /// Recent rate in bytes/s (0 = silent).
    pub rate: f64,
}

/// A block as the scheduler sees it.
#[derive(Debug, Clone)]
pub struct Block {
    pub len: u64,
    /// Bytes from the block's start that are on disk for sure.
    pub secured: u64,
    pub attempts: Vec<Attempt>,
    /// The last attempt on this network delivered nothing: prefer another network.
    pub avoid: Option<NetId>,
    /// Hedges started so far.
    pub hedges: u8,
}

impl Block {
    pub fn complete(&self) -> bool {
        self.secured >= self.len
    }
    /// Furthest any attempt (or the disk) has got.
    pub fn frontier(&self) -> u64 {
        self.attempts
            .iter()
            .map(|a| a.position)
            .fold(self.secured, u64::max)
            .min(self.len)
    }
    fn pending(&self) -> bool {
        !self.complete() && self.attempts.is_empty()
    }
}

/// The stream asking for work.
#[derive(Debug, Clone, Copy)]
pub struct Requester {
    pub stream: StreamId,
    pub network: NetId,
    /// Its rate on its last block, if any.
    pub rate: Option<f64>,
}

/// Everything a decision depends on.
#[derive(Debug, Clone)]
pub struct Snapshot<'a> {
    pub now_ms: u64,
    pub blocks: &'a [Block],
    /// Networks (other than the requester's) that have an idle stream right now.
    pub idle_networks: &'a [NetId],
    /// The disk is behind: don't add hedges (L-27).
    pub disk_behind: bool,
    pub policy: HedgePolicy,
}

/// A decision: fetch `block` starting `from` bytes into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Work {
    pub block: usize,
    pub from: u64,
    pub hedge: bool,
}

/// Picks work for `me`, or `None` when there is nothing useful to do.
pub fn pick_work(s: &Snapshot<'_>, me: Requester) -> Option<Work> {
    if s.blocks
        .iter()
        .any(|b| b.attempts.iter().any(|a| a.stream == me.stream))
    {
        return None; // a stream holds at most one attempt
    }
    primary(s, me).or_else(|| hedge(s, me))
}

fn primary(s: &Snapshot<'_>, me: Requester) -> Option<Work> {
    let someone_else_idle = s.idle_networks.iter().any(|n| *n != me.network);
    let mut fallback = None;
    for (i, b) in s.blocks.iter().enumerate() {
        if !b.pending() {
            continue;
        }
        if b.avoid == Some(me.network) && someone_else_idle {
            fallback.get_or_insert(i);
            continue;
        }
        return Some(Work {
            block: i,
            from: b.secured,
            hedge: false,
        });
    }
    // Never strand a block: if only avoided blocks are left and nobody else
    // takes them now, take one anyway... unless another network is idle and will.
    if !someone_else_idle {
        return fallback.map(|i| Work {
            block: i,
            from: s.blocks[i].secured,
            hedge: false,
        });
    }
    None
}

fn hedge(s: &Snapshot<'_>, me: Requester) -> Option<Work> {
    if s.disk_behind || s.blocks.iter().any(Block::pending) {
        return None;
    }
    let other_net_idle = s.idle_networks.iter().any(|n| *n != me.network);
    let mut best: Option<(f64, usize)> = None;
    for (i, b) in s.blocks.iter().enumerate() {
        if b.complete()
            || b.attempts.is_empty()
            || b.hedges >= s.policy.max_per_block
            || b.avoid == Some(me.network)
        {
            continue;
        }
        if b.attempts
            .iter()
            .any(|a| s.now_ms.saturating_sub(a.started_ms) < s.policy.after_ms)
        {
            continue;
        }
        // Prefer racing from another network: if mine already holds it and some
        // other network is idle, leave it to them.
        if b.attempts.iter().any(|a| a.network == me.network) && other_net_idle {
            continue;
        }
        let remaining = (b.len - b.frontier()) as f64;
        if remaining <= 0.0 {
            continue;
        }
        let fastest = b.attempts.iter().map(|a| a.rate).fold(0.0_f64, f64::max);
        let eta_ms = if fastest > 0.0 {
            remaining / fastest * 1000.0
        } else {
            f64::INFINITY
        };
        let worth = match me.rate {
            Some(r) if r > 0.0 => {
                eta_ms >= 2.0 * (s.policy.startup_ms as f64 + remaining / r * 1000.0)
            }
            _ => eta_ms >= s.policy.after_ms as f64,
        };
        if worth && best.is_none_or(|(e, _)| eta_ms > e) {
            best = Some((eta_ms, i));
        }
    }
    best.map(|(_, i)| Work {
        block: i,
        from: s.blocks[i].frontier(),
        hedge: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const MB: u64 = 1024 * 1024;

    fn idle(len: u64) -> Block {
        Block {
            len,
            secured: 0,
            attempts: vec![],
            avoid: None,
            hedges: 0,
        }
    }
    fn busy(len: u64, net: NetId, stream: StreamId, started: u64, pos: u64, rate: f64) -> Block {
        Block {
            len,
            secured: 0,
            attempts: vec![Attempt {
                stream,
                network: net,
                started_ms: started,
                position: pos,
                rate,
            }],
            avoid: None,
            hedges: 0,
        }
    }
    fn snap(blocks: &[Block]) -> Snapshot<'_> {
        Snapshot {
            now_ms: 10_000,
            blocks,
            idle_networks: &[],
            disk_behind: false,
            policy: HedgePolicy::default(),
        }
    }
    const ME: Requester = Requester {
        stream: 99,
        network: 2,
        rate: Some(4.0 * MB as f64),
    };

    #[test]
    fn takes_the_lowest_pending_block() {
        let blocks = [busy(MB, 1, 1, 0, 10, 1.0), idle(MB), idle(MB)];
        assert_eq!(
            pick_work(&snap(&blocks), ME),
            Some(Work {
                block: 1,
                from: 0,
                hedge: false
            })
        );
    }

    #[test]
    fn resumes_from_secured_bytes() {
        let mut b = idle(MB);
        b.secured = 300;
        assert_eq!(
            pick_work(&snap(&[b]), ME),
            Some(Work {
                block: 0,
                from: 300,
                hedge: false
            })
        );
    }

    #[test]
    fn avoided_block_goes_to_another_network_but_is_never_stranded() {
        let mut b = idle(MB);
        b.avoid = Some(ME.network);
        let blocks = [b];
        let mut s = snap(&blocks);
        s.idle_networks = &[7];
        assert_eq!(
            pick_work(&s, ME),
            None,
            "another network is idle and should take it"
        );
        s.idle_networks = &[];
        assert_eq!(
            pick_work(&s, ME).map(|w| w.block),
            Some(0),
            "nobody else: take it anyway"
        );
    }

    #[test]
    fn races_a_crawling_block_only_when_nothing_is_pending() {
        let crawling = busy(8 * MB, 1, 1, 0, MB, 10_000.0);
        assert_eq!(
            pick_work(&snap(&[crawling.clone(), idle(MB)]), ME).map(|w| w.hedge),
            Some(false)
        );
        let w = pick_work(&snap(&[crawling]), ME).expect("should hedge");
        assert!(w.hedge);
        assert_eq!(w.from, MB, "hedge starts at the frontier");
    }

    #[test]
    fn hedging_respects_disk_age_cap_and_worth() {
        let crawling = busy(8 * MB, 1, 1, 0, 0, 10_000.0);
        let mut s_blocks = [crawling.clone()];
        let mut s = snap(&s_blocks);
        s.disk_behind = true;
        assert_eq!(
            pick_work(&s, ME),
            None,
            "no hedges while the disk is behind"
        );

        let young = busy(8 * MB, 1, 1, 9_000, 0, 10_000.0);
        assert_eq!(
            pick_work(&snap(&[young]), ME),
            None,
            "attempt too young to race"
        );

        s_blocks[0].hedges = 2;
        assert_eq!(pick_work(&snap(&s_blocks), ME), None, "hedge cap reached");

        let fast = busy(8 * MB, 1, 1, 0, 0, 50.0 * MB as f64);
        assert_eq!(
            pick_work(&snap(&[fast]), ME),
            None,
            "not worth racing a fast attempt"
        );
    }

    #[test]
    fn a_silent_attempt_is_always_worth_racing() {
        let silent = busy(4 * MB, 1, 1, 0, 0, 0.0);
        assert!(pick_work(&snap(&[silent]), ME).is_some_and(|w| w.hedge));
    }

    #[test]
    fn prefers_another_network_for_the_race() {
        let mine = busy(8 * MB, ME.network, 1, 0, 0, 1.0);
        let blocks = [mine];
        let mut s = snap(&blocks);
        s.idle_networks = &[5];
        assert_eq!(pick_work(&s, ME), None);
        s.idle_networks = &[];
        assert!(pick_work(&s, ME).is_some());
    }

    #[test]
    fn a_stream_never_holds_two_attempts() {
        let held = busy(MB, ME.network, ME.stream, 0, 0, 1.0);
        assert_eq!(pick_work(&snap(&[held, idle(MB)]), ME), None);
    }

    #[test]
    fn picks_the_block_that_would_finish_last() {
        let slow = busy(8 * MB, 1, 1, 0, 0, 1_000.0);
        let slower = busy(8 * MB, 3, 2, 0, 0, 100.0);
        assert_eq!(
            pick_work(&snap(&[slow, slower]), ME).map(|w| w.block),
            Some(1)
        );
    }

    #[test]
    fn nothing_to_do_when_everything_is_done() {
        let mut b = idle(MB);
        b.secured = MB;
        assert_eq!(pick_work(&snap(&[b]), ME), None);
        assert_eq!(pick_work(&snap(&[]), ME), None);
    }

    fn arb_block() -> impl Strategy<Value = Block> {
        (
            1u64..20 * MB,
            0u64..=100,
            proptest::collection::vec(
                (0u32..50, 0u32..4, 0u64..20_000, 0u64..=100, 0.0f64..1e8),
                0..3,
            ),
            proptest::option::of(0u32..4),
            0u8..4,
        )
            .prop_map(|(len, pct, atts, avoid, hedges)| {
                let secured = len * pct / 100;
                let attempts = atts
                    .into_iter()
                    .map(|(stream, network, started_ms, p, rate)| Attempt {
                        stream,
                        network,
                        started_ms,
                        position: secured + (len - secured) * p / 100,
                        rate,
                    })
                    .collect();
                Block {
                    len,
                    secured,
                    attempts,
                    avoid,
                    hedges,
                }
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(5000))]

        #[test]
        fn decisions_are_always_valid(
            blocks in proptest::collection::vec(arb_block(), 0..40),
            idle_nets in proptest::collection::vec(0u32..4, 0..4),
            disk in any::<bool>(),
            now in 0u64..40_000,
            my_net in 0u32..4,
            my_rate in proptest::option::of(0.0f64..1e8),
        ) {
            let me = Requester { stream: 1000, network: my_net, rate: my_rate };
            let s = Snapshot { now_ms: now, blocks: &blocks, idle_networks: &idle_nets, disk_behind: disk, policy: HedgePolicy::default() };
            let decision = pick_work(&s, me);
            if let Some(w) = decision {
                let b = &blocks[w.block];
                prop_assert!(!b.complete(), "picked a complete block");
                prop_assert!(w.from >= b.secured && w.from < b.len, "from outside the unsecured part");
                if w.hedge {
                    prop_assert!(!disk, "hedged while the disk is behind");
                    prop_assert!(b.hedges < 2);
                    prop_assert!(!b.attempts.is_empty());
                    prop_assert!(!blocks.iter().any(|x| !x.complete() && x.attempts.is_empty()), "hedged while work was pending");
                } else {
                    prop_assert!(b.attempts.is_empty(), "primary on a block already held");
                }
            }
            // Liveness: if a block is pending and no other network is idle, someone (me) takes work.
            let pending = blocks.iter().any(|b| !b.complete() && b.attempts.is_empty());
            let other_idle = idle_nets.iter().any(|n| *n != my_net);
            if pending && !other_idle {
                prop_assert!(decision.is_some_and(|w| !w.hedge), "pending work left with nobody else to take it");
            }
            // Determinism.
            prop_assert_eq!(pick_work(&s, me), decision);
        }
    }
}
