//! Feeds (B10.8): reading RSS 2.0 and Atom, so new podcast episodes, release
//! files and anything else published in a feed can be downloaded by itself.
//! Pure parsing and matching; fetching and remembering live in the app.

use quick_xml::events::{BytesStart, Event};

/// Most items read from one feed (newest first in nearly every feed).
pub const MAX_ITEMS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What stays the same for this item: its guid or id, else its link.
    pub id: String,
    pub title: String,
    /// The file to download (an enclosure, or a link that is a file);
    /// empty when the item has nothing to download.
    pub url: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Feed {
    pub title: String,
    pub items: Vec<Item>,
}

#[derive(Default)]
struct Draft {
    id: String,
    title: String,
    link: String,
    enclosure: String,
    media: String,
    size: Option<u64>,
}

/// File types a plain item link may point at (a link to a web page is not a
/// download).
const FILE_TYPES: &[&str] = &[
    "7z", "aac", "apk", "appimage", "avi", "bz2", "deb", "dmg", "epub", "exe", "flac", "gz", "img",
    "iso", "m4a", "m4b", "m4v", "mkv", "mov", "mp3", "mp4", "msi", "ogg", "opus", "pdf", "pkg",
    "rar", "rpm", "tar", "tgz", "torrent", "wav", "webm", "xz", "zip", "zst",
];

fn looks_like_file(link: &str) -> bool {
    let Ok(u) = url::Url::parse(link) else {
        return false;
    };
    let path = u.path().to_ascii_lowercase();
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| FILE_TYPES.contains(&ext))
}

/// An absolute http, https or magnet link, or nothing.
fn absolute(link: &str, base: Option<&url::Url>) -> String {
    let link = link.trim();
    if link.is_empty() {
        return String::new();
    }
    if link.starts_with("magnet:?") {
        return link.to_string();
    }
    let parsed = match base {
        Some(b) => b.join(link),
        None => url::Url::parse(link),
    };
    match parsed {
        Ok(u) if matches!(u.scheme(), "http" | "https") => u.to_string(),
        _ => String::new(),
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        match rest.find(';').filter(|&j| j <= 10) {
            Some(j) => {
                out.push_str(&entity(&rest[1..j]).unwrap_or_else(|| rest[..=j].to_string()));
                rest = &rest[j + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<String> {
    let c = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        n if n.starts_with("#x") || n.starts_with("#X") => {
            char::from_u32(u32::from_str_radix(&n[2..], 16).ok()?)?
        }
        n if n.starts_with('#') => char::from_u32(n[1..].parse().ok()?)?,
        _ => return None,
    };
    Some(c.to_string())
}

fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find_map(|a| (a.key.local_name().as_ref() == name).then(|| unescape(&a.value)))
}

fn local(e: &BytesStart<'_>) -> String {
    e.local_name().as_ref().to_ascii_lowercase()
}

/// Reads an RSS 2.0 or Atom feed. `base` is the feed's own address, for
/// relative links.
pub fn parse(xml: &str, base: &str) -> Result<Feed, String> {
    let base = url::Url::parse(base).ok();
    // Not trimmed piece by piece: "Tech &amp; Talk" comes in three pieces,
    // and the spaces belong to it. The whole text is trimmed at its end tag.
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut feed = Feed::default();
    let mut path: Vec<String> = Vec::new();
    let mut item: Option<Draft> = None;
    let mut text = String::new();
    let mut seen_root = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("This isn't a readable feed ({e})."))?;
        match event {
            Event::Start(e) => {
                let name = local(&e);
                if path.is_empty() {
                    if !matches!(name.as_str(), "rss" | "feed" | "rdf") {
                        return Err("This isn't a feed: it's not RSS or Atom.".into());
                    }
                    seen_root = true;
                }
                if matches!(name.as_str(), "item" | "entry") && item.is_none() {
                    item = Some(Draft::default());
                }
                if let Some(d) = item.as_mut() {
                    on_tag(d, &name, &e, base.as_ref());
                }
                path.push(name);
                text.clear();
            }
            Event::Empty(e) => {
                let name = local(&e);
                if let Some(d) = item.as_mut() {
                    on_tag(d, &name, &e, base.as_ref());
                }
            }
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::CData(t) => text.push_str(&t),
            Event::GeneralRef(r) => {
                let name = r.xml10_content();
                text.push_str(&entity(&name).unwrap_or_else(|| format!("&{name};")));
            }
            Event::End(_) => {
                let name = path.pop().unwrap_or_default();
                let value = text.trim().to_string();
                text.clear();
                let parent = path.last().map(String::as_str);
                if let Some(d) = item.as_mut() {
                    match name.as_str() {
                        "title" if d.title.is_empty() => d.title = value,
                        "guid" | "id" if d.id.is_empty() => d.id = value,
                        // RSS: the link is the element's text.
                        "link" if d.link.is_empty() && !value.is_empty() => {
                            d.link = absolute(&value, base.as_ref());
                        }
                        "item" | "entry" => {
                            if let Some(done) = item.take().map(finish)
                                && feed.items.len() < MAX_ITEMS
                            {
                                feed.items.push(done);
                            }
                        }
                        _ => {}
                    }
                } else if name == "title"
                    && feed.title.is_empty()
                    && matches!(parent, Some("channel" | "feed"))
                {
                    feed.title = value;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_root {
        return Err("This isn't a feed: it's not RSS or Atom.".into());
    }
    Ok(feed)
}

fn on_tag(d: &mut Draft, name: &str, e: &BytesStart<'_>, base: Option<&url::Url>) {
    match name {
        "enclosure" if d.enclosure.is_empty() => {
            d.enclosure = absolute(&attr(e, "url").unwrap_or_default(), base);
            d.size = attr(e, "length")
                .and_then(|l| l.trim().parse().ok())
                .filter(|&n| n > 0);
        }
        // Atom: <link rel="enclosure" href="…"/> or <link href="…"/>.
        "link" => {
            if let Some(href) = attr(e, "href") {
                let href = absolute(&href, base);
                match attr(e, "rel").as_deref() {
                    Some("enclosure") if d.enclosure.is_empty() => {
                        d.enclosure = href;
                        d.size = attr(e, "length").and_then(|l| l.trim().parse().ok());
                    }
                    None | Some("alternate") if d.link.is_empty() => d.link = href,
                    _ => {}
                }
            }
        }
        // Media RSS, used by some video and podcast feeds.
        "content" if d.media.is_empty() => {
            d.media = absolute(&attr(e, "url").unwrap_or_default(), base);
        }
        _ => {}
    }
}

fn finish(d: Draft) -> Item {
    let url = if !d.enclosure.is_empty() {
        d.enclosure.clone()
    } else if !d.media.is_empty() {
        d.media.clone()
    } else if d.link.starts_with("magnet:") || looks_like_file(&d.link) {
        d.link.clone()
    } else {
        String::new()
    };
    let id = [&d.id, &d.link, &url, &d.title]
        .into_iter()
        .find(|s| !s.is_empty())
        .cloned()
        .unwrap_or_default();
    Item {
        id,
        title: d.title,
        url,
        size: d.size,
    }
}

/// A file name for an item, when its link doesn't make a good one: podcast
/// hosts often call every episode `default.mp3`. Uses the title's last part
/// if it is already a file name ("/2.61/KeePass-2.61-Thai.zip"), else the
/// title with the link's file type. `None` leaves it to the server.
pub fn suggested_name(item: &Item) -> Option<String> {
    let title = item.title.trim();
    let last = title.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if looks_like_file(&format!("https://x/{last}")) && !last.starts_with('.') {
        return Some(fuselane_storage::names::sanitize(last)).filter(|n| !n.is_empty());
    }
    let ext = url::Url::parse(&item.url)
        .ok()?
        .path()
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .filter(|e| FILE_TYPES.contains(&e.as_str()))?;
    // Characters files can't have become spaces ("Episode 42: Bonding"), not
    // underscores, then runs of spaces fold into one.
    let plain: String = title
        .chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) { ' ' } else { c })
        .collect();
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if plain.is_empty() {
        return None;
    }
    let base: String = fuselane_storage::names::sanitize(&plain)
        .chars()
        .take(120)
        .collect();
    let base = base.trim().trim_end_matches('.');
    (!base.is_empty()).then(|| format!("{base}.{ext}"))
}

/// Whether a title passes the filters: every word of `include` appears (any
/// order, any case) and no word of `exclude` does. Empty filters pass all.
pub fn matches(title: &str, include: &str, exclude: &str) -> bool {
    let t = title.to_lowercase();
    include
        .split_whitespace()
        .all(|w| t.contains(&w.to_lowercase()))
        && !exclude
            .split_whitespace()
            .any(|w| t.contains(&w.to_lowercase()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const PODCAST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
<channel>
  <title>Tech &amp; Talk</title>
  <link>https://pod.example/</link>
  <item>
    <title>Episode 42: Bonding &lt;networks&gt;</title>
    <guid isPermaLink="false">ep-42</guid>
    <enclosure url="https://cdn.pod.example/ep42.mp3?x=1&amp;y=2" length="48123456" type="audio/mpeg"/>
  </item>
  <item>
    <title><![CDATA[Episode 41 & friends]]></title>
    <link>https://pod.example/41</link>
    <enclosure url="/media/ep41.mp3" length="0" type="audio/mpeg"/>
  </item>
  <item>
    <title>Show notes only</title>
    <link>https://pod.example/notes</link>
  </item>
  <item>
    <title>Release 2.0</title>
    <link>https://files.example/app-2.0.dmg</link>
  </item>
</channel>
</rss>"#;

    #[test]
    fn rss_items_with_enclosures_links_and_entities() {
        let f = parse(PODCAST, "https://pod.example/feed.xml").unwrap();
        assert_eq!(f.title, "Tech & Talk");
        assert_eq!(f.items.len(), 4);
        let a = &f.items[0];
        assert_eq!(a.title, "Episode 42: Bonding <networks>");
        assert_eq!(a.id, "ep-42");
        assert_eq!(a.url, "https://cdn.pod.example/ep42.mp3?x=1&y=2");
        assert_eq!(a.size, Some(48_123_456));
        let b = &f.items[1];
        assert_eq!(b.title, "Episode 41 & friends");
        assert_eq!(
            b.url, "https://pod.example/media/ep41.mp3",
            "relative to the feed"
        );
        assert_eq!(b.id, "https://pod.example/41", "no guid: the link");
        assert_eq!(b.size, None, "a zero length means unknown");
        assert_eq!(f.items[2].url, "", "a web page isn't a download");
        assert_eq!(f.items[3].url, "https://files.example/app-2.0.dmg");
    }

    #[test]
    fn atom_entries_and_magnets() {
        let atom = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title type="text">Nightly builds</title>
  <entry>
    <title>Build 1001</title>
    <id>tag:builds.example,2026:1001</id>
    <link rel="alternate" href="https://builds.example/1001"/>
    <link rel="enclosure" href="https://builds.example/1001.tar.xz" length="9000"/>
  </entry>
  <entry>
    <title>Linux ISO (torrent)</title>
    <id>urn:2</id>
    <link href="magnet:?xt=urn:btih:abcdef&amp;dn=linux.iso"/>
  </entry>
  <entry>
    <title>Sneaky</title>
    <id>urn:3</id>
    <link rel="enclosure" href="file:///etc/passwd"/>
  </entry>
</feed>"#;
        let f = parse(atom, "https://builds.example/atom").unwrap();
        assert_eq!(f.title, "Nightly builds");
        assert_eq!(f.items[0].url, "https://builds.example/1001.tar.xz");
        assert_eq!(f.items[0].size, Some(9000));
        assert_eq!(f.items[0].id, "tag:builds.example,2026:1001");
        assert_eq!(f.items[1].url, "magnet:?xt=urn:btih:abcdef&dn=linux.iso");
        assert_eq!(f.items[2].url, "", "only http, https and magnet");
    }

    #[test]
    fn not_a_feed_says_so() {
        assert!(
            parse("<html><body>hi</body></html>", "https://x/")
                .unwrap_err()
                .contains("not RSS or Atom")
        );
        assert!(parse("", "https://x/").is_err());
    }

    #[test]
    fn names_come_from_titles_when_links_are_generic() {
        let item = |title: &str, url: &str| Item {
            id: "x".into(),
            title: title.into(),
            url: url.into(),
            size: None,
        };
        assert_eq!(
            suggested_name(&item(
                "Episode 42: Bonding <networks>",
                "https://cdn.example/ep/default.mp3?aid=rss"
            ))
            .as_deref(),
            Some("Episode 42 Bonding networks.mp3")
        );
        assert_eq!(
            suggested_name(&item(
                "/Translations 2.x/2.61/KeePass-2.61-Thai.zip",
                "https://sourceforge.net/projects/keepass/files/x/download"
            ))
            .as_deref(),
            Some("KeePass-2.61-Thai.zip")
        );
        // Nothing to go on: the server names it.
        assert_eq!(
            suggested_name(&item("Build", "https://b.example/get?id=1")),
            None
        );
        assert_eq!(suggested_name(&item("", "https://b.example/a.zip")), None);
    }

    #[test]
    fn filters_are_words_in_any_order() {
        assert!(matches("Show S02E05 1080p WEB", "1080p s02", ""));
        assert!(!matches("Show S02E05 720p", "1080p", ""));
        assert!(!matches("Show S02E05 1080p CAM", "1080p", "cam"));
        assert!(matches("anything", "", ""));
    }
}
