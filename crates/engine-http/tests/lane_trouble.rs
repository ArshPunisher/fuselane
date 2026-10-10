//! A connector that explains why it can't connect (a proxy turning down the
//! login, STEPS 8.4): the engine passes the reason on and, when retrying can't
//! help, stops using that network instead of asking forever.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fuselane_engine_http::download::{
    BoxIo, Connect, JobError, LaneEvent, LaneEventFn, LaneTrouble, Network, Source, Tuning,
    download,
};
use fuselane_testkit::{Content, RangeServer, sha256_file};

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

/// A network whose every connection fails with `message`; counts the attempts.
fn troubled(id: u32, message: &'static str, lasting: bool, tries: Arc<AtomicU32>) -> Network {
    let connect: Connect = Arc::new(move |_| {
        tries.fetch_add(1, Ordering::Relaxed);
        Box::pin(async move {
            Err(LaneTrouble {
                message: message.into(),
                lasting,
            }
            .into_io())
        })
    });
    Network {
        id,
        name: format!("net{id}"),
        connect,
        mirror: None,
    }
}

fn tuning(events: &Arc<Mutex<Vec<LaneEvent>>>) -> Tuning {
    let sink = events.clone();
    Tuning {
        block_size: Some(64 * KB),
        streams_per_network: 3,
        connect_timeout: Duration::from_secs(2),
        retry_delay_scale: 0.05,
        lane_events: Some(LaneEventFn(Arc::new(move |e| {
            sink.lock().unwrap().push(e.clone());
        }))),
        ..Tuning::default()
    }
}

const LOGIN: &str = "net1's proxy turned down the username and password. Check them in Networks.";

#[tokio::test]
async fn a_lasting_reason_is_told_once_and_the_network_stops_asking() {
    let content = Content::new(900 * KB, 91);
    let server = RangeServer::start(content).await.unwrap();
    let events: Arc<Mutex<Vec<LaneEvent>>> = Arc::default();
    let tries = Arc::new(AtomicU32::new(0));
    let dir = tempfile::tempdir().unwrap();
    let report = download(
        Source {
            addr: server.addr(),
            host: "localhost".into(),
            path: server.path().into(),
        },
        vec![troubled(1, LOGIN, true, tries.clone()), plain(2)],
        dir.path(),
        tuning(&events),
    )
    .await
    .unwrap();
    assert_eq!(sha256_file(&report.path).unwrap(), content.sha256());
    let told: Vec<_> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            LaneEvent::Trouble {
                net,
                message,
                lasting,
                ..
            } => Some((*net, message.clone(), *lasting)),
            _ => None,
        })
        .collect();
    assert_eq!(told, vec![(1, LOGIN.to_string(), true)], "told once");
    // The probe tried it, then at most each of its streams once: no endless retries.
    assert!(tries.load(Ordering::Relaxed) <= 9, "{tries:?}");
}

#[tokio::test]
async fn when_no_network_can_connect_the_reason_is_the_error() {
    let server = RangeServer::start(Content::new(64 * KB, 92)).await.unwrap();
    let events: Arc<Mutex<Vec<LaneEvent>>> = Arc::default();
    let dir = tempfile::tempdir().unwrap();
    let err = download(
        Source {
            addr: server.addr(),
            host: "localhost".into(),
            path: server.path().into(),
        },
        vec![troubled(1, LOGIN, true, Arc::default())],
        dir.path(),
        tuning(&events),
    )
    .await
    .unwrap_err();
    assert!(matches!(&err, JobError::Lane(m) if m == LOGIN), "{err:?}");
    assert!(server.requests().is_empty(), "nothing reached the server");
}
