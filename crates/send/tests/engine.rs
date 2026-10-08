//! Fuse Send end to end through Fuselane's own torrent engine on loopback: the
//! sender prepares and seeds, the receiver opens the link and gets the file under
//! its real name, checked against the sender's BLAKE3 (STEPS 6.4, 6.5).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use fuselane_engine_torrent::{EngineOptions, TorrentEngine};
use fuselane_netif::{Interface, Kind};
use fuselane_send::link::Link;
use fuselane_send::share::{self, ShareError};

fn loopback() -> Interface {
    Interface {
        name: "lo0".into(),
        display_name: "Loopback".into(),
        index: 1,
        kind: Kind::Loopback,
        addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
    }
}

fn payload(n: usize) -> Vec<u8> {
    let mut x = 0x2545_f491_u32;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

async fn engine(dir: &std::path::Path, listen: bool) -> TorrentEngine {
    TorrentEngine::start(EngineOptions {
        download_dir: dir.to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: listen.then(|| (Ipv4Addr::LOCALHOST, 0).into()),
        state_dir: None,
        limiter: None,
    })
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_link_brings_the_file_across_under_its_real_name() {
    let (send_dir, recv_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let data = payload(3 * 1024 * 1024 + 99);
    let file = send_dir.path().join("Trip photos.zip");
    std::fs::write(&file, &data).unwrap();
    // Something already has that name on the receiving side: it must survive.
    std::fs::write(recv_dir.path().join("Trip photos.zip"), b"older").unwrap();

    let prepared = share::prepare(&file, |_, _| {}).unwrap();
    let url = prepared.link.url();
    let sender = engine(send_dir.path(), true).await;
    let seeding = share::seed(&sender, &prepared, &file).await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), seeding.finished())
        .await
        .expect("sender check timed out")
        .unwrap();
    let addr = sender.listen_addr().expect("sender listens");

    // The receiver has only the URL someone pasted.
    let link = Link::parse(&url).unwrap();
    let receiver = engine(recv_dir.path(), false).await;
    let r = share::receive(&receiver, &link, recv_dir.path(), vec![addr])
        .await
        .unwrap();
    assert_eq!(r.size, data.len() as u64);
    tokio::time::timeout(Duration::from_secs(60), r.torrent.finished())
        .await
        .expect("receive timed out")
        .unwrap();
    let saved = r.finish().unwrap();

    assert_eq!(saved, recv_dir.path().join("Trip photos (2).zip"));
    assert!(std::fs::read(&saved).unwrap() == data, "byte-exact");
    assert_eq!(
        std::fs::read(recv_dir.path().join("Trip photos.zip")).unwrap(),
        b"older"
    );
    let left: Vec<_> = std::fs::read_dir(recv_dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with(".fuselane-"))
        .collect();
    assert!(left.is_empty(), "side files cleaned up: {left:?}");
    assert!(
        std::fs::read(&file).unwrap() == data,
        "sender's file untouched"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_right_info_hash_with_the_wrong_key_is_caught_and_deleted() {
    let (send_dir, recv_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let file = send_dir.path().join("secret.pdf");
    std::fs::write(&file, payload(500_000)).unwrap();
    let prepared = share::prepare(&file, |_, _| {}).unwrap();
    let sender = engine(send_dir.path(), true).await;
    let seeding = share::seed(&sender, &prepared, &file).await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), seeding.finished())
        .await
        .unwrap()
        .unwrap();
    let addr = sender.listen_addr().unwrap();

    let mut wrong = prepared.link.clone();
    wrong.key[0] ^= 1;
    let receiver = engine(recv_dir.path(), false).await;
    let r = share::receive(&receiver, &wrong, recv_dir.path(), vec![addr])
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), r.torrent.finished())
        .await
        .unwrap()
        .unwrap();
    let err = r.finish().unwrap_err();
    assert!(matches!(err, ShareError::Crypt(_)), "{err:?}");
    assert!(err.to_string().contains("new link"), "{err}");
    assert_eq!(
        std::fs::read_dir(recv_dir.path()).unwrap().count(),
        0,
        "nothing left behind"
    );
}
