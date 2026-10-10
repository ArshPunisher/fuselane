//! Speed test: how fast each network really is, measured the way public speed
//! tests do it rather than by timing one file.
//!
//! Each direction runs for a fixed time over several connections at once, pinned
//! to one network. Bytes are counted ten times a second; the run is cut into 20
//! slices, the slowest 30% (the ramp-up while TCP finds its speed) and the
//! fastest 10% (bursts) are dropped, and the rest averaged. That's the method
//! Ookla described for its HTTP test. Cloudflare's free speed endpoint serves the
//! bytes and takes the uploads; nothing is kept on disk.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use fuselane_netif::Interface;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Cloudflare's speed-test host (free, no account).
pub const HOST: &str = "speed.cloudflare.com";

/// Download or upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down,
    Up,
}

/// How long and how hard one direction runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub duration: Duration,
    /// Connections per network.
    pub streams: usize,
    /// Bytes asked for (or sent) per request; connections are reused.
    pub request_bytes: u64,
    /// Stops early once a network has moved this much, so a gigabit line or a
    /// phone plan isn't flooded.
    pub cap_bytes: u64,
}

impl Plan {
    pub const DOWN: Plan = Plan {
        duration: Duration::from_secs(8),
        streams: 6,
        request_bytes: 25_000_000,
        cap_bytes: 600_000_000,
    };
    pub const UP: Plan = Plan {
        duration: Duration::from_secs(7),
        streams: 4,
        request_bytes: 25_000_000,
        cap_bytes: 300_000_000,
    };
}

/// Where the bytes come from: a host name for the request, its addresses, and
/// whether to speak TLS (tests use plain HTTP on loopback).
#[derive(Debug, Clone)]
pub struct Target {
    pub host: String,
    pub addrs: Vec<SocketAddr>,
    pub tls: bool,
}

/// One network's share of a run.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Throughput {
    /// Bytes per second, or `None` when too little moved to tell.
    pub bps: Option<f64>,
    /// Everything moved, for the data-used line.
    pub bytes: u64,
    pub problem: Option<String>,
}

/// What the screen shows while a run goes: speed now per network (bytes per
/// second over the last half second) and how far through the run it is, 0..=1.
#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub now_bps: Vec<f64>,
    pub progress: f64,
}

/// Slices the run is cut into, and how many slow and fast ones are dropped.
const SLICES: usize = 20;
const DROP_SLOW: usize = 6;
const DROP_FAST: usize = 2;

/// Speed from `(seconds since start, bytes so far)` samples taken every tenth
/// of a second: 20 time slices, the 6 slowest and 2 fastest dropped, the rest
/// averaged. Short runs (a cap reached early) use as many slices as there are.
pub fn summarize(samples: &[(f64, u64)]) -> Option<f64> {
    let (&(t_end, b_end), &(t0, b0)) = (samples.last()?, samples.first()?);
    if t_end - t0 < 0.5 || b_end <= b0 {
        return None;
    }
    let slices = SLICES.min(samples.len() - 1).max(1);
    let width = (t_end - t0) / slices as f64;
    // Bytes at time t, read off the samples (linear between two).
    let at = |t: f64| -> f64 {
        let i = samples.partition_point(|&(s, _)| s <= t);
        if i == 0 {
            return samples[0].1 as f64;
        }
        if i >= samples.len() {
            return samples[samples.len() - 1].1 as f64;
        }
        let (ta, ba) = samples[i - 1];
        let (tb, bb) = samples[i];
        let f = if tb > ta { (t - ta) / (tb - ta) } else { 0.0 };
        ba as f64 + (bb as f64 - ba as f64) * f
    };
    let mut rates: Vec<f64> = (0..slices)
        .map(|k| {
            let a = t0 + width * k as f64;
            (at(a + width) - at(a)) / width
        })
        .collect();
    rates.sort_by(f64::total_cmp);
    let (slow, fast) = if slices == SLICES {
        (DROP_SLOW, DROP_FAST)
    } else {
        // Same shares, rounded down, for shorter runs.
        (slices * 3 / 10, slices / 10)
    };
    let kept = &rates[slow..slices - fast];
    (!kept.is_empty()).then(|| kept.iter().sum::<f64>() / kept.len() as f64)
}

/// Runs `plan` in `dir` over each of `nets` at the same time (one network for a
/// per-network test, several for "every network together"), each with the
/// speed server its own lookup found. `on_tick` is called ten times a second.
/// Returns one result per network, then the combined one.
pub async fn run(
    dir: Direction,
    nets: &[(Interface, Target)],
    plan: Plan,
    stop: Arc<AtomicBool>,
    on_tick: impl Fn(Tick),
) -> (Vec<Throughput>, Throughput) {
    let counters: Vec<Arc<AtomicU64>> = nets.iter().map(|_| Arc::default()).collect();
    let errors: Vec<Arc<std::sync::Mutex<Option<String>>>> =
        nets.iter().map(|_| Arc::default()).collect();
    let start = Instant::now();
    let deadline = start + plan.duration;
    let mut set = tokio::task::JoinSet::new();
    for (i, (iface, target)) in nets.iter().enumerate() {
        for s in 0..plan.streams {
            let (iface, target) = (iface.clone(), target.clone());
            let (count, err, stop) = (counters[i].clone(), errors[i].clone(), stop.clone());
            let addr = target.addrs[s % target.addrs.len().max(1)];
            set.spawn(async move {
                let r = tokio::time::timeout_at(
                    deadline.into(),
                    stream(dir, &iface, addr, &target, plan, &count, &stop),
                )
                .await;
                if let Ok(Err(e)) = r {
                    err.lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .get_or_insert(e);
                }
            });
        }
    }
    // Samples per network: (seconds, bytes so far).
    let mut series: Vec<Vec<(f64, u64)>> = nets.iter().map(|_| vec![(0.0, 0)]).collect();
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tick.tick().await;
    loop {
        tick.tick().await;
        let t = start.elapsed().as_secs_f64();
        let mut now_bps = Vec::with_capacity(nets.len());
        for (i, c) in counters.iter().enumerate() {
            let b = c.load(Ordering::Relaxed);
            series[i].push((t, b));
            // Speed over the last half second (five samples back).
            let s = &series[i];
            let back = s[s.len().saturating_sub(6)];
            now_bps.push((b - back.1) as f64 / (t - back.0).max(0.05));
        }
        let capped = counters
            .iter()
            .all(|c| c.load(Ordering::Relaxed) >= plan.cap_bytes);
        on_tick(Tick {
            now_bps,
            progress: (t / plan.duration.as_secs_f64()).min(1.0),
        });
        if Instant::now() >= deadline
            || capped
            || stop.load(Ordering::Relaxed)
            || set.is_empty()
            || (start.elapsed() > Duration::from_secs(3)
                && counters.iter().all(|c| c.load(Ordering::Relaxed) == 0))
        {
            break;
        }
        // Streams that ended early (errors) leave the set; keep the rest going.
        while let Some(Some(_)) = set.join_next().now_or_never() {}
    }
    stop_streams(&mut set).await;
    let per: Vec<Throughput> = series
        .iter()
        .zip(&errors)
        .map(|(s, e)| {
            let bytes = s.last().map_or(0, |x| x.1);
            let bps = summarize(s);
            let problem = match (bps, e.lock().unwrap_or_else(|p| p.into_inner()).clone()) {
                (None, Some(e)) => Some(e),
                (None, None) => Some("Too little got through to measure.".into()),
                _ => None,
            };
            Throughput {
                bps,
                bytes,
                problem,
            }
        })
        .collect();
    // Together: the samples of every network added up.
    let total: Vec<(f64, u64)> = (0..series.first().map_or(0, Vec::len))
        .map(|k| (series[0][k].0, series.iter().map(|s| s[k].1).sum()))
        .collect();
    let together = Throughput {
        bps: summarize(&total),
        bytes: total.last().map_or(0, |x| x.1),
        problem: None,
    };
    (per, together)
}

/// Ends any stream still going (they stop at the deadline anyway).
async fn stop_streams(set: &mut tokio::task::JoinSet<()>) {
    set.abort_all();
    while set.join_next().await.is_some() {}
}

/// `now_or_never` for a join without pulling in the futures crate.
trait NowOrNever: std::future::Future + Sized {
    fn now_or_never(self) -> Option<Self::Output> {
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        let mut fut = std::pin::pin!(self);
        match fut.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => Some(v),
            std::task::Poll::Pending => None,
        }
    }
}
impl<F: std::future::Future> NowOrNever for F {}

/// One connection, reused for request after request until the run ends.
async fn stream(
    dir: Direction,
    iface: &Interface,
    addr: SocketAddr,
    target: &Target,
    plan: Plan,
    count: &AtomicU64,
    stop: &AtomicBool,
) -> Result<(), String> {
    let tcp = fuselane_transport::connect_pinned(iface, addr, Duration::from_secs(5))
        .await
        .map_err(|e| e.to_string())?;
    if target.tls {
        let tls = fuselane_transport::tls(tcp, &target.host)
            .await
            .map_err(|e| e.to_string())?;
        requests(dir, tls, target, plan, count, stop).await
    } else {
        requests(dir, tcp, target, plan, count, stop).await
    }
}

async fn requests<S: AsyncRead + AsyncWrite + Unpin>(
    dir: Direction,
    mut s: S,
    target: &Target,
    plan: Plan,
    count: &AtomicU64,
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut buf = vec![0u8; 64 * 1024];
    while !stop.load(Ordering::Relaxed) && count.load(Ordering::Relaxed) < plan.cap_bytes {
        match dir {
            Direction::Down => {
                let req = format!(
                    "GET /__down?bytes={} HTTP/1.1\r\nHost: {}\r\nUser-Agent: Fuselane\r\nAccept-Encoding: identity\r\n\r\n",
                    plan.request_bytes, target.host
                );
                s.write_all(req.as_bytes())
                    .await
                    .map_err(|e| e.to_string())?;
                let (len, extra) = read_head(&mut s).await?;
                count.fetch_add(extra.len() as u64, Ordering::Relaxed);
                let mut left = len.saturating_sub(extra.len() as u64);
                while left > 0 {
                    let n = s.read(&mut buf).await.map_err(|e| e.to_string())?;
                    if n == 0 {
                        return Err("The server closed the connection.".into());
                    }
                    left = left.saturating_sub(n as u64);
                    count.fetch_add(n as u64, Ordering::Relaxed);
                    if stop.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                }
            }
            Direction::Up => {
                let head = format!(
                    "POST /__up HTTP/1.1\r\nHost: {}\r\nUser-Agent: Fuselane\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
                    target.host, plan.request_bytes
                );
                s.write_all(head.as_bytes())
                    .await
                    .map_err(|e| e.to_string())?;
                buf.fill(0);
                let mut left = plan.request_bytes;
                while left > 0 {
                    let n = (buf.len() as u64).min(left) as usize;
                    s.write_all(&buf[..n]).await.map_err(|e| e.to_string())?;
                    left -= n as u64;
                    count.fetch_add(n as u64, Ordering::Relaxed);
                    if stop.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                }
                s.flush().await.map_err(|e| e.to_string())?;
                // Read (and skip) the answer before the next request.
                let (len, extra) = read_head(&mut s).await?;
                let mut left = len.saturating_sub(extra.len() as u64);
                while left > 0 {
                    let n = s.read(&mut buf).await.map_err(|e| e.to_string())?;
                    if n == 0 {
                        break;
                    }
                    left = left.saturating_sub(n as u64);
                }
            }
        }
    }
    Ok(())
}

/// Reads a response head; returns the body length and any body bytes read
/// along with it. Anything but 2xx is an error.
async fn read_head<S: AsyncRead + Unpin>(s: &mut S) -> Result<(u64, Vec<u8>), String> {
    let mut head = Vec::with_capacity(1024);
    let mut byte = [0u8; 1024];
    let end = loop {
        let n = s.read(&mut byte).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("The server closed the connection.".into());
        }
        head.extend_from_slice(&byte[..n]);
        if let Some(i) = head.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if head.len() > 64 * 1024 {
            return Err("The server's answer didn't make sense.".into());
        }
    };
    let text = String::from_utf8_lossy(&head[..end]).to_ascii_lowercase();
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if !(200..300).contains(&status) {
        return Err(format!("The speed server answered {status}."));
    }
    let len = text
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    Ok((len, head[end..].to_vec()))
}

/// Fetches a small page (Cloudflare's `/meta`: provider, city, address) over
/// one network and returns its body.
pub async fn fetch_small(iface: &Interface, target: &Target, path: &str) -> Result<String, String> {
    let addr = *target
        .addrs
        .first()
        .ok_or("No address for the speed server.")?;
    let tcp = fuselane_transport::connect_pinned(iface, addr, Duration::from_secs(5))
        .await
        .map_err(|e| e.to_string())?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nUser-Agent: Fuselane\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n",
        target.host
    );
    async fn go<S: AsyncRead + AsyncWrite + Unpin>(mut s: S, req: &str) -> Result<String, String> {
        s.write_all(req.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let (len, mut body) = read_head(&mut s).await?;
        let len = len.min(64 * 1024);
        let mut buf = [0u8; 4096];
        while (body.len() as u64) < len {
            let n = s.read(&mut buf).await.map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            body.extend_from_slice(&buf[..n]);
        }
        Ok(String::from_utf8_lossy(&body).into_owned())
    }
    let work = async {
        if target.tls {
            let tls = fuselane_transport::tls(tcp, &target.host)
                .await
                .map_err(|e| e.to_string())?;
            go(tls, &req).await
        } else {
            go(tcp, &req).await
        }
    };
    tokio::time::timeout(Duration::from_secs(6), work)
        .await
        .map_err(|_| "The speed server didn't answer in time.".to_string())?
}

/// The speed server's addresses as seen through `iface` (its own DNS lookup, so
/// each network gets its nearest server), port 443.
pub async fn target_on(iface: &Interface, dns: SocketAddr) -> Result<Target, String> {
    let ips = fuselane_transport::dns::resolve_on(iface, HOST, &[dns], Duration::from_secs(3))
        .await
        .map_err(|e| format!("Couldn't look up the speed server: {e}"))?;
    let has_v4 = iface.addrs.iter().any(|a| a.is_ipv4());
    let has_v6 = iface.addrs.iter().any(|a| a.is_ipv6());
    let addrs: Vec<SocketAddr> = ips
        .into_iter()
        .filter(|ip| (ip.is_ipv4() && has_v4) || (ip.is_ipv6() && has_v6))
        .map(|ip| SocketAddr::new(ip, 443))
        .collect();
    if addrs.is_empty() {
        return Err("Couldn't look up the speed server.".into());
    }
    Ok(Target {
        host: HOST.into(),
        addrs,
        tls: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samples every 0.1 s for `secs` of a line that ramps up for a second,
    /// then runs at `rate` bytes per second.
    fn ramp(secs: f64, rate: f64) -> Vec<(f64, u64)> {
        let mut out = vec![(0.0, 0)];
        let mut bytes = 0.0;
        let mut t = 0.0;
        while t < secs - 1e-9 {
            t += 0.1;
            let speed = if t < 1.0 { rate * t } else { rate };
            bytes += speed * 0.1;
            out.push((t, bytes as u64));
        }
        out
    }

    #[test]
    fn the_ramp_up_doesnt_drag_the_speed_down() {
        let s = summarize(&ramp(8.0, 10_000_000.0)).unwrap();
        // Timing the whole run would say about 9.4 MB/s; the steady rate is 10.
        assert!((s - 10_000_000.0).abs() / 10_000_000.0 < 0.02, "{s}");
    }

    #[test]
    fn a_burst_doesnt_push_it_up() {
        let mut s = ramp(8.0, 5_000_000.0);
        // One tenth of a second that claims 50 MB more than it should.
        for p in s.iter_mut().skip(40) {
            p.1 += 5_000_000;
        }
        let v = summarize(&s).unwrap();
        assert!((v - 5_000_000.0).abs() / 5_000_000.0 < 0.03, "{v}");
    }

    #[test]
    fn too_little_to_tell_is_none() {
        assert_eq!(summarize(&[(0.0, 0)]), None);
        assert_eq!(summarize(&[(0.0, 0), (0.2, 100)]), None);
        assert_eq!(summarize(&[(0.0, 0), (1.0, 0)]), None);
    }

    #[test]
    fn a_short_capped_run_still_counts() {
        let v = summarize(&ramp(2.0, 50_000_000.0)).unwrap();
        assert!(v > 40_000_000.0 && v < 52_000_000.0, "{v}");
    }

    /// A tiny speed server on loopback: GET /__down?bytes=N sends N bytes,
    /// POST /__up reads the body and answers.
    async fn server() -> SocketAddr {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 64 * 1024];
                    let mut pending: Vec<u8> = vec![];
                    loop {
                        while !pending.windows(4).any(|w| w == b"\r\n\r\n") {
                            let Ok(n) = s.read(&mut buf).await else {
                                return;
                            };
                            if n == 0 {
                                return;
                            }
                            pending.extend_from_slice(&buf[..n]);
                        }
                        let end = pending.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
                        let head = String::from_utf8_lossy(&pending[..end]).to_string();
                        pending.drain(..end);
                        if head.starts_with("GET /__down") {
                            let n: usize = head
                                .split("bytes=")
                                .nth(1)
                                .and_then(|r| r.split_whitespace().next())
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0);
                            let h = format!("HTTP/1.1 200 OK\r\nContent-Length: {n}\r\n\r\n");
                            if s.write_all(h.as_bytes()).await.is_err() {
                                return;
                            }
                            let chunk = vec![7u8; 64 * 1024];
                            let mut left = n;
                            while left > 0 {
                                let k = left.min(chunk.len());
                                if s.write_all(&chunk[..k]).await.is_err() {
                                    return;
                                }
                                left -= k;
                            }
                        } else if head.starts_with("GET /meta") {
                            let body = r#"{"asOrganization":"Test ISP","city":"Mumbai"}"#;
                            let h = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                                body.len()
                            );
                            let _ = s.write_all(h.as_bytes()).await;
                            return;
                        } else {
                            let len: usize = head
                                .lines()
                                .find_map(|l| {
                                    l.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|v| v.trim().to_string())
                                })
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0);
                            let mut got = pending.len().min(len);
                            pending.drain(..got);
                            while got < len {
                                let Ok(n) = s.read(&mut buf).await else {
                                    return;
                                };
                                if n == 0 {
                                    return;
                                }
                                got += n;
                            }
                            if s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                });
            }
        });
        addr
    }

    fn lo() -> Interface {
        Interface {
            name: "lo0".into(),
            display_name: "loopback".into(),
            index: 1,
            kind: fuselane_netif::Kind::Loopback,
            addrs: vec!["127.0.0.1".parse().unwrap()],
        }
    }

    fn quick(streams: usize) -> Plan {
        Plan {
            duration: Duration::from_millis(1500),
            streams,
            request_bytes: 2_000_000,
            cap_bytes: u64::MAX,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn downloads_and_uploads_are_measured_with_live_ticks() {
        let addr = server().await;
        let target = Target {
            host: "localhost".into(),
            addrs: vec![addr],
            tls: false,
        };
        for dir in [Direction::Down, Direction::Up] {
            let ticks = std::sync::Mutex::new(0usize);
            let (per, together) = run(
                dir,
                &[(lo(), target.clone())],
                quick(3),
                Arc::default(),
                |t| {
                    assert_eq!(t.now_bps.len(), 1);
                    *ticks.lock().unwrap() += 1;
                },
            )
            .await;
            assert!(per[0].bps.unwrap() > 0.0, "{dir:?}: {per:?}");
            assert!(per[0].bytes > 2_000_000, "{dir:?}: reused connections");
            assert_eq!(together.bytes, per[0].bytes);
            assert!(*ticks.lock().unwrap() >= 10, "{dir:?}: ticks");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_cap_ends_a_run_early() {
        let addr = server().await;
        let target = Target {
            host: "localhost".into(),
            addrs: vec![addr],
            tls: false,
        };
        let plan = Plan {
            duration: Duration::from_secs(20),
            cap_bytes: 20_000_000,
            ..quick(2)
        };
        let start = Instant::now();
        let (per, _) = run(
            Direction::Down,
            &[(lo(), target)],
            plan,
            Arc::default(),
            |_| {},
        )
        .await;
        assert!(start.elapsed() < Duration::from_secs(10));
        assert!(per[0].bytes >= 20_000_000);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_dead_server_is_a_problem_not_a_hang() {
        // Nothing listens here.
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        let target = Target {
            host: "localhost".into(),
            addrs: vec![addr],
            tls: false,
        };
        let start = Instant::now();
        let (per, together) = run(
            Direction::Down,
            &[(lo(), target)],
            quick(2),
            Arc::default(),
            |_| {},
        )
        .await;
        assert!(start.elapsed() < Duration::from_secs(5));
        assert_eq!(per[0].bps, None);
        assert!(per[0].problem.is_some());
        assert_eq!(together.bps, None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn small_pages_are_read() {
        let addr = server().await;
        let target = Target {
            host: "localhost".into(),
            addrs: vec![addr],
            tls: false,
        };
        let body = fetch_small(&lo(), &target, "/meta").await.unwrap();
        assert!(body.contains("Test ISP"));
    }
}
