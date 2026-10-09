//! What each network saved (B9.6). After a download, Fuselane says how long
//! it would have taken without each network: "Without iPhone USB it would have
//! taken about 6 min (4 min saved)". The estimate assumes the other networks
//! keep their average speed from this download.

use serde::{Deserialize, Serialize};

/// What a finished run did, kept with the download.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    /// How long this run took, in seconds.
    pub secs: f64,
    pub nets: Vec<NetBytes>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetBytes {
    pub label: String,
    pub bytes: u64,
}

/// One network's part, as the window shows it.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetSaving {
    pub label: String,
    pub bytes: u64,
    /// Seconds saved by having it; None when it carried everything (nothing to
    /// compare with) or the run was too short to say.
    pub saved_secs: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReportView {
    pub secs: f64,
    pub nets: Vec<NetSaving>,
}

/// Runs shorter than this say nothing about savings (the start dominates).
const MIN_SECS: f64 = 3.0;

/// Without network k, the others move the same bytes at their own average
/// speed: `T × total / (total − bytes_k)`, so k saved `T × bytes_k / (total − bytes_k)`.
pub fn view(r: &RunReport) -> ReportView {
    let total: u64 = r.nets.iter().map(|n| n.bytes).sum();
    let nets = r
        .nets
        .iter()
        .filter(|n| n.bytes > 0)
        .map(|n| {
            let rest = total.saturating_sub(n.bytes);
            let saved_secs = (r.secs >= MIN_SECS && rest > 0 && total > 0)
                .then(|| r.secs * n.bytes as f64 / rest as f64);
            NetSaving {
                label: n.label.clone(),
                bytes: n.bytes,
                saved_secs,
            }
        })
        .collect();
    ReportView { secs: r.secs, nets }
}

/// "iPhone USB saved 4 min" for the network that saved the most, when it's
/// worth saying (half a minute or more).
pub fn headline(v: &ReportView) -> Option<String> {
    let best = v
        .nets
        .iter()
        .filter_map(|n| n.saved_secs.map(|s| (n, s)))
        .max_by(|a, b| a.1.total_cmp(&b.1))?;
    (best.1 >= 30.0).then(|| format!("{} saved {}", best.0.label, duration(best.1)))
}

/// "45 s", "4 min", "1 h 5 min".
pub fn duration(secs: f64) -> String {
    let s = secs.round().max(0.0) as u64;
    match s {
        0..60 => format!("{s} s"),
        60..3600 => format!("{} min", (s + 30) / 60),
        _ => {
            let m = (s + 30) / 60;
            if m.is_multiple_of(60) {
                format!("{} h", m / 60)
            } else {
                format!("{} h {} min", m / 60, m % 60)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(secs: f64, nets: &[(&str, u64)]) -> RunReport {
        RunReport {
            secs,
            nets: nets
                .iter()
                .map(|(l, b)| NetBytes {
                    label: (*l).into(),
                    bytes: *b,
                })
                .collect(),
        }
    }

    #[test]
    fn each_network_saved_what_the_others_would_have_needed_to_carry_it() {
        // 120 s; Wi-Fi carried 1/3, the phone 2/3. Without the phone, Wi-Fi alone
        // takes 360 s (240 saved); without Wi-Fi, the phone alone takes 180 s (60 saved).
        let v = view(&report(120.0, &[("Wi-Fi", 100), ("iPhone USB", 200)]));
        assert_eq!(v.nets[0].saved_secs, Some(60.0));
        assert_eq!(v.nets[1].saved_secs, Some(240.0));
        assert_eq!(
            headline(&view(&report(
                120.0,
                &[("Wi-Fi", 100), ("iPhone USB", 200)]
            )))
            .as_deref(),
            Some("iPhone USB saved 4 min")
        );
    }

    #[test]
    fn nothing_is_claimed_without_a_comparison() {
        // One network did everything; an idle one is left out; a blink of a run says nothing.
        let v = view(&report(60.0, &[("Wi-Fi", 500), ("Ethernet", 0)]));
        assert_eq!(v.nets.len(), 1);
        assert_eq!(v.nets[0].saved_secs, None);
        assert_eq!(
            view(&report(1.0, &[("a", 1), ("b", 1)])).nets[0].saved_secs,
            None
        );
        // Small savings aren't announced.
        assert_eq!(
            headline(&view(&report(20.0, &[("a", 100), ("b", 100)]))),
            None
        );
        assert_eq!(headline(&view(&report(0.0, &[]))), None);
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration(0.4), "0 s");
        assert_eq!(duration(45.0), "45 s");
        assert_eq!(duration(89.0), "1 min");
        assert_eq!(duration(240.0), "4 min");
        assert_eq!(duration(3600.0), "1 h");
        assert_eq!(duration(3900.0), "1 h 5 min");
    }
}
