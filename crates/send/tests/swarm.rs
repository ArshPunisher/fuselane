//! Two real librqbit sessions on loopback: one seeds a plain file through the
//! encrypted view, the other receives it into a plain file. Proves the view, the
//! torrent builder and the storage adapter work together (STEPS 6.3).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use fuselane_send::crypt::{HEADER_BLOCK, Header, Keys};
use fuselane_send::storage::ShareStorageFactory;
use fuselane_send::torrent;
use fuselane_send::view::{ReadOnly, View};
use librqbit::storage::StorageFactoryExt;
use librqbit::{AddTorrent, AddTorrentOptions, ListenerOptions, Session, SessionOptions};

const LINK_KEY: [u8; 32] = [0x5a; 32];

fn payload(n: usize) -> Vec<u8> {
    let mut x = 0x9e37_79b9_u32;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

fn opts(listen: bool) -> SessionOptions {
    SessionOptions {
        dht: None,
        persistence: None,
        fastresume: false,
        disable_local_service_discovery: true,
        listen: listen.then(|| ListenerOptions {
            listen_addr: (Ipv4Addr::LOCALHOST, 0).into(),
            enable_upnp_port_forwarding: false,
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_encrypted_share_crosses_between_two_sessions_byte_exact() {
    let send_dir = tempfile::tempdir().unwrap();
    let recv_dir = tempfile::tempdir().unwrap();
    let data = payload(2 * 1024 * 1024 + 4321);
    let original = send_dir.path().join("holiday.mp4");
    std::fs::write(&original, &data).unwrap();

    // Sender: seal the header, view the plain file, build the torrent.
    let keys = Keys::derive(&LINK_KEY);
    let header = Header {
        name: "holiday.mp4".into(),
        size: data.len() as u64,
        hash: *blake3::hash(&data).as_bytes(),
    };
    let head_block = keys.seal(&header).unwrap();
    let head_file = send_dir.path().join("head.bin");
    std::fs::write(&head_file, &head_block).unwrap();
    let view = Arc::new(View::new(
        keys,
        Box::new(ReadOnly(std::fs::File::open(&head_file).unwrap())),
        Box::new(ReadOnly(std::fs::File::open(&original).unwrap())),
        data.len() as u64,
    ));
    let built = torrent::build(&view, |_| {}).unwrap();

    let sender = Session::new_with_opts(send_dir.path().to_path_buf(), opts(true))
        .await
        .unwrap();
    let seeding = sender
        .add_torrent(
            AddTorrent::TorrentFileBytes(built.torrent().into()),
            Some(AddTorrentOptions {
                overwrite: true,
                storage_factory: Some(ShareStorageFactory(view).boxed()),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    // Its own check reads every piece through the view: all must already match.
    tokio::time::timeout(Duration::from_secs(30), seeding.wait_until_completed())
        .await
        .expect("sender check timed out")
        .unwrap();
    let addr = sender.listen_addr().unwrap();

    // Receiver: knows only the link (key + info-hash) and the torrent length.
    let total = built.torrent().len();
    assert!(total > 0);
    let open = |name: &str| {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(recv_dir.path().join(name))
            .unwrap()
    };
    let size = data.len() as u64; // = torrent length - HEADER_BLOCK
    let recv_view = Arc::new(View::new(
        Keys::derive(&LINK_KEY),
        Box::new(open(".share.head")),
        Box::new(open(".share.part")),
        size,
    ));
    let receiver = Session::new_with_opts(recv_dir.path().to_path_buf(), opts(false))
        .await
        .unwrap();
    let receiving = receiver
        .add_torrent(
            AddTorrent::TorrentFileBytes(built.torrent().into()),
            Some(AddTorrentOptions {
                overwrite: true,
                initial_peers: Some(vec![addr]),
                storage_factory: Some(ShareStorageFactory(recv_view).boxed()),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(60), receiving.wait_until_completed())
        .await
        .expect("receive timed out")
        .unwrap();

    // The header opens with the link's key and the plain file is byte-exact.
    let head = std::fs::read(recv_dir.path().join(".share.head")).unwrap();
    assert_eq!(head.len(), HEADER_BLOCK);
    let got_header = Keys::derive(&LINK_KEY).open(&head).unwrap();
    assert_eq!(got_header, header);
    let got = std::fs::read(recv_dir.path().join(".share.part")).unwrap();
    assert!(got == data, "received file differs");
    assert_eq!(blake3::hash(&got).as_bytes(), &got_header.hash);
    // The sender's file was never touched.
    assert!(std::fs::read(&original).unwrap() == data);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_file_that_changed_after_sharing_fails_the_senders_check() {
    let dir = tempfile::tempdir().unwrap();
    let data = payload(600_000);
    let file = dir.path().join("doc.pdf");
    std::fs::write(&file, &data).unwrap();
    let keys = Keys::derive(&LINK_KEY);
    let head_block = keys
        .seal(&Header {
            name: "doc.pdf".into(),
            size: data.len() as u64,
            hash: *blake3::hash(&data).as_bytes(),
        })
        .unwrap();
    std::fs::write(dir.path().join("head.bin"), &head_block).unwrap();
    let make_view = || {
        Arc::new(View::new(
            Keys::derive(&LINK_KEY),
            Box::new(ReadOnly(
                std::fs::File::open(dir.path().join("head.bin")).unwrap(),
            )),
            Box::new(ReadOnly(std::fs::File::open(&file).unwrap())),
            data.len() as u64,
        ))
    };
    let built = torrent::build(&make_view(), |_| {}).unwrap();
    // The person edits the file after sharing it.
    let mut changed = data.clone();
    changed[300_000] ^= 0xff;
    std::fs::write(&file, &changed).unwrap();

    let session = Session::new_with_opts(dir.path().to_path_buf(), opts(false))
        .await
        .unwrap();
    let h = session
        .add_torrent(
            AddTorrent::TorrentFileBytes(built.torrent().into()),
            Some(AddTorrentOptions {
                overwrite: true,
                paused: false,
                storage_factory: Some(ShareStorageFactory(make_view()).boxed()),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .into_handle()
        .unwrap();
    // Wait for librqbit's own check to finish, then the changed piece must be
    // missing: exactly one piece short, and never "finished".
    let stats = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let st = h.stats();
            if !matches!(st.state, librqbit::TorrentStatsState::Initializing { .. }) {
                return st;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("check never finished");
    assert!(
        !stats.finished,
        "a changed file must not look complete: {stats:?}"
    );
    let missing = stats.total_bytes - stats.progress_bytes;
    assert_eq!(
        missing,
        u64::from(built.piece_length),
        "only the edited piece: {stats:?}"
    );
    assert!(
        std::fs::read(&file).unwrap() == changed,
        "and is never rewritten"
    );
}
