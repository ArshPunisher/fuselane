//! Remote control (8.7, ADR 0013): aria2 tools add and watch downloads through
//! `fuselane-rpc`. This side keeps the setting, starts and stops the endpoint,
//! answers for the download list, and turns status changes into aria2's
//! WebSocket notifications.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, Weak};

use serde::{Deserialize, Serialize};

use crate::service::{AddRequest, Service, UiError, UiEvent};
use fuselane_rpc::{Add, Event, Job, Notifier, Status};

const SETTING: &str = "remote_control";

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Saved {
    on: bool,
    lan: bool,
    port: u16,
    secret: String,
}

impl Default for Saved {
    fn default() -> Self {
        Saved {
            on: false,
            lan: false,
            port: fuselane_rpc::DEFAULT_PORT,
            secret: fuselane_rpc::new_secret(),
        }
    }
}

/// What Settings shows.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteView {
    pub on: bool,
    pub lan: bool,
    pub port: u16,
    pub secret: String,
    /// Addresses to type into an aria2 tool, this computer's first.
    pub urls: Vec<String>,
    /// Why it isn't running although it's on (the port is taken, say).
    pub problem: Option<String>,
    /// The remote page for a phone, with the secret in its #fragment, and
    /// that link as a QR code (SVG); only while the local network is allowed.
    pub phone_url: Option<String>,
    pub phone_qr: Option<String>,
}

/// A download's state in aria2's words; `None` for a cancelled one, which
/// aria2 tools don't list.
fn aria2_status(status: &str) -> Option<Status> {
    match status {
        "queued" => Some(Status::Waiting),
        "running" => Some(Status::Active),
        "paused" => Some(Status::Paused),
        "completed" => Some(Status::Complete),
        "failed" | "failed-final" => Some(Status::Error),
        _ => None,
    }
}

/// The notification for a download going from `was` to `now` (`None`: not in
/// the list). aria2 says nothing about waiting, about a download added
/// paused, or about clearing a finished one from the list.
fn event_for(was: Option<Status>, now: Option<Status>) -> Option<Event> {
    if was == now {
        return None;
    }
    match now {
        Some(Status::Active) => Some(Event::Start),
        Some(Status::Paused) if was.is_some() => Some(Event::Pause),
        Some(Status::Complete) => Some(Event::Complete),
        Some(Status::Error) => Some(Event::Error),
        None if matches!(was, Some(Status::Active | Status::Waiting | Status::Paused)) => {
            Some(Event::Stop)
        }
        _ => None,
    }
}

/// What changed since the last list, as notifications; `seen` becomes the new list.
fn changes<'a>(
    seen: &mut HashMap<i64, Status>,
    jobs: impl IntoIterator<Item = (i64, &'a str)>,
) -> Vec<(i64, Event)> {
    let now: HashMap<i64, Status> = jobs
        .into_iter()
        .filter_map(|(id, s)| aria2_status(s).map(|s| (id, s)))
        .collect();
    let mut out: Vec<(i64, Event)> = now
        .iter()
        .filter_map(|(&id, &s)| event_for(seen.get(&id).copied(), Some(s)).map(|e| (id, e)))
        .chain(
            seen.iter()
                .filter(|(id, _)| !now.contains_key(id))
                .filter_map(|(&id, &s)| event_for(Some(s), None).map(|e| (id, e))),
        )
        .collect();
    out.sort_unstable_by_key(|&(id, _)| id);
    *seen = now;
    out
}

/// The download list, as aria2 tools see it.
struct Host {
    svc: Weak<Service>,
    /// Latest speed per download, from the live progress events.
    rates: Arc<Mutex<HashMap<i64, u64>>>,
}

fn gone() -> String {
    "Fuselane is closing.".into()
}

impl fuselane_rpc::Host for Host {
    fn jobs(&self) -> Vec<Job> {
        let Some(svc) = self.svc.upgrade() else {
            return vec![];
        };
        let rates = lock(&self.rates).clone();
        svc.jobs()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|j| {
                let status = aria2_status(j.status)?;
                Some(Job {
                    id: j.id,
                    url: j.url,
                    mirrors: j.mirrors,
                    dir: j.dir,
                    name: j.name,
                    status,
                    total: j.total,
                    done: j.written,
                    speed: rates.get(&j.id).copied().unwrap_or(0),
                    error: j.error,
                })
            })
            .collect()
    }

    fn add(&self, add: Add) -> Result<i64, String> {
        let svc = self.svc.upgrade().ok_or_else(gone)?;
        let url = add.uris.first().cloned().unwrap_or_default();
        if url.starts_with("magnet:") {
            return Err("Magnet links aren't taken this way yet. Open them in Fuselane.".into());
        }
        let req = AddRequest {
            name: add.out,
            later: add.paused,
            mirrors: add.uris.into_iter().skip(1).take(8).collect(),
            // Tools add what they're told to; the list shows both.
            allow_duplicate: true,
            ..AddRequest::default()
        };
        svc.add_with(&url, add.dir.as_deref(), &req)
            .map_err(|e| e.message)
    }

    fn add_metalink(
        &self,
        xml: &str,
        dir: Option<String>,
        paused: bool,
    ) -> Result<Vec<i64>, String> {
        let svc = self.svc.upgrade().ok_or_else(gone)?;
        let r = svc
            .add_metalink_text(xml, dir.as_deref(), paused)
            .map_err(|e| e.message)?;
        if r.added.is_empty() {
            let why = r
                .skipped
                .first()
                .map(|s| s.reason.clone())
                .unwrap_or_default();
            return Err(format!("Nothing was added: {why}"));
        }
        Ok(r.added)
    }

    fn pause(&self, id: i64) -> Result<(), String> {
        let svc = self.svc.upgrade().ok_or_else(gone)?;
        svc.pause(id).map_err(|e| e.message)
    }

    fn resume(&self, id: i64) -> Result<(), String> {
        let svc = self.svc.upgrade().ok_or_else(gone)?;
        svc.resume(id).map_err(|e| e.message)
    }

    fn remove(&self, id: i64) -> Result<(), String> {
        let svc = self.svc.upgrade().ok_or_else(gone)?;
        svc.remove(id).map_err(|e| e.message)
    }

    fn default_dir(&self) -> String {
        self.svc
            .upgrade()
            .map(|s| s.default_dir().display().to_string())
            .unwrap_or_default()
    }

    fn max_running(&self) -> usize {
        self.svc.upgrade().map_or(0, |s| s.max_running())
    }

    fn set_max_running(&self, n: usize) -> Result<(), String> {
        let svc = self.svc.upgrade().ok_or_else(gone)?;
        svc.set_max_running(n).map(|_| ()).map_err(|e| e.message)
    }
}

pub struct Remote {
    svc: Weak<Service>,
    saved: Mutex<Saved>,
    server: tokio::sync::Mutex<Option<fuselane_rpc::Server>>,
    problem: Mutex<Option<String>>,
    rates: Arc<Mutex<HashMap<i64, u64>>>,
    /// Reaches the running endpoint's WebSockets; `None` while it's off.
    notifier: Arc<Mutex<Option<Notifier>>>,
    addrs: Arc<dyn Fn() -> Vec<Ipv4Addr> + Send + Sync>,
}

impl std::fmt::Debug for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Remote").finish_non_exhaustive()
    }
}

impl Remote {
    pub fn new(
        svc: &Arc<Service>,
        addrs: Arc<dyn Fn() -> Vec<Ipv4Addr> + Send + Sync>,
    ) -> Arc<Remote> {
        let saved = svc
            .store()
            .setting(SETTING)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<Saved>(&s).ok())
            .unwrap_or_default();
        let rates: Arc<Mutex<HashMap<i64, u64>>> = Arc::default();
        let notifier: Arc<Mutex<Option<Notifier>>> = Arc::default();
        {
            let rates = rates.clone();
            let notifier = notifier.clone();
            // The list as last seen, so only changes are announced (not
            // everything already there at startup).
            let mut first = HashMap::new();
            let jobs = svc.jobs().unwrap_or_default();
            changes(&mut first, jobs.iter().map(|j| (j.id, j.status)));
            let seen = Mutex::new(first);
            svc.listen(Arc::new(move |e| match e {
                UiEvent::Live(l) => {
                    lock(&rates).insert(l.id, l.rate.max(0.0) as u64);
                }
                UiEvent::Jobs { jobs } => {
                    // Only running downloads have a speed.
                    lock(&rates)
                        .retain(|id, _| jobs.iter().any(|j| j.id == *id && j.status == "running"));
                    let events = changes(&mut lock(&seen), jobs.iter().map(|j| (j.id, j.status)));
                    if let Some(n) = lock(&notifier).as_ref() {
                        for (id, event) in events {
                            n.notify(id, event);
                        }
                    }
                }
                _ => {}
            }));
        }
        Arc::new(Remote {
            svc: Arc::downgrade(svc),
            saved: Mutex::new(saved),
            server: tokio::sync::Mutex::new(None),
            problem: Mutex::new(None),
            rates,
            notifier,
            addrs,
        })
    }

    pub fn view(&self) -> RemoteView {
        let s = lock(&self.saved).clone();
        let mut urls = vec![format!("http://127.0.0.1:{}/jsonrpc", s.port)];
        if s.lan {
            urls.extend(
                (self.addrs)()
                    .into_iter()
                    .map(|ip| format!("http://{ip}:{}/jsonrpc", s.port)),
            );
        }
        let phone_url = (s.on && s.lan)
            .then(|| (self.addrs)().into_iter().next())
            .flatten()
            .map(|ip| format!("http://{ip}:{}/#secret={}", s.port, s.secret));
        let phone_qr = phone_url.as_ref().and_then(|u| {
            qrcode::QrCode::new(u.as_bytes()).ok().map(|c| {
                c.render::<qrcode::render::svg::Color<'_>>()
                    .quiet_zone(false)
                    .min_dimensions(160, 160)
                    .dark_color(qrcode::render::svg::Color("#1b1f27"))
                    .light_color(qrcode::render::svg::Color("#ffffff"))
                    .build()
            })
        });
        RemoteView {
            on: s.on,
            lan: s.lan,
            port: s.port,
            secret: s.secret,
            urls,
            problem: lock(&self.problem).clone(),
            phone_url,
            phone_qr,
        }
    }

    fn save(&self) -> Result<(), UiError> {
        let svc = self
            .svc
            .upgrade()
            .ok_or_else(|| UiError::new_public("closing", "Fuselane is closing.", None))?;
        let json = serde_json::to_string(&*lock(&self.saved)).unwrap_or_default();
        svc.store().set_setting(SETTING, &json).map_err(|e| {
            UiError::new_public(
                "store",
                format!("Fuselane couldn't save the setting: {e}"),
                Some("Check that your disk has free space, then try again."),
            )
        })
    }

    /// Starts or stops the endpoint to match the setting.
    pub async fn apply(&self) {
        let s = lock(&self.saved).clone();
        let mut server = self.server.lock().await;
        *lock(&self.notifier) = None;
        if let Some(old) = server.take() {
            old.stop().await; // the port is free before binding it again
        }
        *lock(&self.problem) = None;
        if !s.on {
            return;
        }
        let ip = if s.lan {
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        } else {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        };
        let host = Arc::new(Host {
            svc: self.svc.clone(),
            rates: self.rates.clone(),
        });
        match fuselane_rpc::Server::start(host, SocketAddr::new(ip, s.port), s.secret).await {
            Ok(started) => {
                *lock(&self.notifier) = Some(started.notifier());
                *server = Some(started);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                *lock(&self.problem) = Some(format!(
                    "Port {} is in use by another program (aria2 itself, perhaps). Pick another port.",
                    s.port
                ));
            }
            Err(e) => *lock(&self.problem) = Some(format!("It couldn't start ({e}).")),
        }
    }

    /// Changes the setting: on or off, the local network, the port.
    pub async fn set(&self, on: bool, lan: bool, port: u16) -> Result<RemoteView, UiError> {
        if port < 1024 {
            return Err(UiError::new_public(
                "bad-value",
                "Pick a port from 1024 to 65535.",
                Some("6800 is what aria2 tools expect."),
            ));
        }
        {
            let mut s = lock(&self.saved);
            s.on = on;
            s.lan = lan;
            s.port = port;
        }
        self.save()?;
        self.apply().await;
        Ok(self.view())
    }

    /// A new secret: tools using the old one stop working.
    pub async fn new_secret(&self) -> Result<RemoteView, UiError> {
        lock(&self.saved).secret = fuselane_rpc::new_secret();
        self.save()?;
        self.apply().await;
        Ok(self.view())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn call(port: u16, body: &str) -> String {
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            s,
            "POST /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn aria2_tools_add_and_see_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("db")).unwrap();
        let svc = Service::new(store, dir.path().to_path_buf()).unwrap();
        let remote = Remote::new(&svc, Arc::new(Vec::new));
        assert!(!remote.view().on, "off until turned on");
        assert_eq!(
            remote.set(true, false, 80).await.unwrap_err().code,
            "bad-value"
        );

        // A free port, then on.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let v = remote.set(true, false, port).await.unwrap();
        assert_eq!(v.urls, [format!("http://127.0.0.1:{port}/jsonrpc")]);
        assert_eq!(v.problem, None);
        assert_eq!(
            v.phone_url, None,
            "no phone page while only this computer may connect"
        );
        let secret = v.secret.clone();

        let body = format!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"aria2.addUri","params":["token:{secret}",["https://example.com/big.iso"],{{"pause":"true","out":"big.iso"}}]}}"#
        );
        let out = tokio::task::spawn_blocking(move || call(port, &body))
            .await
            .unwrap();
        assert!(out.starts_with("HTTP/1.1 200"), "{out}");
        let jobs = svc.jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            (jobs[0].name.as_str(), jobs[0].status),
            ("big.iso", "paused")
        );

        let body = format!(
            r#"{{"jsonrpc":"2.0","id":2,"method":"aria2.tellWaiting","params":["token:{secret}",0,10,["status","gid"]]}}"#
        );
        let out = tokio::task::spawn_blocking(move || call(port, &body))
            .await
            .unwrap();
        assert!(out.contains(r#""status":"paused""#), "{out}");

        // The setting survives a restart; a new secret locks the old one out.
        let again = Remote::new(&svc, Arc::new(Vec::new));
        assert_eq!((again.view().on, again.view().port), (true, port));
        let v = remote.new_secret().await.unwrap();
        assert_ne!(v.secret, secret);
        let body = format!(
            r#"{{"jsonrpc":"2.0","id":3,"method":"aria2.getVersion","params":["token:{secret}"]}}"#
        );
        let out = tokio::task::spawn_blocking(move || call(port, &body))
            .await
            .unwrap();
        assert!(out.starts_with("HTTP/1.1 401"), "{out}");

        // The port taken by something else: said, not crashed.
        let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let taken = busy.local_addr().unwrap().port();
        let v = remote.set(true, false, taken).await.unwrap();
        assert!(v.problem.unwrap().contains("in use"));
        assert!(
            remote
                .set(false, false, taken)
                .await
                .unwrap()
                .problem
                .is_none()
        );
    }

    #[test]
    fn status_changes_become_aria2_notifications() {
        let mut seen = HashMap::new();
        // Waiting, or added paused: aria2 says nothing.
        assert_eq!(changes(&mut seen, [(1, "queued"), (2, "paused")]), []);
        assert_eq!(
            changes(&mut seen, [(1, "running"), (2, "paused")]),
            [(1, Event::Start)]
        );
        assert_eq!(
            changes(&mut seen, [(1, "paused"), (2, "queued")]),
            [(1, Event::Pause)]
        );
        assert_eq!(
            changes(&mut seen, [(1, "paused"), (2, "running")]),
            [(2, Event::Start)]
        );
        // The same list again (a speed changed, say): nothing.
        assert_eq!(changes(&mut seen, [(1, "paused"), (2, "running")]), []);
        assert_eq!(
            changes(&mut seen, [(1, "failed"), (2, "completed")]),
            [(1, Event::Error), (2, Event::Complete)]
        );
        // Giving up for good is still the same error; clearing a finished one
        // from the list says nothing.
        assert_eq!(changes(&mut seen, [(1, "failed-final")]), []);
        // Tried again, then started; then one removed and one cancelled.
        assert_eq!(
            changes(&mut seen, [(1, "queued"), (3, "running")]),
            [(3, Event::Start)]
        );
        assert_eq!(
            changes(&mut seen, [(3, "cancelled")]),
            [(1, Event::Stop), (3, Event::Stop)]
        );
        assert!(seen.is_empty(), "cancelled ones aren't listed");
    }

    /// Opens a WebSocket to the endpoint and shows the secret on it.
    async fn ws_signed_in(port: u16, secret: &str) -> tokio::net::TcpStream {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        s.write_all(
            b"GET /jsonrpc HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
        )
        .await
        .unwrap();
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            head.push(s.read_u8().await.unwrap());
        }
        assert!(head.starts_with(b"HTTP/1.1 101"));
        let call = format!(
            r#"{{"jsonrpc":"2.0","id":"hi","method":"aria2.getVersion","params":["token:{secret}"]}}"#
        );
        // A client frame: masked, short.
        let mask = [1u8, 2, 3, 4];
        let mut f = vec![0x81, 0x80 | u8::try_from(call.len()).unwrap()];
        f.extend_from_slice(&mask);
        f.extend(call.bytes().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        s.write_all(&f).await.unwrap();
        assert!(ws_text(&mut s).await.contains(r#""id":"hi""#));
        s
    }

    /// The next text message from the server, within five seconds.
    async fn ws_text(s: &mut tokio::net::TcpStream) -> String {
        use tokio::io::AsyncReadExt;
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let op = s.read_u8().await.unwrap() & 0x0f;
            let len = match s.read_u8().await.unwrap() & 0x7f {
                126 => usize::from(s.read_u16().await.unwrap()),
                n => usize::from(n),
            };
            let mut data = vec![0u8; len];
            s.read_exact(&mut data).await.unwrap();
            assert_eq!(op, 0x1);
            String::from_utf8(data).unwrap()
        })
        .await
        .expect("a message within 5 s")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn aria2_tools_hear_about_changes_on_their_websocket() {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("db")).unwrap();
        let svc = Service::new(store, dir.path().to_path_buf()).unwrap();
        let remote = Remote::new(&svc, Arc::new(Vec::new));
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let secret = remote.set(true, false, port).await.unwrap().secret;
        let req = AddRequest {
            later: true,
            ..AddRequest::default()
        };
        let id = svc
            .add_with("https://example.com/big.iso", None, &req)
            .unwrap();
        let mut ws = ws_signed_in(port, &secret).await;

        // Removing it from Fuselane's own list: aria2 tools hear "stopped".
        svc.remove(id).unwrap();
        let note: serde_json::Value = serde_json::from_str(&ws_text(&mut ws).await).unwrap();
        assert_eq!(
            note,
            serde_json::json!({"jsonrpc": "2.0", "method": "aria2.onDownloadStop",
                               "params": [{"gid": fuselane_rpc::gid(id)}]})
        );

        // Turned off: nothing is sent, and nothing fails.
        remote.set(false, false, port).await.unwrap();
        let again = svc
            .add_with("https://example.com/other.iso", None, &req)
            .unwrap();
        svc.remove(again).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_phone_page_link_carries_the_secret_in_its_fragment() {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("db")).unwrap();
        let svc = Service::new(store, dir.path().to_path_buf()).unwrap();
        let remote = Remote::new(&svc, Arc::new(|| vec![Ipv4Addr::new(192, 168, 1, 24)]));
        let port = std::net::TcpListener::bind("0.0.0.0:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let v = remote.set(true, true, port).await.unwrap();
        assert_eq!(
            v.phone_url.as_deref(),
            Some(format!("http://192.168.1.24:{port}/#secret={}", v.secret).as_str())
        );
        assert!(v.phone_qr.unwrap().contains("<svg"));
        assert_eq!(v.urls.len(), 2, "this computer, then the network address");
        remote.set(false, true, port).await.unwrap();
    }
}
