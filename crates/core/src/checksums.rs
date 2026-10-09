//! Checksums found by themselves (B9.7). Many sites publish a SHA-256 next to
//! the file: `<file>.sha256`, or a `SHA256SUMS` list in the same folder. When
//! a download has no checksum of its own, Fuselane looks for one there, so the
//! finished file is verified without anyone pasting a hash.

/// Biggest checksum file read (a SHA256SUMS for a whole release is a few KB).
pub const MAX_LIST: usize = 256 * 1024;

/// Where a checksum for `link` may be published, most specific first, each
/// with a short name for the window ("SHA256SUMS").
pub fn candidates(link: &str) -> Vec<(String, String)> {
    let Ok(url) = url::Url::parse(link) else {
        return vec![];
    };
    if !matches!(url.scheme(), "http" | "https") || url.path().ends_with('/') {
        return vec![];
    }
    let mut bare = url.clone();
    bare.set_query(None);
    bare.set_fragment(None);
    let file = bare
        .path_segments()
        .and_then(|mut s| s.next_back())
        .unwrap_or("");
    if file.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for ext in [".sha256", ".sha256sum"] {
        let mut u = bare.clone();
        u.set_path(&format!("{}{ext}", bare.path()));
        out.push((u.to_string(), format!("{file}{ext}")));
    }
    for list in ["SHA256SUMS", "sha256sums.txt", "SHA256SUMS.txt"] {
        if let Ok(u) = bare.join(list) {
            out.push((u.to_string(), list.to_string()));
        }
    }
    out
}

/// The file name as checksum lists spell it: the link's last path segment,
/// with %-escapes decoded (`My%20File.iso` is listed as `My File.iso`).
pub fn file_of(link: &str) -> Option<String> {
    let url = url::Url::parse(link).ok()?;
    let last = url.path_segments()?.next_back()?.to_string();
    let bytes = last.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(h), Some(l)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok().filter(|s| !s.is_empty())
}

/// Finds the SHA-256 for `file` in a checksum file's text. Understands
/// `<hex>  name`, `<hex> *name` (sha256sum), `SHA256 (name) = <hex>` (BSD),
/// and a file holding only the hash (when `alone` is allowed: a `<file>.sha256`).
pub fn find_in(text: &str, file: &str, alone: bool) -> Option<String> {
    let is_hex = |s: &str| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit());
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    for line in &lines {
        if let Some(rest) = line.strip_prefix("SHA256 (")
            && let Some((name, hash)) = rest.split_once(") = ")
            && same_name(name, file)
            && is_hex(hash.trim())
        {
            return Some(hash.trim().to_ascii_lowercase());
        }
        if let Some((hash, name)) = line.split_once(char::is_whitespace) {
            let name = name.trim_start().trim_start_matches('*');
            if is_hex(hash) && same_name(name, file) {
                return Some(hash.to_ascii_lowercase());
            }
        }
    }
    if alone {
        // `file.sha256` with just the hash, or the hash and another spelling of the name.
        let first = lines.first()?.split_whitespace().next()?;
        if lines.len() == 1 && is_hex(first) {
            return Some(first.to_ascii_lowercase());
        }
    }
    None
}

/// `./dist/os.iso` and `os.iso` are the same entry; names are compared exactly
/// otherwise (a list for several files must not give another file's hash).
fn same_name(listed: &str, file: &str) -> bool {
    let listed = listed.trim();
    let base = listed.rsplit(['/', '\\']).next().unwrap_or(listed);
    base == file
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    #[test]
    fn checksums_are_looked_for_next_to_the_file() {
        let c = candidates("https://releases.example.org/26.04/os.iso?token=x#y");
        let urls: Vec<&str> = c.iter().map(|(u, _)| u.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://releases.example.org/26.04/os.iso.sha256",
                "https://releases.example.org/26.04/os.iso.sha256sum",
                "https://releases.example.org/26.04/SHA256SUMS",
                "https://releases.example.org/26.04/sha256sums.txt",
                "https://releases.example.org/26.04/SHA256SUMS.txt",
            ]
        );
        assert_eq!(c[2].1, "SHA256SUMS");
        assert!(candidates("https://example.org/dir/").is_empty());
        assert!(candidates("magnet:?xt=urn:btih:abc").is_empty());
        assert!(candidates("not a link").is_empty());
        assert_eq!(
            file_of("https://e.org/a/My%20File%2B1.iso?x=1").as_deref(),
            Some("My File+1.iso")
        );
        assert_eq!(file_of("https://e.org/a/bad%2").as_deref(), Some("bad%2"));
        assert_eq!(file_of("https://e.org/"), None);
    }

    #[test]
    fn every_common_format_is_read_and_only_for_the_right_file() {
        let list = format!(
            "{}  other.iso\n{} *os.iso\n",
            "a".repeat(64),
            H.to_uppercase()
        );
        assert_eq!(find_in(&list, "os.iso", false).as_deref(), Some(H));
        assert_eq!(find_in(&list, "missing.iso", false), None);
        let bsd = format!("SHA256 (os.iso) = {H}\nSHA256 (b.iso) = {}", "b".repeat(64));
        assert_eq!(find_in(&bsd, "os.iso", false).as_deref(), Some(H));
        let pathy = format!("{H}  ./dist/os.iso");
        assert_eq!(find_in(&pathy, "os.iso", false).as_deref(), Some(H));
        // A lone hash counts only in a file named after this one.
        assert_eq!(
            find_in(&format!("{H}\n"), "os.iso", true).as_deref(),
            Some(H)
        );
        assert_eq!(find_in(&format!("{H}\n"), "os.iso", false), None);
        // Not hashes: a web page, an MD5, a truncated hash.
        assert_eq!(
            find_in("<html><body>Not found</body></html>", "os.iso", true),
            None
        );
        assert_eq!(
            find_in(&format!("{}  os.iso", "c".repeat(32)), "os.iso", false),
            None
        );
        assert_eq!(
            find_in(&format!("{}  os.iso", &H[..63]), "os.iso", false),
            None
        );
        // A similar name is a different file.
        assert_eq!(
            find_in(&format!("{H}  os.iso.zsync"), "os.iso", false),
            None
        );
    }
}
