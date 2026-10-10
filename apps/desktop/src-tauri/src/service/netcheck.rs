//! Speedtest (B10.1) in the app: measures every network on its own (ping,
//! download, upload, latency while busy, DNS) and then every network together,
//! with the live speed shown as it goes; keeps the last runs, logs outages from
//! the regular reach checks, and writes a dated report people can send their
//! internet provider as proof.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use fuselane_core::netcheck;
use fuselane_core::speedtest::{self, Direction, Plan, Target};
use fuselane_transport::probe::Reach;

use super::{Service, UiError, UiEvent, lock, store_error};

/// Checks kept, and outages kept.
const HISTORY: usize = 30;
const OUTAGES: usize = 300;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetResult {
    pub name: String,
    pub label: String,
    pub kind: String,
    /// Bytes per second.
    pub down_bps: Option<f64>,
    #[serde(default)]
    pub up_bps: Option<f64>,
    pub idle_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    /// Share of connects that got no answer, 0..=1.
    pub loss: Option<f64>,
    /// Latency while downloading.
    pub loaded_ms: Option<f64>,
    /// Latency while uploading.
    #[serde(default)]
    pub loaded_up_ms: Option<f64>,
    /// Bufferbloat grade, A+ to F, from the worse of the two.
    pub grade: Option<String>,
    pub dns_ms: Option<f64>,
    /// The internet provider behind this network, as the speed server sees it.
    #[serde(default)]
    pub isp: Option<String>,
    /// Where the speed server answering this network is ("Mumbai").
    #[serde(default)]
    pub server: Option<String>,
    /// Data the test used on this network, both ways.
    #[serde(default)]
    pub bytes: u64,
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CheckRun {
    /// Unix seconds.
    pub at: i64,
    pub results: Vec<NetResult>,
    /// Every network at once, bytes per second.
    pub together_bps: Option<f64>,
}

/// The run as it happens, ten times a second (its own event, so the history
/// isn't resent each time).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeedLive {
    /// "ping", "down", "up" or "together".
    pub step: String,
    /// The network being measured (its label); none for "together".
    pub network: Option<String>,
    /// Speed right now, bytes per second.
    pub bps: f64,
    /// How far through this step, 0..=1.
    pub progress: f64,
    /// How far through the whole run, 0..=1.
    pub overall: f64,
    /// This step's speed every tenth of a second so far, bytes per second.
    pub trace: Vec<f64>,
}

/// A time a network stopped reaching the internet.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Outage {
    pub name: String,
    pub label: String,
    /// "offline" or "sign-in" (a page in the way).
    pub kind: String,
    pub from: i64,
    /// None while it's still going on.
    pub to: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetCheckView {
    pub running: bool,
    /// What it's doing now ("Testing Wi-Fi…").
    pub phase: Option<String>,
    pub current: Option<CheckRun>,
    pub history: Vec<CheckRun>,
    pub outages: Vec<Outage>,
}

#[derive(Debug, Default)]
pub(super) struct NetCheckState {
    pub running: bool,
    pub phase: Option<String>,
    pub current: Option<CheckRun>,
    /// Set to stop the run; `None` once stopped.
    pub cancel: Option<Arc<AtomicBool>>,
    /// Steps done so far and in all, in seconds of planned time.
    pub done_secs: f64,
    pub total_secs: f64,
}

/// Seconds planned per step, for the overall progress.
const PING_SECS: f64 = 1.5;

/// Where a speed test reaches: the real internet, or stand-ins in tests.
#[derive(Debug, Clone)]
pub struct CheckTargets {
    /// The speed server; `None` looks it up through each network.
    pub speed: Option<Target>,
    /// Timed for latency (TCP connects).
    pub latency: Option<std::net::SocketAddr>,
    /// Timed for DNS; `None` skips it.
    pub dns: Option<std::net::SocketAddr>,
    pub down: Plan,
    pub up: Plan,
}

impl Default for CheckTargets {
    fn default() -> Self {
        CheckTargets {
            speed: None,
            latency: netcheck::LATENCY_TARGET.parse().ok(),
            dns: netcheck::DNS_SERVER.parse().ok(),
            down: Plan::DOWN,
            up: Plan::UP,
        }
    }
}

fn now() -> i64 {
    super::deadline::unix_now()
}

/// The provider and server city from Cloudflare's `/meta` answer.
fn meta_of(body: &str) -> (Option<String>, Option<String>) {
    let v: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let field = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(String::from)
    };
    (
        field("asOrganization"),
        field("city").or_else(|| field("colo")),
    )
}

/// Latency from timed connects, dropping the first few (they overlap the
/// start, before the line is busy).
fn loaded_of(samples: &[Option<f64>]) -> Option<f64> {
    netcheck::latency_of(&samples[samples.len().min(3)..])
        .or_else(|| netcheck::latency_of(samples))
        .map(|l| l.median_ms)
}

impl Service {
    pub fn netcheck_view(&self) -> NetCheckView {
        let st = lock(&self.netcheck);
        NetCheckView {
            running: st.running,
            phase: st.phase.clone(),
            current: st.current.clone(),
            history: self.check_history(),
            outages: lock(&self.outages).clone(),
        }
    }

    fn check_history(&self) -> Vec<CheckRun> {
        self.store
            .setting("netcheck_history")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn publish_netcheck(&self) {
        self.send(UiEvent::NetCheck {
            view: self.netcheck_view(),
        });
    }

    /// Starts a speed test of every usable network (one at a time, so they
    /// don't slow each other), then all together.
    pub fn start_netcheck(
        self: &Arc<Self>,
        targets: CheckTargets,
    ) -> Result<NetCheckView, UiError> {
        {
            let mut st = lock(&self.netcheck);
            if st.running {
                drop(st);
                return Ok(self.netcheck_view());
            }
            st.running = true;
            st.phase = Some("Starting…".into());
            st.current = Some(CheckRun {
                at: now(),
                ..CheckRun::default()
            });
            st.cancel = Some(Arc::default());
            st.done_secs = 0.0;
        }
        let me = self.clone();
        tokio::spawn(async move { me.run_netcheck(targets).await });
        Ok(self.netcheck_view())
    }

    pub fn cancel_netcheck(&self) {
        let mut st = lock(&self.netcheck);
        if let Some(c) = st.cancel.take() {
            c.store(true, Ordering::Relaxed);
        }
        st.phase = Some("Stopping…".into());
    }

    fn stop_flag(&self) -> Arc<AtomicBool> {
        lock(&self.netcheck)
            .cancel
            .clone()
            .unwrap_or_else(|| Arc::new(AtomicBool::new(true)))
    }

    fn cancelled(&self) -> bool {
        self.stop_flag().load(Ordering::Relaxed)
    }

    fn set_phase(&self, phase: String) {
        lock(&self.netcheck).phase = Some(phase);
        self.publish_netcheck();
    }

    /// Sends where the run is: `step_secs` is this step's planned length.
    fn publish_live(&self, live: SpeedLive, step_secs: f64) {
        let mut live = live;
        {
            let st = lock(&self.netcheck);
            live.overall = ((st.done_secs + live.progress * step_secs) / st.total_secs.max(1.0))
                .clamp(0.0, 1.0);
        }
        self.send(UiEvent::SpeedLive { live: Some(live) });
    }

    fn step_done(&self, secs: f64) {
        lock(&self.netcheck).done_secs += secs;
    }

    /// One direction over `nets`, with the live speed sent as it goes.
    async fn speed_step(
        &self,
        dir: Direction,
        step: &str,
        network: Option<String>,
        nets: &[(fuselane_netif::Interface, Target)],
        plan: Plan,
    ) -> (Vec<speedtest::Throughput>, speedtest::Throughput) {
        let trace = std::sync::Mutex::new(Vec::<f64>::new());
        let secs = plan.duration.as_secs_f64();
        let out = speedtest::run(dir, nets, plan, self.stop_flag(), |t| {
            let now: f64 = t.now_bps.iter().sum();
            let mut tr = lock(&trace);
            tr.push(now);
            self.publish_live(
                SpeedLive {
                    step: step.into(),
                    network: network.clone(),
                    bps: now,
                    progress: t.progress,
                    overall: 0.0,
                    trace: tr.clone(),
                },
                secs,
            );
        })
        .await;
        self.step_done(secs);
        out
    }

    async fn run_netcheck(self: Arc<Self>, targets: CheckTargets) {
        let nets = self.download_networks().unwrap_or_default();
        let (latency_at, dns_at, target) = (targets.latency, targets.dns, targets.speed);
        let (down_plan, up_plan) = (targets.down, targets.up);
        let per_net = PING_SECS + down_plan.duration.as_secs_f64() + up_plan.duration.as_secs_f64();
        lock(&self.netcheck).total_secs = per_net * nets.len() as f64
            + if nets.len() > 1 {
                down_plan.duration.as_secs_f64()
            } else {
                0.0
            };
        // Networks that measured, with the server they found, for "together".
        let mut working = vec![];
        for iface in &nets {
            if self.cancelled() {
                break;
            }
            let label = iface.display_name.clone();
            let mut r = NetResult {
                name: iface.name.clone(),
                label: label.clone(),
                kind: super::kind_word(iface.kind),
                ..NetResult::default()
            };
            // A network past its data allowance isn't run flat out.
            if self.limiter.blocked(&iface.name) {
                r.problem =
                    Some("Skipped: this network has used its data allowance for the month.".into());
                self.step_done(per_net);
                self.push_result(r);
                continue;
            }
            // Ping: find the speed server, who the provider is, idle latency, DNS.
            self.set_phase(format!("Testing {label}…"));
            self.publish_live(
                SpeedLive {
                    step: "ping".into(),
                    network: Some(label.clone()),
                    ..SpeedLive::default()
                },
                PING_SECS,
            );
            let found = match (&target, dns_at) {
                (Some(t), _) => Ok(t.clone()),
                (None, Some(dns)) => speedtest::target_on(iface, dns).await,
                (None, None) => Err("Couldn't look up the speed server.".into()),
            };
            let idle = match latency_at {
                Some(at) => netcheck::latency_of(
                    &netcheck::connect_times(iface, at, 10, std::time::Duration::from_secs(2))
                        .await,
                ),
                None => None,
            };
            r.idle_ms = idle.map(|l| l.median_ms);
            r.jitter_ms = idle.map(|l| l.jitter_ms);
            r.loss = idle.map(|l| l.loss);
            if let Some(dns) = dns_at {
                r.dns_ms = netcheck::dns_time(iface, dns, netcheck::DNS_NAME).await;
            }
            self.step_done(PING_SECS);
            let tgt = match (idle, found) {
                (None, _) => {
                    r.problem = Some(
                        "No answer through this network: it may be offline or behind a sign-in page."
                            .into(),
                    );
                    None
                }
                (_, Err(e)) => {
                    r.problem = Some(e);
                    None
                }
                (_, Ok(t)) => Some(t),
            };
            let Some(tgt) = tgt else {
                // Skip its speed steps in the overall progress.
                self.step_done(per_net - PING_SECS);
                self.push_result(r);
                continue;
            };
            if let Ok(body) = speedtest::fetch_small(iface, &tgt, "/meta").await {
                (r.isp, r.server) = meta_of(&body);
            }
            let one = [(iface.clone(), tgt.clone())];
            for dir in [Direction::Down, Direction::Up] {
                if self.cancelled() {
                    break;
                }
                let (step, plan) = match dir {
                    Direction::Down => ("down", down_plan),
                    Direction::Up => ("up", up_plan),
                };
                // Latency while busy, timed alongside.
                let busy = Arc::new(AtomicBool::new(false));
                let probe = latency_at.map(|at| {
                    tokio::spawn(netcheck::connect_until(iface.clone(), at, busy.clone()))
                });
                let (per, _) = self
                    .speed_step(dir, step, Some(label.clone()), &one, plan)
                    .await;
                busy.store(true, Ordering::Relaxed);
                let loaded = match probe {
                    Some(p) => loaded_of(&p.await.unwrap_or_default()),
                    None => None,
                };
                let t = per.into_iter().next().unwrap_or_default();
                r.bytes += t.bytes;
                // Speed-test data counts toward the network's usage and allowance.
                self.limiter.count(&iface.name, t.bytes);
                match dir {
                    Direction::Down => {
                        r.down_bps = t.bps;
                        r.loaded_ms = loaded;
                    }
                    Direction::Up => {
                        r.up_bps = t.bps;
                        r.loaded_up_ms = loaded;
                    }
                }
                if t.bps.is_none() && r.problem.is_none() {
                    r.problem = t
                        .problem
                        .map(|p| format!("The speed test didn't finish: {p}"));
                }
            }
            let worse = match (r.loaded_ms, r.loaded_up_ms) {
                (Some(d), Some(u)) => Some(d.max(u)),
                (d, u) => d.or(u),
            };
            r.grade = match (r.idle_ms, worse) {
                (Some(i), Some(l)) => Some(netcheck::bloat_grade(i, l).to_string()),
                _ => None,
            };
            if r.down_bps.is_some() {
                working.push((iface.clone(), tgt));
            }
            self.push_result(r);
        }
        // Every network at once: what Fuselane can pull in together.
        if nets.len() > 1 && !self.cancelled() {
            if working.len() > 1 {
                self.set_phase("Testing every network together…".into());
                let (per, all) = self
                    .speed_step(Direction::Down, "together", None, &working, down_plan)
                    .await;
                for ((iface, _), t) in working.iter().zip(&per) {
                    self.limiter.count(&iface.name, t.bytes);
                }
                if let Some(c) = lock(&self.netcheck).current.as_mut() {
                    c.together_bps = all.bps;
                }
            } else {
                self.step_done(down_plan.duration.as_secs_f64());
            }
        }
        let finished = lock(&self.netcheck).current.clone();
        if let Some(run) = finished.filter(|r| !r.results.is_empty()) {
            let mut history = self.check_history();
            history.insert(0, run);
            history.truncate(HISTORY);
            if let Ok(json) = serde_json::to_string(&history) {
                let _ = self.store.set_setting("netcheck_history", &json);
            }
        }
        {
            let mut st = lock(&self.netcheck);
            st.running = false;
            st.phase = None;
            st.cancel = None;
        }
        self.send(UiEvent::SpeedLive { live: None });
        self.publish_netcheck();
    }

    fn push_result(&self, r: NetResult) {
        if let Some(c) = lock(&self.netcheck).current.as_mut() {
            c.results.push(r);
        }
        self.publish_netcheck();
    }

    /// Called after each reach check: starts or ends outages per network.
    pub(super) fn note_reach(
        &self,
        before: &HashMap<String, Reach>,
        after: &HashMap<String, Reach>,
    ) {
        let labels: HashMap<String, String> = fuselane_netif::usable()
            .unwrap_or_default()
            .into_iter()
            .map(|i| (i.name, i.display_name))
            .collect();
        let mut outages = lock(&self.outages);
        let t = now();
        let mut changed = false;
        for (name, r) in after {
            let down = match r {
                Reach::Online => None,
                Reach::Offline(_) => Some("offline"),
                Reach::Portal { .. } => Some("sign-in"),
            };
            let open = outages
                .iter_mut()
                .find(|o| &o.name == name && o.to.is_none());
            match (down, open) {
                (None, Some(o)) => {
                    o.to = Some(t);
                    changed = true;
                }
                // A network seen for the first time that's already down isn't news.
                (Some(kind), None) if before.contains_key(name) => {
                    outages.push(Outage {
                        name: name.clone(),
                        label: labels.get(name).cloned().unwrap_or_else(|| name.clone()),
                        kind: kind.into(),
                        from: t,
                        to: None,
                    });
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            let len = outages.len();
            if len > OUTAGES {
                outages.drain(..len - OUTAGES);
            }
            if let Ok(json) = serde_json::to_string(&*outages) {
                let _ = self.store.set_setting("outages", &json);
            }
        }
    }

    /// Writes the report (HTML, opens in any browser and prints to PDF) into
    /// the downloads folder and returns its path.
    pub fn netcheck_report(&self) -> Result<std::path::PathBuf, UiError> {
        let view = self.netcheck_view();
        if view.history.is_empty() && view.outages.is_empty() {
            return Err(UiError::new(
                "nothing-to-report",
                "Run a network check first.",
                None,
            ));
        }
        let html = report_html(&view, env!("CARGO_PKG_VERSION"));
        let day = chrono::Local::now().format("%Y-%m-%d %H%M").to_string();
        let path = super::free_name(
            self.default_dir(),
            &format!("Fuselane network report {day}.html"),
        );
        std::fs::write(&path, html).map_err(store_error)?;
        Ok(path)
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn mbps(bps: Option<f64>) -> String {
    bps.map_or("–".into(), |b| {
        format!("{:.1} Mbps", b * 8.0 / 1_000_000.0)
    })
}

fn ms(v: Option<f64>) -> String {
    v.map_or("–".into(), |m| format!("{m:.0} ms"))
}

fn when(t: i64) -> String {
    chrono::DateTime::from_timestamp(t, 0)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%a %e %b %Y, %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

/// The report: every check with each network's numbers, and every outage
/// with when it started and how long it lasted.
pub fn report_html(view: &NetCheckView, version: &str) -> String {
    let mut checks = String::new();
    for run in &view.history {
        checks.push_str(&format!("<h3>{}</h3><table><tr><th>Network</th><th>Download</th><th>Upload</th><th>Latency</th><th>Jitter</th><th>Loss</th><th>Under load</th><th>DNS</th><th>Note</th></tr>", esc(&when(run.at))));
        for r in &run.results {
            checks.push_str(&format!(
                "<tr><td>{}{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}{}</td><td>{}</td><td>{}</td></tr>",
                esc(&r.label),
                r.isp.as_deref().map(|i| format!("<br><small>{}</small>", esc(i))).unwrap_or_default(),
                mbps(r.down_bps),
                mbps(r.up_bps),
                ms(r.idle_ms),
                ms(r.jitter_ms),
                r.loss.map_or("–".into(), |l| format!("{:.0}%", l * 100.0)),
                ms(r.loaded_ms),
                r.grade.as_deref().map(|g| format!(" ({g})")).unwrap_or_default(),
                ms(r.dns_ms),
                esc(r.problem.as_deref().unwrap_or(""))
            ));
        }
        if run.together_bps.is_some() {
            checks.push_str(&format!(
                "<tr class=\"all\"><td>All together</td><td>{}</td><td colspan=\"7\"></td></tr>",
                mbps(run.together_bps)
            ));
        }
        checks.push_str("</table>");
    }
    let mut outages = String::new();
    for o in view.outages.iter().rev() {
        let lasted = o.to.map_or("still going on".into(), |to| {
            let m = (to - o.from).max(0) / 60;
            if m < 1 {
                "under a minute".into()
            } else {
                format!("{m} min")
            }
        });
        outages.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(&o.label),
            if o.kind == "sign-in" {
                "Sign-in page in the way"
            } else {
                "No internet"
            },
            esc(&when(o.from)),
            esc(&lasted)
        ));
    }
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Network report</title>
<style>body{{font:15px/1.5 system-ui,sans-serif;max-width:900px;margin:32px auto;padding:0 16px;color:#16181d}}
h1{{font-size:24px;margin:0 0 4px}}p.sub{{color:#5b6070;margin:0 0 24px}}h2{{margin-top:32px;font-size:18px}}h3{{font-size:14px;color:#5b6070;margin:20px 0 6px}}
table{{border-collapse:collapse;width:100%;font-size:13px}}th,td{{text-align:left;padding:6px 8px;border-bottom:1px solid #e3e5ea}}th{{color:#5b6070;font-weight:600}}tr.all td{{font-weight:600}}
.note{{color:#5b6070;font-size:12px;margin-top:28px}}</style></head><body>
<h1>Network report</h1><p class="sub">Made by Fuselane {version} on {made}. Each network measured on its own.</p>
<h2>Checks</h2>{checks}
<h2>Outages</h2>{outage_table}
<p class="note">Download and upload: 8 and 7 seconds over several connections at once to Cloudflare's public speed test, over that network only; the ramp-up and short bursts are left out, as public speed tests do. Latency and jitter: timed connections to 1.1.1.1. Under load: latency while the download runs (bufferbloat, graded A+ to F). Outages are seen by a check every minute, so short drops may be missed.</p>
</body></html>"#,
        made = esc(&when(now())),
        outage_table = if outages.is_empty() {
            "<p>No outages recorded.</p>".to_string()
        } else {
            format!(
                "<table><tr><th>Network</th><th>What</th><th>Started</th><th>Lasted</th></tr>{outages}</table>"
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_lists_checks_and_outages_and_escapes_names() {
        let view = NetCheckView {
            history: vec![CheckRun {
                at: 1_791_600_000,
                results: vec![NetResult {
                    label: "Wi-Fi <home>".into(),
                    down_bps: Some(12_500_000.0),
                    idle_ms: Some(18.0),
                    loaded_ms: Some(240.0),
                    grade: Some("D".into()),
                    ..NetResult::default()
                }],
                together_bps: Some(20_000_000.0),
            }],
            outages: vec![Outage {
                label: "Ethernet".into(),
                kind: "offline".into(),
                from: 1_791_600_000,
                to: Some(1_791_600_000 + 7 * 60),
                ..Outage::default()
            }],
            ..NetCheckView::default()
        };
        let html = report_html(&view, "0.1.0");
        assert!(html.contains("Wi-Fi &lt;home&gt;"));
        assert!(html.contains("100.0 Mbps"), "12.5 MB/s is 100 Mbps");
        assert!(html.contains("160.0 Mbps"), "together");
        assert!(html.contains("240 ms (D)"));
        assert!(html.contains("7 min"));
        assert!(!html.contains("<home>"));
    }
}
