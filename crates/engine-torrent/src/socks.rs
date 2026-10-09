//! An in-process SOCKS5 server (RFC 1928, username/password RFC 1929) that pins
//! each connection to a network chosen by the balancer (ADR 0006). It listens on
//! loopback only and demands random credentials, so other local programs can't
//! use it. Everything a client sends is treated as untrusted.

use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};

use crate::balancer::Balancer;

/// How long a client gets to finish the handshake.
const HANDSHAKE: Duration = Duration::from_secs(10);
/// How long dialling one peer through one network may take.
const DIAL: Duration = Duration::from_secs(6);

/// A running proxy: where it listens and the credentials librqbit must use.
#[derive(Debug, Clone)]
pub struct SocksServer {
    pub addr: SocketAddr,
    pub user: String,
    pub pass: String,
}

impl SocksServer {
    /// `socks5://user:pass@127.0.0.1:port` for librqbit's `proxy_url`.
    pub fn url(&self) -> String {
        format!("socks5://{}:{}@{}", self.user, self.pass, self.addr)
    }
}

fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..2)
        .map(|i| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u64(i);
            h.write_u128(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos()),
            );
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Starts the proxy on a random loopback port.
pub async fn start(balancer: Arc<Balancer>) -> std::io::Result<SocksServer> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let server = SocksServer {
        addr: listener.local_addr()?,
        user: random_token(),
        pass: random_token(),
    };
    let creds = Arc::new((server.user.clone(), server.pass.clone()));
    tokio::spawn(async move {
        while let Ok((client, _)) = listener.accept().await {
            let (balancer, creds) = (balancer.clone(), creds.clone());
            tokio::spawn(async move {
                let _ = serve(client, &balancer, &creds).await;
            });
        }
    });
    Ok(server)
}

/// Reply codes (RFC 1928 §6).
const OK: u8 = 0;
const HOST_UNREACHABLE: u8 = 4;
const CMD_UNSUPPORTED: u8 = 7;
const ATYP_UNSUPPORTED: u8 = 8;

async fn reply(c: &mut TcpStream, code: u8) -> std::io::Result<()> {
    c.write_all(&[5, code, 0, 1, 0, 0, 0, 0, 0, 0]).await
}

/// Reads the greeting, authentication and request; returns the target.
async fn handshake(
    c: &mut TcpStream,
    creds: &(String, String),
) -> std::io::Result<Option<(String, u16)>> {
    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidData, "bad SOCKS5 handshake");
    let mut head = [0u8; 2];
    c.read_exact(&mut head).await?;
    if head[0] != 5 || head[1] == 0 {
        return Err(bad());
    }
    let mut methods = vec![0u8; head[1] as usize];
    c.read_exact(&mut methods).await?;
    if !methods.contains(&2) {
        c.write_all(&[5, 0xff]).await?; // no acceptable method: we require a password
        return Err(bad());
    }
    c.write_all(&[5, 2]).await?;
    // RFC 1929: VER=1, ULEN, UNAME, PLEN, PASSWD.
    let mut v = [0u8; 2];
    c.read_exact(&mut v).await?;
    if v[0] != 1 {
        return Err(bad());
    }
    let mut user = vec![0u8; v[1] as usize];
    c.read_exact(&mut user).await?;
    let mut plen = [0u8; 1];
    c.read_exact(&mut plen).await?;
    let mut pass = vec![0u8; plen[0] as usize];
    c.read_exact(&mut pass).await?;
    let ok = constant_eq(&user, creds.0.as_bytes()) & constant_eq(&pass, creds.1.as_bytes());
    c.write_all(&[1, if ok { 0 } else { 1 }]).await?;
    if !ok {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "wrong SOCKS credentials",
        ));
    }
    // Request: VER CMD RSV ATYP DST.ADDR DST.PORT.
    let mut req = [0u8; 4];
    c.read_exact(&mut req).await?;
    if req[0] != 5 {
        return Err(bad());
    }
    if req[1] != 1 {
        reply(c, CMD_UNSUPPORTED).await?; // CONNECT only: no BIND, no UDP ASSOCIATE
        return Ok(None);
    }
    let host = match req[3] {
        1 => {
            let mut a = [0u8; 4];
            c.read_exact(&mut a).await?;
            Ipv4Addr::from(a).to_string()
        }
        4 => {
            let mut a = [0u8; 16];
            c.read_exact(&mut a).await?;
            Ipv6Addr::from(a).to_string()
        }
        3 => {
            let mut len = [0u8; 1];
            c.read_exact(&mut len).await?;
            let mut name = vec![0u8; len[0] as usize];
            c.read_exact(&mut name).await?;
            match String::from_utf8(name) {
                Ok(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_graphic()) => n,
                _ => {
                    reply(c, ATYP_UNSUPPORTED).await?;
                    return Ok(None);
                }
            }
        }
        _ => {
            reply(c, ATYP_UNSUPPORTED).await?;
            return Ok(None);
        }
    };
    let mut port = [0u8; 2];
    c.read_exact(&mut port).await?;
    Ok(Some((host, u16::from_be_bytes(port))))
}

/// Compares secrets without stopping at the first differing byte.
fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn serve(
    mut client: TcpStream,
    balancer: &Balancer,
    creds: &(String, String),
) -> std::io::Result<()> {
    let target = match tokio::time::timeout(HANDSHAKE, handshake(&mut client, creds)).await {
        Ok(Ok(Some(t))) => t,
        // Refused or broken: close politely so any reply already sent isn't lost to a reset.
        Ok(Ok(None)) => return close(client).await,
        Ok(Err(e)) => {
            tracing::debug!(error = %e, "proxy: handshake refused");
            close(client).await?;
            return Err(e);
        }
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "SOCKS handshake too slow",
            ));
        }
    };
    let (host, port) = target;
    let dests: Vec<SocketAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        // Tracker hostnames: resolved by the system, then dialled per network.
        Err(_) => {
            match tokio::time::timeout(DIAL, tokio::net::lookup_host((host.as_str(), port))).await {
                Ok(Ok(a)) => a.take(8).collect(),
                _ => Vec::new(),
            }
        }
    };
    for dest in dests {
        for (iface, counters) in balancer.order_for(dest) {
            let peer = match fuselane_transport::connect_pinned(&iface, dest, DIAL).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::debug!(net = %iface.name, %dest, error = %e, "proxy: dial failed");
                    counters.failures.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
            };
            tracing::debug!(net = %iface.name, %dest, "proxy: connected");
            reply(&mut client, OK).await?;
            // Which torrent is this? Peer connections open with a handshake naming it.
            let first = read_opening(&mut client).await;
            let mut sinks = vec![counters];
            if let Some(hash) = handshake_info_hash(&first) {
                sinks.push(balancer.torrent_counters(&hash, &iface.name));
            }
            sinks.iter().for_each(|c| {
                c.peers.fetch_add(1, Ordering::Relaxed);
            });
            balancer.route_opened(dest, &iface.name);
            let mut peer = Counted {
                inner: peer,
                sinks: &sinks,
                net: &iface.name,
                limiter: balancer.limiter(),
                wait: None,
            };
            let res = async {
                peer.write_all(&first).await?;
                tokio::io::copy_bidirectional(&mut client, &mut peer).await
            }
            .await;
            sinks.iter().for_each(|c| {
                c.peers.fetch_sub(1, Ordering::Relaxed);
            });
            balancer.route_closed(dest);
            return res.map(|_| ());
        }
    }
    tracing::debug!(%host, port, "proxy: no network could reach the peer");
    reply(&mut client, HOST_UNREACHABLE).await?;
    close(client).await
}

/// Sends FIN, then drains a little of whatever the client still sends. Closing a
/// socket with unread data makes the OS send a reset, which can discard our reply.
async fn close(mut client: TcpStream) -> std::io::Result<()> {
    client.shutdown().await?;
    let mut sink = [0u8; 512];
    let _ = tokio::time::timeout(Duration::from_millis(500), async {
        let mut left = 4096usize;
        while left > 0 {
            match client.read(&mut sink).await {
                Ok(0) | Err(_) => break,
                Ok(n) => left = left.saturating_sub(n),
            }
        }
    })
    .await;
    Ok(())
}

/// The client's whole opening: a BitTorrent handshake is 68 bytes (BEP 3) and is
/// forwarded in one write. librqbit, and other clients, read the peer's handshake
/// in one go and drop the connection if part of it is missing; forwarding the
/// first 48 bytes (enough for the info hash) separately broke every connection on
/// Linux, where the two writes arrive apart. A tracker's HTTP request stops early.
async fn read_opening(client: &mut TcpStream) -> Vec<u8> {
    const WANT: usize = 68;
    let mut buf = vec![0u8; WANT];
    let mut n = 0;
    let _ = tokio::time::timeout(HANDSHAKE, async {
        while n < WANT {
            match client.read(&mut buf[n..]).await {
                Ok(0) | Err(_) => break,
                Ok(k) => n += k,
            }
            // Not a BitTorrent handshake (a tracker's HTTP request): stop waiting.
            if !PROTOCOL[..n.min(PROTOCOL.len())].eq(&buf[..n.min(PROTOCOL.len())]) {
                break;
            }
        }
    })
    .await;
    buf.truncate(n);
    buf
}

const PROTOCOL: &[u8] = b"\x13BitTorrent protocol";

/// BEP 3 handshake: pstrlen 19, "BitTorrent protocol", 8 reserved, info hash.
fn handshake_info_hash(b: &[u8]) -> Option<String> {
    if b.len() < 48 || !b.starts_with(PROTOCOL) {
        return None;
    }
    Some(b[28..48].iter().map(|x| format!("{x:02x}")).collect())
}

/// The peer side of a proxied connection, counting bytes each way for its network.
struct Counted<'a> {
    inner: TcpStream,
    sinks: &'a [Arc<crate::balancer::NetCounters>],
    net: &'a str,
    limiter: Option<&'a Arc<fuselane_limits::Limiter>>,
    /// Speed limit debt: no more reading from the peer until this passes.
    wait: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl AsyncRead for Counted<'_> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Past its data allowance: end the connection (EOF) so librqbit moves on.
        if self.limiter.is_some_and(|l| l.blocked(self.net)) {
            return Poll::Ready(Ok(()));
        }
        if let Some(w) = self.wait.as_mut() {
            if w.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            self.wait = None;
        }
        let before = buf.filled().len();
        let r = Pin::new(&mut self.inner).poll_read(cx, buf);
        let n = (buf.filled().len() - before) as u64;
        self.sinks.iter().for_each(|c| {
            c.down.fetch_add(n, Ordering::Relaxed);
        });
        if n > 0
            && let Some(l) = self.limiter
        {
            // Same limits as HTTP downloads; TCP then slows the sender down.
            let d = l.take(self.net, n);
            if !d.is_zero() {
                self.wait = Some(Box::pin(tokio::time::sleep(d)));
            }
        }
        r
    }
}

impl AsyncWrite for Counted<'_> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let r = Pin::new(&mut self.inner).poll_write(cx, data);
        if let Poll::Ready(Ok(n)) = &r {
            self.sinks.iter().for_each(|c| {
                c.up.fetch_add(*n as u64, Ordering::Relaxed);
            });
            // Sent bytes count toward a data allowance too (plans bill both ways).
            if let Some(l) = self.limiter {
                l.count(self.net, *n as u64);
            }
        }
        r
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuselane_netif::{Interface, Kind};

    fn lo_balancer() -> Arc<Balancer> {
        Arc::new(Balancer::new(vec![Interface {
            name: "lo0".into(),
            display_name: "Loopback".into(),
            index: 1,
            kind: Kind::Ethernet,
            addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        }]))
    }

    async fn echo_server() -> SocketAddr {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let (mut r, mut w) = s.split();
                    let _ = tokio::io::copy(&mut r, &mut w).await;
                });
            }
        });
        addr
    }

    #[tokio::test]
    async fn a_real_socks_client_gets_through_and_bytes_are_counted() {
        let b = lo_balancer();
        let server = start(b.clone()).await.unwrap();
        let target = echo_server().await;
        let mut s = tokio_socks::tcp::Socks5Stream::connect_with_password(
            server.addr,
            target,
            &server.user,
            &server.pass,
        )
        .await
        .unwrap();
        s.write_all(b"hello through the fuse").await.unwrap();
        let mut back = [0u8; 22];
        s.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"hello through the fuse");
        drop(s);
        tokio::time::sleep(Duration::from_millis(50)).await;
        let stat = &b.snapshot()[0];
        assert_eq!((stat.up, stat.down), (22, 22));
        assert_eq!(
            stat.peers, 0,
            "the peer count drops when the connection ends"
        );
        assert!(server.url().starts_with("socks5://"));
    }

    #[tokio::test]
    async fn wrong_or_missing_credentials_are_refused() {
        let server = start(lo_balancer()).await.unwrap();
        let target = echo_server().await;
        assert!(
            tokio_socks::tcp::Socks5Stream::connect_with_password(
                server.addr,
                target,
                &server.user,
                "wrong"
            )
            .await
            .is_err()
        );
        assert!(
            tokio_socks::tcp::Socks5Stream::connect(server.addr, target)
                .await
                .is_err(),
            "no auth must not work"
        );
    }

    async fn raw(server: &SocksServer, bytes: &[u8]) -> Vec<u8> {
        let mut c = TcpStream::connect(server.addr).await.unwrap();
        let _ = c.write_all(bytes).await;
        let mut out = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(2), c.read_to_end(&mut out)).await;
        out
    }

    #[tokio::test]
    async fn hostile_handshakes_are_closed_without_connecting_anywhere() {
        let server = start(lo_balancer()).await.unwrap();
        // Not SOCKS5 at all, SOCKS4, zero methods, truncated, no password method.
        for bytes in [
            &b"GET / HTTP/1.1\r\n\r\n"[..],
            &[4, 1, 0, 80, 127, 0, 0, 1, 0][..],
            &[5, 0][..],
            &[5][..],
            &[5, 1, 0][..],
        ] {
            let out = raw(&server, bytes).await;
            assert!(out.len() <= 2, "{bytes:?} -> {out:?}");
        }
        // Authenticated, then BIND (not CONNECT): refused with "command not supported".
        let mut req = vec![5, 1, 2, 1, server.user.len() as u8];
        req.extend(server.user.as_bytes());
        req.push(server.pass.len() as u8);
        req.extend(server.pass.as_bytes());
        req.extend([5, 2, 0, 1, 127, 0, 0, 1, 0, 80]);
        let out = raw(&server, &req).await;
        assert_eq!(out.get(4..6), Some(&[5u8, CMD_UNSUPPORTED][..]), "{out:?}");
        // Unknown address type.
        let mut req2 = req.clone();
        let n = req2.len();
        req2[n - 9] = 1;
        req2[n - 7] = 9;
        let out = raw(&server, &req2).await;
        assert_eq!(out.get(4..6), Some(&[5u8, ATYP_UNSUPPORTED][..]), "{out:?}");
    }

    #[tokio::test]
    async fn an_unreachable_peer_gets_a_clean_refusal() {
        let server = start(lo_balancer()).await.unwrap();
        let dead: SocketAddr = "127.0.0.1:9".parse().unwrap();
        let r = tokio_socks::tcp::Socks5Stream::connect_with_password(
            server.addr,
            dead,
            &server.user,
            &server.pass,
        )
        .await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn a_handshake_sent_in_pieces_reaches_the_peer_in_one_piece() {
        // The peer reads once, as librqbit does, and needs all 68 bytes in that read.
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target = l.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = [0u8; 256];
            s.read(&mut buf).await.unwrap()
        });
        let server = start(lo_balancer()).await.unwrap();
        let mut c = tokio_socks::tcp::Socks5Stream::connect_with_password(
            server.addr,
            target,
            &server.user,
            &server.pass,
        )
        .await
        .unwrap();
        let mut hs = PROTOCOL.to_vec();
        hs.extend([0u8; 8]);
        hs.extend([7u8; 20]);
        hs.extend([9u8; 20]);
        assert_eq!(hs.len(), 68);
        c.write_all(&hs[..48]).await.unwrap();
        c.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        c.write_all(&hs[48..]).await.unwrap();
        assert_eq!(
            peer.await.unwrap(),
            68,
            "the peer's first read held the whole handshake"
        );
    }

    #[test]
    fn handshakes_name_their_torrent_and_anything_else_does_not() {
        let mut hs = PROTOCOL.to_vec();
        hs.extend([0u8; 8]);
        hs.extend((0u8..20).collect::<Vec<_>>());
        hs.extend([9u8; 20]);
        assert_eq!(
            handshake_info_hash(&hs).as_deref(),
            Some("000102030405060708090a0b0c0d0e0f10111213")
        );
        assert_eq!(handshake_info_hash(&hs[..47]), None, "truncated");
        assert_eq!(
            handshake_info_hash(b"GET /announce?info_hash=... HTTP/1.1\r\n\r\n"),
            None
        );
        let mut wrong = hs.clone();
        wrong[0] = 18;
        assert_eq!(handshake_info_hash(&wrong), None);
    }

    #[test]
    fn secrets_compare_fully() {
        assert!(constant_eq(b"abc", b"abc"));
        assert!(!constant_eq(b"abc", b"abd"));
        assert!(!constant_eq(b"abc", b"ab"));
    }
}
