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

/// One way to read a version, for every comparison: whitespace and one leading `v` dropped,
/// build metadata ignored. None if it isn't semver.
fn parse_version(v: &str) -> Option<semver::Version> {
    let v = v.trim();
    let mut parsed = semver::Version::parse(v.strip_prefix('v').unwrap_or(v)).ok()?;
    parsed.build = semver::BuildMetadata::EMPTY;
    Some(parsed)
}

/// Refuses an update that isn't newer than the running app (semver, build metadata ignored).
/// The feed's own say-so is not enough: a replayed old feed must never roll the app back or
/// reinstall what it already is (SECURITY.md T1). A version that can't be read on either side
/// counts as not newer. (The feed's version is unsigned: `verify` also demands the version
/// inside the signature, so an old genuine package can't be passed off under a made-up number.)
pub fn ensure_newer(current: &str, announced: &str) -> Result<(), String> {
    match (parse_version(current), parse_version(announced)) {
        (Some(cur), Some(new)) if new > cur => Ok(()),
        (Some(_), Some(_)) => Err(format!(
            "version {announced} isn't newer than the {current} that is running"
        )),
        _ => Err(format!(
            "version {announced} can't be compared with the {current} that is running"
        )),
    }
}

fn same_version(signed: &str, announced: &str) -> bool {
    matches!((parse_version(signed), parse_version(announced)), (Some(a), Some(b)) if a == b)
}

/// The rules for the version a signature carries: it must have one (the release pipeline
/// always writes it), it must be the one the feed announced, and it must be newer than the
/// running app. The feed's own version is unsigned, so without this an old genuine package could
/// be served under a made-up newer number and rolled back to (SECURITY.md T1).
pub fn check_signed_version(
    trusted_comment: &str,
    announced: &str,
    current: &str,
) -> Result<(), String> {
    let signed = signed_version(trusted_comment)
        .ok_or("the update's signature doesn't say which version it is")?;
    if !same_version(signed, announced) {
        return Err(format!(
            "the update was signed for version {signed}, but {announced} was announced"
        ));
    }
    ensure_newer(current, signed)
}

/// Checks `bytes` against the release signature and public key (both base64, as in the
/// update feed and tauri.conf.json): the same minisign rules as the updater plugin. Returns
/// the signature's trusted comment, which the signature covers.
fn verify_signature(bytes: &[u8], signature: &str, pubkey: &str) -> Result<String, String> {
    let key = minisign_verify::PublicKey::decode(&decode(pubkey, "update key")?)
        .map_err(|e| format!("the update key can't be read: {e}"))?;
    let sig = minisign_verify::Signature::decode(&decode(signature, "update signature")?)
        .map_err(|e| format!("the update signature can't be read: {e}"))?;
    key.verify(bytes, &sig, true)
        .map_err(|_| "the downloaded update isn't signed by Fuselane's release key".to_string())?;
    Ok(sig.trusted_comment().to_string())
}

/// The one gate every downloaded update passes: signed by the release key, and for the version
/// the feed announced, which must be newer than `current`, the running app's.
pub fn verify(
    bytes: &[u8],
    signature: &str,
    pubkey: &str,
    announced: &str,
    current: &str,
) -> Result<(), String> {
    // Only trusted after the signature checks out: it covers the comment.
    let comment = verify_signature(bytes, signature, pubkey)?;
    check_signed_version(&comment, announced, current)
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
        assert!(verify_signature(PAYLOAD, SIG, KEY).is_ok());
        // One byte changed: refused.
        let mut tampered = PAYLOAD.to_vec();
        tampered[0] ^= 1;
        assert!(
            verify_signature(&tampered, SIG, KEY)
                .unwrap_err()
                .contains("isn't signed")
        );
        // Signed by a different key (the real release key): refused.
        let release_key = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEQxOUE3RThGOUJFNTU3ODIKUldTQ1YrV2JqMzZhMFhnQmppRXB4TkpGTkt0Z0Jna3NFWnVWYlU2NFhnYUhORlVVUHE0VXprV2wK";
        assert!(verify_signature(PAYLOAD, SIG, release_key).is_err());
        // Garbage in either field is an error, not a panic.
        assert!(verify_signature(PAYLOAD, "not base64!", KEY).is_err());
        assert!(verify_signature(PAYLOAD, SIG, "bm90IGEga2V5").is_err());
    }

    /// T1: a feed (or anyone in front of it) must not be able to roll the app back, or
    /// reinstall what it already is, whatever the feed says.
    #[test]
    fn only_a_newer_version_than_the_running_one_is_accepted() {
        let ok = |cur: &str, new: &str| ensure_newer(cur, new).is_ok();
        assert!(ok("1.0.1", "1.0.2"));
        assert!(ok("1.0.1", "1.1.0"));
        assert!(ok("1.0.1", "v2.0.0"), "a leading v is the feed's habit");
        assert!(
            ok("1.0.0-rc.14", "1.0.0"),
            "a release is newer than its release candidate"
        );
        assert!(
            ok("0.1.0-beta.9", "0.1.0-beta.10"),
            "numbers, not text, order pre-releases"
        );
        assert!(!ok("1.0.1", "1.0.1"), "the same version");
        assert!(!ok("1.0.1", "1.0.0"), "older");
        assert!(!ok("1.0.1", "0.9.9"));
        assert!(
            !ok("1.0.0", "1.0.0-rc.14"),
            "a candidate is older than its release"
        );
        assert!(!ok("1.0.1", "v1.0.1"));
        // Build metadata doesn't make a version newer.
        assert!(!ok("1.0.1", "1.0.1+build.7"));
        // Not versions at all: nothing to compare, so no install.
        assert!(!ok("1.0.1", "latest"));
        assert!(!ok("1.0.1", ""));
        assert!(
            !ok("weird", "1.0.2"),
            "a running version that can't be read isn't guessed at"
        );
        let why = ensure_newer("1.0.1", "1.0.0").unwrap_err();
        assert!(why.contains("1.0.0") && why.contains("1.0.1"), "{why}");
    }

    #[test]
    fn a_version_in_the_signature_must_match_the_feed() {
        let tc = "timestamp:1700000000\tfile:Fuselane.app.tar.gz\tversion:0.1.0-beta.8";
        assert_eq!(signed_version(tc), Some("0.1.0-beta.8"));
        assert_eq!(signed_version("timestamp:1\tfile:x"), None);
        assert!(same_version("v0.1.0-beta.8", "0.1.0-beta.8"));
        assert!(!same_version("0.1.0-beta.7", "0.1.0-beta.8"));
        // Anything that isn't a version on either side is not "the same".
        assert!(!same_version("weird", "weird"));
        // Whitespace and build metadata don't make two versions differ, as in ensure_newer.
        assert!(same_version(" v1.0.2+ci.42 ", "1.0.2"));
    }

    /// T1: a genuine old package re-served under a made-up newer version number. The version
    /// inside the signature can't be forged, so that is the one that counts.
    #[test]
    fn the_version_inside_the_signature_decides() {
        let tc = |v: &str| format!("timestamp:1795291375\tfile:Fuselane.app.tar.gz\tversion:{v}");
        // The honest case.
        assert_eq!(check_signed_version(&tc("1.0.2"), "1.0.2", "1.0.1"), Ok(()));
        assert_eq!(
            check_signed_version(&tc("v1.0.2"), "1.0.2", "1.0.1"),
            Ok(())
        );
        // The attack: the old signed package (1.0.0) under a feed that says 9.9.9.
        let why = check_signed_version(&tc("1.0.0"), "9.9.9", "1.0.1").unwrap_err();
        assert!(why.contains("signed for version 1.0.0"), "{why}");
        // The feed honest about an old package: not newer, so refused all the same.
        assert!(check_signed_version(&tc("1.0.0"), "1.0.0", "1.0.1").is_err());
        assert!(check_signed_version(&tc("1.0.1"), "1.0.1", "1.0.1").is_err());
        // A signature that doesn't say which version it is can't be trusted to be a new one.
        let why = check_signed_version("timestamp:1\tfile:x", "9.9.9", "1.0.1").unwrap_err();
        assert!(why.contains("doesn't say which version"), "{why}");
        // The whole gate: a validly signed package without a version is refused.
        let why = verify(PAYLOAD, SIG, KEY, "9.9.9", "1.0.1").unwrap_err();
        assert!(why.contains("doesn't say which version"), "{why}");
        // And a tampered one is refused before the version is even read.
        let mut tampered = PAYLOAD.to_vec();
        tampered[0] ^= 1;
        assert!(
            verify(&tampered, SIG, KEY, "9.9.9", "1.0.1")
                .unwrap_err()
                .contains("isn't signed")
        );
    }

    #[test]
    fn the_feed_and_the_check_read_versions_the_same_way() {
        assert!(ensure_newer("1.0.1", " v1.0.2 ").is_ok());
        assert!(ensure_newer("1.0.1+build.1", "1.0.2").is_ok());
        assert!(ensure_newer("1.0.1+build.1", "1.0.1+build.2").is_err());
        assert!(
            ensure_newer("1.0.1", "vv1.0.2").is_err(),
            "one v, not any number"
        );
        assert!(
            ensure_newer("1.0.1", "1.0.2-rc.1").is_ok(),
            "semver: later than 1.0.1"
        );
    }

    /// The real thing: the published package from the live feed, fetched with
    /// the bonded engine and checked against the release key in tauri.conf.json.
    /// Needs the internet: `cargo nextest run -p fuselane-desktop --run-ignored only real_feed`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "downloads the published update package"]
    async fn the_real_feed_package_verifies() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let key = conf["plugins"]["updater"]["pubkey"].as_str().unwrap();
        let feed = fuselane_core::runner::fetch(
            "https://arshpunisher.github.io/fuselane/updates/latest.json",
            tempfile::tempdir().unwrap().keep(),
            fuselane_core::runner::RunOptions::default(),
        )
        .await
        .unwrap();
        let feed: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&feed.path).unwrap()).unwrap();
        let version = feed["version"].as_str().unwrap();
        let p = &feed["platforms"]["darwin-aarch64"];
        let dir = tempfile::tempdir().unwrap();
        let got = fuselane_core::runner::fetch(
            p["url"].as_str().unwrap(),
            dir.path().to_path_buf(),
            fuselane_core::runner::RunOptions::default(),
        )
        .await
        .unwrap();
        let bytes = std::fs::read(&got.path).unwrap();
        assert_eq!(
            verify(
                &bytes,
                p["signature"].as_str().unwrap(),
                key,
                version,
                "0.0.0"
            ),
            Ok(())
        );
        let mut bad = bytes.clone();
        let mid = bad.len() / 2;
        bad[mid] ^= 0xff;
        assert!(
            verify(
                &bad,
                p["signature"].as_str().unwrap(),
                key,
                version,
                "0.0.0"
            )
            .is_err()
        );
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
