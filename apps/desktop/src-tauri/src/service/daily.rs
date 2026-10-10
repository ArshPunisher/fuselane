//! Data used per network, per day (B10.5): the bytes the allowance meter
//! already counts, kept by day for about two months, so people can see how
//! much each network (the phone above all) carried, without another app.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Service, lock};

/// Days kept.
const KEEP: usize = 62;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayUse {
    /// "2026-10-10"
    pub day: String,
    /// Bytes per network (device name).
    pub nets: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UsageHistory {
    /// Oldest first.
    pub days: Vec<DayUse>,
    /// Device name → what people call it ("en7" → "iPhone USB").
    pub labels: BTreeMap<String, String>,
}

/// Adds `drained` (bytes per network since the last tick) to `day`, keeping
/// the newest `KEEP` days.
pub fn add(days: &mut Vec<DayUse>, day: &str, drained: &[(String, u64)]) {
    if drained.iter().all(|(_, b)| *b == 0) {
        return;
    }
    if days.last().is_none_or(|d| d.day != day) {
        days.push(DayUse {
            day: day.to_string(),
            nets: BTreeMap::new(),
        });
    }
    if let Some(d) = days.last_mut() {
        for (name, bytes) in drained {
            *d.nets.entry(name.clone()).or_insert(0) += bytes;
        }
    }
    let len = days.len();
    if len > KEEP {
        days.drain(..len - KEEP);
    }
}

impl Service {
    pub(super) fn load_daily(store: &fuselane_core::Store) -> Vec<DayUse> {
        store
            .setting("daily_usage")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub(super) fn note_daily(&self, today: &str, drained: &[(String, u64)]) {
        let mut days = lock(&self.daily);
        let before = days.last().map(|d| d.nets.values().sum::<u64>());
        add(&mut days, today, drained);
        if days.last().map(|d| d.nets.values().sum::<u64>()) != before
            && let Ok(json) = serde_json::to_string(&*days)
        {
            let _ = self.store.set_setting("daily_usage", &json);
        }
    }

    pub fn usage_history(&self) -> UsageHistory {
        let mut labels: BTreeMap<String, String> = fuselane_netif::list()
            .unwrap_or_default()
            .into_iter()
            .map(|i| (i.name, i.display_name))
            .collect();
        for p in lock(&self.net_prefs).iter() {
            if let Some(l) = &p.label {
                labels.insert(p.name.clone(), l.clone());
            }
        }
        UsageHistory {
            days: lock(&self.daily).clone(),
            labels,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_add_up_per_day_and_old_days_go() {
        let mut days = vec![];
        add(
            &mut days,
            "2026-10-09",
            &[("en0".into(), 100), ("en7".into(), 5)],
        );
        add(&mut days, "2026-10-09", &[("en0".into(), 50)]);
        add(&mut days, "2026-10-10", &[("en7".into(), 7)]);
        add(&mut days, "2026-10-10", &[("en0".into(), 0)]);
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].nets["en0"], 150);
        assert_eq!(days[0].nets["en7"], 5);
        assert_eq!(
            days[1].nets.get("en0"),
            None,
            "nothing moved, nothing recorded"
        );
        for i in 0..70 {
            add(&mut days, &format!("2027-01-{i:02}"), &[("en0".into(), 1)]);
        }
        assert_eq!(days.len(), KEEP);
        assert_eq!(days.last().unwrap().day, "2027-01-69");
    }
}
