//! Two Nearby devices on loopback: discovery-free sends, pinning, decline, cancel.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use fuselane_nearby::server::{Decision, Ended, Host};
use fuselane_nearby::{
    DeviceInfo, Identity, Outgoing, PrepareUpload, SendError, Server, Target, client,
};

struct Box1 {
    me: DeviceInfo,
    answer: Mutex<Decision>,
    asked: Mutex<Vec<(Option<String>, usize)>>,
    files: Mutex<Vec<PathBuf>>,
    ends: Mutex<Vec<Ended>>,
    sessions: Mutex<Vec<String>>,
    written: AtomicU64,
    /// Receives slowly, so a cancel always lands mid-file even on a busy machine.
    slow: AtomicBool,
}

impl Host for Box1 {
    fn me(&self) -> DeviceInfo {
        self.me.clone()
    }
    fn seen(&self, _: SocketAddr, _: DeviceInfo, _: Option<String>) {}
    fn decide<'a>(
        &'a self,
        _from: SocketAddr,
        verified: Option<String>,
        req: &'a PrepareUpload,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Decision> + Send + 'a>> {
        self.asked.lock().unwrap().push((verified, req.files.len()));
        let d = self.answer.lock().unwrap().clone();
        Box::pin(async move { d })
    }
    fn started(&self, session: &str, _: &DeviceInfo, _: &[String], _: u64) {
        self.sessions.lock().unwrap().push(session.to_string());
    }
    fn progress(&self, _: &str, _: &str, written: u64) {
        self.written
            .store(written, std::sync::atomic::Ordering::Relaxed);
        if self.slow.load(std::sync::atomic::Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    fn file_done(&self, _: &str, _: &str, path: &Path) {
        self.files.lock().unwrap().push(path.to_path_buf());
    }
    fn ended(&self, _: &str, how: Ended) {
        self.ends.lock().unwrap().push(how);
    }
}

fn info(alias: &str, id: &Identity, port: u16) -> DeviceInfo {
    DeviceInfo {
        alias: alias.into(),
        version: "2.1".into(),
        device_model: Some("Test".into()),
        device_type: Some("desktop".into()),
        fingerprint: id.fingerprint.clone(),
        port,
        protocol: "https".into(),
        download: false,
    }
}

struct Device {
    id: Identity,
    host: Arc<Box1>,
    server: Server,
    _dir: tempfile::TempDir,
}

async fn device(alias: &str, inbox: &Path) -> Device {
    let dir = tempfile::tempdir().unwrap();
    let id = Identity::load_or_create(dir.path()).unwrap();
    let host = Arc::new(Box1 {
        me: info(alias, &id, 0),
        answer: Mutex::new(Decision::Accept {
            dir: inbox.to_path_buf(),
            only: None,
        }),
        asked: Mutex::default(),
        files: Mutex::default(),
        ends: Mutex::default(),
        sessions: Mutex::default(),
        written: AtomicU64::new(0),
        slow: AtomicBool::new(false),
    });
    let server = Server::start(&id, host.clone(), 0).await.unwrap();
    Device {
        id,
        host,
        server,
        _dir: dir,
    }
}

fn target(d: &Device, fp: Option<&str>) -> Target {
    Target {
        addr: SocketAddr::from(([127, 0, 0, 1], d.server.addr.port())),
        fingerprint: fp.map(str::to_string),
    }
}

fn outgoing(dir: &Path, name: &str, data: &[u8]) -> Outgoing {
    let path = dir.join(name);
    std::fs::write(&path, data).unwrap();
    Outgoing {
        path,
        name: name.into(),
        size: data.len() as u64,
        mime: "application/octet-stream".into(),
    }
}

#[tokio::test]
async fn a_file_goes_across_pinned_and_lands_exactly() {
    let inbox = tempfile::tempdir().unwrap();
    let outbox = tempfile::tempdir().unwrap();
    let a = device("Receiver", inbox.path()).await;
    let b = device("Sender", tempfile::tempdir().unwrap().path()).await;
    let me = info("Sender", &b.id, b.server.addr.port());
    let big: Vec<u8> = (0..3_000_000u32).map(|i| (i % 253) as u8).collect();
    let files = vec![
        outgoing(outbox.path(), "big.bin", &big),
        outgoing(outbox.path(), "note.txt", b"hello"),
    ];
    // A file already there under the same name is never overwritten.
    std::fs::write(inbox.path().join("note.txt"), b"mine").unwrap();
    let sent = Arc::new(AtomicU64::new(0));
    let n = client::send(
        &me,
        &target(&a, Some(&a.id.fingerprint)),
        &files,
        sent.clone(),
        Arc::new(AtomicBool::new(false)),
    )
    .await
    .unwrap();
    assert_eq!(n, 2);
    assert_eq!(
        sent.load(std::sync::atomic::Ordering::Relaxed),
        big.len() as u64 + 5
    );
    assert_eq!(std::fs::read(inbox.path().join("big.bin")).unwrap(), big);
    assert_eq!(
        std::fs::read(inbox.path().join("note.txt")).unwrap(),
        b"mine"
    );
    assert_eq!(
        std::fs::read(inbox.path().join("note (2).txt")).unwrap(),
        b"hello"
    );
    assert_eq!(a.host.ends.lock().unwrap().as_slice(), &[Ended::Done]);
    // The receiver learned the sender's real fingerprint by connecting back.
    let asked = a.host.asked.lock().unwrap().clone();
    assert_eq!(asked, vec![(Some(b.id.fingerprint.clone()), 2)]);
    // Introductions work both ways.
    let back = client::register(&me, &target(&a, Some(&a.id.fingerprint)))
        .await
        .unwrap();
    assert_eq!(back.alias, "Receiver");
}

#[tokio::test]
async fn a_wrong_certificate_a_decline_and_a_busy_receiver_are_told_apart() {
    let inbox = tempfile::tempdir().unwrap();
    let outbox = tempfile::tempdir().unwrap();
    let a = device("Receiver", inbox.path()).await;
    let b = device("Sender", tempfile::tempdir().unwrap().path()).await;
    let me = info("Sender", &b.id, b.server.addr.port());
    let f = vec![outgoing(outbox.path(), "x.bin", b"x")];
    let go = |t: Target| {
        let (me, f) = (me.clone(), f.clone());
        async move {
            client::send(
                &me,
                &t,
                &f,
                Arc::default(),
                Arc::new(AtomicBool::new(false)),
            )
            .await
        }
    };
    // Someone else's certificate on this address: refused before anything is sent.
    assert_eq!(
        go(target(&a, Some(&b.id.fingerprint))).await,
        Err(SendError::NotTheDevice)
    );
    assert!(a.host.asked.lock().unwrap().is_empty());
    *a.host.answer.lock().unwrap() = Decision::Decline;
    assert_eq!(
        go(target(&a, Some(&a.id.fingerprint))).await,
        Err(SendError::Declined)
    );
    *a.host.answer.lock().unwrap() = Decision::Busy;
    assert_eq!(go(target(&a, None)).await, Err(SendError::Busy));
    assert_eq!(
        std::fs::read_dir(inbox.path()).unwrap().count(),
        0,
        "nothing written"
    );
}

#[tokio::test]
async fn cancelling_mid_file_leaves_no_partial_file() {
    let inbox = tempfile::tempdir().unwrap();
    let outbox = tempfile::tempdir().unwrap();
    let a = device("Receiver", inbox.path()).await;
    let b = device("Sender", tempfile::tempdir().unwrap().path()).await;
    let me = info("Sender", &b.id, b.server.addr.port());
    let big = vec![7u8; 40_000_000];
    let f = vec![outgoing(outbox.path(), "huge.bin", &big)];
    let cancel = Arc::new(AtomicBool::new(false));
    let sent = Arc::new(AtomicU64::new(0));
    let stop = {
        let (cancel, sent) = (cancel.clone(), sent.clone());
        tokio::spawn(async move {
            while sent.load(std::sync::atomic::Ordering::Relaxed) < 1_000_000 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        })
    };
    let r = client::send(&me, &target(&a, Some(&a.id.fingerprint)), &f, sent, cancel).await;
    stop.await.unwrap();
    assert_eq!(r, Err(SendError::Cancelled));
    for _ in 0..100 {
        if std::fs::read_dir(inbox.path()).unwrap().count() == 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let left: Vec<_> = std::fs::read_dir(inbox.path()).unwrap().collect();
    assert!(left.is_empty(), "{left:?}");
}

#[tokio::test]
async fn the_receiver_can_cancel_and_the_sender_is_told() {
    let inbox = tempfile::tempdir().unwrap();
    let outbox = tempfile::tempdir().unwrap();
    let a = Arc::new(device("Receiver", inbox.path()).await);
    a.host
        .slow
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let b = device("Sender", tempfile::tempdir().unwrap().path()).await;
    let me = info("Sender", &b.id, b.server.addr.port());
    let f = vec![outgoing(outbox.path(), "big.iso", &vec![3u8; 40_000_000])];
    let stopper = {
        let a = a.clone();
        tokio::spawn(async move {
            while a.host.written.load(std::sync::atomic::Ordering::Relaxed) < 1_000_000 {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            let s = a.host.sessions.lock().unwrap()[0].clone();
            a.server.cancel(&s);
        })
    };
    let r = client::send(
        &me,
        &target(&a, Some(&a.id.fingerprint)),
        &f,
        Arc::default(),
        Arc::new(AtomicBool::new(false)),
    )
    .await;
    stopper.await.unwrap();
    assert_eq!(r, Err(SendError::CancelledByThem));
    assert_eq!(a.host.ends.lock().unwrap().as_slice(), &[Ended::Cancelled]);
    assert_eq!(
        std::fs::read_dir(inbox.path()).unwrap().count(),
        0,
        "no partial file"
    );
}
