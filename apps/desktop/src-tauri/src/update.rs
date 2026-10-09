//! The app's own update (B8.4): downloaded with Fuselane's engine over every
//! network, with progress for the window, then checked against the release key
//! before it is installed. The updater plugin only verifies what it downloads
//! itself, so bytes fetched here go through the same minisign check first.

use std::time::Instant;

use base64::Engine;
use serde::Serialize;

/// Where an update stands, for the banner.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    /// downloading | installing
    pub phase: &'static str,
    pub done: u64,
    pub total: Option<u64>,
    /// Bytes per second, smoothed.
    pub rate: u64,
    /// How many networks are carrying it (1 when the fallback download is used).
    pub networks: u32,
}

fn decode(b64: &str, what: &str) -> Result<String, String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| format!("the {what} isn't valid base64"))?;
    String::from_utf8(raw).map_err(|_| format!("the {what} isn't text"))
}

/// The `version:` field of a signature's trusted comment (tab-separated
/// `key:value` pairs written by the Tauri CLI), if it has one.
pub fn signed_version(trusted_comment: &str) -> Option<&str> {
    trusted_comment
        .split('\t')
        .find_map(|field| field.strip_prefix("version:"))
}

fn same_version(signed: &str, announced: &str) -> bool {
    match (
        semver::Version::parse(signed.trim_start_matches('v')),
        semver::Version::parse(announced.trim_start_matches('v')),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => signed == announced,
    }
}

/// Checks `bytes` against the release signature and public key (both base64, as
/// in the update feed and tauri.conf.json), and that a version recorded in the
/// signature is the one the feed announced. The same rules as the updater plugin.
pub fn verify(bytes: &[u8], signature: &str, pubkey: &str, announced: &str) -> Result<(), String> {
    let key = minisign_verify::PublicKey::decode(&decode(pubkey, "update key")?)
        .map_err(|e| format!("the update key can't be read: {e}"))?;
    let sig = minisign_verify::Signature::decode(&decode(signature, "update signature")?)
        .map_err(|e| format!("the update signature can't be read: {e}"))?;
    key.verify(bytes, &sig, true)
        .map_err(|_| "the downloaded update isn't signed by Fuselane's release key".to_string())?;
    // Only trusted after verify(): the global signature covers the comment.
    match signed_version(sig.trusted_comment()) {
        Some(v) if !same_version(v, announced) => Err(format!(
            "the update was signed for version {v}, but {announced} was announced"
        )),
        _ => Ok(()),
    }
}

/// Turns byte counts into a steady speed for the banner and limits how often
/// the window hears about it (four times a second).
#[derive(Debug)]
pub struct Meter {
    last: Option<(Instant, u64)>,
    rate: f64,
    sent: Option<Instant>,
}

impl Default for Meter {
    fn default() -> Self {
        Self::new()
    }
}

impl Meter {
    pub fn new() -> Self {
        Self {
            last: None,
            rate: 0.0,
            sent: None,
        }
    }

    /// Records `done` at `now`; returns the smoothed rate when it's time to report.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn tick(&mut self, now: Instant, done: u64, finished: bool) -> Option<u64> {
        if let Some((at, was)) = self.last {
            let dt = now.duration_since(at).as_secs_f64();
            if dt >= 0.2 {
                let r = done.saturating_sub(was) as f64 / dt;
                self.rate = if self.rate == 0.0 {
                    r
                } else {
                    self.rate * 0.6 + r * 0.4
                };
                self.last = Some((now, done));
            }
        } else {
            self.last = Some((now, done));
        }
        let due = self
            .sent
            .is_none_or(|s| now.duration_since(s).as_millis() >= 250);
        if due || finished {
            self.sent = Some(now);
            Some(self.rate as u64)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const PAYLOAD: &[u8] = include_bytes!("update_fixtures/payload.bin");
    const SIG: &str = include_str!("update_fixtures/payload.bin.sig");
    const KEY: &str = include_str!("update_fixtures/test.key.pub");

    #[test]
    fn a_signed_update_passes_and_anything_else_fails() {
        assert_eq!(verify(PAYLOAD, SIG, KEY, "0.1.0-beta.8"), Ok(()));
        // One byte changed: refused.
        let mut tampered = PAYLOAD.to_vec();
        tampered[0] ^= 1;
        assert!(
            verify(&tampered, SIG, KEY, "0.1.0-beta.8")
                .unwrap_err()
                .contains("isn't signed")
        );
        // Signed by a different key (the real release key): refused.
        let release_key = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEQxOUE3RThGOUJFNTU3ODIKUldTQ1YrV2JqMzZhMFhnQmppRXB4TkpGTkt0Z0Jna3NFWnVWYlU2NFhnYUhORlVVUHE0VXprV2wK";
        assert!(verify(PAYLOAD, SIG, release_key, "0.1.0-beta.8").is_err());
        // Garbage in either field is an error, not a panic.
        assert!(verify(PAYLOAD, "not base64!", KEY, "1").is_err());
        assert!(verify(PAYLOAD, SIG, "bm90IGEga2V5", "1").is_err());
    }

    #[test]
    fn a_version_in_the_signature_must_match_the_feed() {
        let tc = "timestamp:1700000000\tfile:Fuselane.app.tar.gz\tversion:0.1.0-beta.8";
        assert_eq!(signed_version(tc), Some("0.1.0-beta.8"));
        assert_eq!(signed_version("timestamp:1\tfile:x"), None);
        assert!(same_version("v0.1.0-beta.8", "0.1.0-beta.8"));
        assert!(!same_version("0.1.0-beta.7", "0.1.0-beta.8"));
        assert!(same_version("weird", "weird"));
    }

    #[test]
    fn the_meter_smooths_and_reports_four_times_a_second() {
        let t0 = Instant::now();
        let mut m = Meter::new();
        assert_eq!(m.tick(t0, 0, false), Some(0), "first report right away");
        assert_eq!(
            m.tick(t0 + Duration::from_millis(100), 100_000, false),
            None
        );
        let r = m
            .tick(t0 + Duration::from_millis(300), 1_000_000, false)
            .unwrap();
        assert!((3_000_000..=3_400_000).contains(&r), "{r}");
        // The last report always goes out.
        assert!(
            m.tick(t0 + Duration::from_millis(310), 1_100_000, true)
                .is_some()
        );
    }
}
