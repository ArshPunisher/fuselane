//! Hostile .torrent files (L-68): each must be refused with a clear message before
//! anything is created on disk.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::net::{IpAddr, Ipv4Addr};

use fuselane_engine_torrent::{AddOptions, EngineOptions, Source, TorrentEngine, TorrentError};
use fuselane_netif::{Interface, Kind};

/// Minimal bencode for building fixtures.
enum B {
    I(i64),
    S(Vec<u8>),
    L(Vec<B>),
    D(Vec<(&'static str, B)>),
}

fn enc(b: &B, out: &mut Vec<u8>) {
    match b {
        B::I(i) => out.extend(format!("i{i}e").bytes()),
        B::S(s) => {
            out.extend(format!("{}:", s.len()).bytes());
            out.extend(s);
        }
        B::L(l) => {
            out.push(b'l');
            l.iter().for_each(|x| enc(x, out));
            out.push(b'e');
        }
        B::D(d) => {
            let mut d: Vec<_> = d.iter().collect();
            d.sort_by_key(|(k, _)| *k);
            out.push(b'd');
            for (k, v) in d {
                enc(&B::S(k.as_bytes().to_vec()), out);
                enc(v, out);
            }
            out.push(b'e');
        }
    }
}

fn s(x: &str) -> B {
    B::S(x.as_bytes().to_vec())
}

/// A multi-file torrent named `name` with one 10-byte file per path.
fn multi(name: &str, paths: &[&[&str]]) -> Vec<u8> {
    let files = paths
        .iter()
        .map(|p| {
            B::D(vec![
                ("length", B::I(10)),
                ("path", B::L(p.iter().map(|c| s(c)).collect())),
            ])
        })
        .collect();
    let info = B::D(vec![
        ("files", B::L(files)),
        ("name", s(name)),
        ("piece length", B::I(16384)),
        ("pieces", B::S(vec![7; 20])),
    ]);
    let mut out = Vec::new();
    enc(&B::D(vec![("info", info)]), &mut out);
    out
}

fn single(name: &str) -> Vec<u8> {
    let info = B::D(vec![
        ("length", B::I(10)),
        ("name", s(name)),
        ("piece length", B::I(16384)),
        ("pieces", B::S(vec![7; 20])),
    ]);
    let mut out = Vec::new();
    enc(&B::D(vec![("info", info)]), &mut out);
    out
}

async fn engine(dir: &std::path::Path) -> TorrentEngine {
    TorrentEngine::start(EngineOptions {
        download_dir: dir.to_path_buf(),
        networks: vec![Interface {
            name: "lo0".into(),
            display_name: "Loopback".into(),
            index: 1,
            kind: Kind::Loopback,
            addrs: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        }],
        dht: false,
        listen: None,
        state_dir: None,
        limiter: None,
        upnp: false,
        local_discovery: false,
    })
    .await
    .unwrap()
}

fn is_empty(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).unwrap().next().is_none()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_well_formed_fixture_is_accepted() {
    // Proves the fixtures themselves are valid, so refusals below are for the right reason.
    let root = tempfile::tempdir().unwrap();
    let dl = root.path().join("dl");
    std::fs::create_dir(&dl).unwrap();
    let e = engine(&dl).await;
    let t = e
        .add(
            Source::File(multi("Fine", &[&["a.txt"], &["sub", "b.txt"]])),
            None,
            vec![],
            AddOptions::default(),
        )
        .await;
    assert!(t.is_ok(), "{t:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hostile_torrents_are_refused_before_touching_the_disk() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "case collision",
            multi("T", &[&["Readme.txt"], &["README.TXT"]]),
        ),
        ("file is also a folder", multi("T", &[&["a"], &["a", "b"]])),
        ("duplicate", multi("T", &[&["x"], &["x"]])),
        ("dot-dot in a path", multi("T", &[&["..", "evil"]])),
        ("slash inside a name", multi("T", &[&["a/b"]])),
        ("backslash inside a name", multi("T", &[&["..\\evil"]])),
        ("empty component", multi("T", &[&["", "x"]])),
        ("no components", multi("T", &[&[]])),
        ("control character", multi("T", &[&["bell\u{7}"]])),
        ("torrent named ..", multi("..", &[&["x"]])),
        ("torrent named .", multi(".", &[&["x"]])),
        (
            "absolute torrent name",
            multi("/tmp/fuselane-escape", &[&["x"]]),
        ),
        (
            "traversing torrent name",
            multi("a/../../escape", &[&["x"]]),
        ),
        ("single file named ..", single("..")),
        ("single file with a slash", single("../escape.txt")),
        ("no files", multi("T", &[])),
    ];
    for (what, bytes) in cases {
        let root = tempfile::tempdir().unwrap();
        let dl = root.path().join("dl");
        std::fs::create_dir(&dl).unwrap();
        let e = engine(&dl).await;
        let r = e
            .add(Source::File(bytes), None, vec![], AddOptions::default())
            .await;
        assert!(
            matches!(
                r,
                Err(TorrentError::UnsafePath(_) | TorrentError::Invalid(_))
            ),
            "{what}: {r:?}"
        );
        assert!(
            is_empty(&dl),
            "{what}: something was written into the download folder"
        );
        let siblings: Vec<_> = std::fs::read_dir(root.path()).unwrap().collect();
        assert_eq!(
            siblings.len(),
            1,
            "{what}: something was written next to it"
        );
        assert!(
            !std::path::Path::new("/tmp/fuselane-escape").exists(),
            "{what}"
        );
    }
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_symlinked_folder_in_the_way_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let dl = root.path().join("dl");
    std::fs::create_dir(&dl).unwrap();
    std::os::unix::fs::symlink(outside.path(), dl.join("T")).unwrap();
    let e = engine(&dl).await;
    let r = e
        .add(
            Source::File(multi("T", &[&["x"]])),
            None,
            vec![],
            AddOptions::default(),
        )
        .await;
    assert!(
        matches!(&r, Err(TorrentError::UnsafePath(m)) if m.contains("link")),
        "{r:?}"
    );
    assert!(is_empty(outside.path()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn messages_name_the_problem() {
    let root = tempfile::tempdir().unwrap();
    let e = engine(root.path()).await;
    let r = e
        .add(
            Source::File(multi("T", &[&["Readme.txt"], &["README.TXT"]])),
            None,
            vec![],
            AddOptions::default(),
        )
        .await;
    let msg = r.unwrap_err().to_string();
    assert!(
        msg.contains("Readme.txt") && msg.contains("upper/lower case"),
        "{msg}"
    );
}
