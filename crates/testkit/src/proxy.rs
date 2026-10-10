//! Small proxies for tests (STEPS 8.4): an HTTP CONNECT proxy and a SOCKS5 one
//! (RFC 1928, login RFC 1929), each with an optional login. They log where
//! clients asked to go and count the bytes they relay, so a test can prove a
//! download really went through the proxy, and they refuse the way real ones
//! do (407, 403, 502; SOCKS reply codes) so error handling can be tested.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Which protocol a [`TestProxy`] speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Http,
    Socks5,
}

#[derive(Debug, Default)]
struct State {
    login: Option<(String, String)>,
    /// `host:port` each client asked for, in order.
    targets: Vec<String>,
    /// Bytes relayed from servers back to clients.
    relayed: u64,
    /// Ports the proxy's rules don't allow.
    refused_ports: Vec<u16>,
    /// Logins tried that were wrong.
    bad_logins: u32,
}

/// A running proxy on 127.0.0.1; stops when dropped.
#[derive(Debug)]
pub struct TestProxy {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TestProxy {
    /// Starts a proxy of `kind` that requires `login` when given.
    pub async fn start(kind: Kind, login: Option<(&str, &str)>) -> std::io::Result<TestProxy> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let state = Arc::new(Mutex::new(State {
            login: login.map(|(u, p)| (u.to_string(), p.to_string())),
            ..State::default()
        }));
        let st = state.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((client, _)) = listener.accept().await else {
                    continue;
                };
                let st = st.clone();
                tokio::spawn(async move {
                    let _ = match kind {
                        Kind::Http => http(client, st).await,
                        Kind::Socks5 => socks5(client, st).await,
                    };
                });
            }
        });
        Ok(TestProxy { addr, state, task })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Bytes relayed from servers back to clients so far.
    pub fn relayed(&self) -> u64 {
        self.lock().relayed
    }

    /// Every `host:port` clients asked for.
    pub fn targets(&self) -> Vec<String> {
        self.lock().targets.clone()
    }

    /// Wrong logins tried so far.
    pub fn bad_logins(&self) -> u32 {
        self.lock().bad_logins
    }

    /// Refuse tunnels to `port` from now on (like proxies that only allow 443).
    pub fn refuse_port(&self, port: u16) {
        self.lock().refused_ports.push(port);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn lock(state: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Connects to `host:port` the way a proxy would (names resolved here).
async fn reach(host: &str, port: u16) -> std::io::Result<TcpStream> {
    TcpStream::connect((host, port)).await
}

/// Copies both ways until either side closes, counting server-to-client bytes.
async fn relay(client: TcpStream, server: TcpStream, state: Arc<Mutex<State>>) {
    let (mut cr, mut cw) = client.into_split();
    let (mut sr, mut sw) = server.into_split();
    let up = async {
        let _ = tokio::io::copy(&mut cr, &mut sw).await;
        let _ = sw.shutdown().await;
    };
    let down = async {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = match sr.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            if cw.write_all(&buf[..n]).await.is_err() {
                break;
            }
            lock(&state).relayed += n as u64;
        }
        let _ = cw.shutdown().await;
    };
    tokio::join!(up, down);
}

async fn read_head<S: AsyncRead + Unpin>(s: &mut S) -> std::io::Result<String> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > 16 * 1024 || s.read(&mut byte).await? == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        head.push(byte[0]);
    }
    Ok(String::from_utf8_lossy(&head).into_owned())
}

async fn answer<S: AsyncWrite + Unpin>(
    s: &mut S,
    status: &str,
    extra: &str,
) -> std::io::Result<()> {
    s.write_all(format!("HTTP/1.1 {status}\r\n{extra}Content-Length: 0\r\n\r\n").as_bytes())
        .await
}

async fn http(mut client: TcpStream, state: Arc<Mutex<State>>) -> std::io::Result<()> {
    let head = read_head(&mut client).await?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (method, target) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    if method != "CONNECT" {
        return answer(&mut client, "405 Method Not Allowed", "").await;
    }
    let auth = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("proxy-authorization"))
        .map(|(_, v)| v.trim().to_string());
    let (host, port) = match target.rsplit_once(':') {
        Some((h, p)) => (
            h.trim_matches(['[', ']']).to_string(),
            p.parse::<u16>().unwrap_or(0),
        ),
        None => (String::new(), 0),
    };
    let (login, refused) = {
        let mut s = lock(&state);
        s.targets.push(target.to_string());
        (s.login.clone(), s.refused_ports.contains(&port))
    };
    if let Some((u, p)) = login {
        let want = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("{u}:{p}"))
        );
        if auth.as_deref() != Some(want.as_str()) {
            if auth.is_some() {
                lock(&state).bad_logins += 1;
            }
            return answer(
                &mut client,
                "407 Proxy Authentication Required",
                "Proxy-Authenticate: Basic realm=\"test\"\r\n",
            )
            .await;
        }
    }
    if refused {
        return answer(&mut client, "403 Forbidden", "").await;
    }
    let Ok(server) = reach(&host, port).await else {
        return answer(&mut client, "502 Bad Gateway", "").await;
    };
    client
        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
        .await?;
    relay(client, server, state).await;
    Ok(())
}

async fn socks5(mut client: TcpStream, state: Arc<Mutex<State>>) -> std::io::Result<()> {
    let mut hello = [0u8; 2];
    client.read_exact(&mut hello).await?;
    if hello[0] != 5 {
        return Ok(()); // not SOCKS5: hang up, like real servers do
    }
    let mut methods = vec![0u8; usize::from(hello[1])];
    client.read_exact(&mut methods).await?;
    let login = lock(&state).login.clone();
    let wanted = if login.is_some() { 2 } else { 0 };
    if !methods.contains(&wanted) {
        return client.write_all(&[5, 0xFF]).await;
    }
    client.write_all(&[5, wanted]).await?;
    if let Some((user, pass)) = login {
        let mut ver_len = [0u8; 2];
        client.read_exact(&mut ver_len).await?;
        let mut u = vec![0u8; usize::from(ver_len[1])];
        client.read_exact(&mut u).await?;
        let mut plen = [0u8; 1];
        client.read_exact(&mut plen).await?;
        let mut p = vec![0u8; usize::from(plen[0])];
        client.read_exact(&mut p).await?;
        if u != user.as_bytes() || p != pass.as_bytes() {
            lock(&state).bad_logins += 1;
            return client.write_all(&[1, 1]).await;
        }
        client.write_all(&[1, 0]).await?;
    }
    let mut req = [0u8; 4];
    client.read_exact(&mut req).await?;
    let host = match req[3] {
        1 => {
            let mut a = [0u8; 4];
            client.read_exact(&mut a).await?;
            IpAddr::V4(Ipv4Addr::from(a)).to_string()
        }
        4 => {
            let mut a = [0u8; 16];
            client.read_exact(&mut a).await?;
            IpAddr::V6(Ipv6Addr::from(a)).to_string()
        }
        3 => {
            let mut len = [0u8; 1];
            client.read_exact(&mut len).await?;
            let mut name = vec![0u8; usize::from(len[0])];
            client.read_exact(&mut name).await?;
            String::from_utf8_lossy(&name).into_owned()
        }
        _ => return client.write_all(&[5, 8, 0, 1, 0, 0, 0, 0, 0, 0]).await,
    };
    let mut port = [0u8; 2];
    client.read_exact(&mut port).await?;
    let port = u16::from_be_bytes(port);
    let refused = {
        let mut s = lock(&state);
        s.targets.push(format!("{host}:{port}"));
        s.refused_ports.contains(&port)
    };
    if req[1] != 1 {
        return client.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await;
    }
    if refused {
        return client.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).await;
    }
    let Ok(server) = reach(&host, port).await else {
        return client.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await;
    };
    // Bound address 0.0.0.0:0 (clients ignore it for CONNECT).
    client.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
    relay(client, server, state).await;
    Ok(())
}
