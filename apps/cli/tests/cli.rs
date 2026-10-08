//! Runs the real `fuselane` binary against the hostile test server.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test-only file (CLAUDE.md)

use std::process::Command;

use fuselane_testkit::{Content, Fault, RangeServer, Rule, sha256_file};

const LOOPBACK: &str = if cfg!(target_os = "linux") {
    "lo"
} else if cfg!(windows) {
    "Loopback Pseudo-Interface 1"
} else {
    "lo0"
};

fn fuselane(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fuselane"))
        .args(args)
        .output()
        .expect("binary runs")
}

#[tokio::test(flavor = "multi_thread")]
async fn get_downloads_a_byte_exact_file() {
    let content = Content::new(1_500_000, 21);
    let server = RangeServer::start(content).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://{}{}", server.addr(), server.path());
    let out = tokio::task::spawn_blocking({
        let d = dir.path().to_path_buf();
        move || fuselane(&["get", &url, "-o", d.to_str().unwrap(), "-n", LOOPBACK, "-q"])
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let file = dir.path().join("file.bin");
    assert_eq!(sha256_file(&file).unwrap(), content.sha256());
    assert!(String::from_utf8_lossy(&out.stdout).contains("Saved"));
}

#[tokio::test(flavor = "multi_thread")]
async fn get_survives_a_lying_server() {
    let content = Content::new(800_000, 22);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: 3,
        fault: Fault::Overrun(999),
    });
    server.add_rule(Rule {
        skip: 0,
        times: 2,
        fault: Fault::ShortBody(100),
    });
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://{}{}", server.addr(), server.path());
    let d = dir.path().to_path_buf();
    let out = tokio::task::spawn_blocking(move || {
        fuselane(&["get", &url, "-o", d.to_str().unwrap(), "-n", LOOPBACK, "-q"])
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        sha256_file(&dir.path().join("file.bin")).unwrap(),
        content.sha256()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_missing_file_is_a_plain_error_with_exit_code_1() {
    let server = RangeServer::start(Content::new(10, 23)).await.unwrap();
    server.add_rule(Rule::always(Fault::Status(404, None)));
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://{}{}", server.addr(), server.path());
    let d = dir.path().to_path_buf();
    let out = tokio::task::spawn_blocking(move || {
        fuselane(&["get", &url, "-o", d.to_str().unwrap(), "-n", LOOPBACK, "-q"])
    })
    .await
    .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("doesn't exist (404)"));
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        0,
        "nothing left behind"
    );
}

#[test]
fn bad_input_exits_2_with_a_reason() {
    for (args, needle) in [
        (
            vec!["get", "ftp://example.com/f"],
            "Use an http:// or https:// link",
        ),
        (vec!["get", "nonsense"], "isn't a valid link"),
        (
            vec!["get", "https://example.com/f", "-n", "no-such-net"],
            "No network called",
        ),
        (vec!["get", "https://example.com/f", "-s", "0"], "0"),
    ] {
        let out = fuselane(&args);
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(needle),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    assert_eq!(fuselane(&[]).status.code(), Some(2));
    assert!(fuselane(&["--version"]).status.success());
}
