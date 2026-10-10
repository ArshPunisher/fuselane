//! Throttle detection end to end (STEPS 8.2): two networks reach the test
//! server through their own lanes, and the server caps one lane the way a phone
//! plan does after its daily quota. Every finished file is checked by SHA-256.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fuselane_engine_http::download::{
    BoxIo, Connect, LaneEvent, LaneEventFn, Network, Report, Source, Tuning, download,
};
use fuselane_engine_http::throttle::ThrottlePolicy;
use fuselane_testkit::{Content, RangeServer, sha256_file};

const KB: u64 = 1024;
const MB: u64 = 1024 * KB;

/// A network whose every connection goes through its own lane of the server.
fn lane_net(id: u32, lane: SocketAddr) -> Network {
    let connect: Connect = Arc::new(move |_| {
        Box::pin(async move { Ok(Box::new(tokio::net::TcpStream::connect(lane).await?) as BoxIo) })
    });
    Network {
        id,
        name: format!("net{id}"),
        connect,
        mirror: None,
    }
}

type Events = Arc<Mutex<Vec<LaneEvent>>>;

/// Short windows so a test takes seconds: judged over 2 s, checked after 1 s.
fn tuning(events: &Events, min_total: u64) -> Tuning {
    let sink = events.clone();
    Tuning {
        block_size: Some(64 * KB),
        streams_per_network: 3,
        auto_streams: false,
        idle_timeout: Duration::from_secs(5),
        first_byte_timeout: Duration::from_secs(5),
        connect_timeout: Duration::from_secs(2),
        retry_delay_scale: 0.05,
        controller_tick: Duration::from_millis(100),
        throttle: ThrottlePolicy {
            window_ms: 2_000,
            best_window_ms: 1_000,
            min_total,
            probe_after_ms: 1_000,
            probe_max_ms: 4_000,
            probe_ms: 1_000,
            probe_wait_ms: 3_000,
            ..ThrottlePolicy::default()
        },
        lane_events: Some(LaneEventFn(Arc::new(move |e| {
            sink.lock().unwrap().push(e.clone());
        }))),
        ..Tuning::default()
    }
}

fn source(s: &RangeServer) -> Source {
    Source {
        addr: s.addr(),
        host: "localhost".into(),
        path: s.path().into(),
    }
}

fn assert_exact(report: &Report, content: Content) {
    assert_eq!(std::fs::metadata(&report.path).unwrap().len(), content.size);
    assert_eq!(
        sha256_file(&report.path).unwrap(),
        content.sha256(),
        "content differs from the server's file"
    );
}

fn throttled(events: &Events, net: u32) -> bool {
    events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, LaneEvent::Throttled { net: n, .. } if *n == net))
}

fn restored(events: &Events, net: u32) -> bool {
    events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, LaneEvent::Restored { net: n, .. } if *n == net))
}

async fn wait_for(what: &str, limit: Duration, done: impl Fn() -> bool) {
    let started = Instant::now();
    while !done() {
        assert!(started.elapsed() < limit, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn run(server: &RangeServer, networks: Vec<Network>, dir: &Path, t: Tuning) -> Report {
    tokio::time::timeout(
        Duration::from_secs(90),
        download(source(server), networks, dir, t),
    )
    .await
    .expect("download hung")
    .unwrap()
}

#[tokio::test]
async fn a_throttled_network_hands_its_work_to_the_others() {
    let content = Content::new(12 * MB, 81);
    let server = RangeServer::start(content).await.unwrap();
    let (a, b) = (server.lane().await.unwrap(), server.lane().await.unwrap());
    server.cap_lane(a, Some(2 * MB), 0);
    // The phone: full speed for its first 512 KB, then the quota runs out.
    server.cap_lane(b, Some(16 * KB), 512 * KB);
    let events: Events = Arc::default();
    let dir = tempfile::tempdir().unwrap();
    let started = Instant::now();
    let report = run(
        &server,
        vec![lane_net(1, a), lane_net(2, b)],
        dir.path(),
        tuning(&events, MB),
    )
    .await;
    assert_exact(&report, content);
    assert!(
        throttled(&events, 2),
        "the phone wasn't noticed: {events:?}"
    );
    assert!(!throttled(&events, 1), "Wi-Fi was benched: {events:?}");
    assert!(
        !restored(&events, 2),
        "came back while still slow: {events:?}"
    );
    let ev = events.lock().unwrap().clone();
    let Some(LaneEvent::Throttled { rate, name, .. }) = ev.first() else {
        panic!("{ev:?}");
    };
    assert_eq!(name, "net2");
    assert!(*rate < 32.0 * KB as f64, "reported {rate} B/s");
    // It stopped taking work: after its fast start it carried only its crawl and
    // a few one-stream checks, and the other network finished the file in good time.
    let phone = server.lane_sent(b);
    assert!(
        phone < 2 * MB,
        "the throttled lane still carried {phone} bytes"
    );
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "took {:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn a_throttled_network_comes_back_when_its_speed_returns() {
    let content = Content::new(24 * MB, 82);
    let server = RangeServer::start(content).await.unwrap();
    let (a, b) = (server.lane().await.unwrap(), server.lane().await.unwrap());
    server.cap_lane(a, Some(2 * MB), 0);
    server.cap_lane(b, Some(16 * KB), 512 * KB);
    let events: Events = Arc::default();
    let dir = tempfile::tempdir().unwrap();
    let t = tuning(&events, MB);
    let networks = vec![lane_net(1, a), lane_net(2, b)];
    let task = {
        let src = source(&server);
        let path = dir.path().to_path_buf();
        tokio::spawn(async move { download(src, networks, &path, t).await })
    };
    wait_for("the phone to be benched", Duration::from_secs(20), || {
        throttled(&events, 2)
    })
    .await;
    // The day's quota resets: the lane is fast again.
    server.cap_lane(b, Some(2 * MB), 0);
    let at_lift = server.lane_sent(b);
    wait_for("the phone to come back", Duration::from_secs(20), || {
        restored(&events, 2)
    })
    .await;
    let report = tokio::time::timeout(Duration::from_secs(90), task)
        .await
        .expect("download hung")
        .unwrap()
        .unwrap();
    assert_exact(&report, content);
    let carried = server.lane_sent(b) - at_lift;
    assert!(
        carried > 2 * MB,
        "back at work, the phone carried only {carried} bytes"
    );
    let ev = events.lock().unwrap().clone();
    let Some(LaneEvent::Restored { rate, .. }) =
        ev.iter().find(|e| matches!(e, LaneEvent::Restored { .. }))
    else {
        panic!("{ev:?}");
    };
    assert!(*rate > 256.0 * KB as f64, "check measured {rate} B/s");
}

#[tokio::test]
async fn when_every_network_is_slow_nothing_is_benched() {
    // Both lanes crawl: that's the server (or the whole connection), not one network.
    let content = Content::new(192 * KB, 83);
    let server = RangeServer::start(content).await.unwrap();
    let (a, b) = (server.lane().await.unwrap(), server.lane().await.unwrap());
    server.cap_lane(a, Some(24 * KB), 0);
    server.cap_lane(b, Some(20 * KB), 0);
    let events: Events = Arc::default();
    let dir = tempfile::tempdir().unwrap();
    let report = run(
        &server,
        vec![lane_net(1, a), lane_net(2, b)],
        dir.path(),
        tuning(&events, 128 * KB),
    )
    .await;
    assert_exact(&report, content);
    assert!(events.lock().unwrap().is_empty(), "{events:?}");
}

#[tokio::test]
async fn a_small_download_is_left_alone() {
    // Under the size where moving work pays: the end-of-download races cover it.
    let content = Content::new(512 * KB, 84);
    let server = RangeServer::start(content).await.unwrap();
    let (a, b) = (server.lane().await.unwrap(), server.lane().await.unwrap());
    server.cap_lane(a, Some(256 * KB), 0);
    server.cap_lane(b, Some(16 * KB), 0);
    let events: Events = Arc::default();
    let dir = tempfile::tempdir().unwrap();
    let report = run(
        &server,
        vec![lane_net(1, a), lane_net(2, b)],
        dir.path(),
        tuning(&events, MB),
    )
    .await;
    assert_exact(&report, content);
    assert!(events.lock().unwrap().is_empty(), "{events:?}");
}
