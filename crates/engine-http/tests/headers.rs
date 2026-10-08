//! A browser session's headers (cookies, referrer, User-Agent) reach the server on
//! every request, and nothing that could split or reshape a request gets through.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::sync::Arc;
use std::time::Duration;

use fuselane_engine_http::download::{BoxIo, Connect, Headers, Network, Source, Tuning, download};
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
    }
}

fn source(s: &RangeServer) -> Source {
    Source {
        addr: s.addr(),
        host: "localhost".into(),
        path: s.path().into(),
    }
}

fn h(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

#[tokio::test]
async fn a_sessions_cookies_go_with_every_request() {
    let content = Content::new(600 * KB, 60);
    let server = RangeServer::start(content).await.unwrap();
    let headers = Headers::checked(&h(&[
        ("Cookie", "session=abc123; theme=dark"),
        ("Referer", "https://example.org/downloads"),
        ("User-Agent", "Mozilla/5.0 (Browser)"),
    ]))
    .unwrap();
    let d = tempfile::tempdir().unwrap();
    let t = Tuning {
        headers,
        block_size: Some(64 * KB),
        streams_per_network: 3,
        retry_delay_scale: 0.05,
        idle_timeout: Duration::from_millis(500),
        ..Tuning::default()
    };
    let report = download(source(&server), vec![plain(1), plain(2)], d.path(), t)
        .await
        .unwrap();
    assert_eq!(sha256_file(&report.path).unwrap(), content.sha256());
    let log = server.requests();
    assert!(log.len() > 2, "several streams");
    for r in &log {
        assert_eq!(r.cookie.as_deref(), Some("session=abc123; theme=dark"));
        assert_eq!(r.referer.as_deref(), Some("https://example.org/downloads"));
        assert_eq!(
            r.user_agent.as_deref(),
            Some("Mozilla/5.0 (Browser)"),
            "the browser's, not ours"
        );
    }
}

#[tokio::test]
async fn without_headers_fuselane_sends_its_own_user_agent_and_no_cookie() {
    let content = Content::new(100 * KB, 61);
    let server = RangeServer::start(content).await.unwrap();
    let d = tempfile::tempdir().unwrap();
    let t = Tuning {
        retry_delay_scale: 0.05,
        ..Tuning::default()
    };
    download(source(&server), vec![plain(1)], d.path(), t)
        .await
        .unwrap();
    for r in server.requests() {
        assert!(r.cookie.is_none());
        assert!(r.user_agent.unwrap_or_default().starts_with("Fuselane/"));
    }
}

#[test]
fn only_harmless_headers_with_clean_values_are_accepted() {
    for (name, value) in [
        ("Host", "evil.example"),
        ("Range", "bytes=0-"),
        ("Content-Length", "0"),
        ("Transfer-Encoding", "chunked"),
        ("Connection", "close"),
        ("Accept-Encoding", "gzip"),
        ("Proxy-Authorization", "x"),
        ("X-Forwarded-For", "1.2.3.4"),
    ] {
        let e = Headers::checked(&h(&[(name, value)])).unwrap_err();
        assert!(e.contains(name), "{name}: {e}");
    }
    for value in ["a\r\nX-Evil: 1", "a\nb", "a\0b"] {
        assert!(
            Headers::checked(&h(&[("Cookie", value)])).is_err(),
            "{value:?}"
        );
    }
    assert!(Headers::checked(&h(&[("Cookie", &"x".repeat(17 * 1024))])).is_err());
    let ok = Headers::checked(&h(&[
        ("cookie", "a=1"),
        ("COOKIE", "a=2"),
        ("Accept-Language", "en-GB"),
    ]))
    .unwrap();
    // A repeated header keeps the last value; names are never shown with values.
    let shown = format!("{ok:?}");
    assert_eq!(shown, r#"["cookie", "accept-language"]"#);
    assert!(!shown.contains("a=2"));
    assert!(Headers::checked(&[]).unwrap().is_empty());
}
