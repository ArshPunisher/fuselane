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

/// Addresses a network can try, in Happy Eyeballs order (RFC 8305 §4): families
/// interleaved starting with the first resolved, and the last address that worked
/// first. Families the network has no address in are left out.
pub fn candidates(
    addrs: &[SocketAddr],
    local: &[std::net::IpAddr],
    preferred: Option<SocketAddr>,
) -> Vec<SocketAddr> {
    let usable: Vec<SocketAddr> = addrs
        .iter()
        .copied()
        .filter(|a| a.ip().is_loopback() || local.iter().any(|l| l.is_ipv4() == a.is_ipv4()))
        .collect();
    let first_v6 = usable.first().is_some_and(SocketAddr::is_ipv6);
    let (mut a, mut b): (Vec<_>, Vec<_>) =
        usable.into_iter().partition(|x| x.is_ipv6() == first_v6);
    a.reverse();
    b.reverse();
    let mut out = Vec::with_capacity(a.len() + b.len());
    while let Some(x) = a.pop() {
        out.push(x);
        if let Some(y) = b.pop() {
            out.push(y);
        }
    }
    while let Some(y) = b.pop() {
        out.push(y);
    }
    if let Some(p) = preferred
        && let Some(pos) = out.iter().position(|x| *x == p)
    {
        let p = out.remove(pos);
        out.insert(0, p);
    }
    out
}

/// Delay before starting the next address while earlier ones are still trying.
pub const ATTEMPT_DELAY: Duration = Duration::from_millis(250);

/// Races connection attempts: starts the first candidate, then another every
/// `ATTEMPT_DELAY` (or at once when one fails), and keeps the first that connects.
/// A broken IPv6 path (L-109) costs 250 ms instead of hanging the download.
pub async fn happy_connect<T, F, Fut>(
    candidates: &[SocketAddr],
    total: Duration,
    connect: F,
) -> std::io::Result<(T, SocketAddr)>
where
    T: Send + 'static,
    F: Fn(SocketAddr) -> Fut,
    Fut: std::future::Future<Output = std::io::Result<T>> + Send + 'static,
{
    use std::io::{Error, ErrorKind};
    if candidates.is_empty() {
        return Err(Error::new(
            ErrorKind::AddrNotAvailable,
            "no address in a family this network has",
        ));
    }
    let race = async {
        let mut set = tokio::task::JoinSet::new();
        let mut next = 0;
        let mut last_err = Error::new(ErrorKind::TimedOut, "connection timed out");
        loop {
            if next < candidates.len() && (set.is_empty() || next == 0) {
                let addr = candidates[next];
                let fut = connect(addr);
                set.spawn(async move { (addr, fut.await) });
                next += 1;
                continue;
            }
            let more = next < candidates.len();
            tokio::select! {
                joined = set.join_next(), if !set.is_empty() => match joined {
                    Some(Ok((addr, Ok(conn)))) => return Ok((conn, addr)),
                    Some(Ok((_, Err(e)))) => last_err = e,
                    Some(Err(e)) => last_err = Error::other(e),
                    None => {}
                },
                () = tokio::time::sleep(ATTEMPT_DELAY), if more => {}
            }
            if next < candidates.len() {
                let addr = candidates[next];
                let fut = connect(addr);
                set.spawn(async move { (addr, fut.await) });
                next += 1;
            } else if set.is_empty() {
                return Err(last_err);
            }
        }
    };
    tokio::time::timeout(total, race)
        .await
        .unwrap_or_else(|_| Err(Error::new(ErrorKind::TimedOut, "connection timed out")))
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
    // The address that last worked on this network is tried first next time.
    let preferred: Arc<std::sync::Mutex<Option<SocketAddr>>> = Arc::default();
    let connect: Connect = Arc::new(move |_| {
        let (iface, addrs, host, preferred) = (
            iface.clone(),
            addrs.clone(),
            host.clone(),
            preferred.clone(),
        );
        Box::pin(async move {
            let last = *preferred
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let order = candidates(&addrs, &iface.addrs, last);
            let (tcp, used) = happy_connect(&order, timeout, |dest| {
                let iface = iface.clone();
                async move {
                    fuselane_transport::connect_pinned(&iface, dest, timeout)
                        .await
                        .map_err(std::io::Error::other)
                }
            })
            .await?;
            *preferred
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(used);
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
        JobError::Unreachable(why) => format!("Couldn't reach the server on any selected network ({why}). Check your connection and the link."),
        JobError::ProbeStatus(404) => "The server says this file doesn't exist (404). Check the link.".into(),
        JobError::ProbeStatus(401 | 403) => "The server refused access to this file. The link may need you to be signed in.".into(),
        JobError::ProbeStatus(429) => "The server is limiting downloads right now (429 Too Many Requests). Wait a few minutes, then resume.".into(),
        JobError::ProbeStatus(s) if fuselane_engine_http::retry::is_busy(*s) => format!("The server is busy (status {s}). Wait a minute, then resume."),
        JobError::ProbeStatus(s) => format!("The server answered with status {s}, so the download couldn't start."),
        JobError::LinkExpired(s) => format!("This link stopped working (the server said {s}). Get a fresh link to the same file and try again."),
        JobError::VersionChanged => "The file on the server changed during the download, so it was stopped to avoid a mixed file. Start it again.".into(),
        JobError::NoSpace { needed, free } => format!(
            "Not enough free space on that disk: this download needs {} more and only {} is free. Free up space or choose another folder.",
            human_bytes(*needed),
            human_bytes(*free)
        ),
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
    /// Shrinks retry waits (tests); `None` keeps real-world delays.
    pub retry_delay_scale: Option<f64>,
    /// Live speed limits shared with other downloads.
    pub limiter: Option<Arc<fuselane_limits::Limiter>>,
    /// Look the server up through each network (public resolvers) as well as the
    /// system resolver. Off by default: those resolvers see the server's name.
    pub per_network_dns: bool,
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

/// Sizes for messages: 1.5 GB, 820 MB.
fn human_bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}

/// The fix the UI offers for a failure (stored with the job; ERRORS.md §2).
pub fn action_for(e: &JobError) -> &'static str {
    match e {
        JobError::LinkExpired(_) | JobError::ProbeStatus(401 | 403 | 404 | 410) => "fix-link",
        JobError::VersionChanged
        | JobError::NotResumable(_)
        | JobError::ChecksumMismatch { .. } => "start-over",
        JobError::Disk(_) | JobError::Staging(_) | JobError::NoSpace { .. } => "free-space",
        JobError::Unreachable(_)
        | JobError::ProbeStatus(_)
        | JobError::AllNetworksFailed(_)
        | JobError::Paused => "retry",
    }
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

/// Everything needed to reach the server: the request target and one `Network`
/// per chosen interface (ids from 1, with their device names for reports).
fn is_local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_matches(|c| c == '[' || c == ']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// The loopback "network", for servers on this computer when no real network is up.
fn this_computer() -> Interface {
    Interface {
        name: "lo".into(),
        display_name: "This computer".into(),
        index: 0,
        kind: fuselane_netif::Kind::Loopback,
        addrs: vec![
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        ],
    }
}

async fn connect_plan(
    link: &str,
    chosen: &[String],
    per_network_dns: bool,
) -> Result<(Source, Vec<Network>, Vec<(u32, String)>), StartError> {
    let (https, host, port, path) = parse_link(link).map_err(StartError::BadInput)?;
    let ifaces = match pick_networks(chosen) {
        Ok(v) => v,
        // A server on this computer needs no network: loopback is never pinned.
        Err(_) if chosen.is_empty() && is_local_host(&host) => vec![this_computer()],
        Err(e) => return Err(StartError::BadInput(e)),
    };
    let literal = host.parse::<std::net::IpAddr>().is_ok();
    // The computer's own resolver, and (when enabled) each network's own lookup.
    let system = async {
        tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::lookup_host((host.as_str(), port)),
        )
        .await
        .ok()
        .and_then(Result::ok)
        .map(|a| a.collect::<Vec<SocketAddr>>())
        .unwrap_or_default()
    };
    let per_net = async {
        if !per_network_dns || literal {
            return vec![Vec::new(); ifaces.len()];
        }
        // All networks look up at once; each answer keeps its network's position.
        let mut set = tokio::task::JoinSet::new();
        for (i, f) in ifaces.iter().enumerate() {
            let (f, host) = (f.clone(), host.clone());
            set.spawn(async move {
                let r = fuselane_transport::dns::resolve_on(
                    &f,
                    &host,
                    &fuselane_transport::dns::PUBLIC_RESOLVERS,
                    Duration::from_millis(1500),
                )
                .await;
                (i, r)
            });
        }
        let mut out = vec![Vec::new(); ifaces.len()];
        while let Some(Ok((i, r))) = set.join_next().await {
            if let (Some(slot), Ok(ips)) = (out.get_mut(i), r) {
                *slot = ips
                    .into_iter()
                    .map(|ip| SocketAddr::new(ip, port))
                    .collect();
            }
        }
        out
    };
    let (system, per_net) = tokio::join!(system, per_net);
    // Each network tries its own answer first, then the system's as a fallback.
    let per_iface: Vec<Vec<SocketAddr>> = per_net
        .into_iter()
        .map(|mut own| {
            for a in &system {
                if !own.contains(a) {
                    own.push(*a);
                }
            }
            own
        })
        .collect();
    let Some(first) = system
        .first()
        .copied()
        .or_else(|| per_iface.iter().flatten().next().copied())
    else {
        return Err(StartError::Setup(format!(
            "couldn't find the server \"{host}\". Check the link and your connection."
        )));
    };
    let host_arc: Arc<str> = Arc::from(host.as_str());
    let names: Vec<(u32, String)> = ifaces
        .iter()
        .enumerate()
        .map(|(i, f)| (i as u32 + 1, f.name.clone()))
        .collect();
    let networks: Vec<Network> = ifaces
        .into_iter()
        .zip(per_iface)
        .enumerate()
        .map(|(i, (f, addrs))| {
            network_for(
                i as u32 + 1,
                f,
                Arc::new(addrs),
                https,
                host_arc.clone(),
                Duration::from_secs(10),
            )
        })
        .collect();
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
    Ok((source, networks, names))
}

/// What a link points at, without downloading it: for the New download dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub filename: String,
    pub total: Option<u64>,
    /// The server answers byte ranges, so the file can be split across networks.
    pub splittable: bool,
}

/// Asks the server about a link (one tiny ranged request, L-01).
pub async fn preview(link: &str) -> Result<Preview, String> {
    let (source, networks, _) = connect_plan(link, &[], false)
        .await
        .map_err(|e| e.to_string())?;
    let tuning = Tuning {
        connect_timeout: Duration::from_secs(6),
        first_byte_timeout: Duration::from_secs(6),
        ..Tuning::default()
    };
    let p = fuselane_engine_http::download::probe(&source, &networks, &tuning)
        .await
        .map_err(|e| describe(&e))?;
    Ok(Preview {
        filename: fuselane_storage::names::sanitize(&p.filename),
        total: p.total,
        splittable: p.ranges,
    })
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
    // One owner per download: the app and the command line share the list.
    let _claim = store
        .lock_job(id)
        .map_err(|e| StartError::Setup(format!("couldn't claim the download: {e}")))?
        .ok_or_else(|| {
            StartError::Setup(
                "This download is already running in Fuselane or in another terminal. Pause it there first."
                    .into(),
            )
        })?;
    let (source, networks, names) =
        connect_plan(link, &opts.networks, opts.per_network_dns).await?;
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
        retry_delay_scale: opts.retry_delay_scale.unwrap_or(defaults.retry_delay_scale),
        limiter: opts.limiter,
        ..defaults
    };
    let _ = store.apply(id, Event::Start, None);
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
                let _ = store.set_error_code(id, action_for(&error));
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

    #[tokio::test(flavor = "multi_thread")]
    async fn a_download_running_elsewhere_is_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(Store::open(&dir.path().join("jobs.db")).unwrap());
        let id = store.create("http://127.0.0.1:9/f", dir.path()).unwrap();
        // The other front end holds it (a second Store on the same file).
        let other = Store::open(&dir.path().join("jobs.db")).unwrap();
        let claim = other.lock_job(id).unwrap().unwrap();
        let r = run(
            store.clone(),
            id,
            "http://127.0.0.1:9/f",
            dir.path().join("f"),
            None,
            RunOptions::default(),
        )
        .await;
        match r {
            Err(StartError::Setup(m)) => assert!(m.contains("already running"), "{m}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert_eq!(store.get(id).unwrap().status, Status::Queued, "untouched");
        drop(claim);
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

    fn sa(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn candidates_interleave_families_and_prefer_what_worked() {
        let v6a = sa("[2001:db8::1]:443");
        let v6b = sa("[2001:db8::2]:443");
        let v4a = sa("192.0.2.1:443");
        let v4b = sa("192.0.2.2:443");
        let both: Vec<std::net::IpAddr> =
            vec!["10.0.0.2".parse().unwrap(), "2001:db8::99".parse().unwrap()];
        let addrs = [v6a, v6b, v4a, v4b];
        assert_eq!(candidates(&addrs, &both, None), vec![v6a, v4a, v6b, v4b]);
        assert_eq!(
            candidates(&addrs, &both, Some(v4b)),
            vec![v4b, v6a, v4a, v6b]
        );
        // A preferred address that is no longer resolved is ignored.
        assert_eq!(
            candidates(&addrs, &both, Some(sa("198.51.100.1:443")))[0],
            v6a
        );
        // IPv4-only network: IPv6 addresses are skipped entirely.
        let v4only: Vec<std::net::IpAddr> = vec!["10.0.0.2".parse().unwrap()];
        assert_eq!(candidates(&addrs, &v4only, None), vec![v4a, v4b]);
        assert!(candidates(&[v6a], &v4only, None).is_empty());
        // Loopback is always allowed (tests, local servers).
        assert_eq!(candidates(&[sa("127.0.0.1:9")], &[], None).len(), 1);
    }

    #[tokio::test]
    async fn a_hanging_ipv6_path_falls_back_to_ipv4_quickly() {
        let dead = sa("[2001:db8::1]:443");
        let good = sa("192.0.2.1:443");
        let start = std::time::Instant::now();
        let (conn, used) = happy_connect(
            &[dead, good],
            Duration::from_secs(10),
            move |a| async move {
                if a == dead {
                    std::future::pending::<()>().await; // SYN never answered
                }
                Ok(a.port())
            },
        )
        .await
        .unwrap();
        assert_eq!((conn, used), (443, good));
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "took {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn a_refused_address_moves_on_at_once_and_all_failing_is_an_error() {
        let a = sa("192.0.2.1:1");
        let b = sa("192.0.2.2:2");
        let start = std::time::Instant::now();
        let r = happy_connect(&[a, b], Duration::from_secs(10), move |x| async move {
            if x == a {
                Err(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    "no",
                ))
            } else {
                Ok(x)
            }
        })
        .await
        .unwrap();
        assert_eq!(r.1, b);
        assert!(
            start.elapsed() < ATTEMPT_DELAY,
            "should not wait after a refusal"
        );
        let all_bad = happy_connect::<(), _, _>(&[a, b], Duration::from_secs(10), |_| async {
            Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "no",
            ))
        })
        .await;
        assert_eq!(
            all_bad.unwrap_err().kind(),
            std::io::ErrorKind::ConnectionRefused
        );
        let hung = happy_connect::<(), _, _>(&[a], Duration::from_millis(100), |_| async {
            std::future::pending::<std::io::Result<()>>().await
        })
        .await;
        assert_eq!(hung.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        assert!(
            happy_connect::<(), _, _>(&[], Duration::from_secs(1), |_| async { Ok(()) })
                .await
                .is_err()
        );
    }

    #[test]
    fn only_this_computer_counts_as_local() {
        for h in [
            "localhost",
            "LOCALHOST",
            "127.0.0.1",
            "127.8.9.1",
            "::1",
            "[::1]",
        ] {
            assert!(is_local_host(h), "{h}");
        }
        for h in [
            "example.com",
            "10.0.0.1",
            "localhost.example.com",
            "128.0.0.1",
        ] {
            assert!(!is_local_host(h), "{h}");
        }
        assert!(
            this_computer()
                .addrs
                .iter()
                .all(std::net::IpAddr::is_loopback)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn preview_reports_name_size_and_whether_it_splits() {
        use fuselane_testkit::{Content, Fault, RangeServer, Rule};
        let server = RangeServer::start(Content::new(123_456, 5)).await.unwrap();
        let link = format!("http://{}{}", server.addr(), server.path());
        let p = preview(&link).await.unwrap();
        assert_eq!(p.total, Some(123_456));
        assert!(p.splittable);
        assert!(!p.filename.is_empty());
        // A server that ignores ranges still previews, but can't be split.
        let whole = RangeServer::start(Content::new(999, 6)).await.unwrap();
        whole.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::IgnoreRange,
        });
        let p = preview(&format!("http://{}{}", whole.addr(), whole.path()))
            .await
            .unwrap();
        assert!(!p.splittable);
        // Failures come back as the catalogue's words.
        let gone = RangeServer::start(Content::new(10, 7)).await.unwrap();
        gone.add_rule(Rule {
            skip: 0,
            times: u32::MAX,
            fault: Fault::Status(404, None),
        });
        let e = preview(&format!("http://{}{}", gone.addr(), gone.path()))
            .await
            .unwrap_err();
        assert!(e.contains("404"), "{e}");
        assert!(
            preview("ftp://example.com/x")
                .await
                .unwrap_err()
                .contains("ftp")
        );
    }

    #[test]
    fn every_failure_offers_a_fitting_fix() {
        assert_eq!(action_for(&JobError::LinkExpired(403)), "fix-link");
        assert_eq!(action_for(&JobError::ProbeStatus(404)), "fix-link");
        assert_eq!(action_for(&JobError::ProbeStatus(503)), "retry");
        assert_eq!(action_for(&JobError::VersionChanged), "start-over");
        assert_eq!(action_for(&JobError::Unreachable("x".into())), "retry");
        // A start-over failure is never offered as a resume.
        assert!(!is_resumable(&JobError::VersionChanged));
    }

    #[test]
    fn integrity_failures_are_not_resumable() {
        assert!(!is_resumable(&JobError::VersionChanged));
        assert!(is_resumable(&JobError::LinkExpired(403)));
    }
}
