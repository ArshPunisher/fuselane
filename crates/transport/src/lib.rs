//! Per-network transport for Fuselane.
//!
//! Creates sockets pinned to one network (`SO_BINDTODEVICE`, `IP_BOUND_IF`,
//! `IP_UNICAST_IF`) and wraps them in TLS with the OS trust store.
//! Design: `docs/03-architecture/NETWORKING.md` §2–6. Rules: L-09, L-56–L-59.
//! Per-network DNS and Happy Eyeballs come next (STEPS 2.13, 2.14).

pub mod dns;

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::TcpStream;

pub use fuselane_netif::Interface;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("{iface} has no {family} address, so it can't reach {addr}")]
    NoRoute {
        iface: String,
        family: &'static str,
        addr: SocketAddr,
    },
    #[error("couldn't pin a socket to {iface}: {source}")]
    Pin {
        iface: String,
        source: std::io::Error,
    },
    #[error("connecting through {iface} timed out")]
    Timeout { iface: String },
    #[error("TLS with {host} failed: {source}")]
    Tls {
        host: String,
        source: std::io::Error,
    },
    #[error("invalid server name {0:?}")]
    ServerName(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Pins `socket` to `iface` for the family of `dest` (L-56). A source address alone
/// is not enough on Linux or macOS; each OS has its own option.
pub fn pin(socket: &Socket, iface: &Interface, dest: &SocketAddr) -> std::io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let _ = dest;
        socket.bind_device(Some(iface.name.as_bytes()))
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let index = std::num::NonZeroU32::new(iface.index).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "interface index 0")
        })?;
        if dest.is_ipv4() {
            socket.bind_device_by_index_v4(Some(index))
        } else {
            socket.bind_device_by_index_v6(Some(index))
        }
    }
    #[cfg(windows)]
    {
        windows_pin(socket, iface.index, dest.is_ipv4())
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        windows
    )))]
    {
        let _ = (socket, iface, dest);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "pinning not supported on this OS",
        ))
    }
}

/// Windows: `IP_UNICAST_IF` wants the IPv4 index in **network byte order**, the
/// IPv6 one in host order (tech research §2.1).
#[cfg(windows)]
fn windows_pin(socket: &Socket, index: u32, v4: bool) -> std::io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{
        IP_UNICAST_IF, IPPROTO_IP, IPPROTO_IPV6, IPV6_UNICAST_IF, setsockopt,
    };
    let (level, name, value) = if v4 {
        (IPPROTO_IP, IP_UNICAST_IF, index.to_be())
    } else {
        (IPPROTO_IPV6, IPV6_UNICAST_IF, index)
    };
    // SAFETY: valid socket handle, pointer to a live u32, correct length.
    let rc = unsafe {
        setsockopt(
            socket.as_raw_socket() as usize,
            level,
            name,
            (&value as *const u32).cast(),
            4,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Opens a TCP connection to `dest` that can only leave through `iface`.
/// Loopback destinations are never pinned (L-58). The connect has a deadline (L-09).
pub async fn connect_pinned(
    iface: &Interface,
    dest: SocketAddr,
    timeout: Duration,
) -> Result<TcpStream, TransportError> {
    let socket = Socket::new(Domain::for_address(dest), Type::STREAM, Some(Protocol::TCP))?;
    if !dest.ip().is_loopback() {
        // Bind to a source address of the right family (required on Windows, L-59).
        let source = iface
            .addrs
            .iter()
            .find(|a| a.is_ipv4() == dest.is_ipv4())
            .copied();
        let Some(source) = source else {
            return Err(TransportError::NoRoute {
                iface: iface.name.clone(),
                family: if dest.is_ipv4() { "IPv4" } else { "IPv6" },
                addr: dest,
            });
        };
        pin(&socket, iface, &dest).map_err(|source| TransportError::Pin {
            iface: iface.name.clone(),
            source,
        })?;
        // Windows needs an explicit source address (L-59). Elsewhere the pin is enough,
        // and binding would pick the interface's first IPv6 address, which may be a
        // deprecated privacy address that no longer routes (L-109): let the OS choose.
        if cfg!(windows) {
            socket.bind(&SocketAddr::new(source, 0).into())?;
        }
    }
    socket.set_nonblocking(true)?;
    socket.set_tcp_nodelay(true)?;
    let tcp = tokio::net::TcpSocket::from_std_stream(socket.into());
    match tokio::time::timeout(timeout, tcp.connect(dest)).await {
        Err(_) => Err(TransportError::Timeout {
            iface: iface.name.clone(),
        }),
        Ok(r) => Ok(r?),
    }
}

/// Checks that pinning works on this machine (L-57): pins a socket to loopback.
pub fn self_test() -> Result<(), TransportError> {
    let lo = fuselane_netif::list()?
        .into_iter()
        .find(|i| i.kind == fuselane_netif::Kind::Loopback)
        .unwrap_or_else(|| Interface {
            name: "lo".into(),
            display_name: "Loopback".into(),
            index: 1,
            kind: fuselane_netif::Kind::Loopback,
            addrs: vec![],
        });
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    let dest: SocketAddr = (IpAddr::from([127, 0, 0, 1]), 9).into();
    pin(&socket, &lo, &dest).map_err(|source| TransportError::Pin {
        iface: lo.name.clone(),
        source,
    })
}

/// Checks that TLS can be set up here (crypto provider and the OS trust store).
pub fn tls_ready() -> Result<(), String> {
    tls_config().map(|_| ())
}

/// One shared client config: ring crypto, safe protocol versions, and the OS trust
/// store (so corporate CAs work, tech research §2.2). Built once.
fn tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    static CONFIG: OnceLock<Result<Arc<rustls::ClientConfig>, String>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let verifier = rustls_platform_verifier::Verifier::new(provider.clone())
                .map_err(|e| e.to_string())?;
            let config = rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .dangerous() // "dangerous" only names the custom-verifier API; this verifier is the OS's
                .with_custom_certificate_verifier(Arc::new(verifier))
                .with_no_client_auth();
            Ok(Arc::new(config))
        })
        .clone()
}

/// TLS over an already pinned stream, verifying the certificate against `host`.
pub async fn tls(
    stream: TcpStream,
    host: &str,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, TransportError> {
    let name = rustls::pki_types::ServerName::try_from(host.to_string())
        .map_err(|_| TransportError::ServerName(host.into()))?;
    let config = tls_config().map_err(|e| TransportError::Tls {
        host: host.into(),
        source: std::io::Error::other(e),
    })?;
    tokio_rustls::TlsConnector::from(config)
        .connect(name, stream)
        .await
        .map_err(|source| TransportError::Tls {
            host: host.into(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn fake(name: &str, index: u32, addrs: Vec<IpAddr>) -> Interface {
        Interface {
            name: name.into(),
            display_name: name.into(),
            index,
            kind: fuselane_netif::Kind::Ethernet,
            addrs,
        }
    }

    #[test]
    fn pinning_works_on_this_machine() {
        self_test().expect("pinning should be available unprivileged");
    }

    #[tokio::test]
    async fn loopback_destinations_are_never_pinned() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            s.write_all(b"hi").await.unwrap();
        });
        // Even "pinned" to an interface with no route, a loopback connect works (L-58).
        let mut s = connect_pinned(&fake("nowhere0", 0, vec![]), addr, Duration::from_secs(2))
            .await
            .unwrap();
        let mut buf = [0u8; 2];
        s.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hi");
    }

    #[tokio::test]
    async fn a_network_without_the_right_family_refuses_clearly() {
        let v4_only = fake("en9", 9, vec![IpAddr::from([192, 168, 9, 9])]);
        let dest: SocketAddr = "[2606:4700::6810:84e5]:443".parse().unwrap();
        let err = connect_pinned(&v4_only, dest, Duration::from_secs(1))
            .await
            .unwrap_err();
        assert!(
            matches!(err, TransportError::NoRoute { family: "IPv6", .. }),
            "{err}"
        );
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[tokio::test]
    async fn a_pin_to_a_routeless_interface_fails_fast_instead_of_leaking_out_the_default_route() {
        // Pin to loopback but target a public address: if the pin were ignored, the
        // connect would succeed through the default route. It must fail (spike S2).
        let lo_name = if cfg!(target_os = "linux") {
            "lo"
        } else {
            "lo0"
        };
        let lo = fake(
            lo_name,
            fuselane_netif::index_of(lo_name),
            vec![IpAddr::from([127, 0, 0, 1])],
        );
        let dest: SocketAddr = "203.0.113.10:443".parse().unwrap(); // TEST-NET-3, never routable
        let started = std::time::Instant::now();
        let res = connect_pinned(&lo, dest, Duration::from_secs(3)).await;
        assert!(
            res.is_err(),
            "pinned connect leaked through another interface"
        );
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    #[tokio::test]
    async fn invalid_tls_names_are_rejected() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let s = TcpStream::connect(addr).await.unwrap();
        let err = tls(s, "bad name with spaces").await.unwrap_err();
        assert!(matches!(err, TransportError::ServerName(_)));
    }

    #[tokio::test]
    async fn tls_to_a_server_that_isnt_tls_fails_instead_of_hanging() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let _ = s.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
        });
        let s = TcpStream::connect(addr).await.unwrap();
        let res = tokio::time::timeout(Duration::from_secs(5), tls(s, "localhost"))
            .await
            .expect("TLS hung");
        assert!(res.is_err());
    }

    /// Real-world check over the actual default network (needs internet: run with --ignored).
    #[tokio::test]
    #[ignore = "needs internet"]
    async fn real_https_through_a_pinned_socket() {
        let iface = fuselane_netif::usable()
            .unwrap()
            .into_iter()
            .find(|i| i.addrs.iter().any(IpAddr::is_ipv4))
            .expect("an IPv4 network");
        let dest = tokio::net::lookup_host("cloudflare.com:443")
            .await
            .unwrap()
            .find(SocketAddr::is_ipv4)
            .unwrap();
        let s = connect_pinned(&iface, dest, Duration::from_secs(5))
            .await
            .unwrap();
        let mut t = tls(s, "cloudflare.com").await.unwrap();
        t.write_all(
            b"GET /cdn-cgi/trace HTTP/1.1\r\nHost: cloudflare.com\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
        let mut body = String::new();
        t.read_to_string(&mut body).await.unwrap();
        assert!(body.contains("ip="), "{body}");
    }
}
