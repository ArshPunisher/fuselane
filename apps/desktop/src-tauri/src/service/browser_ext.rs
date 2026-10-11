//! The browser extension as the app sees it (BROWSER-EXTENSION.md §7): which
//! browsers have it, which are connected, and where their downloads go.
//!
//! "Connected" comes from the extension's own pings (the host adds which store's
//! copy it is); "installed" and "turned off" come from reading the browsers'
//! profile files (`fuselane_api::browsers`). Both are kept as settings so they
//! survive a restart.

use std::sync::Arc;

use serde::Serialize;

use fuselane_api::browsers::{self, ExtState};
use fuselane_api::server::{Handoff, JobAction, Seen};
use fuselane_core::Status;
use serde_json::{Value, json};

use super::deadline::unix_now;
use super::{Service, UiError, lock, store_error};

/// Out of the box: every download goes to the app (REVIEW-2026-10-11 D; size
/// no longer keeps small files in the browser).
pub const DEFAULT_HANDOFF: Handoff = Handoff {
    enabled: true,
    min_bytes: 0,
};

/// A ping older than this no longer counts as connected (the browser may have
/// lost the extension, or not run for a while).
const CONNECTED_FOR: i64 = 30 * 86_400;

/// One browser on the Browser extension page.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrowserView {
    pub id: String,
    pub name: String,
    /// "missing", "off" or "on".
    pub extension: &'static str,
    /// The extension has talked to this app (within the last 30 days).
    pub connected: bool,
    /// When it last did (unix seconds).
    pub last_seen: Option<i64>,
    /// The extension's version then.
    pub version: Option<String>,
    /// Where it installs from: "chrome", "edge" or "firefox".
    pub store: &'static str,
}

/// What a ping left behind: when, which store's copy, which version.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ping {
    at: i64,
    version: String,
}

fn parse_ping(s: &str) -> Option<Ping> {
    let mut parts = s.splitn(3, '|');
    let at = parts.next()?.parse().ok()?;
    let _store = parts.next()?;
    let version = parts.next()?.to_string();
    Some(Ping { at, version })
}

/// Joins what the profile files say with what the pings say, for one browser.
fn view(known: &browsers::Known, state: ExtState, ping: Option<Ping>, now: i64) -> BrowserView {
    let recent = ping.as_ref().is_some_and(|p| now - p.at < CONNECTED_FOR);
    // The profile files win when they say it's off; when they don't mention it
    // (an unusual profile folder) a recent ping still proves it's there.
    let state = match state {
        ExtState::Missing if recent => ExtState::On,
        s => s,
    };
    BrowserView {
        id: known.id.to_string(),
        name: known.name.to_string(),
        extension: state.word(),
        connected: recent && state == ExtState::On,
        last_seen: ping.as_ref().map(|p| p.at),
        version: ping.map(|p| p.version).filter(|v| !v.is_empty()),
        store: known.store.word(),
    }
}

impl Service {
    /// The extension checked in from a browser.
    pub fn extension_seen(&self, seen: Seen) {
        if browsers::known(&seen.browser).is_none() {
            return;
        }
        let value = format!("{}|{}|{}", unix_now(), seen.store, seen.version);
        // Losing one check-in is harmless; the next ping writes it again.
        let _ = self
            .store
            .set_setting(&format!("ext_seen.{}", seen.browser), &value);
    }

    fn last_ping(&self, browser: &str) -> Option<Ping> {
        self.store
            .setting(&format!("ext_seen.{browser}"))
            .ok()
            .flatten()
            .and_then(|s| parse_ping(&s))
    }

    /// Every browser on this computer (and any that checked in), with the
    /// extension's state in each.
    pub fn extension_browsers(&self) -> Vec<BrowserView> {
        let found = browsers::bases()
            .map(|(local, roaming)| browsers::scan(&local, &roaming))
            .unwrap_or_default();
        let now = unix_now();
        browsers::KNOWN
            .iter()
            .filter_map(|k| {
                let ping = self.last_ping(k.id);
                match found.iter().find(|f| f.known.id == k.id) {
                    Some(f) => Some(view(k, f.state, ping, now)),
                    None if ping.is_some() => Some(view(k, ExtState::Missing, ping, now)),
                    None => None,
                }
            })
            .collect()
    }

    /// Where the browser's downloads go.
    pub fn handoff(&self) -> Handoff {
        let enabled = self.store.setting("handoff_enabled").ok().flatten();
        let min = self.store.setting("handoff_min_bytes").ok().flatten();
        Handoff {
            enabled: enabled.map_or(DEFAULT_HANDOFF.enabled, |s| s != "false"),
            min_bytes: min
                .and_then(|s| s.parse().ok())
                .filter(|&n| n <= Handoff::MAX_MIN_BYTES)
                .unwrap_or(DEFAULT_HANDOFF.min_bytes),
        }
    }

    pub fn set_handoff(&self, h: Handoff) -> Result<Handoff, UiError> {
        if h.min_bytes > Handoff::MAX_MIN_BYTES {
            return Err(UiError::new(
                "invalid",
                "That size is too big.",
                Some("Pick a size up to 1 TB."),
            ));
        }
        self.store
            .set_setting("handoff_enabled", if h.enabled { "true" } else { "false" })
            .map_err(store_error)?;
        self.store
            .set_setting("handoff_min_bytes", &h.min_bytes.to_string())
            .map_err(store_error)?;
        Ok(h)
    }
}

/// Finished and failed downloads the popup lists after the active ones.
const POPUP_RECENT: usize = 30;

/// A browser offer for a link added this recently is the same click again
/// (a page that starts its download twice, or the extension asking again after
/// a slow answer), not a request for another copy.
const SAME_CLICK_SECS: i64 = 120;

impl Service {
    /// Adds a download offered by the browser, unless the same link is already
    /// downloading or was added moments ago: then that download's id comes back
    /// and nothing new is made. A link that finished a while ago is downloaded
    /// again, since asking in the browser once more is deliberate.
    pub fn add_offer(self: &Arc<Self>, url: &str, req: &super::AddRequest) -> Result<i64, UiError> {
        let _held = lock(&self.offer_lock);
        let now = unix_now();
        let jobs = self.store.list().map_err(store_error)?;
        let same = jobs.iter().filter(|j| j.url == url.trim()).find(|j| {
            matches!(j.status, Status::Queued | Status::Running | Status::Paused)
                || now - j.created_at < SAME_CLICK_SECS
        });
        if let Some(j) = same {
            return Ok(j.id);
        }
        self.add_with(
            url,
            None,
            &super::AddRequest {
                allow_duplicate: true,
                ..req.clone()
            },
        )
    }

    /// The downloads for the extension's popup: everything active, then the
    /// latest finished ones, each with its live speed; plus the combined speed.
    pub fn extension_jobs(&self) -> Result<Value, UiError> {
        let mut jobs = self.jobs()?;
        let running: std::collections::HashSet<i64> = lock(&self.running).keys().copied().collect();
        {
            // Speeds of downloads that stopped are stale.
            let mut rates = lock(&self.rates);
            rates.retain(|id, _| running.contains(id));
        }
        let rates = lock(&self.rates).clone();
        let active = |s: &str| matches!(s, "running" | "queued" | "paused");
        jobs.sort_by_key(|j| {
            (
                !active(j.status),
                if active(j.status) {
                    j.position
                } else {
                    -j.created_at
                },
            )
        });
        let mut recent = 0;
        let list: Vec<Value> = jobs
            .iter()
            .filter(|j| {
                if active(j.status) {
                    return true;
                }
                recent += 1;
                recent <= POPUP_RECENT
            })
            .map(|j| {
                json!({
                    "id": j.id,
                    "name": j.name,
                    "status": j.status,
                    "written": j.written,
                    "total": j.total,
                    "rate": rates.get(&j.id).copied().unwrap_or(0.0).round(),
                    "error": j.error,
                    "file": j.final_path.is_some(),
                })
            })
            .collect();
        let rate: f64 = rates.values().sum();
        Ok(json!({"jobs": list, "rate": rate.round()}))
    }

    /// Does what the popup asked to download `id`.
    pub fn extension_action(self: &Arc<Self>, id: i64, action: JobAction) -> Result<(), UiError> {
        match action {
            JobAction::Pause => self.pause(id),
            JobAction::Resume => self.resume(id),
            JobAction::Cancel | JobAction::Remove => self.remove(id),
            JobAction::Delete => self.trash_file(id),
            JobAction::Open => {
                let path = self.finished_file(id)?;
                let open = lock(&self.opener).clone();
                open(&path);
                Ok(())
            }
            JobAction::Reveal => {
                let path = self.finished_file(id)?;
                let reveal = lock(&self.revealer).clone();
                reveal(&path);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge() -> &'static browsers::Known {
        browsers::known("edge").unwrap()
    }

    #[test]
    fn a_ping_is_read_back_and_junk_is_ignored() {
        assert_eq!(
            parse_ping("100|edge|0.3.0"),
            Some(Ping {
                at: 100,
                version: "0.3.0".into()
            })
        );
        assert_eq!(parse_ping("x|edge|1"), None);
        assert_eq!(parse_ping("100"), None);
    }

    #[test]
    fn connected_needs_a_recent_ping_and_the_extension_turned_on() {
        let now = 10 * CONNECTED_FOR;
        let fresh = || {
            Some(Ping {
                at: now - 60,
                version: "0.3.0".into(),
            })
        };
        let v = view(edge(), ExtState::On, fresh(), now);
        assert!(v.connected);
        assert_eq!(v.version.as_deref(), Some("0.3.0"));
        // Turned off in the browser since: not connected, even with a fresh ping.
        let v = view(edge(), ExtState::Off, fresh(), now);
        assert_eq!((v.extension, v.connected), ("off", false));
        // An old ping: installed, but not counted as connected.
        let old = Some(Ping {
            at: now - CONNECTED_FOR - 1,
            version: String::new(),
        });
        let v = view(edge(), ExtState::On, old, now);
        assert!(!v.connected);
        assert_eq!(v.version, None);
        // Not in the profile files, but it pinged today: it's there.
        let v = view(edge(), ExtState::Missing, fresh(), now);
        assert_eq!((v.extension, v.connected), ("on", true));
        // Never seen anywhere.
        let v = view(edge(), ExtState::Missing, None, now);
        assert_eq!(
            (v.extension, v.connected, v.store),
            ("missing", false, "edge")
        );
    }
}
