//! A download handed over from another computer (REVIEW-2026-10-11 C3, C4):
//! what that computer had arrives while the rest comes from the server, into
//! the same file. Arriving bytes are never fetched; ones that never arrive are.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::io::{Seek, SeekFrom, Write};
use std::sync::Arc;
use std::time::Duration;

use fuselane_engine_http::download::{
    BoxIo, Connect, Incoming, JobError, Network, Report, Resume, Source, Tuning, download_with,
};
use fuselane_testkit::{Content, RangeServer, sha256_file};

const KB: u64 = 1024;
const BLOCK: u64 = 32 * KB;

fn plain(id: u32) -> Network {
    let connect: Connect = Arc::new(|addr| {
        Box::pin(async move { Ok(Box::new(tokio::net::TcpStream::connect(addr).await?) as BoxIo) })
    });
    Network {
        id,
        name: format!("net{id}"),
        connect,
        mirror: None,
    }
}

fn source(s: &RangeServer) -> Source {
    Source {
        addr: s.addr(),
        host: "localhost".into(),
        path: s.path().into(),
    }
}

/// An empty partial file of the full size, as the receiving computer makes it.
fn empty_part(dir: &std::path::Path, size: u64) -> std::path::PathBuf {
    let path = dir.join("handed.bin.fuselane");
    std::fs::File::create(&path).unwrap().set_len(size).unwrap();
    path
}

async fn run(
    server: &RangeServer,
    part: &std::path::Path,
    content: &Content,
    incoming: Arc<Incoming>,
) -> Result<Report, JobError> {
    let blocks = content.size.div_ceil(BLOCK) as usize;
    let tuning = Tuning {
        retry_delay_scale: 0.05,
        incoming: Some(incoming),
        ..Tuning::default()
    };
    let resume = Resume {
        staging_path: part.to_path_buf(),
        total: content.size,
        block_size: BLOCK,
        secured: vec![0; blocks],
        raw_etag: None,
        last_modified: None,
    };
    tokio::time::timeout(
        Duration::from_secs(30),
        download_with(
            source(server),
            vec![plain(1), plain(2)],
            part.parent().unwrap(),
            tuning,
            Some(resume),
        ),
    )
    .await
    .expect("download hung")
}

#[tokio::test(flavor = "multi_thread")]
async fn bytes_from_the_other_computer_fill_in_while_the_rest_downloads() {
    let content = Content::new(1600 * KB, 77);
    let server = RangeServer::start(content).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let part = empty_part(dir.path(), content.size);
    let blocks = content.size.div_ceil(BLOCK) as usize;
    // Every other block: its first 20 KB come from the other computer.
    let held: Vec<u64> = (0..blocks)
        .map(|b| if b % 2 == 0 { 20 * KB } else { 0 })
        .collect();
    let coming: u64 = held.iter().sum();
    let incoming = Incoming::new(held.clone());
    let writer = {
        let (incoming, part, content) = (incoming.clone(), part.clone(), content);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let mut f = std::fs::OpenOptions::new().write(true).open(&part).unwrap();
            for (b, n) in held.iter().enumerate().filter(|(_, n)| **n > 0) {
                let start = b as u64 * BLOCK;
                let mut buf = vec![0u8; *n as usize];
                content.fill(start, &mut buf);
                f.seek(SeekFrom::Start(start)).unwrap();
                f.write_all(&buf).unwrap();
                f.sync_data().unwrap();
                incoming.arrived(b, *n);
            }
        })
    };
    let report = run(&server, &part, &content, incoming).await.unwrap();
    writer.await.unwrap();
    assert_eq!(sha256_file(&report.path).unwrap(), content.sha256());
    let fetched: u64 = report.bytes_by_network.values().sum();
    assert!(
        fetched <= content.size - coming + 64 * KB,
        "fetched bytes that were arriving: {fetched} of {}",
        content.size - coming
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn bytes_that_never_arrive_are_downloaded_after_all() {
    let content = Content::new(640 * KB, 78);
    let server = RangeServer::start(content).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let part = empty_part(dir.path(), content.size);
    let blocks = content.size.div_ceil(BLOCK) as usize;
    // All of it was coming; the connection drops before any of it arrives.
    let incoming = Incoming::new(vec![BLOCK; blocks]);
    let dropped = {
        let incoming = incoming.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            incoming.release();
        })
    };
    let report = run(&server, &part, &content, incoming).await.unwrap();
    dropped.await.unwrap();
    assert_eq!(sha256_file(&report.path).unwrap(), content.sha256());
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_bytes_from_the_other_computer_are_never_published() {
    let content = Content::new(320 * KB, 79);
    let server = RangeServer::start(content).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let part = empty_part(dir.path(), content.size);
    let blocks = content.size.div_ceil(BLOCK) as usize;
    let incoming = Incoming::new(vec![BLOCK; blocks]);
    {
        // Garbage instead of the file, reported as arrived.
        let mut f = std::fs::OpenOptions::new().write(true).open(&part).unwrap();
        f.write_all(&vec![0xAB; content.size as usize]).unwrap();
        f.sync_data().unwrap();
        for b in 0..blocks {
            incoming.arrived(b, BLOCK);
        }
    }
    let res = run(&server, &part, &content, incoming).await;
    assert!(
        matches!(&res, Err(JobError::NotResumable(m)) if m.contains("other computer")),
        "{res:?}"
    );
    assert!(!dir.path().join("handed.bin").exists(), "nothing published");
}
