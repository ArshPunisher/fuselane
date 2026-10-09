//! Network check (B10.1) in the app: runs `core::netcheck` on every network,
//! shows progress live, keeps the last checks, logs outages from the regular
//! reach checks, and writes a dated report people can send their internet
//! provider as proof.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use fuselane_core::netcheck;
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
    pub idle_ms: Option<f64>,
    pub jitter_ms: Option<f64>,
    /// Share of connects that got no answer, 0..=1.
    pub loss: Option<f64>,
    pub loaded_ms: Option<f64>,
    /// Bufferbloat grade, A+ to F.
    pub grade: Option<String>,
    pub dns_ms: Option<f64>,
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
    pub cancel: Option<fuselane_engine_http::download::Cancel>,
}

fn now() -> i64 {
    super::deadline::unix_now()
}

fn result_from(iface: &fuselane_netif::Interface, m: &netcheck::Measured) -> NetResult {
    NetResult {
        name: iface.name.clone(),
        label: iface.display_name.clone(),
        kind: super::kind_word(iface.kind),
        down_bps: m.down_bps,
        idle_ms: m.idle.map(|l| l.median_ms),
        jitter_ms: m.idle.map(|l| l.jitter_ms),
        loss: m.idle.map(|l| l.loss),
        loaded_ms: m.loaded.map(|l| l.median_ms),
        grade: match (m.idle, m.loaded) {
            (Some(i), Some(l)) => Some(netcheck::bloat_grade(i.median_ms, l.median_ms).to_string()),
            _ => None,
        },
        dns_ms: m.dns_ms,
        problem: m.problem.clone(),
    }
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

    /// Starts a check of every usable network (one at a time, so they don't
    /// slow each other), then all together. `url` is the speed-test file.
    pub fn start_netcheck(self: &Arc<Self>, url: Option<String>) -> Result<NetCheckView, UiError> {
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
            st.cancel = Some(fuselane_engine_http::download::Cancel::new());
        }
        let me = self.clone();
        let url = url.unwrap_or_else(|| netcheck::speed_url(netcheck::SPEED_BYTES));
        tokio::spawn(async move { me.run_netcheck(url).await });
        Ok(self.netcheck_view())
    }

    pub fn cancel_netcheck(&self) {
        let mut st = lock(&self.netcheck);
        if let Some(c) = st.cancel.take() {
            c.cancel();
        }
        st.phase = Some("Stopping…".into());
    }

    fn cancelled(&self) -> bool {
        lock(&self.netcheck).cancel.is_none()
    }

    async fn run_netcheck(self: Arc<Self>, url: String) {
        let nets = self.download_networks().unwrap_or_default();
        let dir = std::env::temp_dir().join("fuselane-netcheck");
        let latency_at = netcheck::LATENCY_TARGET.parse().ok();
        let dns_at = netcheck::DNS_SERVER.parse().ok();
        for iface in &nets {
            if self.cancelled() {
                break;
            }
            lock(&self.netcheck).phase = Some(format!("Testing {}…", iface.display_name));
            self.publish_netcheck();
            let m = match (latency_at, dns_at) {
                (Some(l), Some(d)) => netcheck::measure(iface, &url, l, d, dir.clone()).await,
                _ => netcheck::Measured::default(),
            };
            if let Some(c) = lock(&self.netcheck).current.as_mut() {
                c.results.push(result_from(iface, &m));
            }
            self.publish_netcheck();
        }
        // Every network at once: each downloads its own copy at the same time.
        if nets.len() > 1 && !self.cancelled() {
            lock(&self.netcheck).phase = Some("Testing every network together…".into());
            self.publish_netcheck();
            let mut set = tokio::task::JoinSet::new();
            for iface in nets.iter().filter(|i| {
                lock(&self.netcheck).current.as_ref().is_some_and(|c| {
                    c.results
                        .iter()
                        .any(|r| r.name == i.name && r.down_bps.is_some())
                })
            }) {
                let (url, name, d) = (url.clone(), iface.name.clone(), dir.join(&iface.name));
                set.spawn(async move { netcheck::download_speed(&url, vec![name], d, None).await });
            }
            let mut sum = 0.0;
            let mut any = false;
            while let Some(r) = set.join_next().await {
                if let Ok(Ok(bps)) = r {
                    sum += bps;
                    any = true;
                }
            }
            if let Some(c) = lock(&self.netcheck).current.as_mut() {
                c.together_bps = any.then_some(sum);
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
        let _ = std::fs::remove_dir_all(&dir);
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
        checks.push_str(&format!("<h3>{}</h3><table><tr><th>Network</th><th>Download</th><th>Latency</th><th>Jitter</th><th>Loss</th><th>Under load</th><th>DNS</th><th>Note</th></tr>", esc(&when(run.at))));
        for r in &run.results {
            checks.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}{}</td><td>{}</td><td>{}</td></tr>",
                esc(&r.label),
                mbps(r.down_bps),
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
                "<tr class=\"all\"><td>All together</td><td>{}</td><td colspan=\"6\"></td></tr>",
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
<p class="note">Download speed: a 25 MB file from Cloudflare's public speed test, over that network only. Latency and jitter: timed connections to 1.1.1.1. Under load: latency while the download runs (bufferbloat, graded A+ to F). Outages are seen by a check every minute, so short drops may be missed.</p>
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
