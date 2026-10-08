//! Parsing what servers say, strictly (L-01–L-08).
//!
//! Every function here treats its input as hostile: malformed headers never
//! panic, and anything ambiguous is rejected rather than guessed.

use percent_encoding::percent_decode_str;

/// File name from a `Content-Disposition` value (RFC 6266 with RFC 5987/8187
/// extended values). `filename*` wins over `filename`. Returns the raw name;
/// callers must still pass it through `storage::names::sanitize`.
pub fn content_disposition_filename(value: &str) -> Option<String> {
    let mut plain: Option<String> = None;
    let mut extended: Option<String> = None;
    for (key, val) in parameters(value) {
        match key.to_ascii_lowercase().as_str() {
            "filename*" => {
                if let Some(v) = decode_ext_value(&val) {
                    extended = Some(v);
                }
            }
            "filename" => plain = Some(val),
            _ => {}
        }
    }
    extended.or(plain).filter(|s| !s.trim().is_empty())
}

/// Splits `type; a=b; c="d;e"` into (key, unquoted value) pairs.
fn parameters(value: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = value.chars().peekable();
    // Skip the disposition type.
    for c in chars.by_ref() {
        if c == ';' {
            break;
        }
    }
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ';') {
            chars.next();
        }
        let key: String = chars
            .by_ref()
            .take_while(|c| *c != '=')
            .collect::<String>()
            .trim()
            .to_string();
        if key.is_empty() {
            break;
        }
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let mut val = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            let mut closed = false;
            while let Some(c) = chars.next() {
                match c {
                    '\\' => {
                        if let Some(n) = chars.next() {
                            val.push(n);
                        }
                    }
                    '"' => {
                        closed = true;
                        break;
                    }
                    _ => val.push(c),
                }
            }
            if !closed {
                // Unterminated quote: keep what we have but stop parsing.
                out.push((key, val));
                break;
            }
            for c in chars.by_ref() {
                if c == ';' {
                    break;
                }
            }
        } else {
            for c in chars.by_ref() {
                if c == ';' {
                    break;
                }
                val.push(c);
            }
            val = val.trim().to_string();
        }
        out.push((key, val));
    }
    out
}

/// Decodes `charset'lang'percent-encoded` (RFC 8187). UTF-8 and ISO-8859-1 only;
/// malformed escapes fall back to the raw text after the second quote (L-08).
fn decode_ext_value(v: &str) -> Option<String> {
    let mut parts = v.splitn(3, '\'');
    let charset = parts.next()?.trim().to_ascii_lowercase();
    let _lang = parts.next()?;
    let encoded = parts.next()?;
    if !valid_escapes(encoded) {
        return Some(encoded.to_string());
    }
    let bytes: Vec<u8> = percent_decode_str(encoded).collect();
    match charset.as_str() {
        "utf-8" => Some(String::from_utf8(bytes).unwrap_or_else(|_| encoded.to_string())),
        // ISO-8859-1 maps each byte to the code point with the same value.
        "iso-8859-1" | "latin1" => Some(bytes.iter().map(|b| char::from(*b)).collect()),
        _ => None,
    }
}

fn valid_escapes(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit() {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}

/// Last non-empty path segment of a URL path, percent-decoded when that yields
/// valid UTF-8; the raw segment otherwise. Query strings must already be removed.
pub fn filename_from_path(path: &str) -> Option<String> {
    let seg = path.rsplit('/').find(|s| !s.is_empty())?;
    if !valid_escapes(seg) {
        return Some(seg.to_string());
    }
    let decoded = percent_decode_str(seg)
        .decode_utf8()
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| seg.to_string());
    Some(decoded)
}

/// Normalizes an entity tag so equivalent labels compare equal: drops the weak
/// prefix, the quotes, and compression suffixes added by proxies and CDNs (L-06).
pub fn normalize_etag(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(rest) = s.strip_prefix("W/").or_else(|| s.strip_prefix("w/")) {
        s = rest;
    }
    let mut s = s.trim_matches('"').to_string();
    for suffix in ["-gzip", "-br", "-zstd", "-deflate", ";gzip"] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.to_string();
        }
    }
    s.trim_matches('"').to_string()
}

/// A parsed `Content-Range` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentRange {
    /// `bytes first-last/total` (total may be unknown: `*`).
    Bytes {
        first: u64,
        last: u64,
        total: Option<u64>,
    },
    /// `bytes */total`: unsatisfiable range; total is the full size.
    Unsatisfied { total: u64 },
}

/// Parses `Content-Range` strictly; anything malformed is `None`.
pub fn parse_content_range(value: &str) -> Option<ContentRange> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let total = total.trim();
    let total = if total == "*" {
        None
    } else {
        Some(total.parse::<u64>().ok()?)
    };
    let range = range.trim();
    if range == "*" {
        return total.map(|t| ContentRange::Unsatisfied { total: t });
    }
    let (a, b) = range.split_once('-')?;
    if a.is_empty()
        || b.is_empty()
        || !a.bytes().all(|c| c.is_ascii_digit())
        || !b.bytes().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let first: u64 = a.parse().ok()?;
    let last: u64 = b.parse().ok()?;
    if last < first || total.is_some_and(|t| last >= t) {
        return None;
    }
    Some(ContentRange::Bytes { first, last, total })
}

/// Why a response to a ranged request can't be used (each one is a strike, L-15).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RangeError {
    #[error("the server ignored the range and sent the whole file from byte 0")]
    RangeIgnored,
    #[error("206 without a usable Content-Range header")]
    MissingContentRange,
    #[error("the server answered from byte {got}, not byte {want}")]
    WrongStart { want: u64, got: u64 },
    #[error("the server sent bytes past the requested end ({got} > {want})")]
    Overrun { want: u64, got: u64 },
    #[error("the file size changed: {was} bytes before, {now} now")]
    SizeChanged { was: u64, now: u64 },
    #[error("Content-Length {length} doesn't match the range {first}-{last}")]
    LengthMismatch { first: u64, last: u64, length: u64 },
    #[error("unexpected status {0} for a range request")]
    Status(u16),
}

/// What the body of an accepted response must contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accepted {
    pub first: u64,
    /// Inclusive last byte the body will cover.
    pub last: u64,
}

/// Decides whether a response may be written at `[want_first, want_last]` (L-02, L-03, L-04).
///
/// Accepts a 206 only when Content-Range starts exactly at `want_first`, doesn't run past
/// `want_last`, agrees with the known total size and with Content-Length. Accepts a 200
/// only for a request starting at byte 0. The caller must still check the body length.
pub fn check_range_response(
    want_first: u64,
    want_last: u64,
    known_total: Option<u64>,
    status: u16,
    content_range: Option<&str>,
    content_length: Option<u64>,
) -> Result<Accepted, RangeError> {
    match status {
        206 => {
            let Some(ContentRange::Bytes { first, last, total }) =
                content_range.and_then(parse_content_range)
            else {
                return Err(RangeError::MissingContentRange);
            };
            if let (Some(was), Some(now)) = (known_total, total)
                && was != now
            {
                return Err(RangeError::SizeChanged { was, now });
            }
            if first != want_first {
                return Err(RangeError::WrongStart {
                    want: want_first,
                    got: first,
                });
            }
            if last > want_last {
                return Err(RangeError::Overrun {
                    want: want_last,
                    got: last,
                });
            }
            // Checked: a hostile `bytes 0-18446744073709551615/*` must not overflow.
            if let Some(length) = content_length
                && Some(length) != (last - first).checked_add(1)
            {
                return Err(RangeError::LengthMismatch {
                    first,
                    last,
                    length,
                });
            }
            Ok(Accepted { first, last })
        }
        200 if want_first == 0 => {
            if let (Some(was), Some(now)) = (known_total, content_length)
                && was != now
            {
                return Err(RangeError::SizeChanged { was, now });
            }
            let last = content_length
                .map_or(want_last, |l| l.saturating_sub(1))
                .min(want_last);
            Ok(Accepted { first: 0, last })
        }
        200 => Err(RangeError::RangeIgnored),
        other => Err(RangeError::Status(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn content_disposition_cases() {
        let cases: &[(&str, Option<&str>)] = &[
            ("attachment; filename=\"report.pdf\"", Some("report.pdf")),
            ("attachment; filename=report.pdf", Some("report.pdf")),
            (
                "attachment; filename*=UTF-8''%E2%82%AC%20rates.pdf",
                Some("€ rates.pdf"),
            ),
            (
                "attachment; filename=\"fallback.pdf\"; filename*=UTF-8''real.pdf",
                Some("real.pdf"),
            ),
            (
                "attachment; filename*=UTF-8''real.pdf; filename=\"fallback.pdf\"",
                Some("real.pdf"),
            ),
            (
                "attachment; filename*=iso-8859-1'en'%A3%20rates.txt",
                Some("£ rates.txt"),
            ),
            (
                "attachment; filename=\"semi;colon.txt\"",
                Some("semi;colon.txt"),
            ),
            (
                "attachment; filename=\"esc\\\"aped.txt\"",
                Some("esc\"aped.txt"),
            ),
            ("attachment; FILENAME=\"Upper.TXT\"", Some("Upper.TXT")),
            (
                "attachment; filename*=UTF-8''bad%ZZescape.txt",
                Some("bad%ZZescape.txt"),
            ),
            (
                "attachment; filename*=UTF-8''trunc%E2%8",
                Some("trunc%E2%8"),
            ),
            (
                "attachment; filename*=UTF-8''%FF%FE.bin",
                Some("%FF%FE.bin"),
            ),
            (
                "attachment; filename*=koi8-r''x.txt; filename=y.txt",
                Some("y.txt"),
            ),
            ("attachment; filename=\"unterminated", Some("unterminated")),
            ("attachment; filename=\"\"", None),
            ("attachment", None),
            ("inline;;;", None),
            ("", None),
            ("attachment; filename", None),
        ];
        for (input, want) in cases {
            assert_eq!(
                content_disposition_filename(input).as_deref(),
                *want,
                "input {input:?}"
            );
        }
    }

    #[test]
    fn filename_from_url_path() {
        assert_eq!(
            filename_from_path("/releases/ubuntu-26.04.iso").as_deref(),
            Some("ubuntu-26.04.iso")
        );
        assert_eq!(
            filename_from_path("/a/b%20c.zip").as_deref(),
            Some("b c.zip")
        );
        assert_eq!(filename_from_path("/dir/").as_deref(), Some("dir"));
        assert_eq!(
            filename_from_path("/bad%zz.txt").as_deref(),
            Some("bad%zz.txt")
        );
        assert_eq!(
            filename_from_path("/latin%E9.txt").as_deref(),
            Some("latin%E9.txt")
        );
        assert_eq!(filename_from_path("/").as_deref(), None);
        assert_eq!(filename_from_path("").as_deref(), None);
    }

    #[test]
    fn etag_normalization() {
        assert_eq!(normalize_etag("\"abc\""), "abc");
        assert_eq!(normalize_etag("W/\"abc\""), "abc");
        assert_eq!(normalize_etag("\"abc-gzip\""), "abc");
        assert_eq!(normalize_etag("\"abc\"-gzip"), "abc");
        assert_eq!(normalize_etag("W/\"abc-br\""), "abc");
        assert_eq!(normalize_etag("  abc  "), "abc");
        assert_eq!(normalize_etag(""), "");
        assert_ne!(normalize_etag("\"abc\""), normalize_etag("\"abd\""));
    }

    #[test]
    fn content_range_parsing() {
        assert_eq!(
            parse_content_range("bytes 0-0/100"),
            Some(ContentRange::Bytes {
                first: 0,
                last: 0,
                total: Some(100)
            })
        );
        assert_eq!(
            parse_content_range("bytes 5-9/*"),
            Some(ContentRange::Bytes {
                first: 5,
                last: 9,
                total: None
            })
        );
        assert_eq!(
            parse_content_range("bytes */0"),
            Some(ContentRange::Unsatisfied { total: 0 })
        );
        for bad in [
            "",
            "bytes",
            "bytes 9-5/100",
            "bytes 0-100/100",
            "bytes -5/100",
            "bytes 5-/100",
            "bytes 0-1",
            "items 0-1/2",
            "bytes 0-1/abc",
            "bytes +1-2/3",
            "bytes 0-18446744073709551616/1",
            "bytes */*",
            "bytes 1 -2/3",
        ] {
            assert_eq!(parse_content_range(bad), None, "accepted {bad:?}");
        }
    }

    #[test]
    fn range_response_rules() {
        let ok = check_range_response(10, 19, Some(100), 206, Some("bytes 10-19/100"), Some(10));
        assert_eq!(
            ok,
            Ok(Accepted {
                first: 10,
                last: 19
            })
        );
        // A server capping ranges (Plexo 9918671): shorter is acceptable, the rest is refetched.
        assert_eq!(
            check_range_response(0, 39, Some(40), 206, Some("bytes 0-9/40"), Some(10)),
            Ok(Accepted { first: 0, last: 9 })
        );
        assert_eq!(
            check_range_response(10, 19, Some(100), 206, Some("bytes 0-19/100"), None),
            Err(RangeError::WrongStart { want: 10, got: 0 })
        );
        assert_eq!(
            check_range_response(10, 19, Some(100), 206, Some("bytes 10-29/100"), None),
            Err(RangeError::Overrun { want: 19, got: 29 })
        );
        assert_eq!(
            check_range_response(10, 19, Some(100), 206, Some("bytes 10-19/101"), None),
            Err(RangeError::SizeChanged { was: 100, now: 101 })
        );
        assert_eq!(
            check_range_response(10, 19, Some(100), 206, None, None),
            Err(RangeError::MissingContentRange)
        );
        assert_eq!(
            check_range_response(10, 19, Some(100), 206, Some("garbage"), None),
            Err(RangeError::MissingContentRange)
        );
        assert_eq!(
            check_range_response(10, 19, Some(100), 206, Some("bytes 10-19/100"), Some(11)),
            Err(RangeError::LengthMismatch {
                first: 10,
                last: 19,
                length: 11
            })
        );
        assert_eq!(
            check_range_response(10, 19, Some(100), 200, None, Some(100)),
            Err(RangeError::RangeIgnored)
        );
        assert_eq!(
            check_range_response(0, 99, Some(100), 200, None, Some(100)),
            Ok(Accepted { first: 0, last: 99 })
        );
        assert_eq!(
            check_range_response(0, 99, Some(100), 200, None, Some(150)),
            Err(RangeError::SizeChanged { was: 100, now: 150 })
        );
        assert_eq!(
            check_range_response(0, 9, None, 416, Some("bytes */0"), None),
            Err(RangeError::Status(416))
        );
        assert_eq!(
            check_range_response(0, 9, None, 503, None, None),
            Err(RangeError::Status(503))
        );
        // Open-ended request (unknown size) against a hostile maximal range: must not overflow.
        let max = u64::MAX;
        let r = check_range_response(
            0,
            max,
            None,
            206,
            Some("bytes 0-18446744073709551615/*"),
            Some(5),
        );
        assert!(matches!(r, Err(RangeError::LengthMismatch { .. })), "{r:?}");
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(3000))]

        #[test]
        fn header_parsers_never_panic(s in "\\PC{0,300}") {
            let _ = content_disposition_filename(&s);
            let _ = parse_content_range(&s);
            let _ = filename_from_path(&s);
            let _ = normalize_etag(&s);
        }

        #[test]
        fn disposition_with_arbitrary_bytes_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
            let s = String::from_utf8_lossy(&bytes);
            let _ = content_disposition_filename(&format!("attachment; filename*=UTF-8''{s}"));
            let _ = content_disposition_filename(&format!("attachment; filename=\"{s}"));
        }

        /// The core safety property: anything accepted lies exactly inside the request.
        #[test]
        fn accepted_ranges_never_escape_the_request(
            want_first in prop_oneof![0u64..10_000, Just(0u64), Just(u64::MAX - 1)],
            len in prop_oneof![1u64..5_000, Just(u64::MAX)],
            total in proptest::option::of(prop_oneof![0u64..20_000, Just(u64::MAX)]),
            status in prop_oneof![Just(200u16), Just(206), Just(416), 100u16..600],
            first in prop_oneof![0u64..20_000, Just(0u64), Just(u64::MAX)],
            last in prop_oneof![0u64..20_000, Just(u64::MAX)],
            cr_total in proptest::option::of(prop_oneof![0u64..20_000, Just(u64::MAX)]),
            content_length in proptest::option::of(prop_oneof![0u64..20_000, Just(u64::MAX), Just(0u64)]),
        ) {
            let want_last = want_first.saturating_add(len - 1);
            let header = format!("bytes {first}-{last}/{}", cr_total.map_or("*".to_string(), |t| t.to_string()));
            if let Ok(acc) = check_range_response(want_first, want_last, total, status, Some(&header), content_length) {
                prop_assert_eq!(acc.first, want_first);
                prop_assert!(acc.last <= want_last);
                prop_assert!(acc.first <= acc.last || want_first == 0);
                if status == 206 {
                    if let (Some(t), Some(c)) = (total, cr_total) { prop_assert_eq!(t, c); }
                    if let Some(l) = content_length { prop_assert_eq!(Some(l), (acc.last - acc.first).checked_add(1)); }
                }
            }
        }
    }
}
