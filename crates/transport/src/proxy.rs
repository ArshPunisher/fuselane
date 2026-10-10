//! Reaching a server through a proxy the person set for one network (STEPS 8.4):
//! an HTTP proxy (CONNECT) or SOCKS5 (RFC 1928, login RFC 1929).
//!
//! - The connection to the proxy is pinned to the network like any other
//!   connection (`connect_pinned`), so each network keeps its own proxy.
//! - The proxy only opens a tunnel to `host:port`. HTTPS stays end-to-end TLS
//!   through it (the certificate is checked against the server's name), so the
//!   proxy sees where you connect, never what you ask for.
//! - The server's name is handed to the proxy to resolve, which may be the only
//!   thing that can, and keeps the network's own DNS out of it.
//! - System proxy settings are never read (L-66): only a proxy set here is used.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use base64::Engine as _;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::{Interface, connect_pinned};

/// Which protocol the proxy speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    /// An HTTP proxy, asked with CONNECT for a tunnel.
    Http,
    Socks5,
}

impl ProxyKind {
    /// How people name it.
    pub fn word(self) -> &'static str {
        match self {
            ProxyKind::Http => "HTTP",
            ProxyKind::Socks5 => "SOCKS5",
        }
    }
}

/// A username and password for the proxy. Never printed.
#[derive(Clone, PartialEq, Eq)]
pub struct Login {
    pub user: String,
    pub password: String,
}

impl std::fmt::Debug for Login {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Login(hidden)")
    }
}

/// A proxy for one network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proxy {
    pub kind: ProxyKind,
    /// A name or an IP address (without brackets).
    pub host: String,
    pub port: u16,
    pub login: Option<Login>,
}

impl Proxy {
    /// `host:port`, with brackets around an IPv6 address.
    pub fn address(&self) -> String {
        authority(&self.host, self.port)
    }
}

/// How going through a proxy failed. Each has a plain message in
/// `fuselane_core::proxy::problem`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProxyError {
    #[error("the proxy's name couldn't be found")]
    Lookup,
    #[error("couldn't connect to the proxy: {0}")]
    Unreachable(String),
    #[error("the proxy wants a username and password")]
    LoginNeeded,
    #[error("the proxy turned down the username and password")]
    LoginRefused,
    #[error("the proxy refused to connect to {target}")]
    Refused { target: String },
    #[error("the proxy couldn't reach {target}")]
    TargetUnreachable { target: String },
    #[error("it doesn't answer like a {kind} proxy")]
    NotAProxy { kind: &'static str },
    #[error("the proxy wants a sign-in method Fuselane doesn't support")]
    Unsupported,
    #[error("the proxy didn't answer in time")]
    Timeout,
}

impl ProxyError {
    /// Trying again won't help until the person changes something (the login,
    /// the type, the proxy's rules), so a download stops asking this proxy.
    pub fn lasting(&self) -> bool {
        matches!(
            self,
            ProxyError::LoginNeeded
                | ProxyError::LoginRefused
                | ProxyError::Refused { .. }
                | ProxyError::NotAProxy { .. }
                | ProxyError::Unsupported
        )
    }
}

/// Longest HTTP answer head accepted from a proxy.
const MAX_HEAD: usize = 16 * 1024;

fn authority(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// Opens a tunnel to `host:port` through `proxy`, over a connection pinned to
/// `iface`, all within `timeout`. The stream that comes back talks to the server.
pub async fn connect(
    proxy: &Proxy,
    iface: &Interface,
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<TcpStream, ProxyError> {
    let deadline = tokio::time::Instant::now() + timeout;
    let addrs: Vec<SocketAddr> = match proxy.host.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, proxy.port)],
        Err(_) => tokio::time::timeout_at(
            deadline,
            tokio::net::lookup_host((proxy.host.as_str(), proxy.port)),
        )
        .await
        .map_err(|_| ProxyError::Timeout)?
        .map_err(|_| ProxyError::Lookup)?
        .collect(),
    };
    // Addresses in a family this network has (a proxy on this computer always works).
    let usable: Vec<SocketAddr> = addrs
        .iter()
        .copied()
        .filter(|a| a.ip().is_loopback() || iface.addrs.iter().any(|l| l.is_ipv4() == a.is_ipv4()))
        .collect();
    if usable.is_empty() {
        return Err(if addrs.is_empty() {
            ProxyError::Lookup
        } else {
            ProxyError::Unreachable(format!(
                "{} has no address in the proxy's IP family",
                iface.display_name
            ))
        });
    }
    let mut last = ProxyError::Timeout;
    for addr in usable {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            break;
        }
        match connect_pinned(iface, addr, left).await {
            Ok(mut tcp) => {
                return match tokio::time::timeout_at(
                    deadline,
                    handshake(&mut tcp, proxy, host, port),
                )
                .await
                {
                    Err(_) => Err(ProxyError::Timeout),
                    Ok(r) => r.map(|()| tcp),
                };
            }
            Err(crate::TransportError::Timeout { .. }) => last = ProxyError::Timeout,
            Err(e) => last = ProxyError::Unreachable(short_reason(&e)),
        }
    }
    Err(last)
}

/// The useful part of a connect error ("connection refused").
fn short_reason(e: &crate::TransportError) -> String {
    match e {
        crate::TransportError::Io(io) => match io.kind() {
            std::io::ErrorKind::ConnectionRefused => "connection refused".into(),
            std::io::ErrorKind::HostUnreachable | std::io::ErrorKind::NetworkUnreachable => {
                "no route to it".into()
            }
            _ => io.to_string(),
        },
        other => other.to_string(),
    }
}

/// Asks the proxy on `io` for a tunnel to `host:port`. On success the stream
/// carries the server's bytes, with nothing of the proxy's answer left in it.
pub async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    io: &mut S,
    proxy: &Proxy,
    host: &str,
    port: u16,
) -> Result<(), ProxyError> {
    let host = host.trim_matches(['[', ']']);
    match proxy.kind {
        ProxyKind::Http => http_connect(io, proxy, host, port).await,
        ProxyKind::Socks5 => socks5_connect(io, proxy, host, port).await,
    }
}

/// A read or write that failed half-way: the proxy hung up or broke the line.
fn cut(e: &std::io::Error) -> ProxyError {
    ProxyError::Unreachable(match e.kind() {
        std::io::ErrorKind::UnexpectedEof => "it closed the connection without answering".into(),
        _ => e.to_string(),
    })
}

async fn http_connect<S: AsyncRead + AsyncWrite + Unpin>(
    io: &mut S,
    proxy: &Proxy,
    host: &str,
    port: u16,
) -> Result<(), ProxyError> {
    let target = authority(host, port);
    let mut req =
        format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\nUser-Agent: Fuselane\r\n");
    if let Some(l) = &proxy.login {
        let token =
            base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", l.user, l.password));
        req.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    req.push_str("\r\n");
    // One write: some proxies read the request in a single go (L-123).
    io.write_all(req.as_bytes()).await.map_err(|e| cut(&e))?;
    io.flush().await.map_err(|e| cut(&e))?;
    // Byte by byte up to the blank line, so no tunnel byte is read by mistake.
    let mut head = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= MAX_HEAD {
            return Err(ProxyError::NotAProxy { kind: "HTTP" });
        }
        match io.read(&mut byte).await {
            Ok(0) if head.is_empty() => {
                return Err(ProxyError::Unreachable(
                    "it closed the connection without answering".into(),
                ));
            }
            Ok(0) => return Err(ProxyError::NotAProxy { kind: "HTTP" }),
            Ok(_) => head.push(byte[0]),
            Err(e) => return Err(cut(&e)),
        }
        if head.len() == 5 && !head.starts_with(b"HTTP/") {
            return Err(ProxyError::NotAProxy { kind: "HTTP" });
        }
    }
    let status = std::str::from_utf8(&head)
        .ok()
        .and_then(|h| h.split(' ').nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or(ProxyError::NotAProxy { kind: "HTTP" })?;
    match status {
        200..=299 => Ok(()),
        407 if proxy.login.is_some() => Err(ProxyError::LoginRefused),
        407 => Err(ProxyError::LoginNeeded),
        502..=504 => Err(ProxyError::TargetUnreachable { target }),
        _ => Err(ProxyError::Refused { target }),
    }
}

async fn socks5_connect<S: AsyncRead + AsyncWrite + Unpin>(
    io: &mut S,
    proxy: &Proxy,
    host: &str,
    port: u16,
) -> Result<(), ProxyError> {
    let not_socks = ProxyError::NotAProxy { kind: "SOCKS5" };
    // Greeting: no login, and username/password when we have one.
    let hello: &[u8] = if proxy.login.is_some() {
        &[5, 2, 0, 2]
    } else {
        &[5, 1, 0]
    };
    io.write_all(hello).await.map_err(|e| cut(&e))?;
    let mut choice = [0u8; 2];
    read_exact(io, &mut choice).await?;
    if choice[0] != 5 {
        return Err(not_socks);
    }
    match (choice[1], &proxy.login) {
        (0, _) => {}
        (2, Some(l)) => {
            let (u, p) = (l.user.as_bytes(), l.password.as_bytes());
            let (Ok(ul), Ok(pl)) = (u8::try_from(u.len()), u8::try_from(p.len())) else {
                return Err(ProxyError::LoginRefused); // over 255 bytes: no SOCKS5 proxy accepts it
            };
            let mut msg = Vec::with_capacity(3 + u.len() + p.len());
            msg.extend_from_slice(&[1, ul]);
            msg.extend_from_slice(u);
            msg.push(pl);
            msg.extend_from_slice(p);
            io.write_all(&msg).await.map_err(|e| cut(&e))?;
            let mut status = [0u8; 2];
            read_exact(io, &mut status).await?;
            if status[1] != 0 {
                return Err(ProxyError::LoginRefused);
            }
        }
        (0xFF, None) => return Err(ProxyError::LoginNeeded),
        (0xFF, Some(_)) => return Err(ProxyError::Unsupported),
        _ => return Err(not_socks),
    }
    // The request: the name goes to the proxy to resolve; addresses as they are.
    let mut req = vec![5, 1, 0];
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => {
            req.push(1);
            req.extend_from_slice(&a.octets());
        }
        Ok(IpAddr::V6(a)) => {
            req.push(4);
            req.extend_from_slice(&a.octets());
        }
        Err(_) => {
            let name = host.as_bytes();
            let len = u8::try_from(name.len()).map_err(|_| ProxyError::Refused {
                target: authority(host, port),
            })?;
            req.push(3);
            req.push(len);
            req.extend_from_slice(name);
        }
    }
    req.extend_from_slice(&port.to_be_bytes());
    io.write_all(&req).await.map_err(|e| cut(&e))?;
    let mut reply = [0u8; 4];
    read_exact(io, &mut reply).await?;
    if reply[0] != 5 {
        return Err(not_socks);
    }
    let target = authority(host, port);
    match reply[1] {
        0 => {}
        2 => return Err(ProxyError::Refused { target }),
        1 | 3..=6 => return Err(ProxyError::TargetUnreachable { target }),
        7 | 8 => return Err(ProxyError::Unsupported),
        _ => return Err(not_socks),
    }
    // Skip the bound address so the stream starts at the server's first byte.
    let rest = match reply[3] {
        1 => 4 + 2,
        4 => 16 + 2,
        3 => {
            let mut len = [0u8; 1];
            read_exact(io, &mut len).await?;
            usize::from(len[0]) + 2
        }
        _ => return Err(not_socks),
    };
    let mut skip = vec![0u8; rest];
    read_exact(io, &mut skip).await
}

async fn read_exact<S: AsyncRead + Unpin>(io: &mut S, buf: &mut [u8]) -> Result<(), ProxyError> {
    io.read_exact(buf).await.map(|_| ()).map_err(|e| cut(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuselane_testkit::{ProxyKind as Kind, TestProxy};

    fn proxy(kind: ProxyKind, at: SocketAddr, login: Option<(&str, &str)>) -> Proxy {
        Proxy {
            kind,
            host: at.ip().to_string(),
            port: at.port(),
            login: login.map(|(u, p)| Login {
                user: u.into(),
                password: p.into(),
            }),
        }
    }

    fn lo() -> Interface {
        Interface {
            name: "lo0".into(),
            display_name: "Test network".into(),
            index: 0,
            kind: fuselane_netif::Kind::Ethernet,
            addrs: vec![IpAddr::from([127, 0, 0, 1])],
        }
    }

    /// A server that greets each connection with `hello` and echoes what it gets.
    async fn echo(hello: &'static [u8]) -> SocketAddr {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = l.accept().await else {
                    continue;
                };
                tokio::spawn(async move {
                    let _ = s.write_all(hello).await;
                    let mut buf = [0u8; 1024];
                    while let Ok(n) = s.read(&mut buf).await {
                        if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        addr
    }

    async fn through(p: &Proxy, target: SocketAddr) -> Result<TcpStream, ProxyError> {
        connect(p, &lo(), "localhost", target.port(), Duration::from_secs(5)).await
    }

    /// The tunnel reaches the server: its greeting comes first, then our echo.
    async fn assert_tunnel(mut s: TcpStream) {
        let mut hello = [0u8; 5];
        s.read_exact(&mut hello).await.unwrap();
        assert_eq!(
            &hello, b"hello",
            "the proxy's answer leaked into the stream"
        );
        s.write_all(b"ping").await.unwrap();
        let mut back = [0u8; 4];
        s.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"ping");
    }

    #[tokio::test]
    async fn both_kinds_tunnel_to_the_server_with_and_without_a_login() {
        let target = echo(b"hello").await;
        for (kind, k) in [
            (ProxyKind::Http, Kind::Http),
            (ProxyKind::Socks5, Kind::Socks5),
        ] {
            for login in [None, Some(("ann", "s3cret:with colon"))] {
                let tp = TestProxy::start(k, login).await.unwrap();
                let s = through(&proxy(kind, tp.addr(), login), target)
                    .await
                    .unwrap();
                assert_tunnel(s).await;
                // The name went to the proxy to resolve, not an address.
                assert_eq!(tp.targets(), vec![format!("localhost:{}", target.port())]);
                assert!(tp.relayed() >= 5, "{kind:?}");
            }
        }
    }

    #[tokio::test]
    async fn a_missing_or_wrong_login_is_told_apart() {
        let target = echo(b"hello").await;
        for (kind, k) in [
            (ProxyKind::Http, Kind::Http),
            (ProxyKind::Socks5, Kind::Socks5),
        ] {
            let tp = TestProxy::start(k, Some(("ann", "right"))).await.unwrap();
            let none = through(&proxy(kind, tp.addr(), None), target).await;
            assert_eq!(none.unwrap_err(), ProxyError::LoginNeeded, "{kind:?}");
            let wrong = through(&proxy(kind, tp.addr(), Some(("ann", "wrong"))), target).await;
            let e = wrong.unwrap_err();
            assert_eq!(e, ProxyError::LoginRefused, "{kind:?}");
            assert!(e.lasting());
            assert_eq!(tp.bad_logins(), 1);
        }
    }

    #[tokio::test]
    async fn a_proxy_that_refuses_the_place_or_cant_reach_it_says_so() {
        let target = echo(b"hello").await;
        let closed = {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            l.local_addr().unwrap() // dropped: nothing listens there now
        };
        for (kind, k) in [
            (ProxyKind::Http, Kind::Http),
            (ProxyKind::Socks5, Kind::Socks5),
        ] {
            let tp = TestProxy::start(k, None).await.unwrap();
            tp.refuse_port(target.port());
            let p = proxy(kind, tp.addr(), None);
            let target_name = format!("localhost:{}", target.port());
            assert_eq!(
                through(&p, target).await.unwrap_err(),
                ProxyError::Refused {
                    target: target_name
                },
                "{kind:?}"
            );
            let e = through(&p, closed).await.unwrap_err();
            assert!(
                matches!(e, ProxyError::TargetUnreachable { .. }),
                "{kind:?}: {e:?}"
            );
            assert!(!e.lasting(), "a server that's down may come back");
        }
    }

    #[tokio::test]
    async fn the_wrong_kind_of_proxy_is_named() {
        let target = echo(b"hello").await;
        let http = TestProxy::start(Kind::Http, None).await.unwrap();
        let socks = TestProxy::start(Kind::Socks5, None).await.unwrap();
        // An HTTP proxy waits for the rest of a request that never comes.
        let e = connect(
            &proxy(ProxyKind::Socks5, http.addr(), None),
            &lo(),
            "localhost",
            target.port(),
            Duration::from_millis(500),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(
                e,
                ProxyError::NotAProxy { kind: "SOCKS5" }
                    | ProxyError::Unreachable(_)
                    | ProxyError::Timeout
            ),
            "{e:?}"
        );
        let e = through(&proxy(ProxyKind::Http, socks.addr(), None), target)
            .await
            .unwrap_err();
        assert!(
            matches!(
                e,
                ProxyError::NotAProxy { kind: "HTTP" } | ProxyError::Unreachable(_)
            ),
            "{e:?}"
        );
        // A plain web server isn't a proxy either.
        let web = echo(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        let e = through(&proxy(ProxyKind::Socks5, web, None), target)
            .await
            .unwrap_err();
        assert_eq!(e, ProxyError::NotAProxy { kind: "SOCKS5" });
    }

    #[tokio::test]
    async fn no_proxy_listening_is_unreachable_and_a_silent_one_times_out() {
        let target = echo(b"hello").await;
        let closed = {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            l.local_addr().unwrap()
        };
        let e = through(&proxy(ProxyKind::Http, closed, None), target)
            .await
            .unwrap_err();
        assert!(matches!(e, ProxyError::Unreachable(_)), "{e:?}");
        assert!(!e.lasting());
        // Accepts, then never says a word.
        let silent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let at = silent.local_addr().unwrap();
        let _keep = tokio::spawn(async move {
            let mut held = vec![];
            while let Ok((s, _)) = silent.accept().await {
                held.push(s);
            }
        });
        for kind in [ProxyKind::Http, ProxyKind::Socks5] {
            let started = std::time::Instant::now();
            let e = connect(
                &proxy(kind, at, None),
                &lo(),
                "localhost",
                443,
                Duration::from_millis(300),
            )
            .await
            .unwrap_err();
            assert_eq!(e, ProxyError::Timeout);
            assert!(started.elapsed() < Duration::from_secs(2));
        }
    }

    #[tokio::test]
    async fn an_unknown_proxy_name_is_a_lookup_failure() {
        let p = Proxy {
            kind: ProxyKind::Http,
            host: "no-such-proxy.invalid".into(),
            port: 8080,
            login: None,
        };
        let e = connect(&p, &lo(), "example.com", 443, Duration::from_secs(5))
            .await
            .unwrap_err();
        assert!(
            matches!(e, ProxyError::Lookup | ProxyError::Timeout),
            "{e:?}"
        );
    }

    #[tokio::test]
    async fn ip_addresses_are_sent_as_addresses() {
        let target = echo(b"hello").await;
        for k in [Kind::Http, Kind::Socks5] {
            let tp = TestProxy::start(k, None).await.unwrap();
            let kind = if k == Kind::Http {
                ProxyKind::Http
            } else {
                ProxyKind::Socks5
            };
            let s = connect(
                &proxy(kind, tp.addr(), None),
                &lo(),
                "127.0.0.1",
                target.port(),
                Duration::from_secs(5),
            )
            .await
            .unwrap();
            assert_tunnel(s).await;
            assert_eq!(tp.targets(), vec![format!("127.0.0.1:{}", target.port())]);
        }
    }

    #[test]
    fn ipv6_places_are_bracketed() {
        assert_eq!(authority("2001:db8::1", 443), "[2001:db8::1]:443");
        assert_eq!(authority("example.com", 80), "example.com:80");
        let p = Proxy {
            kind: ProxyKind::Socks5,
            host: "::1".into(),
            port: 1080,
            login: None,
        };
        assert_eq!(p.address(), "[::1]:1080");
    }

    #[test]
    fn a_login_is_never_printed() {
        let p = proxy(
            ProxyKind::Http,
            "127.0.0.1:8080".parse().unwrap(),
            Some(("ann", "hunter2")),
        );
        let shown = format!("{p:?}");
        assert!(
            !shown.contains("hunter2") && !shown.contains("ann"),
            "{shown}"
        );
    }
}
