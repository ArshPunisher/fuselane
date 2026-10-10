//! Every request the app makes for a link uses the network's proxy (STEPS 8.4):
//! with a proxy on every network, the planner looks nothing up itself and the
//! proxy is asked for the server by name. Its own test binary, because it sets
//! the process-wide proxy table.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::collections::HashMap;

use fuselane_core::proxy::{NetProxy, Proxy, ProxyKind};
use fuselane_testkit::{ProxyKind as Kind, TestProxy};

#[tokio::test]
async fn with_a_proxy_on_every_network_the_proxy_resolves_the_name() {
    let Ok(networks) = fuselane_core::runner::pick_networks(&[]) else {
        eprintln!("skipped: no usable network on this machine");
        return;
    };
    let tp = TestProxy::start(Kind::Socks5, None).await.unwrap();
    let table: HashMap<String, NetProxy> = networks
        .iter()
        .map(|n| {
            (
                n.name.clone(),
                NetProxy {
                    proxy: Proxy {
                        kind: ProxyKind::Socks5,
                        host: tp.addr().ip().to_string(),
                        port: tp.addr().port(),
                        login: None,
                    },
                    label: "Test network".into(),
                },
            )
        })
        .collect();
    fuselane_core::proxy::set(table);
    // A name no resolver here could find: only the proxy is asked about it.
    let err = fuselane_core::runner::preview("http://fuselane-proxy-test.invalid/file.bin")
        .await
        .unwrap_err();
    assert!(
        err.contains("couldn't reach fuselane-proxy-test.invalid:80"),
        "{err}"
    );
    assert!(
        tp.targets()
            .iter()
            .all(|t| t == "fuselane-proxy-test.invalid:80"),
        "{:?}",
        tp.targets()
    );
    assert!(!tp.targets().is_empty());
    // A server on this computer is still reached directly, as browsers do.
    let asked = tp.targets().len();
    let server = fuselane_testkit::RangeServer::start(fuselane_testkit::Content::new(4096, 1))
        .await
        .unwrap();
    let p = fuselane_core::runner::preview(&format!("http://{}/file.bin", server.addr()))
        .await
        .unwrap();
    assert_eq!(p.total, Some(4096));
    assert_eq!(
        tp.targets().len(),
        asked,
        "a local server went through the proxy"
    );
    fuselane_core::proxy::set(HashMap::new());
}
