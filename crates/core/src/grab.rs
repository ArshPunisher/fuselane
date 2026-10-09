//! Find files on a page (B9.3). Paste a web page's address and Fuselane lists
//! the downloadable files it links to (archives, videos, disk images…), so
//! many can be picked by type and added at once. Only the page itself is read;
//! nothing it links to is fetched until it's chosen.

/// Biggest page read (an index of a big release can be a few MB).
pub const MAX_PAGE: usize = 8 * 1024 * 1024;
/// Most links listed from one page.
pub const MAX_FOUND: usize = 2000;

/// Extensions that mark a link as a file to download (not another page).
const FILE_EXTS: &[&str] = &[
    "7z", "aac", "apk", "appimage", "avi", "azw3", "bin", "bz2", "cbr", "cbz", "csv", "deb", "dmg",
    "doc", "docx", "epub", "exe", "flac", "flv", "gz", "img", "iso", "jar", "jpeg", "jpg", "json",
    "m4a", "m4v", "mkv", "mobi", "mov", "mp3", "mp4", "msi", "msix", "odp", "ods", "odt", "ogg",
    "opus", "pdf", "pkg", "png", "ppt", "pptx", "rar", "rpm", "srt", "svg", "tar", "tbz2", "tgz",
    "torrent", "txt", "txz", "wav", "webm", "webp", "wmv", "xls", "xlsx", "xz", "zip", "zst",
];

/// A file the page links to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub url: String,
    /// The file name, decoded (`My File.iso`).
    pub name: String,
}

/// The page's `<title>`, tidied, for naming the group.
pub fn title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let open_end = lower[start..].find('>')? + start + 1;
    let end = lower[open_end..].find("</title")? + open_end;
    let t = decode_entities(&html[open_end..end]);
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then(|| t.chars().take(80).collect())
}

/// Every downloadable file linked from `html` (in `href` or `src`), resolved
/// against `base`, http(s) only, each once, in page order.
pub fn files_on_page(html: &str, base: &str) -> Vec<Found> {
    let Ok(base) = url::Url::parse(base) else {
        return vec![];
    };
    let mut out: Vec<Found> = vec![];
    for raw in attribute_values(html) {
        let raw = decode_entities(raw.trim());
        if raw.is_empty() || raw.starts_with('#') || raw.starts_with("javascript:") {
            continue;
        }
        let Ok(mut u) = base.join(&raw) else {
            continue;
        };
        if !matches!(u.scheme(), "http" | "https") {
            continue;
        }
        u.set_fragment(None);
        let Some(name) = crate::checksums::file_of(u.as_str()) else {
            continue;
        };
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
        if !ext.is_some_and(|e| FILE_EXTS.contains(&e.as_str())) {
            continue;
        }
        let url = u.to_string();
        if out.iter().any(|f| f.url == url) {
            continue;
        }
        out.push(Found { url, name });
        if out.len() >= MAX_FOUND {
            break;
        }
    }
    out
}

/// The values of `href=` and `src=` attributes, quoted or not.
fn attribute_values(html: &str) -> Vec<&str> {
    let lower = html.to_ascii_lowercase();
    let mut out = vec![];
    for key in ["href", "src"] {
        let mut from = 0;
        while let Some(i) = lower[from..].find(key) {
            let at = from + i;
            from = at + key.len();
            // A whole attribute name: preceded by whitespace, followed by `=`.
            let before = lower[..at].chars().next_back();
            if !before.is_some_and(char::is_whitespace) {
                continue;
            }
            let rest = &html[from..];
            let rest_trim = rest.trim_start();
            let Some(after_eq) = rest_trim.strip_prefix('=') else {
                continue;
            };
            let v = after_eq.trim_start();
            let value = match v.chars().next() {
                Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or(""),
                Some(_) => v
                    .split(|c: char| c.is_whitespace() || c == '>')
                    .next()
                    .unwrap_or(""),
                None => "",
            };
            out.push((at, value));
        }
    }
    out.sort_by_key(|(at, _)| *at);
    out.into_iter().map(|(_, v)| v).collect()
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_linked_from_a_page_are_found_and_pages_are_not() {
        let html = r##"<html><head><title> Ubuntu 26.04 &amp; friends </title></head><body>
            <a href="ubuntu-26.04-desktop-amd64.iso">desktop</a>
            <a HREF='/pub/SHA256SUMS'>sums</a>
            <a href=../other/server.iso?x=1#top>server</a>
            <a href="https://cdn.example.net/My%20Song.flac">song</a>
            <img src="screens/shot.png" alt="">
            <a href="about.html">about</a> <a href="/docs/">docs</a>
            <a href="#top">top</a> <a href="javascript:void(0)">js</a>
            <a href="ftp://old.example.org/a.zip">ftp</a>
            <a href="ubuntu-26.04-desktop-amd64.iso">again</a>
            <a data-href="x.zip">not an href</a>
        </body></html>"##;
        let found = files_on_page(html, "https://releases.example.org/26.04/index.html");
        let urls: Vec<&str> = found.iter().map(|f| f.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://releases.example.org/26.04/ubuntu-26.04-desktop-amd64.iso",
                "https://releases.example.org/other/server.iso?x=1",
                "https://cdn.example.net/My%20Song.flac",
                "https://releases.example.org/26.04/screens/shot.png",
            ]
        );
        assert_eq!(found[2].name, "My Song.flac");
        assert_eq!(title(html).as_deref(), Some("Ubuntu 26.04 & friends"));
    }

    #[test]
    fn a_page_without_files_or_a_bad_base_finds_nothing() {
        assert!(files_on_page("<a href='a.html'>x</a>", "https://e.org/").is_empty());
        assert!(files_on_page("<a href='a.zip'>x</a>", "not a url").is_empty());
        assert_eq!(title("<p>no title</p>"), None);
    }
}
