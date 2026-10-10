//! L-66: proxies set for the whole system (HTTP_PROXY, HTTPS_PROXY, ALL_PROXY)
//! are never used. They would send every network through one route and defeat
//! pinning; only a proxy set for a network in Fuselane is used (STEPS 8.4).
//! Its own test binary, because it changes the process's environment.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::sync::Arc;
use std::time::Duration;

use fuselane_core::runner::network_for;
use fuselane_engine_http::download::{Source, Tuning, download_with};
use fuselane_testkit::{Content, ProxyKind, RangeServer, TestProxy, sha256_file};

#[tokio::test]
async fn system_proxy_settings_are_ignored() {
    let content = Content::new(512 * 1024, 71);
    let server = RangeServer::start(content).await.unwrap();
    let system = TestProxy::start(ProxyKind::Http, None).await.unwrap();
    let socks = TestProxy::start(ProxyKind::Socks5, None).await.unwrap();
    let http = format!("http://{}", system.addr());
    for (k, v) in [
        ("HTTP_PROXY", http.clone()),
        ("HTTPS_PROXY", http.clone()),
        ("http_proxy", http.clone()),
        ("https_proxy", http),
        ("ALL_PROXY", format!("socks5://{}", socks.addr())),
        ("all_proxy", format!("socks5://{}", socks.addr())),
        ("NO_PROXY", String::new()),
    ] {
        // SAFETY: this binary runs only this test, so no other thread reads the
        // environment while it changes.
        unsafe { std::env::set_var(k, v) };
    }
    let iface = fuselane_netif::Interface {
        name: "lo0".into(),
        display_name: "Test network".into(),
        index: 0,
        kind: fuselane_netif::Kind::Ethernet,
        addrs: vec![std::net::IpAddr::from([127, 0, 0, 1])],
    };
    let host = server.addr().ip().to_string();
    let net = network_for(
        1,
        iface,
        Arc::new(vec![server.addr()]),
        false,
        Arc::from(host.as_str()),
        server.addr().port(),
        Duration::from_secs(5),
        None,
    );
    let dir = tempfile::tempdir().unwrap();
    let report = download_with(
        Source {
            addr: server.addr(),
            host: format!("{host}:{}", server.addr().port()),
            path: server.path().into(),
        },
        vec![net],
        dir.path(),
        Tuning::default(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(sha256_file(&report.path).unwrap(), content.sha256());
    assert!(system.targets().is_empty(), "went through HTTP_PROXY");
    assert!(socks.targets().is_empty(), "went through ALL_PROXY");
}
