//! Proxies set per network (STEPS 8.4): the table every connection the app
//! makes for a link consults, and the plain words for what can go wrong.
//!
//! The table lives here, next to the connection planner, rather than being
//! threaded through every call: downloads, previews, checksum lookups, page and
//! feed reads and the app's own update all reach servers through
//! `runner::connect_plan`, and all of them must use the same proxy for the same
//! network. System proxy settings are never read (L-66); a server on this
//! computer is never reached through a proxy, as browsers do.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, PoisonError, RwLock};

pub use fuselane_transport::proxy::{Login, Proxy, ProxyError, ProxyKind};

/// A network's proxy, and what to call the network in messages ("Wi-Fi").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetProxy {
    pub proxy: Proxy,
    pub label: String,
}

type Table = HashMap<String, Arc<NetProxy>>;

fn table() -> &'static RwLock<Table> {
    static TABLE: OnceLock<RwLock<Table>> = OnceLock::new();
    TABLE.get_or_init(RwLock::default)
}

/// Replaces every network's proxy (keyed by device name). The app calls this
/// at start and whenever the person changes one; connections made afterwards
/// use the new table, ones already open carry on.
pub fn set(proxies: HashMap<String, NetProxy>) {
    *table().write().unwrap_or_else(PoisonError::into_inner) = proxies
        .into_iter()
        .map(|(name, p)| (name, Arc::new(p)))
        .collect();
}

/// The proxies as they are now.
pub fn snapshot() -> HashMap<String, Arc<NetProxy>> {
    table()
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// What went wrong going through `network`'s proxy, and what to do about it.
pub fn problem(e: &ProxyError, network: &str, proxy: &Proxy) -> (String, String) {
    let at = proxy.address();
    let (what, todo) = match e {
        ProxyError::Lookup => (
            format!("{network}'s proxy, {}, couldn't be found.", proxy.host),
            "Check its name in Networks, under Proxy.".to_string(),
        ),
        ProxyError::Unreachable(why) => (
            format!("{network} couldn't connect to its proxy at {at} ({why})."),
            "Check the address and port, and that the proxy is running.".to_string(),
        ),
        ProxyError::LoginNeeded => (
            format!("{network}'s proxy at {at} wants a username and password."),
            "Add them in Networks, under Proxy.".to_string(),
        ),
        ProxyError::LoginRefused => (
            format!("{network}'s proxy at {at} turned down the username and password."),
            "Check them in Networks, under Proxy.".to_string(),
        ),
        ProxyError::Refused { target } => (
            format!("{network}'s proxy refused to connect to {target}."),
            "Its rules may allow only some sites or ports (many allow only https links). Ask whoever runs it, or remove the proxy.".to_string(),
        ),
        ProxyError::TargetUnreachable { target } => (
            format!("{network}'s proxy couldn't reach {target}."),
            "The site may be down, or the proxy can't get out right now.".to_string(),
        ),
        ProxyError::NotAProxy { kind } => (
            format!("{at} doesn't answer like a {kind} proxy."),
            "Check the type (HTTP or SOCKS5) and the port in Networks, under Proxy.".to_string(),
        ),
        ProxyError::Unsupported => (
            format!("{network}'s proxy wants a sign-in method Fuselane doesn't support."),
            "Fuselane signs in with a username and password. Use another proxy, or remove this one.".to_string(),
        ),
        ProxyError::Timeout => (
            format!("{network}'s proxy at {at} didn't answer in time."),
            "Check the address, port and type, and that the proxy is running.".to_string(),
        ),
    };
    (what, todo)
}

/// The proxy `iface` uses to reach `host`, if any: none for a server on this
/// computer (a proxy elsewhere can't reach it).
pub(crate) fn for_host(table: &Table, iface: &str, host: &str) -> Option<Arc<NetProxy>> {
    if crate::runner::is_local_host(host) {
        return None;
    }
    table.get(iface).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(login: bool) -> Proxy {
        Proxy {
            kind: ProxyKind::Socks5,
            host: "proxy.example.net".into(),
            port: 1080,
            login: login.then(|| Login {
                user: "ann".into(),
                password: "hunter2".into(),
            }),
        }
    }

    #[test]
    fn every_problem_says_what_happened_and_what_to_do_without_the_login() {
        let all = [
            ProxyError::Lookup,
            ProxyError::Unreachable("connection refused".into()),
            ProxyError::LoginNeeded,
            ProxyError::LoginRefused,
            ProxyError::Refused {
                target: "example.com:80".into(),
            },
            ProxyError::TargetUnreachable {
                target: "example.com:443".into(),
            },
            ProxyError::NotAProxy { kind: "SOCKS5" },
            ProxyError::Unsupported,
            ProxyError::Timeout,
        ];
        for e in &all {
            let (what, todo) = problem(e, "Wi-Fi", &proxy(true));
            assert!(
                what.ends_with('.') && todo.ends_with('.'),
                "{e:?}: {what} {todo}"
            );
            assert!(!todo.is_empty());
            let text = format!("{what} {todo}");
            assert!(
                !text.contains("hunter2") && !text.contains("ann"),
                "{e:?} leaked the login: {text}"
            );
        }
        let (what, todo) = problem(&ProxyError::LoginRefused, "iPhone USB", &proxy(true));
        assert_eq!(
            what,
            "iPhone USB's proxy at proxy.example.net:1080 turned down the username and password."
        );
        assert!(todo.contains("Networks"));
    }

    use crate::runner::{describe, network_for};
    use fuselane_engine_http::download::{JobError, Source, Tuning, download_with};
    use fuselane_testkit::{Content, ProxyKind as Kind, RangeServer, TestProxy, sha256_file};
    use std::time::Duration;

    fn test_net() -> fuselane_netif::Interface {
        fuselane_netif::Interface {
            name: "lo0".into(),
            display_name: "Test network".into(),
            index: 0,
            kind: fuselane_netif::Kind::Ethernet,
            addrs: vec![std::net::IpAddr::from([127, 0, 0, 1])],
        }
    }

    /// Downloads the test server's file over one network that uses `p`.
    async fn fetch_through(
        server: &RangeServer,
        p: Proxy,
        dir: &std::path::Path,
    ) -> Result<fuselane_engine_http::download::Report, JobError> {
        let host = server.addr().ip().to_string();
        let net = network_for(
            1,
            test_net(),
            Arc::new(vec![server.addr()]),
            false,
            Arc::from(host.as_str()),
            server.addr().port(),
            Duration::from_secs(5),
            Some(Arc::new(NetProxy {
                proxy: p,
                label: "Wi-Fi".into(),
            })),
        );
        let src = Source {
            addr: server.addr(),
            host: format!("{host}:{}", server.addr().port()),
            path: server.path().into(),
        };
        let t = Tuning {
            block_size: Some(256 * 1024),
            streams_per_network: 4,
            retry_delay_scale: 0.05,
            ..Tuning::default()
        };
        download_with(src, vec![net], dir, t, None).await
    }

    fn via(kind: ProxyKind, tp: &TestProxy, login: Option<(&str, &str)>) -> Proxy {
        Proxy {
            kind,
            host: tp.addr().ip().to_string(),
            port: tp.addr().port(),
            login: login.map(|(u, p)| Login {
                user: u.into(),
                password: p.into(),
            }),
        }
    }

    #[tokio::test]
    async fn a_download_really_goes_through_each_kind_of_proxy() {
        for (kind, k) in [
            (ProxyKind::Http, Kind::Http),
            (ProxyKind::Socks5, Kind::Socks5),
        ] {
            let content = Content::new(3 * 1024 * 1024 + 7, 61);
            let server = RangeServer::start(content).await.unwrap();
            let tp = TestProxy::start(k, Some(("ann", "pa55 word")))
                .await
                .unwrap();
            let dir = tempfile::tempdir().unwrap();
            let report = fetch_through(
                &server,
                via(kind, &tp, Some(("ann", "pa55 word"))),
                dir.path(),
            )
            .await
            .unwrap();
            assert_eq!(
                sha256_file(&report.path).unwrap(),
                content.sha256(),
                "{kind:?}"
            );
            // Every byte came back through the proxy, and every tunnel led to the server.
            assert!(
                tp.relayed() >= content.size,
                "{kind:?}: relayed {}",
                tp.relayed()
            );
            let want = format!("{}:{}", server.addr().ip(), server.addr().port());
            let targets = tp.targets();
            assert!(targets.len() >= 2, "{kind:?}: {targets:?}");
            assert!(targets.iter().all(|t| *t == want), "{kind:?}: {targets:?}");
            assert_eq!(tp.bad_logins(), 0);
        }
    }

    #[tokio::test]
    async fn a_wrong_or_missing_proxy_login_fails_in_plain_words() {
        for (kind, k) in [
            (ProxyKind::Http, Kind::Http),
            (ProxyKind::Socks5, Kind::Socks5),
        ] {
            let server = RangeServer::start(Content::new(64 * 1024, 62))
                .await
                .unwrap();
            let tp = TestProxy::start(k, Some(("ann", "right"))).await.unwrap();
            let dir = tempfile::tempdir().unwrap();
            let err = fetch_through(&server, via(kind, &tp, Some(("ann", "wrong"))), dir.path())
                .await
                .unwrap_err();
            let msg = describe(&err);
            assert!(
                msg.starts_with("Wi-Fi's proxy at ")
                    && msg.contains("turned down the username and password"),
                "{kind:?}: {msg}"
            );
            assert!(
                !msg.contains("wrong") && !msg.contains("ann"),
                "login leaked: {msg}"
            );
            let err = fetch_through(&server, via(kind, &tp, None), dir.path())
                .await
                .unwrap_err();
            assert!(
                describe(&err).contains("wants a username and password"),
                "{kind:?}"
            );
            assert!(server.requests().is_empty(), "nothing reached the server");
        }
    }

    #[tokio::test]
    async fn a_proxy_that_refuses_the_place_says_so() {
        let server = RangeServer::start(Content::new(64 * 1024, 63))
            .await
            .unwrap();
        let tp = TestProxy::start(Kind::Http, None).await.unwrap();
        tp.refuse_port(server.addr().port());
        let dir = tempfile::tempdir().unwrap();
        let err = fetch_through(&server, via(ProxyKind::Http, &tp, None), dir.path())
            .await
            .unwrap_err();
        let msg = describe(&err);
        assert!(msg.contains("refused to connect to 127.0.0.1:"), "{msg}");
        assert!(msg.contains("https links"), "{msg}");
    }

    #[test]
    fn a_server_on_this_computer_is_never_reached_through_a_proxy() {
        let mut t = Table::new();
        t.insert(
            "en0".into(),
            Arc::new(NetProxy {
                proxy: proxy(false),
                label: "Wi-Fi".into(),
            }),
        );
        assert!(for_host(&t, "en0", "example.com").is_some());
        assert!(for_host(&t, "en1", "example.com").is_none());
        for local in ["localhost", "127.0.0.1", "::1", "[::1]"] {
            assert!(for_host(&t, "en0", local).is_none(), "{local}");
        }
    }
}
