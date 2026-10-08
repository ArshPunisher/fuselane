//! Splitting a file into blocks (ENGINE-DOWNLOAD.md §3, L-22, L-23).
//!
//! The block is the unit of work (and of loss when an attempt fails). It is
//! sized from the file and the number of networks, never from how the UI draws it.

/// Smallest block, so small files still give every stream something to do.
pub const MIN_BLOCK: u64 = 1024 * 1024;
/// Largest block: caps what a failed or raced attempt can cost.
pub const MAX_BLOCK: u64 = 8 * 1024 * 1024;
/// Most streams one network may run (the concurrency controller's ceiling).
pub const MAX_STREAMS_PER_NETWORK: u64 = 32;
/// Aim for at least this many blocks per stream so work can be balanced.
pub const BLOCKS_PER_STREAM: u64 = 2;

/// How a download is divided. Fixed for the life of the download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    /// Total size, or `None` when the server didn't say.
    pub total: Option<u64>,
    pub block_size: u64,
    pub blocks: u64,
}

impl Plan {
    /// Plans a download of `total` bytes over `networks` networks.
    ///
    /// Unknown size, no range support, or an empty file gives a single block
    /// fetched by one stream on one network.
    pub fn new(total: Option<u64>, splittable: bool, networks: u32) -> Plan {
        match total {
            Some(total) if splittable && total > 0 => {
                let target =
                    u64::from(networks.max(1)) * MAX_STREAMS_PER_NETWORK * BLOCKS_PER_STREAM;
                let block_size = total.div_ceil(target).clamp(MIN_BLOCK, MAX_BLOCK);
                Plan {
                    total: Some(total),
                    block_size,
                    blocks: total.div_ceil(block_size),
                }
            }
            Some(total) => Plan {
                total: Some(total),
                block_size: total.max(1),
                blocks: 1,
            },
            None => Plan {
                total: None,
                block_size: u64::MAX,
                blocks: 1,
            },
        }
    }

    /// Byte range of block `index`: `(start, len)`; `len` is `u64::MAX` for an unknown size.
    pub fn block(&self, index: u64) -> Option<(u64, u64)> {
        if index >= self.blocks {
            return None;
        }
        match self.total {
            None => Some((0, u64::MAX)),
            Some(total) => {
                let start = index * self.block_size;
                Some((start, (total - start).min(self.block_size)))
            }
        }
    }
}

/// Orders stream starts so every network gets a first block before any gets a
/// second (L-23): `[[a1, a2], [b1]]` → `a1, b1, a2`.
pub fn interleave<T: Clone>(groups: &[Vec<T>]) -> Vec<T> {
    let longest = groups.iter().map(Vec::len).max().unwrap_or(0);
    (0..longest)
        .flat_map(|i| groups.iter().filter_map(move |g| g.get(i).cloned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn examples() {
        let gib = 1024 * 1024 * 1024;
        assert_eq!(Plan::new(Some(gib), true, 2).block_size, MAX_BLOCK);
        assert_eq!(
            Plan::new(Some(100 * 1024 * 1024), true, 2).block_size,
            MIN_BLOCK
        );
        assert_eq!(
            Plan::new(Some(0), true, 2),
            Plan {
                total: Some(0),
                block_size: 1,
                blocks: 1
            }
        );
        assert_eq!(
            Plan::new(None, true, 3),
            Plan {
                total: None,
                block_size: u64::MAX,
                blocks: 1
            }
        );
        assert_eq!(Plan::new(Some(5), false, 3).blocks, 1);
        assert_eq!(
            Plan::new(Some(gib), true, 0).blocks,
            Plan::new(Some(gib), true, 1).blocks
        );
        assert_eq!(
            interleave(&[vec!["a1", "a2", "a3"], vec!["b1"], vec![]]),
            vec!["a1", "b1", "a2", "a3"]
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(5000))]

        /// Blocks tile [0, total) exactly: no gap, no overlap, nothing past the end.
        /// Small plans are walked block by block; huge ones are checked at the seams.
        #[test]
        fn blocks_cover_the_file_exactly(total in prop_oneof![0u64..2_000_000_000, Just(u64::MAX / 2), Just(1)], nets in 0u32..8, split in any::<bool>()) {
            let p = Plan::new(Some(total), split, nets);
            prop_assert!(p.blocks >= 1);
            let check = |i: u64| -> (u64, u64) { p.block(i).unwrap() };
            if p.blocks <= 4096 {
                let mut next = 0u64;
                for i in 0..p.blocks {
                    let (start, len) = check(i);
                    prop_assert_eq!(start, next);
                    prop_assert!(len >= 1 || total == 0);
                    prop_assert!(len <= p.block_size);
                    next = start + len;
                }
                prop_assert_eq!(next, total);
            } else {
                for i in [0, 1, p.blocks / 2, p.blocks - 2] {
                    let (s0, l0) = check(i);
                    let (s1, _) = check(i + 1);
                    prop_assert_eq!(s0 + l0, s1);
                }
                let (s, l) = check(p.blocks - 1);
                prop_assert_eq!(s + l, total);
                prop_assert!(l >= 1 && l <= p.block_size);
            }
            prop_assert_eq!(p.block(p.blocks), None);
        }

        #[test]
        fn block_size_stays_in_bounds(total in (MIN_BLOCK * 2)..1u64 << 44, nets in 1u32..8) {
            let p = Plan::new(Some(total), true, nets);
            prop_assert!((MIN_BLOCK..=MAX_BLOCK).contains(&p.block_size));
        }

        #[test]
        fn interleave_keeps_every_item_once(groups in proptest::collection::vec(proptest::collection::vec(0u32..1000, 0..10), 0..6)) {
            let mut flat: Vec<u32> = groups.iter().flatten().copied().collect();
            let mut out = interleave(&groups);
            flat.sort_unstable();
            out.sort_unstable();
            prop_assert_eq!(out, flat);
        }
    }
}
