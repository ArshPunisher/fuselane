//! Finding out about bugs without a crash-reporting service (ADR 0009):
//!
//! - A crash writes a short report to `<home>/crashes/` (version, OS, where it
//!   happened). The next launch offers to report it.
//! - Problems worth knowing about go to `<home>/logs/fuselane.log`, rotated at
//!   1 MB, never with links, file names or addresses.
//! - "Report a problem" opens a GitHub issue filled in with the diagnostics,
//!   which the person reads and submits themselves. Nothing is sent by itself.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const ISSUES: &str = "https://github.com/ArshPunisher/fuselane/issues/new";
/// A log bigger than this is moved to `fuselane.log.1` (one old file kept).
const LOG_MAX: u64 = 1024 * 1024;
/// Longest crash message kept (panic messages can be long).
const MESSAGE_MAX: usize = 600;
/// GitHub refuses very long new-issue links; the body is cut to fit.
const BODY_MAX: usize = 6000;

static HOME: OnceLock<PathBuf> = OnceLock::new();
static LOG: Mutex<()> = Mutex::new(());

/// Where crash reports and logs live; set once at startup.
pub fn init(home: PathBuf) {
    let _ = HOME.set(home);
    install_crash_hook();
}

fn home() -> Option<&'static Path> {
    HOME.get().map(PathBuf::as_path)
}

/// Appends one line to the log, rotating it at 1 MB. Callers pass only codes
/// and counts, never links, names or addresses.
pub fn log(what: &str) {
    let Some(dir) = home().map(|h| h.join("logs")) else {
        return;
    };
    let _held = LOG
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("fuselane.log");
    if std::fs::metadata(&file).is_ok_and(|m| m.len() > LOG_MAX) {
        let _ = std::fs::rename(&file, dir.join("fuselane.log.1"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
    {
        let _ = writeln!(f, "{} {}", stamp(), one_line(what));
    }
}

/// The last `n` log lines, oldest first, for the diagnostics.
pub fn recent_log(n: usize) -> Vec<String> {
    let Some(file) = home().map(|h| h.join("logs").join("fuselane.log")) else {
        return vec![];
    };
    let text = std::fs::read_to_string(file).unwrap_or_default();
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    lines[lines.len().saturating_sub(n)..].to_vec()
}

fn install_crash_hook() {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let at = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        write_crash(&crash_text(&message, &at));
        before(info);
    }));
}

/// What a crash report says: the version, the OS, where, and a short message
/// with anything that looks like a path, link or address taken out.
pub fn crash_text(message: &str, at: &str) -> String {
    format!(
        "Fuselane {} on {} {}\nWhere: {at}\nWhat: {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        scrub(message).chars().take(MESSAGE_MAX).collect::<String>()
    )
}

fn write_crash(text: &str) {
    let Some(dir) = home().map(|h| h.join("crashes")) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("crash-{}.txt", stamp_file())), text);
    log(&format!(
        "crash: {}",
        text.lines().nth(1).unwrap_or_default()
    ));
}

/// The newest crash report not yet offered, which is then marked as offered.
pub fn unseen_crash() -> Option<String> {
    let dir = home()?.join("crashes");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    files.sort();
    let newest = files.pop()?;
    let seen = dir.join("offered");
    let name = newest.file_name()?.to_string_lossy().into_owned();
    if std::fs::read_to_string(&seen).is_ok_and(|s| s.trim() == name) {
        return None;
    }
    let _ = std::fs::write(&seen, &name);
    // Old reports aren't needed once a newer one exists.
    for old in files.iter().rev().skip(4) {
        let _ = std::fs::remove_file(old);
    }
    std::fs::read_to_string(newest).ok()
}

/// A new-issue link with a title and the report filled in, for the person to
/// read and submit (or not) on GitHub.
pub fn issue_url(title: &str, diagnostics: &str, crash: Option<&str>, log: &[String]) -> String {
    let mut body = String::from(
        "**What happened?**\n\n\n**What did you expect?**\n\n\n**Steps to make it happen again**\n1. \n\n",
    );
    if let Some(c) = crash {
        body.push_str(&format!("**Crash report**\n```\n{}\n```\n\n", c.trim()));
    }
    body.push_str(&format!(
        "**Diagnostics** (no addresses, links or file names)\n```\n{}\n```\n",
        diagnostics.trim()
    ));
    if !log.is_empty() {
        body.push_str(&format!("\n**Recent log**\n```\n{}\n```\n", log.join("\n")));
    }
    if body.len() > BODY_MAX {
        let mut cut = BODY_MAX;
        while !body.is_char_boundary(cut) {
            cut -= 1;
        }
        body.truncate(cut);
        body.push_str("\n```\n(cut to fit; paste the rest from Settings → Copy diagnostics)");
    }
    let q: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("title", title)
        .append_pair("labels", "bug")
        .append_pair("body", &body)
        .finish();
    format!("{ISSUES}?{q}")
}

/// Takes out what could identify a person or a file: paths, links, addresses.
pub fn scrub(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let looks_private = w.contains("://")
                || w.contains('/')
                || w.contains('\\')
                || w.contains('@')
                || w.parse::<std::net::IpAddr>().is_ok()
                || w.split(':')
                    .next()
                    .is_some_and(|h| h.parse::<std::net::Ipv4Addr>().is_ok());
            if looks_private { "[removed]" } else { w }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn one_line(s: &str) -> String {
    scrub(&s.replace(['\n', '\r'], " "))
}

fn stamp() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn stamp_file() -> String {
    chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_details_are_taken_out_of_reports() {
        let s = scrub(
            "failed reading /Users/arsh/Downloads/x.iso from https://e.org/a 192.168.1.4 10.0.0.2:8080 me@x.org ok",
        );
        assert_eq!(
            s,
            "failed reading [removed] from [removed] [removed] [removed] [removed] ok"
        );
        let c = crash_text(
            "index out of bounds at C:\\Users\\a\\b.txt",
            "src/service.rs:42",
        );
        assert!(c.contains("Where: src/service.rs:42"));
        assert!(!c.contains("Users"));
        assert!(c.starts_with(&format!("Fuselane {}", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn the_issue_link_is_filled_in_and_never_too_long() {
        let u = issue_url(
            "Crash report",
            "Fuselane 0.1 on macos",
            Some("Where: x.rs:1"),
            &["a".into()],
        );
        assert!(
            u.starts_with("https://github.com/ArshPunisher/fuselane/issues/new?title=Crash+report")
        );
        assert!(u.contains("labels=bug"));
        assert!(u.contains("Crash+report%2A%2A") || u.contains("Crash+report"));
        let long = "x".repeat(20_000);
        let u = issue_url("t", &long, None, &[]);
        assert!(u.len() < BODY_MAX * 3 + 400, "{}", u.len());
    }

    #[test]
    fn crashes_are_offered_once_and_the_log_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let _ = HOME.set(dir.path().to_path_buf());
        let home = home().unwrap().to_path_buf();
        write_crash("Fuselane test\nWhere: a.rs:1\nWhat: boom\n");
        let first = unseen_crash().expect("offered");
        assert!(first.contains("boom"));
        assert_eq!(unseen_crash(), None, "only once");
        log("download #3 failed (retry)");
        assert!(
            recent_log(5)
                .iter()
                .any(|l| l.ends_with("download #3 failed (retry)"))
        );
        std::fs::write(
            home.join("logs/fuselane.log"),
            vec![b'x'; (LOG_MAX + 1) as usize],
        )
        .unwrap();
        log("after rotation");
        assert!(home.join("logs/fuselane.log.1").exists());
        assert_eq!(recent_log(5).len(), 1);
    }
}
