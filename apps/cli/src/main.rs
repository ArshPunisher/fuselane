//! `fuselane`: the command-line interface.
//!
//! `fuselane get <url>` downloads one file over every usable network at once.
//! Ctrl-C pauses (progress is saved); `fuselane resume <id>` continues, even after
//! a crash. `fuselane ls` lists downloads, `fuselane rm <id>` removes one.
//! `fuselane nets` shows the networks Fuselane can see.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use fuselane_core::runner::{self, describe, parse_link, parse_sha256};
use fuselane_core::{Outcome, RunOptions, StartError, Status, Store};
use fuselane_engine_http::download::{Cancel, ProgressFn, Resume};

/// Fuse every connection into one fast lane.
#[derive(Debug, Parser)]
#[command(name = "fuselane", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Download a file over every usable network at once.
    Get {
        /// http:// or https:// link to the file.
        url: String,
        /// Folder to save into (default: the current folder).
        #[arg(short, long, default_value = ".")]
        out: PathBuf,
        /// Networks to use, by device name (default: every usable one). Example: en0,en5
        #[arg(short, long, value_delimiter = ',')]
        networks: Vec<String>,
        /// Fixed streams per network, 1 to 32 (default: Auto, 8 growing to 32).
        #[arg(short, long, value_parser = clap::value_parser!(u32).range(1..=32))]
        streams: Option<u32>,
        /// No progress line (for scripts).
        #[arg(short, long)]
        quiet: bool,
        /// Expected SHA-256 (64 hex characters); the file is only saved if it matches.
        #[arg(long, value_parser = parse_sha256)]
        sha256: Option<[u8; 32]>,
    },
    /// Continue a paused or interrupted download.
    Resume {
        /// Download number from `fuselane ls`, or `last`.
        id: String,
        #[arg(short, long, value_delimiter = ',')]
        networks: Vec<String>,
        #[arg(short, long, value_parser = clap::value_parser!(u32).range(1..=32))]
        streams: Option<u32>,
        #[arg(short, long)]
        quiet: bool,
    },
    /// List downloads, newest first.
    Ls {
        /// Print JSON (for scripts) instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Remove a download from the list (and its partial file, if unfinished).
    Rm {
        /// Download number from `fuselane ls`.
        id: i64,
    },
    /// Let browsers start this fuselane as the extension's helper (native messaging).
    /// The desktop app does this by itself on every launch.
    Browsers {
        /// Print JSON (for scripts) instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// List the networks Fuselane can use.
    Nets {
        /// Include loopback, tunnels and virtual adapters.
        #[arg(short, long)]
        all: bool,
        /// Print JSON (for scripts) instead of a table.
        #[arg(long)]
        json: bool,
    },
}

/// Started by a browser as the extension's native-messaging host: relay its
/// messages to the app until the browser closes the pipe.
fn native_host() -> ExitCode {
    let Ok(home) = fuselane_core::home::home() else {
        return ExitCode::FAILURE;
    };
    ExitCode::from(u8::try_from(fuselane_api::native::run_host(&home)).unwrap_or(1))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if fuselane_api::native::is_host_invocation(&args) {
        return native_host();
    }
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        eprintln!("Usage: fuselane get <url>   (try `fuselane --help`)");
        return ExitCode::from(2);
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("fuselane: couldn't start: {e}");
            return ExitCode::FAILURE;
        }
    };
    runtime.block_on(async {
        match command {
            Command::Nets { all, json } => nets(all, json),
            Command::Browsers { json } => browsers(json),
            Command::Ls { json } => ls(json),
            Command::Rm { id } => rm(id),
            Command::Resume {
                id,
                networks,
                streams,
                quiet,
            } => resume(&id, &networks, streams, quiet).await,
            Command::Get {
                url,
                out,
                networks,
                streams,
                quiet,
                sha256,
            } => get(&url, out, &networks, streams, quiet, sha256).await,
        }
    })
}

/// Where downloads are remembered: `FUSELANE_HOME`, else the OS app-data folder
/// (ARCHITECTURE.md §7).
fn open_store() -> Result<Store, String> {
    let store = fuselane_core::open_default().map_err(|e| e.to_string())?;
    if let Some(aside) = &store.recovered_from {
        eprintln!(
            "fuselane: the download list was damaged, so a fresh one was started. The old file is kept at {}.",
            aside.display()
        );
    }
    Ok(store)
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Queued => "queued",
        Status::Running => "running",
        Status::Paused => "paused",
        Status::Failed { resumable: true } => "failed (can resume)",
        Status::Failed { resumable: false } => "failed",
        Status::Completed => "done",
        Status::Cancelled => "cancelled",
    }
}

/// The status word scripts see; the same words the desktop app uses.
fn status_key(s: Status) -> &'static str {
    match s {
        Status::Queued => "queued",
        Status::Running => "running",
        Status::Paused => "paused",
        Status::Failed { resumable: true } => "failed",
        Status::Failed { resumable: false } => "failed-final",
        Status::Completed => "completed",
        Status::Cancelled => "cancelled",
    }
}

fn ls(json: bool) -> ExitCode {
    let store = match open_store() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("fuselane: {e}");
            return ExitCode::FAILURE;
        }
    };
    let jobs = match store.list() {
        Ok(j) => j,
        Err(e) => {
            eprintln!("fuselane: {e}");
            return ExitCode::FAILURE;
        }
    };
    if json {
        let rows: Vec<serde_json::Value> = jobs
            .iter()
            .map(|j| {
                // A finished download keeps no progress map: it saved everything,
                // and its name is the saved file's.
                let done = j.status == Status::Completed;
                let name = j.filename.clone().or_else(|| {
                    j.final_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().into_owned())
                });
                let saved = if done {
                    j.total.unwrap_or_else(|| j.secured_bytes())
                } else {
                    j.secured_bytes()
                };
                serde_json::json!({
                    "id": j.id,
                    "status": status_key(j.status),
                    "url": j.url,
                    "name": name,
                    "saved": saved,
                    "total": j.total,
                    "path": j.final_path,
                    "error": j.error,
                })
            })
            .collect();
        println!("{}", serde_json::Value::Array(rows));
        return ExitCode::SUCCESS;
    }
    if jobs.is_empty() {
        println!("No downloads yet. Try: fuselane get <url>");
        return ExitCode::SUCCESS;
    }
    for j in jobs {
        let name = j.filename.clone().unwrap_or_else(|| j.url.clone());
        let pct = match (j.status, j.total) {
            (Status::Completed, _) => "100%".to_string(),
            (_, Some(t)) if t > 0 => format!("{:.0}%", j.secured_bytes() as f64 * 100.0 / t as f64),
            _ => "".into(),
        };
        println!(
            "{:>4}  {:<20} {:>5}  {name}",
            j.id,
            status_word(j.status),
            pct
        );
    }
    ExitCode::SUCCESS
}

fn browsers(json: bool) -> ExitCode {
    use fuselane_api::hosts::{self, Outcome};
    let (Ok(exe), Ok(home)) = (std::env::current_exe(), fuselane_core::home::home()) else {
        eprintln!("fuselane: couldn't tell where fuselane or its data folder is.");
        return ExitCode::FAILURE;
    };
    let outcomes = match hosts::register(&exe, &home) {
        Ok(o) => o,
        Err(problem) => {
            eprintln!("fuselane: {problem}");
            return ExitCode::FAILURE;
        }
    };
    let failed = outcomes
        .iter()
        .any(|(_, o)| matches!(o, Outcome::Failed(_)));
    if json {
        let rows: Vec<serde_json::Value> = outcomes
            .iter()
            .map(|(browser, o)| {
                let (state, detail) = match o {
                    Outcome::Written(p) => ("registered", p.display().to_string()),
                    Outcome::Unchanged => ("registered", String::new()),
                    Outcome::NotInstalled => ("not-installed", String::new()),
                    Outcome::Failed(e) => ("failed", e.clone()),
                };
                serde_json::json!({"browser": browser, "state": state, "detail": detail})
            })
            .collect();
        println!("{}", serde_json::Value::Array(rows));
    } else {
        for (browser, o) in &outcomes {
            let line = match o {
                Outcome::Written(_) | Outcome::Unchanged => "ready".to_string(),
                Outcome::NotInstalled => "not installed".to_string(),
                Outcome::Failed(e) => format!("failed: {e}"),
            };
            println!("{browser:<14} {line}");
        }
    }
    if failed {
        eprintln!("fuselane: some browsers couldn't be set up; see above.");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn rm(id: i64) -> ExitCode {
    let store = match open_store() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("fuselane: {e}");
            return ExitCode::FAILURE;
        }
    };
    let job = match store.get(id) {
        Ok(j) => j,
        Err(_) => {
            eprintln!("fuselane: there's no download {id}. Run `fuselane ls` to see them.");
            return ExitCode::from(2);
        }
    };
    match runner::remove(&store, job.id) {
        Ok(()) => {
            println!("Removed download {id}.");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fuselane: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn resume(id: &str, names: &[String], streams: Option<u32>, quiet: bool) -> ExitCode {
    let store = match open_store() {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("fuselane: {e}");
            return ExitCode::FAILURE;
        }
    };
    let job = match runner::job_to_resume(&store, id) {
        Ok(j) => j,
        Err(e) => return start_failed(&e),
    };
    let r = job.resume();
    run(
        store,
        job.id,
        &job.url,
        job.dir.clone(),
        names,
        streams,
        quiet,
        r,
        None,
    )
    .await
}

fn start_failed(e: &StartError) -> ExitCode {
    eprintln!("fuselane: {e}");
    match e {
        StartError::BadInput(_) => ExitCode::from(2),
        StartError::Setup(_) => ExitCode::FAILURE,
    }
}

fn nets(all: bool, json: bool) -> ExitCode {
    match if all {
        fuselane_netif::list()
    } else {
        fuselane_netif::usable()
    } {
        Ok(list) if json => {
            let rows: Vec<serde_json::Value> = list
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "name": i.name,
                        "label": i.display_name,
                        "kind": format!("{:?}", i.kind).to_lowercase(),
                        "usable": i.usable(),
                        "addresses": i.addrs.iter().map(ToString::to_string).collect::<Vec<_>>(),
                    })
                })
                .collect();
            let empty = rows.is_empty();
            println!("{}", serde_json::Value::Array(rows));
            if empty {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Ok(list) if list.is_empty() => {
            eprintln!(
                "No usable networks. Join a Wi-Fi network, plug in Ethernet, or tether a phone over USB."
            );
            ExitCode::FAILURE
        }
        Ok(list) => {
            for i in list {
                let addrs: Vec<String> = i.addrs.iter().map(ToString::to_string).collect();
                println!(
                    "{:<8} {:<22} {:<9} {}",
                    i.name,
                    i.display_name,
                    format!("{:?}", i.kind),
                    addrs.join(", ")
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fuselane: couldn't list networks: {e}");
            ExitCode::FAILURE
        }
    }
}

fn human(bytes: f64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0} {}", U[i])
    } else {
        format!("{v:.1} {}", U[i])
    }
}

async fn get(
    link: &str,
    out: PathBuf,
    names: &[String],
    streams: Option<u32>,
    quiet: bool,
    sha256: Option<[u8; 32]>,
) -> ExitCode {
    if let Err(msg) = parse_link(link) {
        eprintln!("fuselane: {msg}");
        return ExitCode::from(2);
    }
    // A server on this computer needs no network: the same rule as the download.
    if let Err(msg) = runner::check_networks(link, names) {
        eprintln!("fuselane: {msg}");
        return ExitCode::from(2);
    }
    let store = match open_store() {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("fuselane: {e}");
            return ExitCode::FAILURE;
        }
    };
    let out = std::fs::canonicalize(&out).unwrap_or(out);
    let id = match store.create(link, &out) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("fuselane: {e}");
            return ExitCode::FAILURE;
        }
    };
    run(store, id, link, out, names, streams, quiet, None, sha256).await
}

/// Runs (or continues) download `id` and prints how it went.
#[allow(clippy::too_many_arguments)]
async fn run(
    store: Arc<Store>,
    id: i64,
    link: &str,
    out: PathBuf,
    names: &[String],
    streams: Option<u32>,
    quiet: bool,
    resume: Option<Resume>,
    sha256: Option<[u8; 32]>,
) -> ExitCode {
    let started = Instant::now();
    let last_draw = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(1)));
    let progress = (!quiet).then(|| {
        let last_draw = last_draw.clone();
        ProgressFn(Arc::new(move |written, total| {
            let mut last = last_draw
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if last.elapsed() < Duration::from_millis(250) {
                return;
            }
            *last = Instant::now();
            let speed = written as f64 / started.elapsed().as_secs_f64().max(0.001);
            let pct = total
                .filter(|t| *t > 0)
                .map(|t| format!("{:>5.1}%", written as f64 * 100.0 / t as f64))
                .unwrap_or_default();
            eprint!(
                "\r{pct}  {} of {}  {}/s   ",
                human(written as f64),
                total.map_or("?".into(), |t| human(t as f64)),
                human(speed)
            );
            let _ = std::io::stderr().flush();
        }))
    });
    // Ctrl-C pauses cleanly.
    let cancel = Cancel::new();
    tokio::spawn({
        let cancel = cancel.clone();
        async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancel.cancel();
            }
        }
    });
    let opts = RunOptions {
        networks: names.to_vec(),
        streams,
        sha256,
        progress,
        cancel: Some(cancel),
        checkpoint_every: std::env::var("FUSELANE_CHECKPOINT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_millis),
        ..RunOptions::default()
    };
    let outcome = runner::run(store, id, link, out, resume, opts).await;
    if !quiet {
        eprintln!();
    }
    match outcome {
        Err(e) => start_failed(&e),
        Ok(Outcome::Completed { report, networks }) => {
            let secs = started.elapsed().as_secs_f64().max(0.001);
            println!(
                "Saved {} ({}) in {:.1} s, {}/s",
                report.path.display(),
                human(report.total as f64),
                secs,
                human(report.total as f64 / secs)
            );
            let sum: u64 = report.bytes_by_network.values().sum::<u64>().max(1);
            for (id, name) in &networks {
                let b = report.bytes_by_network.get(id).copied().unwrap_or(0);
                println!("  {name:<10} {:>9}  {:>3}%", human(b as f64), b * 100 / sum);
            }
            ExitCode::SUCCESS
        }
        Ok(Outcome::Paused) => {
            eprintln!("fuselane: paused. Progress is saved; continue with `fuselane resume {id}`.");
            ExitCode::from(130)
        }
        Ok(Outcome::Failed { error, resumable }) => {
            eprintln!("fuselane: {}", describe(&error));
            if resumable {
                eprintln!("          Progress is saved; try again with `fuselane resume {id}`.");
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn version_matches_the_workspace() {
        assert_eq!(
            Cli::command().get_version(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(human(0.0), "0 B");
        assert_eq!(human(1536.0), "1.5 KB");
        assert_eq!(human(5.0 * 1024.0 * 1024.0 * 1024.0), "5.0 GB");
    }
}
