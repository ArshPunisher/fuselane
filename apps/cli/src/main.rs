//! `fuselane`: the command-line interface.
//!
//! `fuselane get <url>` downloads one file over every usable network at once.
//! `fuselane nets` shows the networks Fuselane can see.

use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use fuselane_engine_http::download::{
    BoxIo, Connect, JobError, Network, ProgressFn, Source, Tuning, download,
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
            Command::Get {
                url,
                out,
                networks,
                streams,
                quiet,
            } => get(&url, out, &networks, streams, quiet).await,
        }
    })
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
        JobError::Staging(e) => format!("Couldn't save the file: {e}"),
    }
}

async fn get(
    link: &str,
    out: PathBuf,
    names: &[String],
    streams: Option<u32>,
    quiet: bool,
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
    let tuning = Tuning {
        auto_streams: streams.is_none(),
        streams_per_network: streams.unwrap_or(crate_default_streams()),
        progress,
        ..Tuning::default()
    };
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

    let result = download(source, networks, &out, tuning).await;
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
            let sum: u64 = report.bytes_by_network.values().sum::<u64>().max(1);
            for (id, name) in &names_for_report {
                let b = report.bytes_by_network.get(id).copied().unwrap_or(0);
                println!("  {name:<10} {:>9}  {:>3}%", human(b as f64), b * 100 / sum);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fuselane: {}", describe(&e));
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
    fn sizes_read_naturally() {
        assert_eq!(human(0.0), "0 B");
        assert_eq!(human(1536.0), "1.5 KB");
        assert_eq!(human(5.0 * 1024.0 * 1024.0 * 1024.0), "5.0 GB");
    }
}
