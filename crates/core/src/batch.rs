//! Adding many downloads at once: links pulled out of pasted text, and patterns
//! like `file[01-20].zip` expanded (IDM's "batch download").

/// More than this from one paste or pattern is surely a mistake.
pub const MAX_LINKS: usize = 1000;

/// Every http(s) link in `text`, in order, without repeats. Trailing punctuation
/// that text usually puts after a link (a full stop, a closing bracket) is dropped.
pub fn links_in(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split(|c: char| c.is_whitespace() || c == '"' || c == '<' || c == '>') {
        let lower = word.to_ascii_lowercase();
        let Some(start) = lower.find("http://").or_else(|| lower.find("https://")) else {
            continue;
        };
        let link = word[start..].trim_end_matches(['.', ',', ';', ')', '\'']);
        if !out.iter().any(|l| l == link) && out.len() < MAX_LINKS {
            out.push(link.to_string());
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Text(String),
    Range(Vec<String>),
}

/// `[01-20]` → 01..20 (padding kept), `[1-5]`, `[a-e]`, `[A-E]`. Anything else
/// (an IPv6 address like `[::1]`) isn't a range.
fn range(inner: &str) -> Option<Vec<String>> {
    let (a, b) = inner.split_once('-')?;
    if a.is_empty() || b.is_empty() {
        return None;
    }
    if a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit()) {
        let (x, y): (u64, u64) = (a.parse().ok()?, b.parse().ok()?);
        let width = if a.starts_with('0') && a.len() > 1 {
            a.len()
        } else {
            0
        };
        let (lo, hi) = (x.min(y), x.max(y));
        if hi - lo >= MAX_LINKS as u64 {
            return Some(vec![String::new(); MAX_LINKS + 1]); // too many, caught below
        }
        return Some((lo..=hi).map(|n| format!("{n:0width$}")).collect());
    }
    let one = |s: &str| {
        let mut c = s.chars();
        match (c.next(), c.next()) {
            (Some(ch), None) if ch.is_ascii_alphabetic() => Some(ch),
            _ => None,
        }
    };
    let (x, y) = (one(a)?, one(b)?);
    if x.is_ascii_lowercase() != y.is_ascii_lowercase() {
        return None;
    }
    let (lo, hi) = (x.min(y), x.max(y));
    Some((lo..=hi).map(String::from).collect())
}

fn parts(pattern: &str) -> Vec<Part> {
    let mut out = Vec::new();
    let mut text = String::new();
    let mut rest = pattern;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']').map(|c| open + c) else {
            break;
        };
        match range(&rest[open + 1..close]) {
            Some(values) => {
                text.push_str(&rest[..open]);
                out.push(Part::Text(std::mem::take(&mut text)));
                out.push(Part::Range(values));
            }
            None => text.push_str(&rest[..=close]),
        }
        rest = &rest[close + 1..];
    }
    text.push_str(rest);
    out.push(Part::Text(text));
    out
}

/// Every link a pattern stands for, in order. A link without a range is itself.
pub fn expand(pattern: &str) -> Result<Vec<String>, String> {
    let mut links = vec![String::new()];
    for part in parts(pattern) {
        match part {
            Part::Text(t) => links.iter_mut().for_each(|l| l.push_str(&t)),
            Part::Range(values) => {
                let total = links.len().saturating_mul(values.len());
                if total > MAX_LINKS {
                    return Err(format!(
                        "That pattern makes more than {MAX_LINKS} links. Use a smaller range."
                    ));
                }
                links = links
                    .iter()
                    .flat_map(|l| values.iter().map(move |v| format!("{l}{v}")))
                    .collect();
            }
        }
    }
    Ok(links)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_found_in_messy_pasted_text() {
        let text = "Here: https://a.example/x.zip, and (https://b.example/y.iso).\n\
                    <http://c.example/z> \"https://a.example/x.zip\" ftp://no.example/f HTTPS://D.example/W";
        assert_eq!(
            links_in(text),
            [
                "https://a.example/x.zip",
                "https://b.example/y.iso",
                "http://c.example/z",
                "HTTPS://D.example/W",
            ]
        );
        assert!(links_in("no links here").is_empty());
    }

    #[test]
    fn numbered_ranges_keep_their_padding() {
        assert_eq!(
            expand("https://x/part[08-11].rar").unwrap(),
            [
                "https://x/part08.rar",
                "https://x/part09.rar",
                "https://x/part10.rar",
                "https://x/part11.rar",
            ]
        );
        assert_eq!(
            expand("https://x/[1-3]").unwrap(),
            ["https://x/1", "https://x/2", "https://x/3"]
        );
        assert_eq!(
            expand("https://x/[3-1]").unwrap().len(),
            3,
            "backwards is fine"
        );
    }

    #[test]
    fn letter_ranges_and_several_ranges_multiply() {
        assert_eq!(
            expand("https://x/[a-c].zip").unwrap(),
            ["https://x/a.zip", "https://x/b.zip", "https://x/c.zip"]
        );
        let both = expand("https://x/s[1-2]e[01-03].mkv").unwrap();
        assert_eq!(both.len(), 6);
        assert_eq!(both[0], "https://x/s1e01.mkv");
        assert_eq!(both[5], "https://x/s2e03.mkv");
    }

    #[test]
    fn brackets_that_are_not_ranges_are_left_alone() {
        for link in [
            "http://[::1]:8080/file.bin",
            "https://x/[draft].pdf",
            "https://x/[a-1].bin",
            "https://x/[aa-bb].bin",
            "https://x/[a-Z].bin",
            "https://x/unclosed[1-3",
            "https://x/[-3].bin",
        ] {
            assert_eq!(expand(link).unwrap(), [link], "{link}");
        }
    }

    #[test]
    fn huge_patterns_are_refused_not_expanded() {
        assert!(expand("https://x/[1-5000]").is_err());
        assert!(
            expand("https://x/[0-99]/[0-99]").is_err(),
            "10,000 combined"
        );
        assert_eq!(expand("https://x/[1-1000]").unwrap().len(), 1000);
        assert!(
            expand("https://x/[1-99999999999999999999]").unwrap().len() == 1,
            "unparseable is literal"
        );
    }
}
