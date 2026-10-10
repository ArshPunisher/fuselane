//! The phone only when it's worth it (B9.5). Each network can be used always,
//! only for long downloads, or never. "Long" means the download would take more
//! than a set number of minutes without those networks. A download starts
//! without them; once its speed shows it's long, it carries on with them.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{NetPref, Service, UiError, lock, store_error};

/// When a network helps.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NetUse {
    #[default]
    Always,
    /// Only for downloads that would take longer than the set minutes without it.
    Long,
    Never,
}

/// Default for "long": five minutes.
pub const LONG_MINUTES: u32 = 5;
/// How long a download runs before its speed is trusted for the decision.
pub const SETTLE_SECS: u64 = 8;

/// Splits the networks for one download: the ones to use now, and the names
/// held back for long downloads. "Never" networks are left out, unless leaving
/// them out would leave nothing at all (then the download still runs).
pub fn split_by_use(
    all: Vec<fuselane_netif::Interface>,
    prefs: &[NetPref],
    long: bool,
) -> (Vec<fuselane_netif::Interface>, Vec<String>) {
    let use_of = |name: &str| {
        prefs
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.use_for)
            .unwrap_or_default()
    };
    let (mut now, mut held) = (vec![], vec![]);
    for i in &all {
        match use_of(&i.name) {
            NetUse::Always => now.push(i.clone()),
            NetUse::Long if long => now.push(i.clone()),
            NetUse::Long => held.push(i.clone()),
            NetUse::Never => {}
        }
    }
    if now.is_empty() {
        // Nothing else reaches the internet: better slow than not at all.
        now = std::mem::take(&mut held);
    }
    if now.is_empty() {
        now = all;
    }
    (now, held.into_iter().map(|i| i.name).collect())
}

/// Whether `remaining` bytes at `rate` bytes/s take longer than `minutes`;
/// None when the speed isn't known yet.
pub fn is_long(remaining: u64, rate: f64, minutes: u32) -> Option<bool> {
    (rate.is_finite() && rate > 0.0).then(|| remaining as f64 / rate > f64::from(minutes) * 60.0)
}

impl Service {
    pub fn long_minutes(&self) -> u32 {
        self.long_minutes.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// What "long" means, in minutes (1–600).
    pub fn set_long_minutes(&self, minutes: u32) -> Result<u32, UiError> {
        if !(1..=600).contains(&minutes) {
            return Err(UiError::new(
                "bad-value",
                "Pick between 1 and 600 minutes.",
                None,
            ));
        }
        self.store
            .set_setting("long_minutes", &minutes.to_string())
            .map_err(store_error)?;
        self.long_minutes
            .store(minutes, std::sync::atomic::Ordering::Relaxed);
        Ok(minutes)
    }

    /// The networks a download starts with, and the ones held back for long
    /// downloads. A download already known to be long gets them all; one whose
    /// size is known is judged from the networks' recent speeds.
    pub(super) fn networks_by_use(
        &self,
        job: &fuselane_core::Job,
        all: Vec<fuselane_netif::Interface>,
    ) -> (Vec<fuselane_netif::Interface>, Vec<String>) {
        let mut prefs = lock(&self.net_prefs).clone();
        // Outside its hours a network counts as Never (B10.6).
        let minute = self.now().minute;
        for p in &mut prefs {
            if p.hours.is_some_and(|h| !h.contains(minute)) {
                p.use_for = NetUse::Never;
            }
        }
        let long = lock(&self.known_long).contains(&job.id);
        let (now, held) = split_by_use(all.clone(), &prefs, long);
        if held.is_empty() {
            return (now, held);
        }
        let rates = lock(&self.net_rates).clone();
        let rate: f64 = now.iter().filter_map(|i| rates.get(&i.name)).sum();
        let remaining = job.total.map(|t| t.saturating_sub(job.secured_bytes()));
        if remaining.and_then(|r| is_long(r, rate, self.long_minutes())) == Some(true) {
            lock(&self.known_long).insert(job.id);
            return split_by_use(all, &prefs, true);
        }
        (now, held)
    }

    /// Keeps a running average of each network's speed (from live snapshots),
    /// so the next download can be judged before it starts.
    pub(super) fn note_rates(&self, nets: &[super::LiveNet]) {
        let mut rates = lock(&self.net_rates);
        for n in nets.iter().filter(|n| n.rate.is_finite() && n.rate > 0.0) {
            let r = rates.entry(n.name.clone()).or_insert(n.rate);
            *r = *r * 0.8 + n.rate * 0.2;
        }
    }

    /// Watches a download that started without its "long only" networks: once
    /// its speed settles and shows it's long, it restarts with them (resuming
    /// exactly where it was).
    pub(super) fn watch_if_long(
        self: &Arc<Self>,
        id: i64,
        last: Arc<std::sync::Mutex<Option<super::Live>>>,
    ) {
        let me = self.clone();
        tokio::spawn(async move {
            let started = std::time::Instant::now();
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if !lock(&me.running).contains_key(&id) {
                    return;
                }
                if started.elapsed().as_secs() < SETTLE_SECS {
                    continue;
                }
                let Some(l) = lock(&last).clone() else {
                    continue;
                };
                let Some(total) = l.total else {
                    continue;
                };
                let remaining = total.saturating_sub(l.written);
                if is_long(remaining, l.rate, me.long_minutes()) == Some(true) {
                    lock(&me.known_long).insert(id);
                    if let Some(r) = lock(&me.running).get_mut(&id) {
                        r.replan = true;
                        r.cancel.cancel();
                    }
                    return;
                }
            }
        });
    }
}

/// Saved preferences keep their old shape: a missing `useFor` means Always.
pub(super) fn keep(pref: &NetPref) -> bool {
    pref.label.is_some()
        || pref.lane.is_some()
        || pref.use_for != NetUse::Always
        || pref.hours.is_some()
        || pref.proxy.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuselane_netif::{Interface, Kind};

    fn iface(name: &str) -> Interface {
        Interface {
            name: name.into(),
            display_name: name.into(),
            index: 1,
            kind: Kind::Wifi,
            addrs: vec![],
        }
    }

    fn pref(name: &str, use_for: NetUse) -> NetPref {
        NetPref {
            name: name.into(),
            label: None,
            lane: None,
            use_for,
            hours: None,
            proxy: None,
        }
    }

    fn names(v: &[Interface]) -> Vec<&str> {
        v.iter().map(|i| i.name.as_str()).collect()
    }

    #[test]
    fn long_only_networks_wait_for_long_downloads_and_never_means_never() {
        let all = vec![iface("en0"), iface("en7"), iface("en5")];
        let prefs = [pref("en7", NetUse::Long), pref("en5", NetUse::Never)];
        let (now, held) = split_by_use(all.clone(), &prefs, false);
        assert_eq!(names(&now), ["en0"]);
        assert_eq!(held, ["en7"]);
        let (now, held) = split_by_use(all, &prefs, true);
        assert_eq!(names(&now), ["en0", "en7"]);
        assert!(held.is_empty());
    }

    #[test]
    fn a_download_never_ends_up_with_no_network() {
        let prefs = [pref("en7", NetUse::Long), pref("en5", NetUse::Never)];
        // Only the phone left: it's used even for a short download.
        let (now, _) = split_by_use(vec![iface("en7")], &prefs, false);
        assert_eq!(names(&now), ["en7"]);
        // Only a "never" network left: still better than nothing.
        let (now, _) = split_by_use(vec![iface("en5")], &prefs, false);
        assert_eq!(names(&now), ["en5"]);
    }

    #[test]
    fn long_means_longer_than_the_set_minutes_at_the_current_speed() {
        assert_eq!(is_long(600 * 1_000_000, 1_000_000.0, 5), Some(true)); // 10 min
        assert_eq!(is_long(240 * 1_000_000, 1_000_000.0, 5), Some(false)); // 4 min
        assert_eq!(is_long(1, 0.0, 5), None);
        assert_eq!(is_long(1, f64::NAN, 5), None);
    }
}
