//! Finding devices: LocalSend's multicast announcements on UDP 224.0.0.167:53317,
//! joined on each local IPv4 address the app hands over (Wi-Fi, Ethernet).

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use crate::proto::{self, Announcement, MULTICAST, PORT};

/// A multicast listener and announcer.
#[derive(Debug)]
pub struct Discovery {
    sock: tokio::net::UdpSocket,
    port: u16,
    ifaces: Vec<Ipv4Addr>,
}

impl Discovery {
    /// Joins the group on each of `ifaces` (port 53317 unless `port` says
    /// otherwise; tests use their own).
    pub fn start(ifaces: &[Ipv4Addr], port: u16) -> std::io::Result<Discovery> {
        use socket2::{Domain, Protocol, Socket, Type};
        let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        s.set_reuse_address(true)?;
        #[cfg(all(unix, not(target_os = "solaris")))]
        s.set_reuse_port(true)?;
        s.bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())?;
        s.set_multicast_loop_v4(true)?;
        s.set_multicast_ttl_v4(1)?;
        let mut joined = vec![];
        for ip in ifaces {
            if s.join_multicast_v4(&MULTICAST, ip).is_ok() {
                joined.push(*ip);
            }
        }
        if joined.is_empty() && !ifaces.is_empty() {
            return Err(std::io::Error::other(
                "couldn't listen for devices on any network",
            ));
        }
        s.set_nonblocking(true)?;
        let sock = tokio::net::UdpSocket::from_std(s.into())?;
        Ok(Discovery {
            sock,
            port,
            ifaces: joined,
        })
    }

    /// Sends `a` on every joined network.
    pub async fn announce(&self, a: &Announcement) -> std::io::Result<()> {
        let body = serde_json::to_vec(a).map_err(std::io::Error::other)?;
        let to = SocketAddr::V4(SocketAddrV4::new(MULTICAST, self.port));
        let sock = socket2::SockRef::from(&self.sock);
        for ip in &self.ifaces {
            sock.set_multicast_if_v4(ip)?;
            let _ = self.sock.send_to(&body, to).await;
        }
        Ok(())
    }

    /// The next valid announcement and where it came from.
    pub async fn recv(&self) -> std::io::Result<(SocketAddr, Announcement)> {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            if let Ok(a) = proto::parse_announcement(&buf[..n]) {
                return Ok((from, a));
            }
        }
    }

    pub fn default_port() -> u16 {
        PORT
    }
}
