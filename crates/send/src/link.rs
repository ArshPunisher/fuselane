//! The share link (FUSE-SEND.md §2):
//!
//! ```text
//! https://fuselane.app/s#v1.<base64url(info-hash ‖ key ‖ flags)>
//! ```
//!
//! Everything after `#` stays in the browser, so the key never reaches a server.
//! The same token works as `fuselane://send/v1.…` or pasted on its own.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// The static page that hands links to the app.
pub const PAGE: &str = "https://fuselane.app/s";
/// Where the page used to be: links made by older versions still open.
pub const OLD_PAGES: &[&str] = &["https://arshpunisher.github.io/fuselane/s"];
pub const SCHEME: &str = "fuselane://send/";
/// The newest link version this build understands.
pub const VERSION: u32 = 1;

const HASH: usize = 20;
const KEY: usize = 32;
const BODY: usize = HASH + KEY + 1;

/// What the link says about the share, beyond where and how to decrypt it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags {
    /// A folder rather than a single file.
    pub folder: bool,
}

impl Flags {
    const FOLDER: u8 = 1;
    const KNOWN: u8 = Self::FOLDER;

    fn byte(self) -> u8 {
        if self.folder { Self::FOLDER } else { 0 }
    }
}

/// A parsed share link. `Debug` never prints the key.
#[derive(Clone, PartialEq, Eq)]
pub struct Link {
    /// The torrent's info-hash (v1, SHA-1) over the encrypted bytes.
    pub info_hash: [u8; HASH],
    /// The share's encryption key; only ever in the link.
    pub key: [u8; KEY],
    pub flags: Flags,
}

impl fmt::Debug for Link {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Link")
            .field("info_hash", &hex(&self.info_hash))
            .field("key", &"<hidden>")
            .field("flags", &self.flags)
            .finish()
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Why a link can't be opened, worded for the person who pasted it (ERRORS.md).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("This isn't a Fuse Send link. It should start with {PAGE}#v1.")]
    NotALink,
    #[error("This link isn't complete. Ask the sender to copy it again.")]
    Incomplete,
    #[error("This link was made by a newer Fuselane. Update Fuselane to open it.")]
    TooNew,
}

impl Link {
    /// A new share with a fresh random key.
    pub fn new(info_hash: [u8; HASH], flags: Flags) -> Result<Link, getrandom::Error> {
        let mut key = [0u8; KEY];
        getrandom::fill(&mut key)?;
        Ok(Link {
            info_hash,
            key,
            flags,
        })
    }

    /// The token after `#`: `v1.<base64url>`.
    pub fn token(&self) -> String {
        let mut body = Vec::with_capacity(BODY);
        body.extend_from_slice(&self.info_hash);
        body.extend_from_slice(&self.key);
        body.push(self.flags.byte());
        format!("v{VERSION}.{}", URL_SAFE_NO_PAD.encode(body))
    }

    /// The link people share.
    pub fn url(&self) -> String {
        format!("{PAGE}#{}", self.token())
    }

    /// The same link for opening the app directly.
    pub fn app_url(&self) -> String {
        format!("{SCHEME}{}", self.token())
    }

    /// Accepts the shared URL, the `fuselane://send/` form, or the bare token,
    /// with surrounding whitespace (pasted text often has some).
    pub fn parse(text: &str) -> Result<Link, LinkError> {
        let text = text.trim();
        let token = if let Some(rest) = text.strip_prefix(SCHEME) {
            rest
        } else if let Some((page, fragment)) = text.split_once('#') {
            let page = page.trim_end_matches('/');
            let page = page
                .strip_prefix("http://")
                .map_or(page.to_string(), |p| format!("https://{p}"));
            if page != PAGE && !OLD_PAGES.contains(&page.as_str()) {
                return Err(LinkError::NotALink);
            }
            fragment
        } else {
            text
        };
        let (version, body) = token
            .strip_prefix('v')
            .and_then(|t| t.split_once('.'))
            .ok_or(LinkError::NotALink)?;
        let version: u32 = version.parse().map_err(|_| LinkError::NotALink)?;
        if version > VERSION {
            return Err(LinkError::TooNew);
        }
        if version == 0 {
            return Err(LinkError::NotALink);
        }
        let bytes = URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|_| LinkError::Incomplete)?;
        if bytes.len() != BODY {
            return Err(LinkError::Incomplete);
        }
        let flags = bytes[BODY - 1];
        // A flag this build doesn't know could change how the share must be read.
        if flags & !Flags::KNOWN != 0 {
            return Err(LinkError::TooNew);
        }
        let mut info_hash = [0u8; HASH];
        let mut key = [0u8; KEY];
        info_hash.copy_from_slice(&bytes[..HASH]);
        key.copy_from_slice(&bytes[HASH..HASH + KEY]);
        Ok(Link {
            info_hash,
            key,
            flags: Flags {
                folder: flags & Flags::FOLDER != 0,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed vector, so other implementations (the link page) can check theirs.
    fn vector() -> Link {
        let mut info_hash = [0u8; HASH];
        let mut key = [0u8; KEY];
        for (i, b) in info_hash.iter_mut().enumerate() {
            *b = i as u8;
        }
        for (i, b) in key.iter_mut().enumerate() {
            *b = 0xff - i as u8;
        }
        Link {
            info_hash,
            key,
            flags: Flags { folder: true },
        }
    }

    /// Computed independently: Python `base64.urlsafe_b64encode`, padding removed.
    const VECTOR: &str =
        "v1.AAECAwQFBgcICQoLDA0ODxAREhP__v38-_r5-Pf29fTz8vHw7-7t7Ovq6ejn5uXk4-Lh4AE";

    #[test]
    fn the_vector_prints_and_parses_exactly() {
        assert_eq!(vector().token(), VECTOR);
        assert_eq!(Link::parse(VECTOR), Ok(vector()));
    }

    #[test]
    fn every_form_people_paste_is_accepted() {
        let l = vector();
        for text in [
            l.url(),
            l.app_url(),
            l.token(),
            format!("  {}\n", l.url()),
            format!("{PAGE}/#{}", l.token()),
            format!("http://arshpunisher.github.io/fuselane/s#{}", l.token()),
            // Links made before the page moved to fuselane.app still open.
            format!("https://arshpunisher.github.io/fuselane/s#{}", l.token()),
            format!("http://fuselane.app/s/#{}", l.token()),
        ] {
            assert_eq!(Link::parse(&text), Ok(l.clone()), "{text}");
        }
    }

    #[test]
    fn new_links_get_a_fresh_key_and_round_trip() {
        let a = Link::new([7; HASH], Flags::default()).unwrap();
        let b = Link::new([7; HASH], Flags::default()).unwrap();
        assert_ne!(a.key, b.key, "every share has its own key");
        assert_ne!(a.key, [0; KEY]);
        assert_eq!(Link::parse(&a.url()), Ok(a));
    }

    #[test]
    fn the_key_never_shows_in_debug_output() {
        let l = vector();
        let shown = format!("{l:?}");
        assert!(shown.contains("<hidden>"));
        assert!(!shown.contains(&URL_SAFE_NO_PAD.encode(l.key)));
        assert!(!shown.contains("255"), "no raw key bytes: {shown}");
    }

    #[test]
    fn cut_short_or_damaged_links_say_they_are_incomplete() {
        let t = vector().token();
        for broken in [
            t[..t.len() - 1].to_string(),
            t[..10].to_string(),
            format!("{t}AA"),
            "v1.".to_string(),
            format!("v1.{}", t[3..].replace('A', "!")),
        ] {
            assert_eq!(Link::parse(&broken), Err(LinkError::Incomplete), "{broken}");
        }
    }

    #[test]
    fn links_from_a_newer_fuselane_ask_for_an_update() {
        let body = &vector().token()[3..];
        assert_eq!(Link::parse(&format!("v2.{body}")), Err(LinkError::TooNew));
        assert_eq!(Link::parse(&format!("v99.{body}")), Err(LinkError::TooNew));
        // Same version, but a flag this build doesn't know.
        let mut bytes = URL_SAFE_NO_PAD.decode(body).unwrap();
        *bytes.last_mut().unwrap() = 0x22;
        let unknown = format!("v1.{}", URL_SAFE_NO_PAD.encode(&bytes));
        assert_eq!(Link::parse(&unknown), Err(LinkError::TooNew));
    }

    #[test]
    fn other_text_is_not_a_link() {
        let t = vector().token();
        for text in [
            String::new(),
            "hello".into(),
            "https://example.com/".into(),
            format!("https://evil.example/s#{t}"),
            format!("https://arshpunisher.github.io/other#{t}"),
            format!("v0.{}", &t[3..]),
            format!("vx.{}", &t[3..]),
            "magnet:?xt=urn:btih:abc".into(),
        ] {
            assert_eq!(Link::parse(&text), Err(LinkError::NotALink), "{text}");
        }
    }

    #[test]
    fn error_messages_say_what_to_do() {
        assert!(LinkError::Incomplete.to_string().contains("copy it again"));
        assert!(LinkError::TooNew.to_string().contains("Update Fuselane"));
        assert!(LinkError::NotALink.to_string().contains(PAGE));
    }
}
