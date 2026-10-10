//! Runs the network check on this computer's networks and prints the results.
//! `cargo run -p fuselane-core --example netcheck`

#![allow(clippy::unwrap_used)] // a developer tool: a bad constant should stop it loudly
use fuselane_core::netcheck;

#[tokio::main]
async fn main() {
    let url = netcheck::speed_url(netcheck::SPEED_BYTES);
    let dir = std::env::temp_dir().join("fuselane-netcheck-example");
    for iface in fuselane_netif::usable().unwrap_or_default() {
        let m = netcheck::measure(
            &iface,
            &url,
            netcheck::LATENCY_TARGET.parse().unwrap(),
            netcheck::DNS_SERVER.parse().unwrap(),
            dir.clone(),
        )
        .await;
        println!(
            "{:<12} down {:>7}  latency {:>6}  jitter {:>5}  loaded {:>6} ({})  dns {:>5}  {}",
            iface.display_name,
            m.down_bps
                .map_or("-".into(), |b| format!("{:.1}Mb", b * 8.0 / 1e6)),
            m.idle
                .map_or("-".into(), |l| format!("{:.0}ms", l.median_ms)),
            m.idle
                .map_or("-".into(), |l| format!("{:.0}ms", l.jitter_ms)),
            m.loaded
                .map_or("-".into(), |l| format!("{:.0}ms", l.median_ms)),
            match (m.idle, m.loaded) {
                (Some(i), Some(l)) => netcheck::bloat_grade(i.median_ms, l.median_ms),
                _ => "-",
            },
            m.dns_ms.map_or("-".into(), |d| format!("{d:.0}ms")),
            m.problem.unwrap_or_default()
        );
    }
}
