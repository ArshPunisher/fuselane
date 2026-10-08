//! A real two-client swarm on loopback: a plain librqbit seeder and a Fuselane
//! leecher whose every peer connection must go through Fuselane's SOCKS5 proxy.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use fuselane_engine_torrent::{EngineOptions, Phase, Source, TorrentEngine, TorrentError};
use fuselane_netif::{Interface, Kind};
use librqbit::{
    AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session, SessionOptions,
};

fn loopback() -> Interface {
    Interface {
        name: "lo0".into(),
        display_name: "Loopback".into(),
        index: 1,
        kind: Kind::Loopback,
        addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
    }
}

/// Deterministic, incompressible-looking bytes.
fn payload(len: usize) -> Vec<u8> {
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

async fn seeder(
    dir: &std::path::Path,
    data: &[u8],
) -> (std::sync::Arc<Session>, SocketAddr, Vec<u8>) {
    let file = dir.join("payload.bin");
    std::fs::write(&file, data).unwrap();
    let spawner = librqbit::spawn_utils::BlockingSpawner::new(2);
    let torrent = librqbit::create_torrent(
        &file,
        CreateTorrentOptions {
            name: Some("payload.bin"),
            trackers: vec![],
            piece_length: Some(64 * 1024),
        },
        &spawner,
    )
    .await
    .unwrap();
    let bytes = torrent.as_bytes().unwrap().to_vec();
    let session = Session::new_with_opts(
        dir.to_path_buf(),
        SessionOptions {
            dht: None,
            persistence: None,
            fastresume: false,
            disable_local_service_discovery: true,
            listen: Some(ListenerOptions {
                listen_addr: (Ipv4Addr::LOCALHOST, 0).into(),
                enable_upnp_port_forwarding: false,
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let handle = session
        .add_torrent(
            AddTorrent::TorrentFileBytes(bytes.clone().into()),
            Some(AddTorrentOptions {
                output_folder: Some(dir.to_string_lossy().into_owned()),
                overwrite: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), handle.wait_until_completed())
        .await
        .unwrap()
        .unwrap();
    let addr = session.listen_addr().expect("seeder listens");
    (session, addr, bytes)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_torrent_downloads_byte_exact_through_the_proxy() {
    let seed_dir = tempfile::tempdir().unwrap();
    let leech_dir = tempfile::tempdir().unwrap();
    let data = payload(3 * 1024 * 1024 + 12_345);
    let (_seeder, seeder_addr, torrent) = seeder(seed_dir.path(), &data).await;

    let engine = TorrentEngine::start(EngineOptions {
        download_dir: leech_dir.path().to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: None,
    })
    .await
    .unwrap();
    let t = engine
        .add(Source::File(torrent), None, vec![seeder_addr])
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), t.finished())
        .await
        .expect("download timed out")
        .unwrap();

    let got = std::fs::read(leech_dir.path().join("payload.bin")).unwrap();
    assert!(got == data, "downloaded file differs from the seed");
    let p = t.progress();
    assert_eq!((p.done, p.total), (data.len() as u64, data.len() as u64));
    assert!(matches!(p.phase, Phase::Seeding), "{:?}", p.phase);
    let net = &engine.networks()[0];
    assert!(
        net.down >= data.len() as u64,
        "every piece came through the proxy: {net:?}"
    );
    engine.remove(t, true).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn broken_inputs_get_clear_errors() {
    let dir = tempfile::tempdir().unwrap();
    let none = TorrentEngine::start(EngineOptions {
        download_dir: dir.path().to_path_buf(),
        networks: vec![],
        dht: false,
        listen: None,
    })
    .await;
    assert!(matches!(none, Err(TorrentError::NoNetworks)));

    let engine = TorrentEngine::start(EngineOptions {
        download_dir: dir.path().to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: None,
    })
    .await
    .unwrap();
    for junk in [
        b"not bencode at all".to_vec(),
        b"d4:infoi1ee".to_vec(),
        vec![],
        b"d8:announce".to_vec(),
    ] {
        let r = engine.add(Source::File(junk.clone()), None, vec![]).await;
        assert!(
            matches!(r, Err(TorrentError::Invalid(_))),
            "{junk:?} -> {r:?}"
        );
    }
    let r = engine
        .add(
            Source::Magnet("magnet:?xt=urn:btih:nothex".into()),
            None,
            vec![],
        )
        .await;
    assert!(r.is_err(), "{r:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_existing_file_is_never_overwritten() {
    let seed_dir = tempfile::tempdir().unwrap();
    let leech_dir = tempfile::tempdir().unwrap();
    let (_seeder, seeder_addr, torrent) = seeder(seed_dir.path(), &payload(200_000)).await;
    std::fs::write(leech_dir.path().join("payload.bin"), b"my own file").unwrap();
    let engine = TorrentEngine::start(EngineOptions {
        download_dir: leech_dir.path().to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: None,
    })
    .await
    .unwrap();
    let r = engine
        .add(Source::File(torrent), None, vec![seeder_addr])
        .await;
    assert!(matches!(r, Err(TorrentError::FileExists(_))), "{r:?}");
    assert_eq!(
        std::fs::read(leech_dir.path().join("payload.bin")).unwrap(),
        b"my own file"
    );
}
