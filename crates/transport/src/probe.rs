//! Is this network really online, or behind a sign-in page (captive portal)?
//! Hotel and café Wi-Fi answer every plain-HTTP request with their login page
//! until you sign in, so downloads over it fail in confusing ways (L-62, 2.15).
//!
//! The check asks Fuselane's own site (GitHub Pages, already used for updates),
//! never Apple or Google: `http://arshpunisher.github.io/fuselane/probe` is always
//! answered with a permanent redirect to the same path over https. Anything else
//! that answers is something in the way; no answer at all means offline.

use std::net::SocketAddr;
use std::time::Duration;

use fuselane_netif::Interface;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const HOST: &str = "arshpunisher.github.io";
pub const PATH: &str = "/fuselane/probe";
/// Headers longer than this mean it isn't the short redirect we expect.
const MAX_HEAD: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    Online,
    /// Something answered in place of the real site: usually a sign-in page.
    /// `location` is where it wanted to send the browser, when it said.
    Portal {
        location: Option<String>,
    },
    /// Nothing answered (no route, refused, timed out).
    Offline(String),
}

/// Judges the response head (status line and headers).
pub fn classify(head: &[u8], expected_location: &str) -> Reach {
    let portal = |location: Option<String>| Reach::Portal { location };
    let Ok(text) = std::str::from_utf8(head) else {
        return portal(None);
    };
    let mut lines = text.split("\r\n");
    let status = lines.next().unwrap_or_default();
    let code = status.split(' ').nth(1).and_then(|c| c.parse::<u16>().ok());
    let location = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("location"))
        .map(|(_, v)| v.trim().to_owned());
    match (status.starts_with("HTTP/1."), code, location) {
        (true, Some(301 | 308), Some(loc)) if loc == expected_location => Reach::Online,
        (_, _, loc) => portal(loc.filter(|l| l.len() <= 2048)),
    }
}

async fn ask(
    iface: &Interface,
    addr: SocketAddr,
    host: &str,
    path: &str,
    wait: Duration,
) -> Result<Vec<u8>, String> {
    let mut s = crate::connect_pinned(iface, addr, wait)
        .await
        .map_err(|e| e.to_string())?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: Fuselane\r\nConnection: close\r\n\r\n"
    );
    tokio::time::timeout(wait, async {
        s.write_all(req.as_bytes()).await?;
        let mut head = Vec::new();
        let mut buf = [0u8; 1024];
        while head.len() < MAX_HEAD && !head.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = s.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            head.extend_from_slice(&buf[..n]);
        }
        Ok::<_, std::io::Error>(head)
    })
    .await
    .map_err(|_| "no answer in time".to_string())?
    .map_err(|e| e.to_string())
}

/// Checks one network against `host` at `addrs` (already looked up).
pub async fn check_at(
    iface: &Interface,
    addrs: &[SocketAddr],
    host: &str,
    path: &str,
    wait: Duration,
) -> Reach {
    let expected = format!("https://{host}{path}");
    let mut last = "no address for the check".to_string();
    // Only addresses this network can reach (its own family).
    for addr in addrs
        .iter()
        .filter(|a| a.ip().is_loopback() || iface.addrs.iter().any(|i| i.is_ipv4() == a.is_ipv4()))
    {
        match ask(iface, *addr, host, path, wait).await {
            Ok(head) if head.is_empty() => last = "closed without answering".into(),
            Ok(head) => {
                let end = head
                    .windows(4)
                    .position(|w| w == b"\r\n\r\n")
                    .map_or(head.len(), |p| p + 4);
                return classify(&head[..end], &expected);
            }
            Err(e) => last = e,
        }
    }
    Reach::Offline(last)
}

/// Checks one network against Fuselane's site.
pub async fn check(iface: &Interface, wait: Duration) -> Reach {
    let addrs: Vec<SocketAddr> =
        match tokio::time::timeout(wait, tokio::net::lookup_host((HOST, 80))).await {
            Ok(Ok(a)) => a.collect(),
            _ => return Reach::Offline("couldn't look up Fuselane's site".into()),
        };
    check_at(iface, &addrs, HOST, PATH, wait).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::net::TcpListener;

    const EXPECT: &str = "https://example.test/fuselane/probe";

    #[test]
    fn only_the_exact_redirect_counts_as_online() {
        let ok = format!(
            "HTTP/1.1 301 Moved Permanently\r\nServer: GitHub.com\r\nLocation: {EXPECT}\r\n\r\n"
        );
        assert_eq!(classify(ok.as_bytes(), EXPECT), Reach::Online);
        let ok308 = format!("HTTP/1.1 308 Permanent Redirect\r\nlocation:{EXPECT}\r\n\r\n");
        assert_eq!(classify(ok308.as_bytes(), EXPECT), Reach::Online);
        // A portal's login page, its own redirect, a look-alike, or garbage.
        assert_eq!(
            classify(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n",
                EXPECT
            ),
            Reach::Portal { location: None }
        );
        assert_eq!(
            classify(
                b"HTTP/1.1 302 Found\r\nLocation: http://login.hotel.test/\r\n\r\n",
                EXPECT
            ),
            Reach::Portal {
                location: Some("http://login.hotel.test/".into())
            }
        );
        let lookalike = format!("HTTP/1.1 301 Moved\r\nLocation: {EXPECT}.evil.test\r\n\r\n");
        assert!(matches!(
            classify(lookalike.as_bytes(), EXPECT),
            Reach::Portal { .. }
        ));
        let wrong_code = format!("HTTP/1.1 302 Found\r\nLocation: {EXPECT}\r\n\r\n");
        assert!(matches!(
            classify(wrong_code.as_bytes(), EXPECT),
            Reach::Portal { .. }
        ));
        assert!(matches!(
            classify(b"\xff\xfe garbage", EXPECT),
            Reach::Portal { location: None }
        ));
        assert!(matches!(
            classify(b"SSH-2.0-OpenSSH\r\n\r\n", EXPECT),
            Reach::Portal { .. }
        ));
    }

    fn lo() -> Interface {
        Interface {
            name: "lo0".into(),
            display_name: "Loopback".into(),
            index: 1,
            kind: fuselane_netif::Kind::Loopback,
            addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        }
    }

    /// A server that answers the first request with `reply` (None: never answers).
    async fn server(reply: Option<Vec<u8>>) -> SocketAddr {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let reply = reply.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let _ = s.read(&mut buf).await;
                    match reply {
                        Some(r) => {
                            let _ = s.write_all(&r).await;
                        }
                        None => tokio::time::sleep(Duration::from_secs(30)).await,
                    }
                });
            }
        });
        addr
    }

    async fn probe(reply: Option<Vec<u8>>) -> Reach {
        let addr = server(reply).await;
        check_at(
            &lo(),
            &[addr],
            "example.test",
            "/fuselane/probe",
            Duration::from_millis(800),
        )
        .await
    }

    #[tokio::test]
    async fn real_answers_are_judged_over_the_wire() {
        let ok = format!(
            "HTTP/1.1 301 Moved Permanently\r\nLocation: {EXPECT}\r\nContent-Length: 0\r\n\r\n"
        );
        assert_eq!(probe(Some(ok.into_bytes())).await, Reach::Online);
        let portal =
            b"HTTP/1.1 302 Found\r\nLocation: http://10.0.0.1/login\r\n\r\n<html>".to_vec();
        assert_eq!(
            probe(Some(portal)).await,
            Reach::Portal {
                location: Some("http://10.0.0.1/login".into())
            }
        );
        // Endless headers stop at the cap and count as something in the way.
        let mut huge = b"HTTP/1.1 200 OK\r\n".to_vec();
        huge.extend(std::iter::repeat_n(b'x', 64 * 1024));
        assert!(matches!(probe(Some(huge)).await, Reach::Portal { .. }));
    }

    #[tokio::test]
    async fn silence_refusal_and_no_address_are_offline() {
        assert!(matches!(probe(None).await, Reach::Offline(m) if m.contains("no answer")));
        let dead: SocketAddr = "127.0.0.1:9".parse().unwrap();
        assert!(matches!(
            check_at(
                &lo(),
                &[dead],
                "example.test",
                "/p",
                Duration::from_millis(500)
            )
            .await,
            Reach::Offline(_)
        ));
        assert!(matches!(
            check_at(&lo(), &[], "example.test", "/p", Duration::from_millis(500)).await,
            Reach::Offline(_)
        ));
        assert!(matches!(probe(Some(Vec::new())).await, Reach::Offline(m) if m.contains("closed")));
    }

    #[tokio::test]
    #[ignore = "uses the real network; run with --ignored"]
    async fn every_usable_network_here_is_online() {
        for iface in fuselane_netif::usable().unwrap() {
            let r = check(&iface, Duration::from_secs(5)).await;
            println!("{}: {r:?}", iface.name);
            assert_eq!(r, Reach::Online, "{}", iface.name);
        }
    }
}
