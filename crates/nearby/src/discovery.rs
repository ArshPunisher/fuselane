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

    /// The local addresses this listener joined the group on.
    pub fn ifaces(&self) -> &[Ipv4Addr] {
        &self.ifaces
    }

    /// Sends `a` on every joined network. A network whose address has gone
    /// (Wi-Fi changed, the computer slept) is skipped rather than stopping the
    /// others; an error only when it went out on none.
    pub async fn announce(&self, a: &Announcement) -> std::io::Result<()> {
        let body = serde_json::to_vec(a).map_err(std::io::Error::other)?;
        let to = SocketAddr::V4(SocketAddrV4::new(MULTICAST, self.port));
        let sock = socket2::SockRef::from(&self.sock);
        let mut last = None;
        let mut sent = false;
        for ip in &self.ifaces {
            if let Err(e) = sock.set_multicast_if_v4(ip) {
                last = Some(e);
                continue;
            }
            match self.sock.send_to(&body, to).await {
                Ok(_) => sent = true,
                Err(e) => last = Some(e),
            }
        }
        match last {
            Some(e) if !sent => Err(e),
            _ => Ok(()),
        }
    }

    /// The next valid announcement and where it came from. Windows reports an
    /// earlier send's ICMP "port unreachable" as a reset on the next receive;
    /// that says nothing about this socket, so listening goes on.
    pub async fn recv(&self) -> std::io::Result<(SocketAddr, Announcement)> {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let (n, from) = match self.sock.recv_from(&mut buf).await {
                Ok(x) => x,
                Err(e) if passing(&e) => continue,
                Err(e) => return Err(e),
            };
            if let Ok(a) = proto::parse_announcement(&buf[..n]) {
                return Ok((from, a));
            }
        }
    }

    pub fn default_port() -> u16 {
        PORT
    }
}

/// Receive errors that don't mean the socket is broken.
fn passing(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::{ConnectionRefused, ConnectionReset, Interrupted, WouldBlock};
    matches!(
        e.kind(),
        ConnectionReset | ConnectionRefused | Interrupted | WouldBlock
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reset_from_an_earlier_send_does_not_stop_listening() {
        use std::io::{Error, ErrorKind};
        assert!(passing(&Error::from(ErrorKind::ConnectionReset)));
        assert!(passing(&Error::from(ErrorKind::Interrupted)));
        assert!(!passing(&Error::from(ErrorKind::NotConnected)));
        assert!(!passing(&Error::other("socket closed")));
    }
}
