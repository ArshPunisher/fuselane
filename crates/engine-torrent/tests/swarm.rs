//! A real two-client swarm on loopback: a plain librqbit seeder and a Fuselane
//! leecher whose every peer connection must go through Fuselane's SOCKS5 proxy.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use fuselane_engine_torrent::{
    AddOptions, EngineOptions, Phase, Source, TorrentEngine, TorrentError,
};
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
        state_dir: None,
    })
    .await
    .unwrap();
    let t = engine
        .add(
            Source::File(torrent),
            None,
            vec![seeder_addr],
            AddOptions::default(),
        )
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
        state_dir: None,
    })
    .await;
    assert!(matches!(none, Err(TorrentError::NoNetworks)));

    let engine = TorrentEngine::start(EngineOptions {
        download_dir: dir.path().to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: None,
        state_dir: None,
    })
    .await
    .unwrap();
    for junk in [
        b"not bencode at all".to_vec(),
        b"d4:infoi1ee".to_vec(),
        vec![],
        b"d8:announce".to_vec(),
    ] {
        let r = engine
            .add(
                Source::File(junk.clone()),
                None,
                vec![],
                AddOptions::default(),
            )
            .await;
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
            AddOptions::default(),
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
        state_dir: None,
    })
    .await
    .unwrap();
    let r = engine
        .add(
            Source::File(torrent),
            None,
            vec![seeder_addr],
            AddOptions::default(),
        )
        .await;
    assert!(matches!(r, Err(TorrentError::FileExists(_))), "{r:?}");
    assert_eq!(
        std::fs::read(leech_dir.path().join("payload.bin")).unwrap(),
        b"my own file"
    );
}

/// Seeds a folder `T` holding the given files; returns the seeder and torrent bytes.
async fn seed_folder(
    root: &std::path::Path,
    files: &[(&str, &[u8])],
) -> (std::sync::Arc<Session>, SocketAddr, Vec<u8>) {
    let folder = root.join("T");
    std::fs::create_dir_all(&folder).unwrap();
    for (name, data) in files {
        std::fs::write(folder.join(name), data).unwrap();
    }
    let spawner = librqbit::spawn_utils::BlockingSpawner::new(2);
    let torrent = librqbit::create_torrent(
        &folder,
        CreateTorrentOptions {
            name: Some("T"),
            trackers: vec![],
            piece_length: Some(64 * 1024),
        },
        &spawner,
    )
    .await
    .unwrap();
    let bytes = torrent.as_bytes().unwrap().to_vec();
    let session = Session::new_with_opts(
        root.to_path_buf(),
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
                output_folder: Some(folder.to_string_lossy().into_owned()),
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
    let addr = session.listen_addr().unwrap();
    (session, addr, bytes)
}

async fn until_finished(t: &fuselane_engine_torrent::Torrent) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let p = t.progress();
            if p.phase == Phase::Seeding && p.done == p.total {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("download timed out");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chosen_files_download_and_edge_pieces_are_cleaned_up() {
    let seed = tempfile::tempdir().unwrap();
    let leech = tempfile::tempdir().unwrap();
    // Sizes that aren't piece-aligned, so neighbours share edge pieces (64 KiB).
    let (a, b, c) = (payload(100_000), payload(150_001), payload(70_003));
    let (_s, addr, torrent) =
        seed_folder(seed.path(), &[("a.bin", &a), ("b.bin", &b), ("c.bin", &c)]).await;
    let engine = TorrentEngine::start(EngineOptions {
        download_dir: leech.path().to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: None,
        state_dir: None,
    })
    .await
    .unwrap();
    let listing = engine
        .inspect(Source::File(torrent), None, vec![addr])
        .await
        .unwrap();
    assert_eq!(listing.name, "T");
    assert_eq!(listing.folder, leech.path().join("T"));
    let names: Vec<String> = listing.files.iter().map(|f| f.parts.join("/")).collect();
    let idx = |n: &str| names.iter().position(|x| x == n).unwrap();
    assert_eq!(listing.total(), (a.len() + b.len() + c.len()) as u64);
    assert!(!leech.path().join("T").exists(), "inspect writes nothing");

    let t = engine
        .add_listed(
            listing,
            AddOptions {
                only: Some([idx("b.bin")].into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    until_finished(&t).await;
    let dir = leech.path().join("T");
    assert!(
        std::fs::read(dir.join("b.bin")).unwrap() == b,
        "b.bin differs"
    );

    // Ask for c too while the torrent is live.
    engine
        .select(&t, [idx("b.bin"), idx("c.bin")].into())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    until_finished(&t).await;
    assert!(
        std::fs::read(dir.join("c.bin")).unwrap() == c,
        "c.bin differs"
    );

    // Bad selections are refused with a clear error and change nothing.
    assert!(matches!(
        engine.select(&t, [].into()).await,
        Err(TorrentError::NothingSelected)
    ));
    assert!(matches!(
        engine.select(&t, [99].into()).await,
        Err(TorrentError::NoSuchFile(99))
    ));
    assert_eq!(t.selected(), [idx("b.bin"), idx("c.bin")].into());

    // a.bin was never chosen: librqbit created it and wrote edge bytes into it.
    assert!(
        dir.join("a.bin").exists(),
        "librqbit created the unchosen file"
    );
    let cleanup = engine.release(t).await.unwrap();
    assert!(cleanup.skipped.is_empty(), "{cleanup:?}");
    assert!(!dir.join("a.bin").exists(), "the unchosen file is gone");
    assert!(
        std::fs::read(dir.join("b.bin")).unwrap() == b
            && std::fs::read(dir.join("c.bin")).unwrap() == c
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn removing_with_files_deletes_the_torrent_folder() {
    let seed = tempfile::tempdir().unwrap();
    let leech = tempfile::tempdir().unwrap();
    let (_s, addr, torrent) = seed_folder(
        seed.path(),
        &[("a.bin", &payload(90_000)), ("b.bin", &payload(1_000))],
    )
    .await;
    let engine = TorrentEngine::start(EngineOptions {
        download_dir: leech.path().to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: None,
        state_dir: None,
    })
    .await
    .unwrap();
    let t = engine
        .add(
            Source::File(torrent),
            None,
            vec![addr],
            AddOptions::default(),
        )
        .await
        .unwrap();
    until_finished(&t).await;
    let r = engine.remove(t, true).await.unwrap();
    assert_eq!(r.removed, 2, "{r:?}");
    assert!(!leech.path().join("T").exists());
}

/// Another seeder for an existing torrent whose single file is already in `dir`.
async fn seed_again(
    dir: &std::path::Path,
    torrent: &[u8],
) -> (std::sync::Arc<Session>, SocketAddr) {
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
            AddTorrent::TorrentFileBytes(torrent.to_vec().into()),
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
    let addr = session.listen_addr().unwrap();
    (session, addr)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_networks_share_a_torrent_and_credit_sums_to_the_file() {
    let (s1, s2, leech) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let data = payload(4 * 1024 * 1024 + 777);
    let (_a, addr1, torrent) = seeder(s1.path(), &data).await;
    std::fs::write(s2.path().join("payload.bin"), &data).unwrap();
    let (_b, addr2) = seed_again(s2.path(), &torrent).await;

    let mut second = loopback();
    second.name = "lo0-b".into();
    let engine = TorrentEngine::start(EngineOptions {
        download_dir: leech.path().to_path_buf(),
        networks: vec![loopback(), second],
        dht: false,
        listen: None,
        state_dir: None,
    })
    .await
    .unwrap();
    let t = engine
        .add(
            Source::File(torrent),
            None,
            vec![addr1, addr2],
            AddOptions::default(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), t.finished())
        .await
        .expect("timed out")
        .unwrap();
    assert!(std::fs::read(leech.path().join("payload.bin")).unwrap() == data);

    let shares = t.networks();
    assert_eq!(shares.len(), 2);
    assert_eq!(
        shares.iter().map(|s| s.credited).sum::<u64>(),
        data.len() as u64,
        "{shares:?}"
    );
    assert!(
        shares.iter().all(|s| s.received > 0),
        "both networks carried part of it: {shares:?}"
    );
    assert!(shares.iter().map(|s| s.received).sum::<u64>() >= data.len() as u64);
    // The session-wide view agrees with the per-torrent one.
    let all = engine.networks();
    assert!(
        all.iter().zip(&shares).all(|(n, s)| n.down >= s.received),
        "{all:?} {shares:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn after_a_restart_saved_files_are_rechecked_not_downloaded_again() {
    let seed = tempfile::tempdir().unwrap();
    let leech = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let data = payload(700_001);
    let (seeder_session, addr, torrent) = seeder(seed.path(), &data).await;
    let opts = |dir: &std::path::Path| EngineOptions {
        download_dir: dir.to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: None,
        state_dir: Some(state.path().to_path_buf()),
    };
    let first = TorrentEngine::start(opts(leech.path())).await.unwrap();
    let t = first
        .add(
            Source::File(torrent.clone()),
            None,
            vec![addr],
            AddOptions::default(),
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), t.finished())
        .await
        .unwrap()
        .unwrap();
    first.release(t).await.unwrap();
    drop(first);
    // The seeder is gone: anything that finishes now came from the disk.
    drop(seeder_session);

    let again = TorrentEngine::start(opts(leech.path())).await.unwrap();
    let refused = again
        .add(
            Source::File(torrent.clone()),
            None,
            vec![],
            AddOptions::default(),
        )
        .await;
    assert!(
        matches!(refused, Err(TorrentError::FileExists(_))),
        "{refused:?}"
    );
    let t = again
        .add(
            Source::File(torrent),
            None,
            vec![],
            AddOptions {
                resume: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), t.finished())
        .await
        .expect("recheck timed out")
        .unwrap();
    assert_eq!(t.progress().done, data.len() as u64);
    assert!(std::fs::read(leech.path().join("payload.bin")).unwrap() == data);
    assert!(
        state.path().read_dir().unwrap().next().is_none(),
        "no DHT, so nothing saved"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_torrent_can_start_paused() {
    let seed = tempfile::tempdir().unwrap();
    let leech = tempfile::tempdir().unwrap();
    let (_s, addr, torrent) = seeder(seed.path(), &payload(300_000)).await;
    let engine = TorrentEngine::start(EngineOptions {
        download_dir: leech.path().to_path_buf(),
        networks: vec![loopback()],
        dht: false,
        listen: None,
        state_dir: None,
    })
    .await
    .unwrap();
    let t = engine
        .add(
            Source::File(torrent),
            None,
            vec![addr],
            AddOptions {
                paused: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let p = t.progress();
    assert_eq!((p.phase, p.done), (Phase::Paused, 0), "{p:?}");
    engine.resume(&t).await.unwrap();
    tokio::time::timeout(Duration::from_secs(30), t.finished())
        .await
        .unwrap()
        .unwrap();
}
