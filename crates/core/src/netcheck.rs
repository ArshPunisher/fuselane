//! Network check (B10.1): "why is my internet bad?", measured per network.
//!
//! For each network on its own: latency and jitter (timed TCP connects),
//! download speed (a real download through the engine, pinned to that network),
//! latency while that download runs (bufferbloat, graded A+ to F) and how long
//! a DNS lookup takes. Then every network together. Free to run: the speed test
//! uses Cloudflare's public speed endpoint, and nothing is uploaded.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fuselane_netif::Interface;

use crate::RunOptions;

/// Downloaded per network for the speed test (enough to leave slow start).
pub const SPEED_BYTES: u64 = 25_000_000;
/// Cloudflare's public speed-test endpoint (free, no account).
pub fn speed_url(bytes: u64) -> String {
    format!("https://speed.cloudflare.com/__down?bytes={bytes}")
}
/// Where latency is timed: a TCP connect to Cloudflare's resolver on 443.
pub const LATENCY_TARGET: &str = "1.1.1.1:443";
/// The resolver timed for DNS, and the name asked for.
pub const DNS_SERVER: &str = "1.1.1.1:53";
pub const DNS_NAME: &str = "example.com";

/// Median, jitter and loss of a set of round trips.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Latency {
    pub median_ms: f64,
    /// Mean difference between one round trip and the next.
    pub jitter_ms: f64,
    /// Share of attempts that got no answer, 0..=1.
    pub loss: f64,
}

/// `None` entries are attempts that timed out.
pub fn latency_of(samples: &[Option<f64>]) -> Option<Latency> {
    let ok: Vec<f64> = samples.iter().flatten().copied().collect();
    if ok.is_empty() {
        return None;
    }
    let mut sorted = ok.clone();
    sorted.sort_by(f64::total_cmp);
    let mid = sorted.len() / 2;
    let median_ms = if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    };
    let jitter_ms = if ok.len() > 1 {
        ok.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (ok.len() - 1) as f64
    } else {
        0.0
    };
    Some(Latency {
        median_ms,
        jitter_ms,
        loss: 1.0 - ok.len() as f64 / samples.len() as f64,
    })
}

/// Bufferbloat grade from how much latency a busy download adds (the same
/// scale people know from Waveform's test).
pub fn bloat_grade(idle_ms: f64, loaded_ms: f64) -> &'static str {
    match (loaded_ms - idle_ms).max(0.0) {
        x if x < 5.0 => "A+",
        x if x < 30.0 => "A",
        x if x < 60.0 => "B",
        x if x < 200.0 => "C",
        x if x < 400.0 => "D",
        _ => "F",
    }
}

/// One network's (or all networks') result.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Measured {
    pub idle: Option<Latency>,
    pub loaded: Option<Latency>,
    /// Download speed in bytes per second.
    pub down_bps: Option<f64>,
    pub dns_ms: Option<f64>,
    /// What went wrong, in plain words, when something couldn't be measured.
    pub problem: Option<String>,
}

/// Times `n` TCP connects to `dest` through `iface`, one after another.
pub async fn connect_times(
    iface: &Interface,
    dest: SocketAddr,
    n: usize,
    wait: Duration,
) -> Vec<Option<f64>> {
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let start = Instant::now();
        let ok = fuselane_transport::connect_pinned(iface, dest, wait)
            .await
            .is_ok();
        out.push(ok.then(|| start.elapsed().as_secs_f64() * 1000.0));
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
    out
}

/// Keeps timing connects until `stop` is set: latency under load.
async fn connect_until(
    iface: Interface,
    dest: SocketAddr,
    stop: Arc<AtomicBool>,
) -> Vec<Option<f64>> {
    let mut out = vec![];
    while !stop.load(Ordering::Relaxed) && out.len() < 200 {
        let start = Instant::now();
        let ok = fuselane_transport::connect_pinned(&iface, dest, Duration::from_secs(2))
            .await
            .is_ok();
        out.push(ok.then(|| start.elapsed().as_secs_f64() * 1000.0));
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    out
}

/// Times one DNS lookup through `iface`.
pub async fn dns_time(iface: &Interface, server: SocketAddr, name: &str) -> Option<f64> {
    let start = Instant::now();
    fuselane_transport::dns::resolve_on(iface, name, &[server], Duration::from_secs(3))
        .await
        .ok()
        .filter(|a| !a.is_empty())
        .map(|_| start.elapsed().as_secs_f64() * 1000.0)
}

/// Downloads `url` through `networks` (empty = all) into `dir` and returns
/// bytes per second; the file is removed afterwards.
pub async fn download_speed(
    url: &str,
    networks: Vec<String>,
    dir: PathBuf,
    cancel: Option<fuselane_engine_http::download::Cancel>,
) -> Result<f64, String> {
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let opts = RunOptions {
        networks,
        cancel,
        ..RunOptions::default()
    };
    let report = crate::runner::fetch(url, dir, opts).await;
    let secs = start.elapsed().as_secs_f64();
    if let Ok(r) = &report {
        let _ = std::fs::remove_file(&r.path);
    }
    let r = report?;
    Ok(r.total as f64 / secs.max(0.001))
}

/// Everything for one network: idle latency, then a download with latency
/// timed alongside, then DNS.
pub async fn measure(
    iface: &Interface,
    url: &str,
    latency_at: SocketAddr,
    dns_at: SocketAddr,
    dir: PathBuf,
) -> Measured {
    let mut m = Measured {
        idle: latency_of(&connect_times(iface, latency_at, 8, Duration::from_secs(2)).await),
        ..Measured::default()
    };
    if m.idle.is_none() {
        m.problem = Some(
            "No answer through this network: it may be offline or behind a sign-in page.".into(),
        );
        return m;
    }
    let stop = Arc::new(AtomicBool::new(false));
    let loaded = tokio::spawn(connect_until(iface.clone(), latency_at, stop.clone()));
    // Let the download get going before loaded latency counts.
    let speed = download_speed(url, vec![iface.name.clone()], dir, None).await;
    stop.store(true, Ordering::Relaxed);
    let loaded = loaded.await.unwrap_or_default();
    m.loaded = latency_of(&loaded[loaded.len().min(3)..]).or_else(|| latency_of(&loaded));
    match speed {
        Ok(bps) => m.down_bps = Some(bps),
        Err(e) => m.problem = Some(format!("The speed test didn't finish: {e}")),
    }
    m.dns_ms = dns_time(iface, dns_at, DNS_NAME).await;
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_has_a_median_jitter_and_loss() {
        let l = latency_of(&[Some(10.0), Some(20.0), None, Some(14.0)]).unwrap();
        assert_eq!(l.median_ms, 14.0);
        assert!((l.jitter_ms - 8.0).abs() < 1e-9, "{}", l.jitter_ms); // |20-10|, |14-20|
        assert!((l.loss - 0.25).abs() < 1e-9);
        assert_eq!(latency_of(&[None, None]), None);
        assert_eq!(latency_of(&[Some(5.0), Some(7.0)]).unwrap().median_ms, 6.0);
    }

    #[test]
    fn bufferbloat_is_graded_by_the_latency_a_download_adds() {
        assert_eq!(bloat_grade(20.0, 22.0), "A+");
        assert_eq!(bloat_grade(20.0, 45.0), "A");
        assert_eq!(bloat_grade(20.0, 70.0), "B");
        assert_eq!(bloat_grade(20.0, 150.0), "C");
        assert_eq!(bloat_grade(20.0, 400.0), "D");
        assert_eq!(bloat_grade(20.0, 900.0), "F");
        assert_eq!(bloat_grade(50.0, 30.0), "A+", "less under load is fine");
    }

    #[tokio::test]
    async fn connects_and_downloads_are_timed() {
        use fuselane_testkit::{Content, RangeServer};
        let server = RangeServer::start(Content::new(400 * 1024, 7))
            .await
            .unwrap();
        let lo = Interface {
            name: "lo0".into(),
            display_name: "loopback".into(),
            index: 1,
            kind: fuselane_netif::Kind::Loopback,
            addrs: vec!["127.0.0.1".parse().unwrap()],
        };
        let times = connect_times(&lo, server.addr(), 3, Duration::from_secs(2)).await;
        assert!(times.iter().all(Option::is_some));
        let dir = tempfile::tempdir().unwrap();
        let url = format!("http://{}{}", server.addr(), server.path());
        let bps = download_speed(&url, vec![], dir.path().to_path_buf(), None)
            .await
            .unwrap();
        assert!(bps > 0.0);
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            0,
            "nothing left behind"
        );
    }
}
