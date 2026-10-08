//! Chooses a network for each torrent peer and keeps per-network counts
//! (TORRENT.md "Choosing a network"). v1: the network with the fewest live peers
//! that has an address in the peer's family; ties go to the earlier network.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

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
}

impl Balancer {
    pub fn new(networks: Vec<Interface>) -> Balancer {
        Balancer {
            nets: networks
                .into_iter()
                .map(|i| (i, Arc::new(NetCounters::default())))
                .collect(),
            next: Mutex::new(0),
        }
    }

    /// Networks to try for `dest`, best first. Loopback peers (local tests) may use any.
    pub fn order_for(&self, dest: SocketAddr) -> Vec<(Interface, Arc<NetCounters>)> {
        let mut usable: Vec<(usize, &(Interface, Arc<NetCounters>))> = self
            .nets
            .iter()
            .enumerate()
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
