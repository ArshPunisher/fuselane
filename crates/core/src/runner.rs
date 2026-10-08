//! Runs one download job end to end, keeping the store in step with the engine.
//!
//! Shared by the CLI and the desktop app so both behave the same way (L-29): they
//! only differ in how they show progress and report the outcome.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use fuselane_engine_http::download::{
    BoxIo, Cancel, CheckpointFn, Connect, JobError, Network, ProgressFn, Report, Resume,
    SnapshotFn, Source, Tuning, download_with,
};
use fuselane_netif::Interface;

use crate::{Event, Job, Status, Store};

/// Only http and https (L-97). Returns (https?, host, port, path+query).
pub fn parse_link(link: &str) -> Result<(bool, String, u16, String), String> {
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

pub fn pick_networks(names: &[String]) -> Result<Vec<Interface>, String> {
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

pub fn network_for(
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

/// The plain-language message for each failure (ERRORS.md §2).
pub fn describe(e: &JobError) -> String {
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

pub fn parse_sha256(s: &str) -> Result<[u8; 32], String> {
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

/// How to run a job. Everything here is optional except `cancel`.
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Device names to use; empty means every usable network.
    pub networks: Vec<String>,
    /// Fixed streams per network; `None` means Auto.
    pub streams: Option<u32>,
    pub sha256: Option<[u8; 32]>,
    pub progress: Option<ProgressFn>,
    pub snapshot: Option<SnapshotFn>,
    pub cancel: Option<Cancel>,
    /// `None` keeps the engine's default.
    pub checkpoint_every: Option<Duration>,
}

/// Why a job couldn't start at all (nothing in the store changed state).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StartError {
    /// The user's input is wrong (bad link, unknown network): exit code 2 in the CLI.
    #[error("{0}")]
    BadInput(String),
    /// The environment let us down (DNS, store): exit code 1.
    #[error("{0}")]
    Setup(String),
}

/// How a job that did start ended.
#[derive(Debug)]
pub enum Outcome {
    /// Saved; `networks` maps engine network ids to device names for the report.
    Completed {
        report: Report,
        networks: Vec<(u32, String)>,
    },
    Paused,
    Failed {
        error: JobError,
        resumable: bool,
    },
}

/// Whether a failure leaves something worth resuming.
pub fn is_resumable(e: &JobError) -> bool {
    !matches!(
        e,
        JobError::VersionChanged | JobError::NotResumable(_) | JobError::ChecksumMismatch { .. }
    )
}

/// Finds a job to resume (`"last"` or a number) and repairs one left `running` by a crash.
pub fn job_to_resume(store: &Store, id: &str) -> Result<Job, StartError> {
    let job = if id == "last" {
        store
            .list()
            .ok()
            .and_then(|j| j.into_iter().find(|j| !j.status.finished()))
    } else {
        id.parse::<i64>().ok().and_then(|n| store.get(n).ok())
    };
    let Some(job) = job else {
        return Err(StartError::BadInput(format!(
            "there's no download \"{id}\" to resume. Run `fuselane ls` to see them."
        )));
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
        return Err(StartError::BadInput(format!(
            "download {} is {} and can't be resumed. Start it again.",
            job.id,
            job.status.as_str()
        )));
    }
    Ok(job)
}

/// Removes a job from the list, and its partial file if it never finished.
pub fn remove(store: &Store, id: i64) -> Result<(), crate::StoreError> {
    let job = store.get(id)?;
    if job.status != Status::Completed
        && let Some(p) = &job.staging_path
    {
        // Only ever delete our own staging file (L-69).
        if p.extension().is_some_and(|e| e == "fuselane") {
            let _ = std::fs::remove_file(p);
        }
    }
    store.delete(id)
}

/// Runs (or continues) job `id` and records how it ended in the store.
pub async fn run(
    store: Arc<Store>,
    id: i64,
    link: &str,
    out: PathBuf,
    resume: Option<Resume>,
    opts: RunOptions,
) -> Result<Outcome, StartError> {
    let (https, host, port, path) = parse_link(link).map_err(StartError::BadInput)?;
    let ifaces = pick_networks(&opts.networks).map_err(StartError::BadInput)?;
    let addrs: Vec<SocketAddr> = match tokio::time::timeout(
        Duration::from_secs(10),
        tokio::net::lookup_host((host.as_str(), port)),
    )
    .await
    {
        Ok(Ok(a)) => a.collect(),
        _ => {
            return Err(StartError::Setup(format!(
                "couldn't find the server \"{host}\". Check the link and your connection."
            )));
        }
    };
    let Some(first) = addrs.first().copied() else {
        return Err(StartError::Setup(format!("\"{host}\" has no addresses.")));
    };
    let addrs = Arc::new(addrs);
    let host_arc: Arc<str> = Arc::from(host.as_str());
    let names: Vec<(u32, String)> = ifaces
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
    let sink = {
        let store = store.clone();
        CheckpointFn(Arc::new(move |cp| {
            let _ = store.save_checkpoint(id, cp);
        }))
    };
    let defaults = Tuning::default();
    let tuning = Tuning {
        auto_streams: opts.streams.is_none(),
        streams_per_network: opts.streams.unwrap_or(defaults.streams_per_network),
        progress: opts.progress,
        snapshot: opts.snapshot,
        checkpoint: Some(sink),
        checkpoint_every: opts.checkpoint_every.unwrap_or(defaults.checkpoint_every),
        cancel: opts.cancel,
        expected_sha256: opts.sha256,
        ..defaults
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
    Ok(
        match download_with(source, networks, &out, tuning, resume).await {
            Ok(report) => {
                let _ = store.set_finished(id, &report.path, report.total);
                let _ = store.apply(id, Event::Complete, None);
                Outcome::Completed {
                    report,
                    networks: names,
                }
            }
            Err(JobError::Paused) => {
                let _ = store.apply(id, Event::Pause, None);
                Outcome::Paused
            }
            Err(error) => {
                let resumable = is_resumable(&error);
                // The plain-language message, so every front end shows the same words.
                let _ = store.apply(id, Event::Fail { resumable }, Some(&describe(&error)));
                Outcome::Failed { error, resumable }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
            "http://exa mple.com/f",
            "https://example.com:99999/f",
        ] {
            assert!(parse_link(bad).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn sha256_is_validated() {
        assert!(parse_sha256(&"ab".repeat(32)).is_ok());
        assert!(parse_sha256(&"AB".repeat(32)).is_ok());
        for bad in [
            "",
            "abc",
            &"zz".repeat(32),
            &"ab".repeat(33),
            &"é".repeat(32),
        ] {
            assert!(parse_sha256(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn unknown_networks_are_bad_input() {
        let e = pick_networks(&["definitely-not-a-nic0".into()]).unwrap_err();
        assert!(e.contains("fuselane nets"), "{e}");
    }

    #[test]
    fn resume_refuses_missing_and_finished_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("db")).unwrap();
        assert!(matches!(
            job_to_resume(&store, "last"),
            Err(StartError::BadInput(_))
        ));
        assert!(matches!(
            job_to_resume(&store, "42"),
            Err(StartError::BadInput(_))
        ));
        assert!(matches!(
            job_to_resume(&store, "-1; drop"),
            Err(StartError::BadInput(_))
        ));
        let id = store.create("http://x/f", dir.path()).unwrap();
        store.apply(id, Event::Start, None).unwrap();
        // A crash left it running: resuming repairs it to paused.
        assert_eq!(
            job_to_resume(&store, "last").unwrap().status,
            Status::Paused
        );
        store.apply(id, Event::Cancel, None).unwrap();
        assert!(matches!(
            job_to_resume(&store, &id.to_string()),
            Err(StartError::BadInput(_))
        ));
    }

    #[test]
    fn integrity_failures_are_not_resumable() {
        assert!(!is_resumable(&JobError::VersionChanged));
        assert!(is_resumable(&JobError::LinkExpired(403)));
    }
}
