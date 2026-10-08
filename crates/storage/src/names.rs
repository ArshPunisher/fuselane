//! File names that are safe on every OS (L-39, L-40, T6).
//!
//! Server-supplied names (Content-Disposition, URL paths, torrent metadata) are
//! hostile input. [`sanitize`] turns any string into a single path component that
//! is valid on Windows, macOS and Linux, can't traverse directories, can't spoof
//! its extension with bidi controls, and leaves room for collision suffixes.

/// Longest component every mainstream file system accepts, in bytes (UTF-8).
pub const MAX_COMPONENT_BYTES: usize = 255;

/// Bytes kept free for `" (9999)"` plus the `.fuselane` staging suffix.
pub const SUFFIX_RESERVE_BYTES: usize = 24;

/// Longest extension we preserve when truncating (longer ones are treated as stem).
const MAX_EXTENSION_BYTES: usize = 16;

/// Used when nothing usable is left.
pub const FALLBACK_NAME: &str = "download";

/// Characters Windows forbids in names (plus both path separators).
const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Unicode controls that reorder text, used to disguise extensions
/// (e.g. `invoice\u{202E}fdp.exe` displays as `invoiceexe.pdf`).
fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{061C}')
}

/// Windows device names, reserved with or without an extension, any case.
/// Includes the superscript digits Windows also treats as COM/LPT ports.
fn is_reserved_windows_name(stem: &str) -> bool {
    let upper = stem.trim_end_matches([' ', '.']).to_uppercase();
    matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|p| {
        upper.strip_prefix(p).is_some_and(|rest| {
            let mut chars = rest.chars();
            matches!(
                (chars.next(), chars.next()),
                (Some('0'..='9' | '¹' | '²' | '³'), None)
            )
        })
    })
}

/// Splits `name` into (stem, extension-with-dot). A leading dot is part of the stem.
pub fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= MAX_EXTENSION_BYTES + 1 && i + 1 < name.len() => {
            name.split_at(i)
        }
        _ => (name, ""),
    }
}

/// Trims what no OS keeps reliably at the ends of a name: dots and any Unicode
/// whitespace. Leading dots are trimmed too, so a downloaded file is never hidden.
fn trim_ends(s: &str) -> &str {
    s.trim_matches(|c: char| c == '.' || c.is_whitespace())
}

/// Cuts `s` to at most `max` bytes on a character boundary.
fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Turns any string into a safe, non-empty single path component.
///
/// Guarantees (all property-tested):
/// - no separators, no Windows-forbidden characters, no control or bidi characters;
/// - not `.`/`..`/dots-only, not a Windows device name, no leading or trailing dot or
///   whitespace (a downloaded file is never hidden);
/// - at most `MAX_COMPONENT_BYTES - SUFFIX_RESERVE_BYTES` bytes, extension kept when possible;
/// - idempotent: `sanitize(sanitize(x)) == sanitize(x)`.
pub fn sanitize(input: &str) -> String {
    let mut out: String = input
        .chars()
        .filter(|c| !is_bidi_control(*c))
        .map(|c| {
            if c.is_control() || FORBIDDEN.contains(&c) {
                '_'
            } else {
                c
            }
        })
        .collect();

    out = trim_ends(&out).to_string();
    if out
        .chars()
        .all(|c| c == '.' || c == '_' || c.is_whitespace())
    {
        return FALLBACK_NAME.to_string();
    }

    let budget = MAX_COMPONENT_BYTES - SUFFIX_RESERVE_BYTES;
    if out.len() > budget {
        let (stem, ext) = split_extension(&out);
        let ext = ext.to_string();
        let stem = trim_ends(truncate_bytes(stem, budget - ext.len())).to_string();
        out = if stem.is_empty() {
            FALLBACK_NAME.to_string()
        } else {
            stem + &ext
        };
    }

    let (stem, _) = split_extension(&out);
    if is_reserved_windows_name(stem) {
        out.insert(0, '_');
        if out.len() > budget {
            out = trim_ends(truncate_bytes(&out, budget)).to_string();
        }
    }
    out
}

/// The `n`th alternative for a taken name: `report.pdf` → `report (n).pdf`.
pub fn numbered(name: &str, n: u32) -> String {
    let (stem, ext) = split_extension(name);
    format!("{stem} ({n}){ext}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn assert_safe(out: &str) {
        assert!(!out.is_empty(), "empty name");
        assert!(
            out.len() <= MAX_COMPONENT_BYTES - SUFFIX_RESERVE_BYTES,
            "too long: {} bytes",
            out.len()
        );
        assert!(
            !out.chars()
                .any(|c| FORBIDDEN.contains(&c) || c.is_control() || is_bidi_control(c)),
            "bad char in {out:?}"
        );
        assert!(
            !out.ends_with('.') && !out.ends_with(char::is_whitespace),
            "trailing dot/space in {out:?}"
        );
        assert!(
            !out.starts_with('.') && !out.starts_with(char::is_whitespace),
            "hidden or padded name {out:?}"
        );
        assert!(out != "." && out != "..", "dot name");
        assert!(
            !is_reserved_windows_name(split_extension(out).0),
            "reserved name {out:?}"
        );
    }

    #[test]
    fn hostile_names_become_safe() {
        let cases: &[(&str, &str)] = &[
            ("../../etc/passwd", "_.._etc_passwd"),
            ("..\\..\\Windows\\System32", "_.._Windows_System32"),
            ("C:\\boot.ini", "C__boot.ini"),
            ("report.pdf", "report.pdf"),
            ("CON", "_CON"),
            ("con.txt", "_con.txt"),
            ("LPT1.log", "_LPT1.log"),
            ("COM¹", "_COM¹"),
            ("COM10", "COM10"),
            ("NUL ", "_NUL"),
            ("trailing dots...", "trailing dots"),
            ("   ", FALLBACK_NAME),
            ("", FALLBACK_NAME),
            ("..", FALLBACK_NAME),
            ("....", FALLBACK_NAME),
            ("///", FALLBACK_NAME),
            ("a\u{0000}b\nc\td", "a_b_c_d"),
            ("invoice\u{202E}fdp.exe", "invoicefdp.exe"),
            ("file:stream.txt", "file_stream.txt"),
            ("what?*<>|\".zip", "what______.zip"),
            (".bashrc", "bashrc"),
            ("name\u{a0}", "name"),
            ("\u{3000}spaced\u{2003}", "spaced"),
            ("日本語のファイル.mp4", "日本語のファイル.mp4"),
        ];
        for (input, want) in cases {
            let got = sanitize(input);
            assert_eq!(&got, want, "input {input:?}");
            assert_safe(&got);
        }
    }

    #[test]
    fn long_names_keep_their_extension_and_whole_characters() {
        let long = format!("{}.iso", "é".repeat(400));
        let got = sanitize(&long);
        assert!(got.ends_with(".iso"));
        assert_safe(&got);
        let emoji = format!("{}.tar.gz", "🦀".repeat(200));
        let got = sanitize(&emoji);
        assert!(got.ends_with(".gz"));
        assert_safe(&got);
    }

    #[test]
    fn numbered_names_fit_after_sanitizing() {
        let base = sanitize(&"x".repeat(1000));
        let alt = numbered(&base, 9999);
        assert!(alt.len() + ".fuselane".len() <= MAX_COMPONENT_BYTES);
        assert_eq!(numbered("report.pdf", 2), "report (2).pdf");
        assert_eq!(numbered(".bashrc", 1), ".bashrc (1)");
        assert_eq!(numbered("archive", 3), "archive (3)");
    }

    #[test]
    fn extension_split_rules() {
        assert_eq!(split_extension("a.tar.gz"), ("a.tar", ".gz"));
        assert_eq!(split_extension(".hidden"), (".hidden", ""));
        assert_eq!(split_extension("noext"), ("noext", ""));
        assert_eq!(split_extension("dot."), ("dot.", ""));
        assert_eq!(
            split_extension("x.thisextensioniswaytoolong"),
            ("x.thisextensioniswaytoolong", "")
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(4000))]

        #[test]
        fn any_input_is_made_safe(input in "\\PC{0,600}") {
            assert_safe(&sanitize(&input));
        }

        #[test]
        fn arbitrary_unicode_and_controls_are_made_safe(input in proptest::collection::vec(any::<char>(), 0..400)) {
            let s: String = input.into_iter().collect();
            assert_safe(&sanitize(&s));
        }

        #[test]
        fn sanitizing_is_idempotent(input in "\\PC{0,400}") {
            let once = sanitize(&input);
            prop_assert_eq!(sanitize(&once), once);
        }

        #[test]
        fn safe_names_pass_through_unchanged(stem in "[A-Za-z0-9][A-Za-z0-9 _-]{0,40}[A-Za-z0-9]", ext in "[a-z0-9]{1,5}") {
            let name = format!("{stem}.{ext}");
            prop_assume!(!is_reserved_windows_name(&stem));
            prop_assert_eq!(sanitize(&name), name);
        }
    }
}
