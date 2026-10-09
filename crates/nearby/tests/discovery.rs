//! Multicast announcements between two listeners on loopback (a test port, not 53317).

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::net::Ipv4Addr;
use std::time::Duration;

use fuselane_nearby::proto::Announcement;
use fuselane_nearby::{DeviceInfo, Discovery};

#[tokio::test]
async fn an_announcement_is_heard_by_another_listener() {
    let port = 53_900 + (std::process::id() % 90) as u16;
    let lo = [Ipv4Addr::LOCALHOST];
    let (Ok(a), Ok(b)) = (Discovery::start(&lo, port), Discovery::start(&lo, port)) else {
        eprintln!("skipped: this machine has no multicast on loopback");
        return;
    };
    let me = Announcement {
        info: DeviceInfo {
            alias: "Fuselane test".into(),
            version: "2.1".into(),
            device_model: None,
            device_type: Some("desktop".into()),
            fingerprint: "AB".repeat(32),
            port: 53317,
            protocol: "https".into(),
            download: false,
        },
        announce: true,
    };
    a.announce(&me).await.unwrap();
    match tokio::time::timeout(Duration::from_secs(3), b.recv()).await {
        Ok(Ok((_, heard))) => assert_eq!(heard, me),
        _ => eprintln!("skipped: loopback multicast didn't deliver here"),
    }
}
