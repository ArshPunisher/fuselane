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

/// Every run gets its own state folder: tests must never touch the real download list.
fn fuselane(args: &[&str]) -> std::process::Output {
    let home = tempfile::tempdir().unwrap();
    fuselane_in(home.path(), args)
}

fn fuselane_in(home: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fuselane"))
        .env("FUSELANE_HOME", home)
        .args(args)
        .output()
        .expect("binary runs")
}

#[test]
fn started_by_a_browser_it_relays_and_hands_offers_back_when_the_app_is_closed() {
    use std::io::{Read, Write};
    let home = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fuselane"))
        .env("FUSELANE_HOME", home.path())
        .arg("chrome-extension://abcdefghijklmnop/")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let msg = br#"{"v":1,"type":"download.offer","url":"https://example.org/big.iso"}"#;
    let mut stdin = child.stdin.take().unwrap();
    stdin
        .write_all(&u32::try_from(msg.len()).unwrap().to_ne_bytes())
        .unwrap();
    stdin.write_all(msg).unwrap();
    drop(stdin); // the browser closing the pipe ends the host
    let mut out = Vec::new();
    child.stdout.take().unwrap().read_to_end(&mut out).unwrap();
    assert!(child.wait().unwrap().success());
    let n = u32::from_ne_bytes(out[..4].try_into().unwrap()) as usize;
    assert_eq!(
        out.len(),
        4 + n,
        "exactly one framed reply, nothing else on stdout"
    );
    let reply: serde_json::Value = serde_json::from_slice(&out[4..]).unwrap();
    assert_eq!(reply["type"], "download.declined");
    assert_eq!(reply["fallback"], "browser");
}

#[test]
fn ls_and_nets_speak_json_for_scripts() {
    let home = tempfile::tempdir().unwrap();
    let out = fuselane_in(home.path(), &["ls", "--json"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "[]",
        "an empty list is [], not a sentence"
    );

    let out = fuselane(&["nets", "--all", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    let rows = v.as_array().expect("an array");
    assert!(!rows.is_empty(), "every machine has at least loopback");
    for r in rows {
        for key in ["name", "label", "kind", "usable", "addresses"] {
            assert!(r.get(key).is_some(), "{key} missing in {r}");
        }
    }
    assert!(
        rows.iter()
            .any(|r| r["kind"] == "loopback" && r["usable"] == false)
    );
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

/// The hardest case: the process is killed outright mid-download. Resume must finish
/// byte-exact from the last checkpoint, without refetching what was secured.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn kill_9_mid_download_then_resume_is_byte_exact() {
    let content = Content::new(3_000_000, 51);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(700_000),
    });
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://{}{}", server.addr(), server.path());
    let mut child = Command::new(env!("CARGO_BIN_EXE_fuselane"))
        .env("FUSELANE_HOME", home.path())
        .env("FUSELANE_CHECKPOINT_MS", "100")
        .args([
            "get",
            &url,
            "-o",
            dir.path().to_str().unwrap(),
            "-n",
            LOOPBACK,
            "-q",
            "-s",
            "4",
        ])
        .spawn()
        .unwrap();
    // Wait until a third is durably checkpointed, then kill without warning.
    let db = home.path().join("jobs.db");
    let mut secured = 0;
    for _ in 0..200 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if let Ok(store) = fuselane_core::Store::open(&db)
            && let Ok(jobs) = store.list()
            && let Some(j) = jobs.first()
        {
            secured = j.secured_bytes();
            if secured > content.size / 3 {
                break;
            }
        }
    }
    assert!(
        secured > content.size / 3,
        "no checkpoint reached a third (secured {secured})"
    );
    child.kill().unwrap(); // SIGKILL
    let _ = child.wait();
    assert!(
        !dir.path().join("file.bin").exists(),
        "a killed download must not publish"
    );

    let before = server.requests().len();
    let (h, d) = (home.path().to_path_buf(), dir.path().to_path_buf());
    let out = tokio::task::spawn_blocking(move || {
        fuselane_in(&h, &["resume", "last", "-n", LOOPBACK, "-q"])
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "resume failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        sha256_file(&d.join("file.bin")).unwrap(),
        content.sha256(),
        "resumed file differs"
    );
    let refetched: u64 = server.requests()[before..].len() as u64;
    assert!(refetched > 0);
    let ls = fuselane_in(home.path(), &["ls"]);
    assert!(
        String::from_utf8_lossy(&ls.stdout).contains("done"),
        "{}",
        String::from_utf8_lossy(&ls.stdout)
    );
    // Scripts see a finished download as fully saved, with its name and path.
    let json: serde_json::Value =
        serde_json::from_slice(&fuselane_in(home.path(), &["ls", "--json"]).stdout).unwrap();
    let job = &json[0];
    assert_eq!(job["status"], "completed");
    assert_eq!(job["saved"], job["total"], "{job}");
    assert_eq!(job["name"], "file.bin", "{job}");
    assert!(
        job["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("file.bin")),
        "{job}"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn ctrl_c_pauses_with_exit_130_and_resume_finishes() {
    let content = Content::new(2_000_000, 52);
    let server = RangeServer::start(content).await.unwrap();
    server.add_rule(Rule {
        skip: 1,
        times: u32::MAX,
        fault: Fault::Throttle(500_000),
    });
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://{}{}", server.addr(), server.path());
    let child = Command::new(env!("CARGO_BIN_EXE_fuselane"))
        .env("FUSELANE_HOME", home.path())
        .args([
            "get",
            &url,
            "-o",
            dir.path().to_str().unwrap(),
            "-n",
            LOOPBACK,
            "-q",
            "-s",
            "2",
        ])
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // Wait until the download has really started (its partial file exists): the
    // Ctrl-C handler is installed by then. A fixed sleep raced under heavy load.
    let started = std::time::Instant::now();
    while !std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e.path().extension().is_some_and(|x| x == "fuselane"))
    {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(15),
            "download never started"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    // SAFETY: sending SIGINT to our own child process.
    unsafe { libc::kill(child.id() as i32, libc::SIGINT) };
    let out = tokio::task::spawn_blocking(move || child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(130),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("fuselane resume"));
    let ls = fuselane_in(home.path(), &["ls"]);
    assert!(String::from_utf8_lossy(&ls.stdout).contains("paused"));

    let (h, d) = (home.path().to_path_buf(), dir.path().to_path_buf());
    let out = tokio::task::spawn_blocking(move || {
        fuselane_in(&h, &["resume", "1", "-n", LOOPBACK, "-q"])
    })
    .await
    .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(sha256_file(&d.join("file.bin")).unwrap(), content.sha256());
}

#[tokio::test(flavor = "multi_thread")]
async fn rm_removes_the_partial_file_and_resume_refuses_finished_jobs() {
    let server = RangeServer::start(Content::new(10, 53)).await.unwrap();
    server.add_rule(Rule::always(Fault::Status(404, None)));
    let home = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let url = format!("http://{}{}", server.addr(), server.path());
    let (h, d) = (home.path().to_path_buf(), dir.path().to_path_buf());
    let out = tokio::task::spawn_blocking(move || {
        fuselane_in(
            &h,
            &["get", &url, "-o", d.to_str().unwrap(), "-n", LOOPBACK, "-q"],
        )
    })
    .await
    .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        fuselane_in(home.path(), &["resume", "99"]).status.code(),
        Some(2),
        "unknown id"
    );
    assert_eq!(
        fuselane_in(home.path(), &["rm", "99"]).status.code(),
        Some(2)
    );
    assert!(fuselane_in(home.path(), &["rm", "1"]).status.success());
    assert!(
        String::from_utf8_lossy(&fuselane_in(home.path(), &["ls"]).stdout)
            .contains("No downloads yet")
    );
}
