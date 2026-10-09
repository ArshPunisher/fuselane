//! End-to-end attack suite: real downloads over several fake networks against a
//! hostile range server. Every completed file is checked byte-for-byte (L-90).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use fuselane_engine_http::download::{BoxIo, Connect, JobError, Network, Source, Tuning, download};
use fuselane_testkit::{Content, Fault, RangeServer, Rule, sha256_file};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

const KB: u64 = 1024;

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

fn dead(id: u32) -> Network {
    let connect: Connect = Arc::new(|_| {
        Box::pin(async {
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "network down",
            ))
        })
    });
    Network {
        id,
        name: format!("dead{id}"),
        connect,
        mirror: None,
    }
}

/// A network that delivers at most `bps` bytes per second (reads are throttled).
fn slow(id: u32, bps: u64) -> Network {
    let connect: Connect = Arc::new(move |addr| {
        Box::pin(async move {
            let s = tokio::net::TcpStream::connect(addr).await?;
            Ok(Box::new(Throttled {
                inner: s,
                bps,
                sleep: None,
            }) as BoxIo)
        })
    });
    Network {
        id,
        name: format!("slow{id}"),
        connect,
        mirror: None,
    }
}

struct Throttled {
    inner: tokio::net::TcpStream,
    bps: u64,
    sleep: Option<Pin<Box<tokio::time::Sleep>>>,
}

impl AsyncRead for Throttled {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if let Some(s) = self.sleep.as_mut() {
            if s.as_mut().poll(cx).is_pending() {
                return Poll::Pending;
            }
            self.sleep = None;
        }
        let chunk = (self.bps / 20).max(1) as usize; // 20 reads per second
        let mut limited = vec![0u8; chunk.min(buf.remaining())];
        let mut rb = ReadBuf::new(&mut limited);
        match Pin::new(&mut self.inner).poll_read(cx, &mut rb) {
            Poll::Ready(Ok(())) => {
                let n = rb.filled().len();
                buf.put_slice(rb.filled());
                if n > 0 {
                    self.sleep = Some(Box::pin(tokio::time::sleep(Duration::from_millis(50))));
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

use std::future::Future;

impl AsyncWrite for Throttled {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        b: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, b)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

fn source(s: &RangeServer) -> Source {
    Source {
        addr: s.addr(),
        host: "localhost".into(),
        path: s.path().into(),
    }
}

fn tuning() -> Tuning {
    Tuning {
        block_size: Some(64 * KB),
        streams_per_network: 3,
        idle_timeout: Duration::from_millis(500),
        first_byte_timeout: Duration::from_secs(2),
        connect_timeout: Duration::from_secs(2),
        retry_delay_scale: 0.05,
        ..Tuning::default()
    }
}

async fn run(
    content: Content,
    rules: &[Rule],
    networks: Vec<Network>,
) -> (
    Result<fuselane_engine_http::download::Report, JobError>,
    RangeServer,
    tempfile::TempDir,
) {
    let server = RangeServer::start(content).await.unwrap();
    for r in rules {
        server.add_rule(r.clone());
    }
    let dir = tempfile::tempdir().unwrap();
    let res = tokio::time::timeout(
        Duration::from_secs(60),
        download(source(&server), networks, dir.path(), tuning()),
    )
    .await
    .expect("download hung");
    (res, server, dir)
}

fn assert_exact(report: &fuselane_engine_http::download::Report, content: Content) {
    // Each byte is credited to exactly one network, even when hedges race (L-117).
    assert_eq!(
        report.bytes_by_network.values().sum::<u64>(),
        content.size,
        "per-network bytes must add up to the file: {:?}",
        report.bytes_by_network
    );
    assert_eq!(
        std::fs::metadata(&report.path).unwrap().len(),
        content.size,
        "size"
    );
    assert_eq!(
        sha256_file(&report.path).unwrap(),
        content.sha256(),
        "content differs from the server's file"
    );
}

#[tokio::test]
async fn bonded_download_is_byte_exact_and_uses_every_network() {
    let content = Content::new(3 * 1024 * KB + 17, 1);
    let (res, server, dir) = run(content, &[], vec![plain(1), plain(2), plain(3)]).await;
    let report = res.unwrap();
    assert_exact(&report, content);
    for net in [1, 2, 3] {
        assert!(
            report.bytes_by_network.get(&net).copied().unwrap_or(0) > 0,
            "network {net} did no work"
        );
    }
    let total: u64 = report.bytes_by_network.values().sum();
    assert!(total >= content.size);
    let log = server.requests();
    assert!(
        log.iter()
            .all(|r| r.accept_encoding.as_deref() == Some("identity")),
        "must ask for identity encoding (L-07)"
    );
    assert!(
        log.iter().skip(1).all(|r| r.if_range.is_some()),
        "segments must carry If-Range (L-05)"
    );
    assert!(
        !dir.path().join("file.bin.fuselane").exists(),
        "staging file renamed away"
    );
    assert_eq!(report.path.file_name().unwrap(), "file.bin");
}

#[tokio::test]
async fn a_dead_network_doesnt_stop_the_others() {
    let content = Content::new(700 * KB, 2);
    let (res, _server, _dir) = run(content, &[], vec![dead(1), plain(2)]).await;
    assert_exact(&res.unwrap(), content);
}

#[tokio::test]
async fn no_reachable_network_is_a_clear_error() {
    let (res, _server, _dir) = run(Content::new(10 * KB, 3), &[], vec![dead(1), dead(2)]).await;
    assert!(matches!(res, Err(JobError::Unreachable(_))), "{res:?}");
}

#[tokio::test]
async fn lying_servers_never_corrupt_the_file() {
    let content = Content::new(900 * KB, 4);
    for fault in [
        Fault::Overrun(5_000),
        Fault::WrongStart(4_096),
        Fault::ShortBody(10_000),
        Fault::NoContentRange,
        Fault::Reset,
        Fault::StallAt(20_000),
        Fault::CapRange(7_000),
    ] {
        // Skip the probe, then misbehave on several segments.
        let rules = [Rule {
            skip: 1,
            times: 4,
            fault: fault.clone(),
        }];
        let (res, server, _dir) = run(content, &rules, vec![plain(1), plain(2)]).await;
        let report = res.unwrap_or_else(|e| panic!("{fault:?}: {e}"));
        assert_exact(&report, content);
        assert!(
            server
                .requests()
                .iter()
                .filter(|r| r.fault.is_some())
                .count()
                >= 1,
            "{fault:?} was never exercised"
        );
    }
}

#[tokio::test]
async fn a_server_that_always_caps_ranges_still_completes() {
    let content = Content::new(400 * KB, 5);
    let (res, _server, _dir) = run(
        content,
        &[Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::CapRange(10_000),
        }],
        vec![plain(1), plain(2)],
    )
    .await;
    assert_exact(&res.unwrap(), content);
}

#[tokio::test]
async fn busy_server_with_retry_after_is_waited_out() {
    let content = Content::new(300 * KB, 6);
    let rules = [Rule {
        skip: 1,
        times: 3,
        fault: Fault::Status(503, Some("1".into())),
    }];
    let (res, _server, _dir) = run(content, &rules, vec![plain(1)]).await;
    let report = res.unwrap();
    assert_exact(&report, content);
    assert!(report.retries >= 3);
}

#[tokio::test]
async fn a_server_without_ranges_downloads_on_one_stream() {
    let content = Content::new(200 * KB, 7);
    let (res, server, _d) = run(
        content,
        &[Rule::always(Fault::IgnoreRange)],
        vec![plain(1), plain(2)],
    )
    .await;
    let report = res.unwrap();
    assert_exact(&report, content);
    assert_eq!(
        report.bytes_by_network.len(),
        1,
        "non-splittable: one network only"
    );
    assert_eq!(server.requests().len(), 2, "probe + one full fetch");
}

#[tokio::test]
async fn an_expired_link_stops_with_fix_link_and_keeps_the_partial_file() {
    let content = Content::new(500 * KB, 8);
    let (res, _s, dir) = run(
        content,
        &[Rule {
            skip: 1,
            times: u32::MAX,
            fault: Fault::Status(410, None),
        }],
        vec![plain(1), plain(2)],
    )
    .await;
    assert!(matches!(res, Err(JobError::LinkExpired(410))), "{res:?}");
    assert!(
        dir.path().join("file.bin.fuselane").exists(),
        "partial data kept for Fix link"
    );
}

#[tokio::test]
async fn brief_403s_from_rate_limiting_dont_end_the_download() {
    // Regression for chaos seed 43: four transient 403s were taken for an expired link.
    let content = Content::new(400 * KB, 43);
    let rules = [Rule {
        skip: 1,
        times: 6,
        fault: Fault::Status(403, None),
    }];
    let (res, _server, _dir) = run(content, &rules, vec![plain(1), plain(2)]).await;
    assert_exact(&res.unwrap(), content);
}

#[tokio::test]
async fn an_empty_file_publishes_an_empty_file() {
    let content = Content::new(0, 9);
    let (res, _server, _dir) = run(content, &[], vec![plain(1)]).await;
    let report = res.unwrap();
    assert_eq!(std::fs::metadata(&report.path).unwrap().len(), 0);
}

#[tokio::test]
async fn a_file_that_changes_size_mid_download_is_refused_and_discarded() {
    let content = Content::new(2 * 1024 * KB, 10);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(400 * KB),
    });
    server.swap_at_request(3, Content::new(3 * 1024 * KB, 99), "\"v2\"");
    let dir = tempfile::tempdir().unwrap();
    let res = tokio::time::timeout(
        Duration::from_secs(60),
        download(
            source(&server),
            vec![plain(1), plain(2)],
            dir.path(),
            tuning(),
        ),
    )
    .await
    .unwrap();
    assert!(matches!(res, Err(JobError::VersionChanged)), "{res:?}");
    assert!(
        !dir.path().join("file.bin").exists(),
        "never publish a mixed file"
    );
    assert!(
        !dir.path().join("file.bin.fuselane").exists(),
        "useless partial data is discarded"
    );
}

#[tokio::test]
async fn same_size_new_bytes_with_a_new_etag_is_detected_by_sampling() {
    let content = Content::new(2 * 1024 * KB, 11);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(400 * KB),
    });
    let mut t = tuning();
    t.auto_streams = false;
    t.streams_per_network = 2;
    // Swap once some segments are already on disk: same size, different bytes, new label.
    server.swap_at_request(8, Content::new(2 * 1024 * KB, 12), "\"v2\"");
    let dir = tempfile::tempdir().unwrap();
    let res = tokio::time::timeout(
        Duration::from_secs(60),
        download(source(&server), vec![plain(1), plain(2)], dir.path(), t),
    )
    .await
    .unwrap();
    assert!(matches!(res, Err(JobError::VersionChanged)), "{res:?}");
    assert!(!dir.path().join("file.bin").exists());
}

#[tokio::test]
async fn a_relabelled_etag_with_the_same_bytes_is_accepted() {
    let content = Content::new(2 * 1024 * KB, 13);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(600 * KB),
    });
    let mut t = tuning();
    t.auto_streams = false;
    t.streams_per_network = 2;
    // A load balancer's different label for the same file.
    server.swap_at_request(8, content, "\"v1-other-node\"");
    let dir = tempfile::tempdir().unwrap();
    let res = tokio::time::timeout(
        Duration::from_secs(60),
        download(source(&server), vec![plain(1), plain(2)], dir.path(), t),
    )
    .await
    .unwrap();
    assert_exact(&res.unwrap(), content);
}

#[tokio::test]
async fn a_crawling_network_gets_its_tail_raced_by_a_fast_one() {
    let content = Content::new(1024 * KB, 14);
    let server = RangeServer::start(content).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut t = tuning();
    t.block_size = Some(256 * KB);
    t.streams_per_network = 2;
    t.hedge.after_ms = 300;
    t.hedge.startup_ms = 50;
    let started = std::time::Instant::now();
    let res = tokio::time::timeout(
        Duration::from_secs(60),
        download(
            source(&server),
            vec![slow(1, 20 * KB), plain(2)],
            dir.path(),
            t,
        ),
    )
    .await
    .unwrap();
    let report = res.unwrap();
    assert_exact(&report, content);
    assert!(
        report.hedges > 0,
        "the slow network's blocks should have been raced"
    );
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "racing should keep a 20 KB/s network from holding the download up: {:?}",
        started.elapsed()
    );
}

/// Chaos: random fault scripts, random dead networks, tiny blocks. The invariant
/// (L-90): either a byte-exact file or a clean error; never wrong bytes, never a hang.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn chaos_never_publishes_wrong_bytes_or_hangs() {
    let seeds: u64 = std::env::var("FUSELANE_CHAOS_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let mut ok = 0;
    let mut failed_cleanly = 0;
    for seed in 0..seeds {
        let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = |n: u64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x % n
        };
        let content = Content::new(64 * KB + next(400 * KB), seed);
        let mut rules = Vec::new();
        for _ in 0..next(5) {
            let fault = match next(10) {
                0 => Fault::Overrun(1 + next(9_000)),
                1 => Fault::WrongStart(1 + next(9_000)),
                2 => Fault::ShortBody(next(20_000)),
                3 => Fault::StallAt(next(20_000)),
                4 => Fault::NoContentRange,
                5 => Fault::Reset,
                6 => Fault::CapRange(1 + next(30_000)),
                7 => Fault::Status(
                    [429, 500, 502, 503, 504][next(5) as usize],
                    Some("0".into()),
                ),
                8 => Fault::Throttle(200 * KB + next(800 * KB)),
                _ => Fault::Status(403, None),
            };
            rules.push(Rule {
                skip: 1 + next(6) as u32,
                times: 1 + next(4) as u32,
                fault,
            });
        }
        let mut nets = vec![plain(1), plain(2)];
        if next(3) == 0 {
            nets.push(dead(3));
        }
        let server = RangeServer::start(content).await.unwrap();
        for r in &rules {
            server.add_rule(r.clone());
        }
        let dir = tempfile::tempdir().unwrap();
        let mut t = tuning();
        t.block_size = Some(4 * KB + next(60 * KB));
        t.streams_per_network = 1 + next(4) as u32;
        t.idle_timeout = Duration::from_millis(300);
        let res = tokio::time::timeout(
            Duration::from_secs(45),
            download(source(&server), nets, dir.path(), t),
        )
        .await
        .unwrap_or_else(|_| panic!("seed {seed} hung; rules {rules:?}"));
        match res {
            Ok(report) => {
                assert_eq!(
                    sha256_file(&report.path).unwrap(),
                    content.sha256(),
                    "seed {seed} published wrong bytes; rules {rules:?}"
                );
                ok += 1;
            }
            Err(e) => {
                eprintln!("seed {seed} failed cleanly: {e}; rules {rules:?}");
                assert!(
                    !dir.path().join("file.bin").exists(),
                    "seed {seed}: error {e} but a final file exists"
                );
                failed_cleanly += 1;
            }
        }
    }
    eprintln!("chaos: {ok} exact downloads, {failed_cleanly} clean failures out of {seeds}");
    assert!(
        ok * 10 >= seeds * 8,
        "too many failures: only {ok}/{seeds} completed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auto_streams_grow_while_everyone_is_served() {
    // Each connection is throttled, so more streams genuinely help (like per-connection caps on CDNs).
    let content = Content::new(4 * 1024 * KB, 31);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(150 * KB),
    });
    let dir = tempfile::tempdir().unwrap();
    let mut t = tuning();
    t.block_size = Some(32 * KB);
    t.auto_streams = true;
    t.controller_tick = Duration::from_millis(100);
    let res = tokio::time::timeout(
        Duration::from_secs(60),
        download(source(&server), vec![plain(1)], dir.path(), t),
    )
    .await
    .unwrap();
    let report = res.unwrap();
    assert_exact(&report, content);
    let peak = report.peak_streams.get(&1).copied().unwrap_or(0);
    assert!(peak > 8, "Auto should grow past 8 streams; peak was {peak}");
    assert!(peak <= 32, "never beyond 32 per network; peak was {peak}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_server_that_caps_connections_is_respected_not_hammered() {
    let content = Content::new(3 * 1024 * KB, 32);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(300 * KB),
    });
    server.set_max_concurrent(Some(6));
    let dir = tempfile::tempdir().unwrap();
    let mut t = tuning();
    t.block_size = Some(48 * KB);
    t.auto_streams = true;
    t.controller_tick = Duration::from_millis(100);
    let res = tokio::time::timeout(
        Duration::from_secs(90),
        download(source(&server), vec![plain(1)], dir.path(), t),
    )
    .await
    .unwrap();
    assert_exact(&res.unwrap(), content);
    // Judge only the second half: by then the controller has reacted to the refusals.
    let log = server.requests();
    let late = &log[log.len() / 2..];
    let refused_late = late
        .iter()
        .filter(|r| r.fault == Some(Fault::Status(503, None)))
        .count();
    assert!(
        refused_late * 10 < late.len(),
        "the controller should settle at the server's cap: {refused_late} of the last {} requests refused",
        late.len()
    );
}

#[tokio::test]
async fn an_etag_change_before_any_byte_is_secured_is_adopted_not_looped() {
    // Regression: confirming with nothing on disk returned "same" without adopting the
    // new validator, so every If-Range mismatch repeated forever.
    let content = Content::new(600 * KB, 61);
    let server = RangeServer::start(content).await.unwrap();
    server.swap_at_request(2, content, "\"relabelled-at-once\"");
    let (dir, mut t) = (tempfile::tempdir().unwrap(), tuning());
    t.auto_streams = false;
    t.streams_per_network = 2;
    let res = tokio::time::timeout(
        Duration::from_secs(20),
        download(source(&server), vec![plain(1)], dir.path(), t),
    )
    .await
    .expect("looped instead of adopting the new ETag");
    assert_exact(&res.unwrap(), content);
}

#[tokio::test(flavor = "multi_thread")]
async fn snapshots_describe_the_job_for_the_ui() {
    use fuselane_engine_http::download::{Snapshot, SnapshotFn, TICKS};
    let content = Content::new(2 * 1024 * KB, 71);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(900 * KB),
    });
    let seen: Arc<std::sync::Mutex<Vec<Snapshot>>> = Arc::default();
    let mut t = tuning();
    t.snapshot_every = Duration::from_millis(50);
    t.snapshot = Some(SnapshotFn(Arc::new({
        let seen = seen.clone();
        move |s| seen.lock().unwrap().push(s.clone())
    })));
    let dir = tempfile::tempdir().unwrap();
    let report = download(source(&server), vec![plain(1), plain(2)], dir.path(), t)
        .await
        .unwrap();
    assert_exact(&report, content);
    let snaps = seen.lock().unwrap();
    assert!(snaps.len() >= 3, "only {} snapshots", snaps.len());
    let mut last_written = 0;
    for s in snaps.iter() {
        assert!(s.written >= last_written, "written went backwards");
        last_written = s.written;
        assert!(s.ticks.len() <= TICKS && !s.ticks.is_empty());
        assert!(s.ticks.iter().all(|t| (0.0..=1.0).contains(&t.fill)));
        assert!(s.rate.is_finite() && s.rate >= 0.0);
        assert_eq!(s.total, Some(content.size));
        assert_eq!(s.networks.len(), 2);
    }
    // The last snapshot is the finished picture: every byte, every tick full and owned.
    let last = snaps.last().unwrap();
    assert_eq!(last.written, content.size);
    assert!(
        last.ticks
            .iter()
            .all(|t| t.fill == 1.0 && t.owner.is_some()),
        "final ticks incomplete"
    );
    assert_eq!(
        last.networks.iter().map(|n| n.bytes).sum::<u64>(),
        content.size
    );
    let mid = &snaps[snaps.len() / 2];
    assert!(
        mid.ticks.iter().any(|t| t.owner.is_some()),
        "ticks should know who fetched them"
    );
    assert!(mid.networks.iter().all(|n| n.bytes > 0 || n.streams > 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rate_limited_probe_waits_and_then_downloads() {
    let content = Content::new(256 * KB, 72);
    let server = RangeServer::start(content).await.unwrap();
    // The very first answers are 429 with Retry-After, as a throttling mirror does.
    server.add_rule(Rule {
        skip: 0,
        times: 2,
        fault: Fault::Status(429, Some("1".into())),
    });
    let dir = tempfile::tempdir().unwrap();
    let report = download(source(&server), vec![plain(1)], dir.path(), tuning())
        .await
        .unwrap();
    assert_exact(&report, content);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_never_stops_rate_limiting_reports_429() {
    let content = Content::new(64 * KB, 73);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 0,
        times: u32::MAX,
        fault: Fault::Status(429, Some("3600".into())),
    });
    let dir = tempfile::tempdir().unwrap();
    let start = std::time::Instant::now();
    let err = download(source(&server), vec![plain(1)], dir.path(), tuning())
        .await
        .unwrap_err();
    assert!(matches!(err, JobError::ProbeStatus(429)), "{err:?}");
    // Retry-After is capped, so an hour-long ask doesn't hang the probe.
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_speed_limit_holds_the_whole_download_near_the_limit() {
    use fuselane_limits::{LimitSettings, Limiter};
    let content = Content::new(1536 * KB, 74);
    let server = RangeServer::start(content).await.unwrap();
    let limiter = Arc::new(Limiter::default());
    // 1 MiB/s overall, across two networks: about 1.5 s for 1.5 MiB.
    limiter.apply(&LimitSettings {
        global: 1024 * KB,
        networks: vec![],
    });
    let mut t = tuning();
    t.limiter = Some(limiter);
    let dir = tempfile::tempdir().unwrap();
    let start = std::time::Instant::now();
    let report = download(source(&server), vec![plain(1), plain(2)], dir.path(), t)
        .await
        .unwrap();
    assert_exact(&report, content);
    let secs = start.elapsed().as_secs_f64();
    assert!(secs > 1.1, "limit ignored: {secs:.2} s");
    assert!(secs < 3.5, "far too slow: {secs:.2} s");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_download_limit_holds_it_back_and_lifting_it_lets_it_finish() {
    use fuselane_limits::JobLimit;
    let content = Content::new(1536 * KB, 77);
    let server = RangeServer::start(content).await.unwrap();
    // 512 KiB/s for this download only: 1.5 MiB would take about 3 s.
    let job = Arc::new(JobLimit::new(512 * KB));
    let mut t = tuning();
    t.job_limit = Some(job.clone());
    let dir = tempfile::tempdir().unwrap();
    let start = std::time::Instant::now();
    let lift = {
        let job = job.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1200)).await;
            job.set_rate(0);
        })
    };
    let report = download(source(&server), vec![plain(1), plain(2)], dir.path(), t)
        .await
        .unwrap();
    lift.await.unwrap();
    assert_exact(&report, content);
    let secs = start.elapsed().as_secs_f64();
    assert!(secs > 1.0, "limit ignored: {secs:.2} s");
    assert!(
        secs < 2.8,
        "lifting the limit didn't take effect: {secs:.2} s"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_capped_network_carries_less_while_the_other_takes_up_the_slack() {
    use fuselane_limits::{LimitSettings, Limiter};
    let content = Content::new(2 * 1024 * KB, 75);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(1024 * KB),
    });
    let limiter = Arc::new(Limiter::default());
    limiter.apply(&LimitSettings {
        global: 0,
        networks: vec![("net1".into(), 96 * KB)],
    });
    let mut t = tuning();
    t.limiter = Some(limiter);
    let dir = tempfile::tempdir().unwrap();
    let report = download(source(&server), vec![plain(1), plain(2)], dir.path(), t)
        .await
        .unwrap();
    assert_exact(&report, content);
    let capped = report.bytes_by_network.get(&1).copied().unwrap_or(0);
    let free = report.bytes_by_network.get(&2).copied().unwrap_or(0);
    assert!(
        capped * 3 < free,
        "capped net carried {capped}, free net {free}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_network_past_its_allowance_carries_nothing_and_the_rest_finish() {
    use fuselane_limits::Limiter;
    let content = Content::new(1024 * KB, 76);
    let server = RangeServer::start(content).await.unwrap();
    let limiter = Arc::new(Limiter::default());
    limiter.set_blocked(["net1".to_string()]);
    let mut t = tuning();
    t.limiter = Some(limiter);
    let dir = tempfile::tempdir().unwrap();
    let report = download(source(&server), vec![plain(1), plain(2)], dir.path(), t)
        .await
        .unwrap();
    assert_exact(&report, content);
    assert_eq!(report.bytes_by_network.get(&1).copied().unwrap_or(0), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn blocking_a_network_mid_download_hands_its_work_to_the_others() {
    use fuselane_limits::Limiter;
    let content = Content::new(6 * 1024 * KB, 77);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(256 * KB),
    });
    let limiter = Arc::new(Limiter::default());
    let mut t = tuning();
    t.limiter = Some(limiter.clone());
    let dir = tempfile::tempdir().unwrap();
    let blocker = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        limiter.set_blocked(["net1".to_string()]);
    });
    let report = download(source(&server), vec![plain(1), plain(2)], dir.path(), t)
        .await
        .unwrap();
    blocker.await.unwrap();
    assert_exact(&report, content);
    let blocked_share = report.bytes_by_network.get(&1).copied().unwrap_or(0);
    assert!(
        blocked_share < content.size / 2,
        "net1 kept downloading after its block: {blocked_share}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn with_every_network_blocked_the_download_waits_and_can_still_pause() {
    use fuselane_engine_http::download::{Cancel, download_with};
    use fuselane_limits::Limiter;
    let content = Content::new(512 * KB, 78);
    let server = RangeServer::start(content).await.unwrap();
    let limiter = Arc::new(Limiter::default());
    limiter.set_blocked(["net1".to_string(), "net2".to_string()]);
    let cancel = Cancel::new();
    let mut t = tuning();
    t.limiter = Some(limiter);
    t.cancel = Some(cancel.clone());
    let dir = tempfile::tempdir().unwrap();
    let stopper = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1200)).await;
        cancel.cancel();
    });
    let r = download_with(
        source(&server),
        vec![plain(1), plain(2)],
        dir.path(),
        t,
        None,
    )
    .await;
    stopper.await.unwrap();
    assert!(matches!(r, Err(JobError::Paused)), "{r:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_bigger_than_the_disk_fails_before_anything_is_written() {
    // The server claims 10 PB; no disk has that, so the download must stop at once.
    let content = Content::new(10 * 1024_u64.pow(5), 79);
    let server = RangeServer::start(content).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let start = std::time::Instant::now();
    let err = download(source(&server), vec![plain(1)], dir.path(), tuning())
        .await
        .unwrap_err();
    match err {
        JobError::NoSpace { needed, free } => {
            assert_eq!(needed, content.size);
            assert!(free < needed);
        }
        other => panic!("expected NoSpace, got {other:?}"),
    }
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "no partial file left behind"
    );
}

fn mirror_lane(
    id: u32,
    mirror: &RangeServer,
    probe: &fuselane_engine_http::download::Probe,
) -> Network {
    Network {
        mirror: Some(Arc::new(fuselane_engine_http::download::Mirror {
            source: source(mirror),
            if_range: probe.raw_etag.clone(),
            etag: probe.etag.clone(),
        })),
        ..plain(id)
    }
}

#[tokio::test]
async fn a_mirror_carries_part_of_the_file_and_it_stays_exact() {
    let content = Content::new(2 * 1024 * KB + 5, 31);
    let main = RangeServer::start(content).await.unwrap();
    let mirror = RangeServer::start(content).await.unwrap();
    let t = tuning();
    let mp = fuselane_engine_http::download::probe(&source(&mirror), &[plain(9)], &t)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let report = tokio::time::timeout(
        Duration::from_secs(60),
        download(
            source(&main),
            vec![plain(1), mirror_lane(2, &mirror, &mp)],
            dir.path(),
            t,
        ),
    )
    .await
    .expect("download hung")
    .unwrap();
    assert_exact(&report, content);
    assert!(
        report.bytes_by_network.get(&2).copied().unwrap_or(0) > 0,
        "the mirror did no work"
    );
    assert!(!mirror.requests().is_empty());
    // The mirror is asked with its own validator, never the main server's.
    assert!(
        mirror
            .requests()
            .iter()
            .all(|r| r.if_range == mp.raw_etag || r.if_range.is_none())
    );
    // The spot check reads the same bytes from both.
    let a = fuselane_engine_http::download::read_range(
        &source(&main),
        &plain(1),
        &tuning(),
        1000,
        1999,
    )
    .await
    .unwrap();
    let b = fuselane_engine_http::download::read_range(
        &source(&mirror),
        &plain(2),
        &tuning(),
        1000,
        1999,
    )
    .await
    .unwrap();
    assert_eq!(a, b);
}

#[tokio::test]
async fn a_mirror_with_a_different_file_is_dropped_and_the_file_stays_exact() {
    let content = Content::new(1024 * KB, 32);
    let main = RangeServer::start(content).await.unwrap();
    // Same name, different size: every answer it gives fails the size check.
    let other = RangeServer::start(Content::new(1024 * KB + 1, 33))
        .await
        .unwrap();
    let t = tuning();
    let op = fuselane_engine_http::download::probe(&source(&other), &[plain(9)], &t)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let report = tokio::time::timeout(
        Duration::from_secs(60),
        download(
            source(&main),
            vec![plain(1), mirror_lane(2, &other, &op)],
            dir.path(),
            t,
        ),
    )
    .await
    .expect("download hung")
    .unwrap();
    assert_exact(&report, content);
    assert_eq!(report.bytes_by_network.get(&2).copied().unwrap_or(0), 0);
}
