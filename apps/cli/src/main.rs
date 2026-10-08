//! `fuselane`: the command-line interface.
//!
//! `fuselane get <url>` downloads one file over every usable network at once.
//! Ctrl-C pauses (progress is saved); `fuselane resume <id>` continues, even after
//! a crash. `fuselane ls` lists downloads, `fuselane rm <id>` removes one.
//! `fuselane nets` shows the networks Fuselane can see.

use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use fuselane_core::{Event, Job, Status, Store};
use fuselane_engine_http::download::{
    BoxIo, Cancel, CheckpointFn, Connect, JobError, Network, ProgressFn, Resume, Source, Tuning,
    download_with,
};
use fuselane_netif::Interface;

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
    Ls,
    /// Remove a download from the list (and its partial file, if unfinished).
    Rm {
        /// Download number from `fuselane ls`.
        id: i64,
    },
    /// List the networks Fuselane can use.
    Nets {
        /// Include loopback, tunnels and virtual adapters.
        #[arg(short, long)]
        all: bool,
    },
}

fn main() -> ExitCode {
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
            Command::Nets { all } => nets(all),
            Command::Ls => ls(),
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
    let dir = match std::env::var_os("FUSELANE_HOME") {
        Some(h) => PathBuf::from(h),
        None => {
            let base = dirs::data_dir().ok_or("couldn't find the app-data folder")?;
            base.join(if cfg!(target_os = "macos") {
                "app.fuselane"
            } else if cfg!(windows) {
                "Fuselane"
            } else {
                "fuselane"
            })
        }
    };
    let store = Store::open(&dir.join("jobs.db"))
        .map_err(|e| format!("couldn't open the download list: {e}"))?;
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

fn ls() -> ExitCode {
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
    if job.status != Status::Completed
        && let Some(p) = &job.staging_path
    {
        // Only ever delete our own staging file (L-69).
        if p.extension().is_some_and(|e| e == "fuselane") {
            let _ = std::fs::remove_file(p);
        }
    }
    match store.delete(id) {
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
    let job: Option<Job> = if id == "last" {
        store
            .list()
            .ok()
            .and_then(|j| j.into_iter().find(|j| !j.status.finished()))
    } else {
        id.parse::<i64>().ok().and_then(|n| store.get(n).ok())
    };
    let Some(job) = job else {
        eprintln!(
            "fuselane: there's no download \"{id}\" to resume. Run `fuselane ls` to see them."
        );
        return ExitCode::from(2);
    };
    if job.status == Status::Running {
        // Left "running" by a process that crashed or was killed: it's interrupted.
        let _ = store.apply(job.id, Event::Pause, None);
    }
    let job = store.get(job.id).unwrap_or(job);
    if matches!(
        job.status,
        Status::Completed | Status::Cancelled | Status::Failed { resumable: false }
    ) {
        eprintln!(
            "fuselane: download {} is {} and can't be resumed. Start it again with `fuselane get`.",
            job.id,
            status_word(job.status)
        );
        return ExitCode::from(2);
    }
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

fn nets(all: bool) -> ExitCode {
    match if all {
        fuselane_netif::list()
    } else {
        fuselane_netif::usable()
    } {
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
                    "{:<10} {:<9} {}",
                    i.name,
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

/// Only http and https (L-97). Returns (https?, host, port, path+query).
fn parse_link(link: &str) -> Result<(bool, String, u16, String), String> {
    let url =
        url::Url::parse(link.trim()).map_err(|_| format!("\"{link}\" isn't a valid link."))?;
    let https = match url.scheme() {
        "https" => true,
        "http" => false,
        other => {
            return Err(format!(
                "{other}: links aren't supported. Use an http:// or https:// link."
            ));
        }
    };
    let host = url
        .host_str()
        .filter(|h| !h.is_empty())
        .ok_or("The link has no host name.")?
        .trim_matches(['[', ']'])
        .to_string();
    let port = url
        .port_or_known_default()
        .unwrap_or(if https { 443 } else { 80 });
    let mut path = url.path().to_string();
    if let Some(q) = url.query() {
        path.push('?');
        path.push_str(q);
    }
    Ok((https, host, port, path))
}

fn pick_networks(names: &[String]) -> Result<Vec<Interface>, String> {
    let all = fuselane_netif::list().map_err(|e| format!("couldn't list networks: {e}"))?;
    if names.is_empty() {
        let usable: Vec<Interface> = all.into_iter().filter(Interface::usable).collect();
        if usable.is_empty() {
            return Err("No usable networks. Join a Wi-Fi network, plug in Ethernet, or tether a phone over USB.".into());
        }
        return Ok(usable);
    }
    names
        .iter()
        .map(|n| {
            all.iter().find(|i| &i.name == n).cloned().ok_or_else(|| {
                format!("No network called \"{n}\". Run `fuselane nets` to see them.")
            })
        })
        .collect()
}

fn network_for(
    id: u32,
    iface: Interface,
    addrs: Arc<Vec<SocketAddr>>,
    https: bool,
    host: Arc<str>,
    timeout: Duration,
) -> Network {
    let name = iface.name.clone();
    let iface = Arc::new(iface);
    let connect: Connect = Arc::new(move |_| {
        let (iface, addrs, host) = (iface.clone(), addrs.clone(), host.clone());
        Box::pin(async move {
            // Use the first resolved address this network can reach (matching family).
            let dest = addrs
                .iter()
                .find(|a| {
                    a.ip().is_loopback() || iface.addrs.iter().any(|l| l.is_ipv4() == a.is_ipv4())
                })
                .copied()
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::AddrNotAvailable,
                        "no address in a family this network has",
                    )
                })?;
            let tcp = fuselane_transport::connect_pinned(&iface, dest, timeout)
                .await
                .map_err(std::io::Error::other)?;
            if https {
                let tls = fuselane_transport::tls(tcp, &host)
                    .await
                    .map_err(std::io::Error::other)?;
                Ok(Box::new(tls) as BoxIo)
            } else {
                Ok(Box::new(tcp) as BoxIo)
            }
        })
    });
    Network { id, name, connect }
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

fn crate_default_streams() -> u32 {
    Tuning::default().streams_per_network
}

/// The plain-language message for each failure (ERRORS.md §2).
fn describe(e: &JobError) -> String {
    match e {
        JobError::Unreachable(_) => "Couldn't reach the server on any selected network. Check your connection and the link.".into(),
        JobError::ProbeStatus(404) => "The server says this file doesn't exist (404). Check the link.".into(),
        JobError::ProbeStatus(401 | 403) => "The server refused access to this file. The link may need you to be signed in.".into(),
        JobError::ProbeStatus(s) => format!("The server answered with status {s}, so the download couldn't start."),
        JobError::LinkExpired(s) => format!("This link stopped working (the server said {s}). Get a fresh link to the same file and try again."),
        JobError::VersionChanged => "The file on the server changed during the download, so it was stopped to avoid a mixed file. Start it again.".into(),
        JobError::Disk(d) => format!("Saving failed ({d:?}). Check free space and that the folder is writable."),
        JobError::AllNetworksFailed(last) => format!("Every network failed. Last problem: {last}"),
        JobError::ChecksumMismatch { .. } => "The downloaded file doesn't match the SHA-256 you gave, so it wasn't saved under its name. The partial file is kept for inspection.".into(),
        JobError::Paused => "Paused. Progress is saved.".into(),
        JobError::NotResumable(why) => format!("This download can't be resumed ({why}). Start it again."),
        JobError::Staging(e) => format!("Couldn't save the file: {e}"),
    }
}

fn parse_sha256(s: &str) -> Result<[u8; 32], String> {
    let s = s.trim();
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("a SHA-256 is 64 hexadecimal characters".into());
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
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
    if let Err(msg) = pick_networks(names) {
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

/// Runs (or continues) download `id`, keeping the store in step with the engine.
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
    let (https, host, port, path) = match parse_link(link) {
        Ok(p) => p,
        Err(msg) => {
            eprintln!("fuselane: {msg}");
            return ExitCode::from(2);
        }
    };
    let ifaces = match pick_networks(names) {
        Ok(v) => v,
        Err(msg) => {
            eprintln!("fuselane: {msg}");
            return ExitCode::from(2);
        }
    };
    let addrs: Vec<SocketAddr> = match tokio::time::timeout(
        Duration::from_secs(10),
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    {
        Ok(Ok(a)) => a.collect(),
        _ => {
            eprintln!(
                "fuselane: couldn't find the server \"{host}\". Check the link and your connection."
            );
            return ExitCode::FAILURE;
        }
    };
    let Some(first) = addrs.first().copied() else {
        eprintln!("fuselane: \"{host}\" has no addresses.");
        return ExitCode::FAILURE;
    };
    let addrs = Arc::new(addrs);
    let host_arc: Arc<str> = Arc::from(host.as_str());
    let names_for_report: Vec<(u32, String)> = ifaces
        .iter()
        .enumerate()
        .map(|(i, f)| (i as u32 + 1, f.name.clone()))
        .collect();
    let networks: Vec<Network> = ifaces
        .into_iter()
        .enumerate()
        .map(|(i, f)| {
            network_for(
                i as u32 + 1,
                f,
                addrs.clone(),
                https,
                host_arc.clone(),
                Duration::from_secs(10),
            )
        })
        .collect();

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
    // Checkpoints go to the store; Ctrl-C pauses cleanly.
    let cancel = Cancel::new();
    tokio::spawn({
        let cancel = cancel.clone();
        async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancel.cancel();
            }
        }
    });
    let sink = {
        let store = store.clone();
        CheckpointFn(Arc::new(move |cp| {
            let _ = store.save_checkpoint(id, cp);
        }))
    };
    let checkpoint_every = std::env::var("FUSELANE_CHECKPOINT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(Tuning::default().checkpoint_every, Duration::from_millis);
    let tuning = Tuning {
        auto_streams: streams.is_none(),
        streams_per_network: streams.unwrap_or(crate_default_streams()),
        progress,
        checkpoint: Some(sink),
        checkpoint_every,
        cancel: Some(cancel),
        expected_sha256: sha256,
        ..Tuning::default()
    };
    let _ = store.apply(id, Event::Start, None);
    let host_header = if (https && port == 443) || (!https && port == 80) {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    let source = Source {
        addr: first,
        host: host_header,
        path,
    };

    let result = download_with(source, networks, &out, tuning, resume).await;
    if !quiet {
        eprintln!();
    }
    match result {
        Ok(report) => {
            let secs = started.elapsed().as_secs_f64().max(0.001);
            println!(
                "Saved {} ({}) in {:.1} s, {}/s",
                report.path.display(),
                human(report.total as f64),
                secs,
                human(report.total as f64 / secs)
            );
            let _ = store.set_final_path(id, &report.path);
            let _ = store.apply(id, Event::Complete, None);
            let sum: u64 = report.bytes_by_network.values().sum::<u64>().max(1);
            for (id, name) in &names_for_report {
                let b = report.bytes_by_network.get(id).copied().unwrap_or(0);
                println!("  {name:<10} {:>9}  {:>3}%", human(b as f64), b * 100 / sum);
            }
            ExitCode::SUCCESS
        }
        Err(JobError::Paused) => {
            let _ = store.apply(id, Event::Pause, None);
            eprintln!("fuselane: paused. Progress is saved; continue with `fuselane resume {id}`.");
            ExitCode::from(130)
        }
        Err(e) => {
            let resumable = !matches!(
                e,
                JobError::VersionChanged
                    | JobError::NotResumable(_)
                    | JobError::ChecksumMismatch { .. }
            );
            let _ = store.apply(id, Event::Fail { resumable }, Some(&e.to_string()));
            eprintln!("fuselane: {}", describe(&e));
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
    fn only_http_and_https_links_are_accepted() {
        assert_eq!(
            parse_link("https://example.com/a/b.iso?x=1").unwrap(),
            (true, "example.com".into(), 443, "/a/b.iso?x=1".into())
        );
        assert_eq!(
            parse_link("http://example.com:8080/f").unwrap(),
            (false, "example.com".into(), 8080, "/f".into())
        );
        assert_eq!(parse_link("http://[::1]:9/f").unwrap().1, "::1");
        for bad in [
            "ftp://example.com/f",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "not a link",
            "",
            "https://",
            "magnet:?xt=urn:btih:abc",
        ] {
            assert!(parse_link(bad).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn sha256_flag_is_validated() {
        assert!(parse_sha256(&"ab".repeat(32)).is_ok());
        for bad in ["", "abc", &"zz".repeat(32), &"ab".repeat(33)] {
            assert!(parse_sha256(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(human(0.0), "0 B");
        assert_eq!(human(1536.0), "1.5 KB");
        assert_eq!(human(5.0 * 1024.0 * 1024.0 * 1024.0), "5.0 GB");
    }
}
