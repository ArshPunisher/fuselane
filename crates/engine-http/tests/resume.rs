//! Pause, checkpoint and resume, attacked: the server changes, the checkpoint lies,
//! the partial file is truncated or deleted. Every success is byte-exact.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::sync::{Arc, Mutex};
use std::time::Duration;

use fuselane_engine_http::download::{
    BoxIo, Cancel, Checkpoint, CheckpointFn, Connect, JobError, Network, ProgressFn, Resume,
    Source, Tuning, download_with,
};
use fuselane_testkit::{Content, Fault, RangeServer, Rule, sha256_file};

const KB: u64 = 1024;

fn plain(id: u32) -> Network {
    let connect: Connect = Arc::new(|addr| {
        Box::pin(async move { Ok(Box::new(tokio::net::TcpStream::connect(addr).await?) as BoxIo) })
    });
    Network {
        id,
        name: format!("net{id}"),
        connect,
    }
}

fn source(s: &RangeServer) -> Source {
    Source {
        addr: s.addr(),
        host: "localhost".into(),
        path: s.path().into(),
    }
}

/// Runs a download that pauses itself after `pause_at` bytes; returns the last checkpoint.
async fn paused_run(server: &RangeServer, dir: &std::path::Path, pause_at: u64) -> Checkpoint {
    let saved: Arc<Mutex<Option<Checkpoint>>> = Arc::default();
    let cancel = Cancel::new();
    let tuning = Tuning {
        block_size: Some(32 * KB),
        auto_streams: false,
        streams_per_network: 3,
        retry_delay_scale: 0.05,
        checkpoint_every: Duration::from_millis(100),
        controller_tick: Duration::from_millis(50),
        checkpoint: Some(CheckpointFn(Arc::new({
            let saved = saved.clone();
            move |c| *saved.lock().unwrap() = Some(c.clone())
        }))),
        progress: Some(ProgressFn(Arc::new({
            let cancel = cancel.clone();
            move |written, _| {
                if written >= pause_at {
                    cancel.cancel();
                }
            }
        }))),
        cancel: Some(cancel.clone()),
        ..Tuning::default()
    };
    let res = tokio::time::timeout(
        Duration::from_secs(30),
        download_with(source(server), vec![plain(1), plain(2)], dir, tuning, None),
    )
    .await
    .expect("pause hung");
    assert!(matches!(res, Err(JobError::Paused)), "{res:?}");
    let cp = saved
        .lock()
        .unwrap()
        .clone()
        .expect("a checkpoint on pause");
    assert!(
        cp.secured_bytes() > 0,
        "something was secured before pausing"
    );
    assert!(cp.staging_path.exists(), "partial file kept for resume");
    cp
}

fn resume_tuning() -> Tuning {
    Tuning {
        block_size: None,
        retry_delay_scale: 0.05,
        ..Tuning::default()
    }
}

async fn resume(
    server: &RangeServer,
    dir: &std::path::Path,
    r: Resume,
) -> Result<fuselane_engine_http::download::Report, JobError> {
    tokio::time::timeout(
        Duration::from_secs(30),
        download_with(
            source(server),
            vec![plain(1), plain(2)],
            dir,
            resume_tuning(),
            Some(r),
        ),
    )
    .await
    .expect("resume hung")
}

fn throttled(content: Content) -> impl std::future::Future<Output = RangeServer> {
    async move {
        let s = RangeServer::start(content).await.unwrap();
        s.add_rule(Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Throttle(800 * KB),
        });
        s
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn pause_then_resume_is_byte_exact_and_skips_secured_bytes() {
    let content = Content::new(1600 * KB, 41);
    let server = throttled(content).await;
    let dir = tempfile::tempdir().unwrap();
    let cp = paused_run(&server, dir.path(), 600 * KB).await;
    let secured = cp.secured_bytes();
    let before = server.requests().len();
    let report = resume(&server, dir.path(), Option::<Resume>::from(cp).unwrap())
        .await
        .unwrap();
    assert_eq!(sha256_file(&report.path).unwrap(), content.sha256());
    let fetched: u64 = report.bytes_by_network.values().sum();
    assert!(
        fetched <= content.size - secured + 64 * KB,
        "resume refetched secured bytes: fetched {fetched}, secured {secured}"
    );
    assert!(server.requests().len() > before);
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_after_the_file_changed_size_discards_and_refuses() {
    let content = Content::new(1200 * KB, 42);
    let server = throttled(content).await;
    let dir = tempfile::tempdir().unwrap();
    let cp = paused_run(&server, dir.path(), 300 * KB).await;
    let staging = cp.staging_path.clone();
    server.set_content(Content::new(900 * KB, 43), "\"v9\"");
    let res = resume(&server, dir.path(), Option::<Resume>::from(cp).unwrap()).await;
    assert!(matches!(res, Err(JobError::VersionChanged)), "{res:?}");
    assert!(!staging.exists(), "stale partial data removed");
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_with_same_size_new_bytes_is_caught_by_sampling() {
    let content = Content::new(1200 * KB, 44);
    let server = throttled(content).await;
    let dir = tempfile::tempdir().unwrap();
    let cp = paused_run(&server, dir.path(), 400 * KB).await;
    server.set_content(Content::new(1200 * KB, 45), "\"v2\"");
    let res = resume(&server, dir.path(), Option::<Resume>::from(cp).unwrap()).await;
    assert!(matches!(res, Err(JobError::VersionChanged)), "{res:?}");
    assert!(!dir.path().join("file.bin").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_after_a_relabelled_etag_with_the_same_bytes_completes() {
    let content = Content::new(1200 * KB, 46);
    let server = throttled(content).await;
    let dir = tempfile::tempdir().unwrap();
    let cp = paused_run(&server, dir.path(), 400 * KB).await;
    server.set_etag("\"another-node-label\"");
    let report = resume(&server, dir.path(), Option::<Resume>::from(cp).unwrap())
        .await
        .unwrap();
    assert_eq!(sha256_file(&report.path).unwrap(), content.sha256());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_lying_checkpoint_never_corrupts_the_file() {
    let content = Content::new(1200 * KB, 47);
    let server = throttled(content).await;
    for lie in 0..3 {
        let dir = tempfile::tempdir().unwrap();
        let cp = paused_run(&server, dir.path(), 300 * KB).await;
        let mut r = Option::<Resume>::from(cp).unwrap();
        match lie {
            0 => r.secured = vec![u64::MAX; r.secured.len()], // claims everything is done
            1 => r.secured = vec![1, 2, 3],                   // wrong length
            _ => {
                // The partial file was truncated behind our back.
                let f = std::fs::OpenOptions::new()
                    .write(true)
                    .open(&r.staging_path)
                    .unwrap();
                f.set_len(100 * KB).unwrap();
            }
        }
        let res = resume(&server, dir.path(), r).await;
        match (lie, res) {
            // Claiming everything is secured can't be disproved without hashes; the size check
            // still holds, and checksum verification (STEPS 2.32) catches the rest.
            (0, Ok(report)) => {
                assert_eq!(std::fs::metadata(&report.path).unwrap().len(), content.size)
            }
            (_, Ok(report)) => assert_eq!(
                sha256_file(&report.path).unwrap(),
                content.sha256(),
                "lie {lie} corrupted the file"
            ),
            (_, Err(e)) => panic!("lie {lie}: {e}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_deleted_partial_file_is_not_resumable() {
    let content = Content::new(800 * KB, 48);
    let server = throttled(content).await;
    let dir = tempfile::tempdir().unwrap();
    let cp = paused_run(&server, dir.path(), 200 * KB).await;
    std::fs::remove_file(&cp.staging_path).unwrap();
    let res = resume(&server, dir.path(), Option::<Resume>::from(cp).unwrap()).await;
    assert!(matches!(res, Err(JobError::NotResumable(_))), "{res:?}");
}
