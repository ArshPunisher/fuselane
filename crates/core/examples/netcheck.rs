//! Runs the speed test on this computer's networks and prints the results, to
//! compare with a public speed test by hand.
//! `cargo run -p fuselane-core --example netcheck`

#![allow(clippy::unwrap_used)] // a developer tool: a bad constant should stop it loudly
use std::sync::Arc;

use fuselane_core::netcheck;
use fuselane_core::speedtest::{self, Direction, Plan};

fn mbps(bps: Option<f64>) -> String {
    bps.map_or("-".into(), |b| format!("{:.1} Mbps", b * 8.0 / 1e6))
}

#[tokio::main]
async fn main() {
    let dns = netcheck::DNS_SERVER.parse().unwrap();
    let mut all = vec![];
    for iface in fuselane_netif::usable().unwrap_or_default() {
        let target = match speedtest::target_on(&iface, dns).await {
            Ok(t) => t,
            Err(e) => {
                println!("{:<12} {e}", iface.display_name);
                continue;
            }
        };
        let idle = netcheck::latency_of(
            &netcheck::connect_times(
                &iface,
                netcheck::LATENCY_TARGET.parse().unwrap(),
                10,
                std::time::Duration::from_secs(2),
            )
            .await,
        );
        let one = [(iface.clone(), target.clone())];
        let (down, _) =
            speedtest::run(Direction::Down, &one, Plan::DOWN, Arc::default(), |_| {}).await;
        let (up, _) = speedtest::run(Direction::Up, &one, Plan::UP, Arc::default(), |_| {}).await;
        println!(
            "{:<12} down {:>12}  up {:>12}  ping {:>6}  used {:.0} MB  {}",
            iface.display_name,
            mbps(down[0].bps),
            mbps(up[0].bps),
            idle.map_or("-".into(), |l| format!("{:.0}ms", l.median_ms)),
            (down[0].bytes + up[0].bytes) as f64 / 1e6,
            down[0].problem.clone().unwrap_or_default()
        );
        all.push((iface, target));
    }
    if all.len() > 1 {
        let (_, together) =
            speedtest::run(Direction::Down, &all, Plan::DOWN, Arc::default(), |_| {}).await;
        println!("{:<12} down {:>12}", "Together", mbps(together.bps));
    }
}
