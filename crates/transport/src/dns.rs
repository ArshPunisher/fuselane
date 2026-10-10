//! DNS through one network (STEPS 2.13, L-65). Each network asks a public
//! resolver over a socket pinned to it, so CDNs answer with a server near that
//! network's own exit, not the one near the default route. Packets are built and
//! read with hickory-proto: answers come from the network and are untrusted.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use fuselane_netif::Interface;
use hickory_proto::op::{Message, MessageType, OpCode, Query, ResponseCode};
use hickory_proto::rr::{Name, RData, RecordType};
use socket2::{Domain, Protocol, Socket, Type};

/// Cloudflare and Google, IPv4 and IPv6. Tried in order per network.
pub const PUBLIC_RESOLVERS: [SocketAddr; 4] = [
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
    SocketAddr::new(
        IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
        53,
    ),
    SocketAddr::new(
        IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)),
        53,
    ),
];

/// How long the second answer (A or AAAA) gets once the first has arrived.
const SECOND_ANSWER_WAIT: Duration = Duration::from_millis(500);

/// Most addresses kept from one answer (a hostile reply can't balloon memory).
const MAX_ADDRS: usize = 32;

fn query(id: u16, name: &Name, kind: RecordType) -> std::io::Result<Vec<u8>> {
    let mut m = Message::new(id, MessageType::Query, OpCode::Query);
    m.metadata.recursion_desired = true;
    m.add_query(Query::query(name.clone(), kind));
    m.to_vec().map_err(std::io::Error::other)
}

/// The addresses in a reply to query `id` for `name`; `None` if the packet isn't
/// a valid answer to our question (wrong id, wrong name, junk).
pub fn parse_answer(buf: &[u8], id: u16, name: &Name) -> Option<Result<Vec<IpAddr>, ResponseCode>> {
    let m = Message::from_vec(buf).ok()?;
    if m.metadata.id != id || m.metadata.message_type != MessageType::Response {
        return None;
    }
    // Names compare without case or the trailing root dot ("a.b" == "A.B.").
    let norm = |n: &Name| n.to_ascii().trim_end_matches('.').to_ascii_lowercase();
    let want = norm(name);
    if !m.queries.iter().any(|q| norm(q.name()) == want) {
        return None;
    }
    if m.metadata.response_code != ResponseCode::NoError {
        return Some(Err(m.metadata.response_code));
    }
    let addrs = m
        .answers
        .iter()
        .filter_map(|r| match &r.data {
            RData::A(a) => Some(IpAddr::V4(a.0)),
            RData::AAAA(a) => Some(IpAddr::V6(a.0)),
            _ => None,
        })
        .take(MAX_ADDRS)
        .collect();
    Some(Ok(addrs))
}

fn random_id() -> u16 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.finish() as u16
}

/// Asks one server for A and AAAA over a UDP socket pinned to `iface`.
async fn ask(
    iface: &Interface,
    server: SocketAddr,
    name: &Name,
    timeout: Duration,
) -> std::io::Result<Vec<IpAddr>> {
    let socket = Socket::new(
        Domain::for_address(server),
        Type::DGRAM,
        Some(Protocol::UDP),
    )?;
    if !server.ip().is_loopback() {
        crate::pin(&socket, iface, &server)?;
    }
    let any: SocketAddr = if server.is_ipv4() {
        (Ipv4Addr::UNSPECIFIED, 0).into()
    } else {
        (Ipv6Addr::UNSPECIFIED, 0).into()
    };
    socket.bind(&any.into())?;
    socket.set_nonblocking(true)?;
    let udp = tokio::net::UdpSocket::from_std(socket.into())?;
    udp.connect(server).await?; // replies from anyone else are dropped by the OS
    let (id_a, id_aaaa) = {
        let base = random_id();
        (base, base.wrapping_add(1))
    };
    udp.send(&query(id_a, name, RecordType::A)?).await?;
    udp.send(&query(id_aaaa, name, RecordType::AAAA)?).await?;
    let mut found: Vec<IpAddr> = Vec::new();
    let mut answered = [false, false];
    let mut nxdomain = false;
    let mut buf = [0u8; 4096];
    let mut deadline = tokio::time::Instant::now() + timeout;
    // Some networks drop the second of two queries sent back to back. Once one
    // answer is in, the other is asked for again once and given a short wait,
    // instead of the whole timeout.
    let mut resent = false;
    while !(answered[0] && answered[1]) {
        if !resent && (answered[0] || answered[1]) {
            resent = true;
            let (id, kind) = if answered[0] {
                (id_aaaa, RecordType::AAAA)
            } else {
                (id_a, RecordType::A)
            };
            udp.send(&query(id, name, kind)?).await?;
            deadline = deadline.min(tokio::time::Instant::now() + SECOND_ANSWER_WAIT);
        }
        let n = match tokio::time::timeout_at(deadline, udp.recv(&mut buf)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e),
            Err(_) => break, // deadline: keep what we have
        };
        for (slot, id) in [(0, id_a), (1, id_aaaa)] {
            if answered[slot] {
                continue;
            }
            match parse_answer(&buf[..n], id, name) {
                Some(Ok(addrs)) => {
                    answered[slot] = true;
                    for a in addrs {
                        if !found.contains(&a) && found.len() < MAX_ADDRS {
                            found.push(a);
                        }
                    }
                }
                Some(Err(ResponseCode::NXDomain)) => {
                    answered[slot] = true;
                    nxdomain = true;
                }
                Some(Err(_)) => answered[slot] = true,
                None => {}
            }
        }
    }
    if found.is_empty() {
        let kind = if nxdomain {
            std::io::ErrorKind::NotFound
        } else {
            std::io::ErrorKind::TimedOut
        };
        return Err(std::io::Error::new(
            kind,
            format!("{server} gave no addresses"),
        ));
    }
    Ok(found)
}

/// Resolves `host` through `iface`, trying each server of a family the network
/// has, in order, until one answers with addresses.
pub async fn resolve_on(
    iface: &Interface,
    host: &str,
    servers: &[SocketAddr],
    per_server: Duration,
) -> std::io::Result<Vec<IpAddr>> {
    let name = Name::from_ascii(host)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e.to_string()))?;
    let mut last = std::io::Error::new(
        std::io::ErrorKind::AddrNotAvailable,
        "no resolver this network can reach",
    );
    for server in servers {
        let reachable = server.ip().is_loopback()
            || iface.addrs.iter().any(|a| a.is_ipv4() == server.is_ipv4());
        if !reachable {
            continue;
        }
        match ask(iface, *server, &name, per_server).await {
            Ok(addrs) => return Ok(addrs),
            Err(e) => last = e,
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::Record;
    use hickory_proto::rr::rdata::{A, AAAA};

    fn lo() -> Interface {
        Interface {
            name: "lo0".into(),
            display_name: "Loopback".into(),
            index: 1,
            kind: fuselane_netif::Kind::Loopback,
            addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        }
    }

    #[derive(Clone, Copy)]
    enum Mode {
        Good,
        WrongId,
        Garbage,
        NxDomain,
        /// Drops the second query it ever gets (some networks do).
        DropSecond,
    }

    /// A fake DNS server on loopback that answers each query according to `mode`.
    async fn fake(mode: Mode) -> SocketAddr {
        let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = sock.local_addr().unwrap();
        tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            let mut seen = 0;
            while let Ok((n, from)) = sock.recv_from(&mut buf).await {
                seen += 1;
                if matches!(mode, Mode::DropSecond) && seen == 2 {
                    continue;
                }
                let Ok(q) = Message::from_vec(&buf[..n]) else {
                    continue;
                };
                let reply = match mode {
                    Mode::Garbage => vec![0xde, 0xad, 0xbe, 0xef, 1, 2, 3],
                    _ => {
                        let id = if matches!(mode, Mode::WrongId) {
                            q.metadata.id.wrapping_add(7)
                        } else {
                            q.metadata.id
                        };
                        let mut m = Message::new(id, MessageType::Response, OpCode::Query);
                        for query in &q.queries {
                            m.add_query(query.clone());
                            if matches!(mode, Mode::NxDomain) {
                                continue;
                            }
                            let name = query.name().clone();
                            let data = match query.query_type() {
                                RecordType::A => RData::A(A(Ipv4Addr::new(203, 0, 113, 7))),
                                _ => RData::AAAA(AAAA("2001:db8::7".parse().unwrap())),
                            };
                            m.add_answer(Record::from_rdata(name, 60, data));
                        }
                        if matches!(mode, Mode::NxDomain) {
                            m.metadata.response_code = ResponseCode::NXDomain;
                        }
                        m.to_vec().unwrap()
                    }
                };
                let _ = sock.send_to(&reply, from).await;
            }
        });
        addr
    }

    #[tokio::test]
    async fn a_resolver_answers_with_both_families() {
        let s = fake(Mode::Good).await;
        let addrs = resolve_on(&lo(), "cdn.example.org", &[s], Duration::from_secs(2))
            .await
            .unwrap();
        assert!(addrs.contains(&"203.0.113.7".parse().unwrap()));
        assert!(addrs.contains(&"2001:db8::7".parse().unwrap()));
    }

    #[tokio::test]
    async fn a_dropped_second_query_is_asked_again_instead_of_waiting_it_out() {
        let s = fake(Mode::DropSecond).await;
        let start = std::time::Instant::now();
        let addrs = resolve_on(&lo(), "cdn.example.org", &[s], Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(addrs.len(), 2, "both families, thanks to the second ask");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "{:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn spoofed_and_junk_replies_are_ignored_and_the_next_server_is_used() {
        let spoof = fake(Mode::WrongId).await;
        let junk = fake(Mode::Garbage).await;
        let good = fake(Mode::Good).await;
        let start = std::time::Instant::now();
        let addrs = resolve_on(
            &lo(),
            "cdn.example.org",
            &[spoof, junk, good],
            Duration::from_millis(300),
        )
        .await
        .unwrap();
        assert_eq!(addrs.len(), 2);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn no_such_name_and_dead_servers_are_errors() {
        let nx = fake(Mode::NxDomain).await;
        let e = resolve_on(&lo(), "nope.example.org", &[nx], Duration::from_millis(300))
            .await
            .unwrap_err();
        assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
        let dead: SocketAddr = "127.0.0.1:9".parse().unwrap();
        assert!(
            resolve_on(&lo(), "x.example.org", &[dead], Duration::from_millis(200))
                .await
                .is_err()
        );
        assert!(
            resolve_on(
                &lo(),
                "bad name with spaces",
                &[nx],
                Duration::from_millis(200)
            )
            .await
            .is_err()
        );
    }

    #[test]
    fn answers_to_someone_elses_question_are_rejected() {
        let asked = Name::from_ascii("a.example.org").unwrap();
        let other = Name::from_ascii("b.example.org").unwrap();
        let mut m = Message::new(42, MessageType::Response, OpCode::Query);
        m.add_query(Query::query(other.clone(), RecordType::A));
        m.add_answer(Record::from_rdata(
            other,
            60,
            RData::A(A(Ipv4Addr::new(1, 2, 3, 4))),
        ));
        let bytes = m.to_vec().unwrap();
        assert!(parse_answer(&bytes, 42, &asked).is_none());
        assert!(parse_answer(&[], 42, &asked).is_none());
    }
}

#[cfg(test)]
mod real_network {
    use super::*;

    /// Needs the internet: `cargo test -p fuselane-transport real_network -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "needs the internet"]
    async fn each_network_resolves_through_itself() {
        for iface in fuselane_netif::usable().unwrap() {
            let r = resolve_on(
                &iface,
                "dl.google.com",
                &PUBLIC_RESOLVERS,
                Duration::from_secs(2),
            )
            .await;
            println!(
                "{} ({}): {:?}",
                iface.name,
                iface.display_name,
                r.as_ref().map(|v| v.iter().take(3).collect::<Vec<_>>())
            );
            assert!(r.is_ok(), "{}: {r:?}", iface.name);
        }
    }
}
