//! Remote control (8.7, ADR 0013): aria2 tools add and watch downloads through
//! `fuselane-rpc`. This side keeps the setting, starts and stops the endpoint,
//! and answers for the download list.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, Weak};

use serde::{Deserialize, Serialize};

use crate::service::{AddRequest, Service, UiError, UiEvent};
use fuselane_rpc::{Add, Job, Status};

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
                let status = match j.status {
                    "queued" => Status::Waiting,
                    "running" => Status::Active,
                    "paused" => Status::Paused,
                    "completed" => Status::Complete,
                    "failed" | "failed-final" => Status::Error,
                    _ => return None, // cancelled: not in the list
                };
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
        {
            let rates = rates.clone();
            svc.listen(Arc::new(move |e| match e {
                UiEvent::Live(l) => {
                    lock(&rates).insert(l.id, l.rate.max(0.0) as u64);
                }
                UiEvent::Jobs { jobs } => {
                    // Only running downloads have a speed.
                    lock(&rates)
                        .retain(|id, _| jobs.iter().any(|j| j.id == *id && j.status == "running"));
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
        RemoteView {
            on: s.on,
            lan: s.lan,
            port: s.port,
            secret: s.secret,
            urls,
            problem: lock(&self.problem).clone(),
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
            Ok(started) => *server = Some(started),
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
}
