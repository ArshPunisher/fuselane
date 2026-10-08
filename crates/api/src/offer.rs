//! Download offers from the browser extension, schema v1, validated exactly as
//! `packages/capture/src/messages.ts` does (BROWSER-EXTENSION.md §3, L-97). Both
//! run the same vectors (`packages/capture/vectors/offers.json`).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

pub const MAX_URL: usize = 8 * 1024;
pub const MAX_FILENAME: usize = 255;
pub const MAX_MIME: usize = 255;
pub const MAX_COOKIES: usize = 16 * 1024;
pub const MAX_HEADERS: usize = 32;
pub const MAX_HEADER_VALUE: usize = 8 * 1024;
pub const MAX_USER_AGENT: usize = 1024;

/// Headers the app sets itself; an offer may not choose them.
const RESERVED: [&str; 6] = [
    "host",
    "range",
    "content-length",
    "connection",
    "transfer-encoding",
    "cookie",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub url: String,
    pub final_url: Option<String>,
    pub referrer: Option<String>,
    pub filename: Option<String>,
    pub mime: Option<String>,
    pub size: Option<u64>,
    pub cookies: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub user_agent: Option<String>,
    pub source: Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    Auto,
    ContextMenu,
}

impl Offer {
    /// True when the browser's session (cookies or credentials) came along; the
    /// app must then fetch with them or hand the download back.
    pub fn needs_session(&self) -> bool {
        self.cookies.as_deref().is_some_and(|c| !c.is_empty()) || !self.headers.is_empty()
    }

    /// What to send with the download so the server sees the same browser session:
    /// cookies, the page it came from, the browser's User-Agent and any extra
    /// headers, as (name, value) pairs. The engine checks them again before use.
    pub fn session_headers(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if let Some(c) = self.cookies.as_deref().filter(|c| !c.is_empty()) {
            out.push(("Cookie".into(), c.into()));
        }
        if let Some(r) = &self.referrer {
            out.push(("Referer".into(), r.clone()));
        }
        if let Some(ua) = self.user_agent.as_deref().filter(|u| !u.is_empty()) {
            out.push(("User-Agent".into(), ua.into()));
        }
        out
    }
}

fn link(v: &Value, what: &str, schemes: &[&str]) -> Result<String, String> {
    let s = v
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{what} is missing"))?;
    // Lengths in UTF-16 units, as JavaScript counts them, so both sides agree.
    if s.encode_utf16().count() > MAX_URL {
        return Err(format!("{what} is too long"));
    }
    if s.chars().any(|c| c.is_whitespace() || c == '\0') {
        return Err(format!("{what} has spaces or control characters"));
    }
    let scheme = s
        .split_once(':')
        .map(|(sch, _)| format!("{}:", sch.to_ascii_lowercase()))
        .unwrap_or_default();
    if !schemes.contains(&scheme.as_str()) {
        return Err(format!("{what} must be {}", schemes.join(" or ")));
    }
    if scheme != "magnet:" && url::Url::parse(s).is_err() {
        return Err(format!("{what} isn't a valid link"));
    }
    Ok(s.to_owned())
}

fn opt_link(v: Option<&Value>, what: &str, schemes: &[&str]) -> Result<Option<String>, String> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(v) => link(v, what, schemes).map(Some),
    }
}

fn opt_text(v: Option<&Value>, what: &str, max: usize) -> Result<Option<String>, String> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            if s.encode_utf16().count() > max {
                Err(format!("{what} is too long"))
            } else if s.contains(['\r', '\n', '\0']) {
                Err(format!("{what} has line breaks"))
            } else {
                Ok(Some(s.clone()))
            }
        }
        Some(_) => Err(format!("{what} must be text")),
    }
}

fn header_name_ok(k: &str) -> bool {
    (1..=64).contains(&k.len())
        && k.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Checks a message claiming to be a download offer.
pub fn check_offer(x: &Value) -> Result<Offer, String> {
    let o: &Map<String, Value> = x.as_object().ok_or("not an object")?;
    if o.get("v") != Some(&Value::from(1)) {
        return Err("unknown schema version".into());
    }
    if o.get("type").and_then(Value::as_str) != Some("download.offer") {
        return Err("not a download offer".into());
    }
    let url = link(
        o.get("url").unwrap_or(&Value::Null),
        "url",
        &["http:", "https:", "magnet:"],
    )?;
    let final_url = opt_link(o.get("finalUrl"), "finalUrl", &["http:", "https:"])?;
    let referrer = opt_link(o.get("referrer"), "referrer", &["http:", "https:"])?;
    let filename = opt_text(o.get("filename"), "filename", MAX_FILENAME)?;
    if filename.as_deref().is_some_and(|f| f.contains(['/', '\\'])) {
        return Err("filename may not contain folders".into());
    }
    let mime = opt_text(o.get("mime"), "mime", MAX_MIME)?;
    let cookies = opt_text(o.get("cookies"), "cookies", MAX_COOKIES)?;
    let user_agent = opt_text(o.get("userAgent"), "userAgent", MAX_USER_AGENT)?;
    let size = match o.get("size") {
        None | Some(Value::Null) => None,
        // A whole, non-negative number of bytes within JavaScript's safe range.
        Some(Value::Number(n)) => match n.as_u64() {
            Some(s) if s < (1u64 << 53) => Some(s),
            _ => return Err("size must be a whole number of bytes".into()),
        },
        Some(_) => return Err("size must be a whole number of bytes".into()),
    };
    let mut headers = BTreeMap::new();
    match o.get("headers") {
        None | Some(Value::Null) => {}
        Some(Value::Object(h)) => {
            if h.len() > MAX_HEADERS {
                return Err("too many headers".into());
            }
            for (k, v) in h {
                if !header_name_ok(k) {
                    return Err(format!(
                        "header name {:?} isn't allowed",
                        k.chars().take(40).collect::<String>()
                    ));
                }
                if RESERVED.contains(&k.to_ascii_lowercase().as_str()) {
                    return Err(format!("header {k} is set by the app"));
                }
                if let Some(val) = opt_text(Some(v), &format!("header {k}"), MAX_HEADER_VALUE)? {
                    headers.insert(k.clone(), val);
                }
            }
        }
        Some(_) => return Err("headers must be an object".into()),
    }
    let source = match o.get("source").and_then(Value::as_str) {
        None if o.get("source").is_none_or(Value::is_null) => Source::Auto,
        Some("auto") => Source::Auto,
        Some("contextMenu") => Source::ContextMenu,
        _ => return Err("unknown source".into()),
    };
    Ok(Offer {
        url,
        final_url,
        referrer,
        filename,
        mime,
        size,
        cookies,
        headers,
        user_agent,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vectors() -> Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/capture/vectors/offers.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).expect("shared vectors"))
            .expect("valid JSON")
    }

    #[test]
    fn the_shared_valid_vectors_are_accepted() {
        let v = vectors();
        for case in v["valid"].as_array().unwrap() {
            let r = check_offer(&case["offer"]);
            assert!(r.is_ok(), "{}: {r:?}", case["why"]);
        }
    }

    #[test]
    fn the_shared_hostile_vectors_are_refused() {
        let v = vectors();
        let cases = v["invalid"].as_array().unwrap();
        assert!(cases.len() >= 25);
        for case in cases {
            let r = check_offer(&case["offer"]);
            assert!(r.is_err(), "{} was accepted: {r:?}", case["why"]);
        }
    }

    #[test]
    fn the_session_becomes_headers_the_engine_can_check() {
        let o = check_offer(&serde_json::json!({
            "v": 1, "type": "download.offer", "url": "https://e.org/f.zip",
            "cookies": "a=1; b=2", "referrer": "https://e.org/page",
            "userAgent": "Mozilla/5.0", "headers": {"Authorization": "Bearer t"},
        }))
        .unwrap();
        let h = o.session_headers();
        for want in [
            ("Cookie", "a=1; b=2"),
            ("Referer", "https://e.org/page"),
            ("User-Agent", "Mozilla/5.0"),
            ("Authorization", "Bearer t"),
        ] {
            assert!(
                h.iter().any(|(k, v)| k == want.0 && v == want.1),
                "{want:?} in {h:?}"
            );
        }
        let bare = check_offer(
            &serde_json::json!({"v": 1, "type": "download.offer", "url": "https://e.org/f"}),
        )
        .unwrap();
        assert!(bare.session_headers().is_empty());
    }

    #[test]
    fn defaults_and_session_detection() {
        let o = check_offer(
            &serde_json::json!({"v": 1, "type": "download.offer", "url": "https://example.org/a"}),
        )
        .unwrap();
        assert_eq!(o.source, Source::Auto);
        assert!(!o.needs_session());
        let o = check_offer(&serde_json::json!({
            "v": 1, "type": "download.offer", "url": "https://example.org/a",
            "cookies": "s=1", "headers": {"X-Empty": null}
        }))
        .unwrap();
        assert!(o.headers.is_empty(), "null header values are dropped");
        assert!(o.needs_session());
        // A size past JavaScript's safe integers is refused on both sides.
        let big = serde_json::json!({"v": 1, "type": "download.offer", "url": "https://e.org/a", "size": 9_007_199_254_740_992u64});
        assert!(check_offer(&big).is_err());
    }
}
