//! Chooses a network for each torrent peer and keeps per-network counts
//! (TORRENT.md "Choosing a network"). v1: the network with the fewest live peers
//! that has an address in the peer's family; ties go to the earlier network.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use fuselane_limits::Limiter;
use fuselane_netif::Interface;

/// One network's live numbers.
#[derive(Debug, Default)]
pub struct NetCounters {
    pub peers: AtomicUsize,
    pub down: AtomicU64,
    pub up: AtomicU64,
    pub failures: AtomicU64,
}

/// What the UI shows per network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetStat {
    pub name: String,
    pub peers: usize,
    pub down: u64,
    pub up: u64,
}

#[derive(Debug)]
pub struct Balancer {
    nets: Vec<(Interface, Arc<NetCounters>)>,
    /// Round-robin cursor so equal networks share new peers.
    next: Mutex<usize>,
    /// Per torrent (info hash, lowercase hex) and network name, learned from the
    /// BitTorrent handshake each peer connection starts with.
    torrents: Mutex<HashMap<(String, String), Arc<NetCounters>>>,
    /// The app's speed limits and data allowances, shared with HTTP downloads.
    limiter: Option<Arc<Limiter>>,
    /// Networks that take no new peers for now (metered ones while only seeding).
    avoid: Mutex<std::collections::HashSet<String>>,
}

/// One network's part in one torrent: raw bytes moved and the verified bytes it
/// is credited with (raw share of the torrent's verified bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetShare {
    pub name: String,
    pub peers: usize,
    pub received: u64,
    pub sent: u64,
    pub credited: u64,
}

/// Splits `verified` bytes by each network's raw share. Shares always sum to
/// `verified` exactly (L-117); the remainder goes to the biggest contributor.
pub fn credit(received: &[u64], verified: u64) -> Vec<u64> {
    let total: u128 = received.iter().map(|r| u128::from(*r)).sum();
    if total == 0 {
        return vec![0; received.len()];
    }
    let mut out: Vec<u64> = received
        .iter()
        .map(|r| u64::try_from(u128::from(*r) * u128::from(verified) / total).unwrap_or(u64::MAX))
        .collect();
    let given: u64 = out.iter().sum();
    if let Some(big) = (0..received.len()).max_by_key(|i| received[*i]) {
        out[big] += verified.saturating_sub(given);
    }
    out
}

impl Balancer {
    pub fn new(networks: Vec<Interface>) -> Balancer {
        Balancer {
            nets: networks
                .into_iter()
                .map(|i| (i, Arc::new(NetCounters::default())))
                .collect(),
            next: Mutex::new(0),
            torrents: Mutex::new(HashMap::new()),
            limiter: None,
            avoid: Mutex::default(),
        }
    }

    /// Replaces the networks that take no new peers. Open connections carry on.
    pub fn set_avoid<I: IntoIterator<Item = String>>(&self, names: I) {
        *self
            .avoid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = names.into_iter().collect();
    }

    /// The networks, for callers that decide what to avoid.
    pub fn networks(&self) -> Vec<Interface> {
        self.nets.iter().map(|(i, _)| i.clone()).collect()
    }

    /// Applies the app's speed limits and data allowances to torrent traffic.
    pub fn with_limiter(mut self, limiter: Option<Arc<Limiter>>) -> Balancer {
        self.limiter = limiter;
        self
    }

    pub fn limiter(&self) -> Option<&Arc<Limiter>> {
        self.limiter.as_ref()
    }

    /// Networks to try for `dest`, best first. Loopback peers (local tests) may use any.
    pub fn order_for(&self, dest: SocketAddr) -> Vec<(Interface, Arc<NetCounters>)> {
        let mut usable: Vec<(usize, &(Interface, Arc<NetCounters>))> = self
            .nets
            .iter()
            .enumerate()
            .filter(|(_, (i, _))| {
                !self
                    .avoid
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains(&i.name)
            })
            // A network past its data allowance takes no new peers.
            .filter(|(_, (i, _))| !self.limiter.as_ref().is_some_and(|l| l.blocked(&i.name)))
            .filter(|(_, (i, _))| {
                dest.ip().is_loopback() || i.addrs.iter().any(|a| a.is_ipv4() == dest.is_ipv4())
            })
            .collect();
        let start = {
            let mut n = self
                .next
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *n = n.wrapping_add(1);
            *n
        };
        let len = self.nets.len().max(1);
        usable.sort_by_key(|(idx, (_, c))| {
            (
                c.peers.load(Ordering::Relaxed),
                (idx + len - start % len) % len,
            )
        });
        usable
            .into_iter()
            .map(|(_, n)| (n.0.clone(), n.1.clone()))
            .collect()
    }

    /// Counters for one torrent on one network (created on first use).
    pub fn torrent_counters(&self, info_hash: &str, net: &str) -> Arc<NetCounters> {
        let mut m = self
            .torrents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        m.entry((info_hash.to_owned(), net.to_owned()))
            .or_default()
            .clone()
    }

    /// Every network's part in this torrent, in network order, credited against
    /// `verified` bytes.
    pub fn torrent_shares(&self, info_hash: &str, verified: u64) -> Vec<NetShare> {
        let m = self
            .torrents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let rows: Vec<(String, usize, u64, u64)> = self
            .nets
            .iter()
            .map(
                |(i, _)| match m.get(&(info_hash.to_owned(), i.name.clone())) {
                    Some(c) => (
                        i.name.clone(),
                        c.peers.load(Ordering::Relaxed),
                        c.down.load(Ordering::Relaxed),
                        c.up.load(Ordering::Relaxed),
                    ),
                    None => (i.name.clone(), 0, 0, 0),
                },
            )
            .collect();
        let credited = credit(&rows.iter().map(|r| r.2).collect::<Vec<_>>(), verified);
        rows.into_iter()
            .zip(credited)
            .map(|((name, peers, received, sent), credited)| NetShare {
                name,
                peers,
                received,
                sent,
                credited,
            })
            .collect()
    }

    /// Drops a removed torrent's counters.
    pub fn forget(&self, info_hash: &str) {
        let mut m = self
            .torrents
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        m.retain(|(h, _), _| h != info_hash);
    }

    pub fn snapshot(&self) -> Vec<NetStat> {
        self.nets
            .iter()
            .map(|(i, c)| NetStat {
                name: i.name.clone(),
                peers: c.peers.load(Ordering::Relaxed),
                down: c.down.load(Ordering::Relaxed),
                up: c.up.load(Ordering::Relaxed),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuselane_netif::Kind;
    use std::net::IpAddr;

    fn iface(name: &str, addrs: &[&str]) -> Interface {
        Interface {
            name: name.into(),
            display_name: name.into(),
            index: 1,
            kind: Kind::Ethernet,
            addrs: addrs.iter().map(|a| a.parse::<IpAddr>().unwrap()).collect(),
        }
    }

    #[test]
    fn new_peers_go_to_the_least_busy_network_with_the_right_family() {
        let b = Balancer::new(vec![
            iface("en0", &["10.0.0.2"]),
            iface("en1", &["192.168.1.2", "2001:db8::2"]),
        ]);
        let v4: SocketAddr = "203.0.113.9:6881".parse().unwrap();
        let v6: SocketAddr = "[2001:db8::9]:6881".parse().unwrap();
        // Only en1 has IPv6.
        let order = b.order_for(v6);
        assert_eq!(order.len(), 1);
        assert_eq!(order[0].0.name, "en1");
        // en0 is busier, so en1 comes first for an IPv4 peer.
        b.nets[0].1.peers.store(3, Ordering::Relaxed);
        assert_eq!(b.order_for(v4)[0].0.name, "en1");
        b.nets[1].1.peers.store(5, Ordering::Relaxed);
        assert_eq!(b.order_for(v4)[0].0.name, "en0");
    }

    #[test]
    fn a_network_past_its_allowance_gets_no_new_peers() {
        let limiter = Arc::new(Limiter::default());
        let b = Balancer::new(vec![
            iface("en0", &["10.0.0.2"]),
            iface("en1", &["192.168.1.2"]),
        ])
        .with_limiter(Some(limiter.clone()));
        let v4: SocketAddr = "203.0.113.9:6881".parse().unwrap();
        limiter.set_blocked(["en0".to_string()]);
        for _ in 0..4 {
            let order = b.order_for(v4);
            assert_eq!(
                order
                    .iter()
                    .map(|(i, _)| i.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["en1"]
            );
        }
        limiter.set_blocked(["en0".to_string(), "en1".to_string()]);
        assert!(
            b.order_for(v4).is_empty(),
            "no route at all once every network is used up"
        );
    }

    #[test]
    fn avoided_networks_take_no_new_peers_until_cleared() {
        let b = Balancer::new(vec![
            iface("en0", &["10.0.0.2"]),
            iface("en7", &["172.20.10.2"]),
        ]);
        let v4: SocketAddr = "203.0.113.9:6881".parse().unwrap();
        b.set_avoid(["en7".to_string()]);
        for _ in 0..4 {
            assert_eq!(b.order_for(v4).len(), 1);
            assert_eq!(b.order_for(v4)[0].0.name, "en0");
        }
        b.set_avoid(Vec::new());
        assert_eq!(b.order_for(v4).len(), 2);
        assert_eq!(b.networks().len(), 2);
    }

    #[test]
    fn credit_always_sums_to_the_verified_bytes() {
        assert_eq!(credit(&[], 10), Vec::<u64>::new());
        assert_eq!(credit(&[0, 0], 10), vec![0, 0]);
        assert_eq!(credit(&[300, 100], 200), vec![150, 50]);
        // Raw bytes include protocol overhead and wasted pieces: credit never exceeds verified.
        for (raw, verified) in [
            (vec![1u64, 1, 1], 100u64),
            (vec![7, 0, 3], 1_000_003),
            (vec![u64::MAX, u64::MAX], u64::MAX),
        ] {
            let c = credit(&raw, verified);
            assert_eq!(
                c.iter().map(|x| u128::from(*x)).sum::<u128>(),
                u128::from(verified),
                "{raw:?}"
            );
        }
        assert_eq!(
            credit(&[0, 5], 9)[0],
            0,
            "a network that moved nothing gets nothing"
        );
    }

    #[test]
    fn torrent_shares_list_every_network_and_forget_cleans_up() {
        let b = Balancer::new(vec![
            iface("en0", &["10.0.0.2"]),
            iface("en1", &["192.168.1.2"]),
        ]);
        b.torrent_counters("aa", "en1")
            .down
            .store(80, Ordering::Relaxed);
        b.torrent_counters("aa", "en0")
            .down
            .store(20, Ordering::Relaxed);
        b.torrent_counters("bb", "en0")
            .down
            .store(999, Ordering::Relaxed);
        let s = b.torrent_shares("aa", 50);
        assert_eq!(
            s.iter()
                .map(|x| (x.name.as_str(), x.credited))
                .collect::<Vec<_>>(),
            vec![("en0", 10), ("en1", 40)]
        );
        b.forget("aa");
        assert!(b.torrent_shares("aa", 50).iter().all(|x| x.received == 0));
        assert_eq!(b.torrent_shares("bb", 1)[0].received, 999);
    }

    #[test]
    fn equally_busy_networks_take_turns() {
        let b = Balancer::new(vec![
            iface("en0", &["10.0.0.2"]),
            iface("en1", &["192.168.1.2"]),
        ]);
        let v4: SocketAddr = "203.0.113.9:6881".parse().unwrap();
        let firsts: Vec<String> = (0..4).map(|_| b.order_for(v4)[0].0.name.clone()).collect();
        assert!(
            firsts.contains(&"en0".to_string()) && firsts.contains(&"en1".to_string()),
            "{firsts:?}"
        );
    }
}
