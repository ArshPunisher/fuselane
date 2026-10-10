//! Metalink (8.3): a small XML file that lists, for each file, every place it
//! can be downloaded from and its checksum. Fuselane uses the places as
//! mirrors (B8.9) and the SHA-256 to check the result (B9.7). Reads Metalink 4
//! (RFC 5854, `.meta4`) and the older 3.0 (`.metalink`).

use quick_xml::events::{BytesStart, Event};

/// Most files taken from one Metalink.
pub const MAX_FILES: usize = 200;
/// Most places kept per file: the first is the download, the rest mirrors.
pub const MAX_URLS: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub name: String,
    pub size: Option<u64>,
    /// Lowercase hex, when the Metalink has one.
    pub sha256: Option<String>,
    /// http and https only, best first (by priority or preference).
    pub urls: Vec<String>,
}

/// Whether a text is a Metalink (its root element is `metalink`).
pub fn looks_like(text: &str) -> bool {
    let head: String = text.chars().take(2048).collect();
    head.contains("<metalink")
}

#[derive(Default)]
struct Draft {
    name: String,
    size: Option<u64>,
    sha256: Option<String>,
    /// (rank, url): lower ranks first.
    urls: Vec<(i64, String)>,
}

fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find_map(|a| (a.key.local_name().as_ref() == name).then(|| a.value.trim().to_string()))
}

/// A file name from the Metalink: its last part only, so a name like
/// `../../x` can't leave the downloads folder.
fn clean_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if last.is_empty() || last == "." || last == ".." {
        return String::new();
    }
    fuselane_storage::names::sanitize(last)
}

pub fn parse(xml: &str) -> Result<Vec<File>, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut files = Vec::new();
    let mut draft: Option<Draft> = None;
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    // What the element being read means.
    let mut url_rank: Option<i64> = None;
    let mut hash_type: Option<String> = None;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("This Metalink can't be read ({e})."))?;
        match event {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_ascii_lowercase();
                if path.is_empty() && name != "metalink" {
                    return Err("This isn't a Metalink file.".into());
                }
                match name.as_str() {
                    "file" => {
                        draft = Some(Draft {
                            name: attr(&e, "name").unwrap_or_default(),
                            ..Draft::default()
                        });
                    }
                    "url" => {
                        // v4: priority 1 is best. v3: preference 100 is best.
                        let rank = attr(&e, "priority")
                            .and_then(|p| p.parse::<i64>().ok())
                            .or_else(|| {
                                attr(&e, "preference")
                                    .and_then(|p| p.parse::<i64>().ok())
                                    .map(|p| 1000 - p)
                            })
                            .unwrap_or(999_999);
                        url_rank = Some(rank);
                    }
                    "hash" => hash_type = attr(&e, "type").map(|t| t.to_ascii_lowercase()),
                    _ => {}
                }
                path.push(name);
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::CData(t) => text.push_str(&t),
            Event::GeneralRef(r) => {
                let name = r.xml10_content();
                text.push_str(match name.as_ref() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    _ => "",
                });
            }
            Event::End(_) => {
                let name = path.pop().unwrap_or_default();
                let value = text.trim().to_string();
                text.clear();
                // A `hash` inside `pieces` is a piece hash, not the file's.
                let in_pieces = path.iter().any(|p| p == "pieces");
                if let Some(d) = draft.as_mut() {
                    match name.as_str() {
                        "size" => d.size = value.parse().ok(),
                        "hash" if !in_pieces => {
                            let is_sha256 = hash_type
                                .as_deref()
                                .is_some_and(|t| t == "sha-256" || t == "sha256");
                            if is_sha256
                                && value.len() == 64
                                && value.chars().all(|c| c.is_ascii_hexdigit())
                            {
                                d.sha256 = Some(value.to_ascii_lowercase());
                            }
                            hash_type = None;
                        }
                        "url" => {
                            let ok = url::Url::parse(&value)
                                .is_ok_and(|u| matches!(u.scheme(), "http" | "https"));
                            if ok && !d.urls.iter().any(|(_, u)| u == &value) {
                                d.urls.push((url_rank.unwrap_or(999_999), value));
                            }
                            url_rank = None;
                        }
                        "file" => {
                            if let Some(mut d) = draft.take() {
                                d.urls.sort_by_key(|(rank, _)| *rank);
                                let name = clean_name(&d.name);
                                if !d.urls.is_empty() && !name.is_empty() && files.len() < MAX_FILES
                                {
                                    files.push(File {
                                        name,
                                        size: d.size,
                                        sha256: d.sha256,
                                        urls: d
                                            .urls
                                            .into_iter()
                                            .map(|(_, u)| u)
                                            .take(MAX_URLS)
                                            .collect(),
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if files.is_empty() {
        return Err("This Metalink has no files Fuselane can download (http or https).".into());
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const V4: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<metalink xmlns="urn:ietf:params:xml:ns:metalink">
  <published>2026-10-01T00:00:00Z</published>
  <file name="ubuntu-26.04-desktop-amd64.iso">
    <size>6114656256</size>
    <hash type="sha-1">0000000000000000000000000000000000000000</hash>
    <hash type="sha-256">C9E15763F722F23E98A29DECDFAE341B98D53056C9E15763F722F23E98A29DEC</hash>
    <pieces length="262144" type="sha-256">
      <hash>aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</hash>
    </pieces>
    <url location="de" priority="2">https://mirror.de.example/ubuntu.iso</url>
    <url location="us" priority="1">https://mirror.us.example/ubuntu.iso</url>
    <url priority="3">ftp://old.example/ubuntu.iso</url>
    <url priority="4">https://mirror.us.example/ubuntu.iso</url>
    <metaurl mediatype="torrent">https://x.example/ubuntu.torrent</metaurl>
  </file>
  <file name="../../../etc/SHA256SUMS">
    <url>https://mirror.us.example/SHA256SUMS?a=1&amp;b=2</url>
  </file>
  <file name="nothing-to-get.iso">
    <url>ftp://only.example/x.iso</url>
  </file>
</metalink>"#;

    #[test]
    fn metalink_4_files_mirrors_and_sha256() {
        assert!(looks_like(V4));
        let files = parse(V4).unwrap();
        assert_eq!(files.len(), 2, "one without http links is left out");
        let a = &files[0];
        assert_eq!(a.name, "ubuntu-26.04-desktop-amd64.iso");
        assert_eq!(a.size, Some(6_114_656_256));
        assert_eq!(
            a.sha256.as_deref(),
            Some("c9e15763f722f23e98a29decdfae341b98d53056c9e15763f722f23e98a29dec"),
            "the file's hash, not a piece hash"
        );
        assert_eq!(
            a.urls,
            [
                "https://mirror.us.example/ubuntu.iso",
                "https://mirror.de.example/ubuntu.iso"
            ],
            "best first, ftp and repeats dropped"
        );
        assert_eq!(files[1].name, "SHA256SUMS", "no climbing out of the folder");
        assert_eq!(
            files[1].urls,
            ["https://mirror.us.example/SHA256SUMS?a=1&b=2"]
        );
    }

    #[test]
    fn metalink_3_uses_preference() {
        let v3 = r#"<?xml version="1.0"?>
<metalink version="3.0" xmlns="http://www.metalinker.org/">
  <files>
    <file name="app.tar.gz">
      <size>1000</size>
      <verification><hash type="sha256">aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</hash></verification>
      <resources>
        <url type="http" preference="10">http://slow.example/app.tar.gz</url>
        <url type="https" preference="100">https://fast.example/app.tar.gz</url>
      </resources>
    </file>
  </files>
</metalink>"#;
        let f = parse(v3).unwrap();
        assert_eq!(f[0].urls[0], "https://fast.example/app.tar.gz");
        assert_eq!(f[0].sha256.as_deref().map(str::len), Some(64));
    }

    #[test]
    fn other_files_are_refused() {
        assert!(!looks_like("<rss></rss>"));
        assert!(
            parse("<rss></rss>")
                .unwrap_err()
                .contains("isn't a Metalink")
        );
        assert!(
            parse("<metalink></metalink>")
                .unwrap_err()
                .contains("no files")
        );
    }
}
