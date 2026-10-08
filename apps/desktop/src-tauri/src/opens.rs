//! Things the OS asks Fuselane to open: a magnet link, or a .torrent file (from
//! Finder/Explorer "Open with", a double-click, or a browser). They arrive as
//! command-line arguments (Windows, Linux, second launches) or as URLs (macOS).
//! Anything else is ignored: these strings come from outside the app.

use std::path::{Path, PathBuf};

/// Longer than any real magnet link (they carry a hash, a name and trackers).
const MAX_MAGNET: usize = 16 * 1024;

/// What the window should open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Magnet(String),
    TorrentFile(PathBuf),
}

impl Target {
    /// The string handed to the window's New download dialog.
    pub fn as_draft(&self) -> String {
        match self {
            Target::Magnet(m) => m.clone(),
            Target::TorrentFile(p) => p.display().to_string(),
        }
    }
}

fn is_torrent_file(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("torrent"))
        && p.is_file()
}

/// Decodes %XX in a file:// URL path; None for anything malformed.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// One argument or URL from the OS, if it is something Fuselane opens.
pub fn parse(raw: &str) -> Option<Target> {
    let s = raw.trim();
    if s.len() >= 8 && s[..8].eq_ignore_ascii_case("magnet:?") {
        let ok = s.len() <= MAX_MAGNET
            && s.to_ascii_lowercase().contains("xt=urn:btih:")
            && !s.chars().any(char::is_control);
        return ok.then(|| Target::Magnet(s.to_owned()));
    }
    let path = if s.len() >= 7 && s[..7].eq_ignore_ascii_case("file://") {
        // file:///Users/x/a.torrent (macOS, Linux); file://localhost/... too.
        let rest = &s[7..];
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        let decoded = percent_decode(rest)?;
        // file:///C:/x.torrent on Windows: drop the slash before the drive.
        let decoded = if cfg!(windows) {
            decoded.trim_start_matches('/').to_owned()
        } else {
            decoded
        };
        PathBuf::from(decoded)
    } else {
        PathBuf::from(s)
    };
    is_torrent_file(&path).then_some(Target::TorrentFile(path))
}

/// Every target among command-line arguments (the program's own path first is skipped).
pub fn from_args<I: IntoIterator<Item = String>>(args: I) -> Vec<Target> {
    from_args_in(args, None)
}

/// Like `from_args`, with relative paths taken from `cwd` (a second launch's
/// folder, which isn't this process's).
pub fn from_args_in<I: IntoIterator<Item = String>>(args: I, cwd: Option<&Path>) -> Vec<Target> {
    args.into_iter()
        .skip(1)
        .filter_map(|a| {
            let relative = !a.trim().contains(':') && Path::new(a.trim()).is_relative();
            match cwd {
                Some(dir) if relative => parse(&dir.join(a.trim()).to_string_lossy()),
                _ => parse(&a),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAGNET: &str = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=x";

    #[test]
    fn magnets_are_accepted_only_when_they_look_real() {
        assert_eq!(parse(MAGNET), Some(Target::Magnet(MAGNET.into())));
        assert_eq!(
            parse(&format!("  {}  ", MAGNET.to_uppercase())).map(|t| t.as_draft().len()),
            Some(MAGNET.len())
        );
        assert_eq!(parse("magnet:?dn=no-hash"), None);
        assert_eq!(
            parse("magnet:?xt=urn:btih:abc\n--evil"),
            None,
            "control characters"
        );
        assert_eq!(
            parse(&format!("{MAGNET}{}", "a".repeat(MAX_MAGNET))),
            None,
            "too long"
        );
    }

    #[test]
    fn torrent_files_must_exist_and_be_torrent_files() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("My File.torrent");
        std::fs::write(&t, b"d4:infod4:name1:aee").unwrap();
        let upper = dir.path().join("B.TORRENT");
        std::fs::write(&upper, b"d").unwrap();
        let other = dir.path().join("notes.txt");
        std::fs::write(&other, b"x").unwrap();
        std::fs::create_dir(dir.path().join("folder.torrent")).unwrap();

        assert_eq!(
            parse(t.to_str().unwrap()),
            Some(Target::TorrentFile(t.clone()))
        );
        assert!(
            parse(upper.to_str().unwrap()).is_some(),
            "extension case doesn't matter"
        );
        assert_eq!(parse(other.to_str().unwrap()), None);
        assert_eq!(
            parse(dir.path().join("missing.torrent").to_str().unwrap()),
            None
        );
        assert_eq!(
            parse(dir.path().join("folder.torrent").to_str().unwrap()),
            None,
            "a folder"
        );
        assert_eq!(
            parse("https://example.com/a.torrent"),
            None,
            "web links go through the dialog"
        );
        assert_eq!(parse("--self-test"), None);
        assert_eq!(parse(""), None);
    }

    #[cfg(unix)]
    #[test]
    fn file_urls_from_macos_and_linux_are_decoded() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("My File.torrent");
        std::fs::write(&t, b"d").unwrap();
        let url = format!("file://{}", t.display().to_string().replace(' ', "%20"));
        assert_eq!(parse(&url), Some(Target::TorrentFile(t.clone())));
        let local = format!(
            "file://localhost{}",
            t.display().to_string().replace(' ', "%20")
        );
        assert_eq!(parse(&local), Some(Target::TorrentFile(t)));
        assert_eq!(parse("file:///tmp/%zz.torrent"), None, "broken escapes");
    }

    #[test]
    fn a_second_launchs_relative_paths_use_its_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.torrent"), b"d").unwrap();
        let got = from_args_in(["app".to_string(), "a.torrent".into()], Some(dir.path()));
        assert_eq!(got, vec![Target::TorrentFile(dir.path().join("a.torrent"))]);
        assert!(
            from_args(["app".to_string(), "a.torrent".into()]).is_empty()
                || std::path::Path::new("a.torrent").is_file()
        );
    }

    #[test]
    fn arguments_skip_the_program_and_keep_order() {
        let got = from_args([
            "fuselane-desktop".to_string(),
            "--flag".into(),
            MAGNET.into(),
        ]);
        assert_eq!(got, vec![Target::Magnet(MAGNET.into())]);
        assert!(
            from_args([MAGNET.to_string()]).is_empty(),
            "the first argument is the program"
        );
    }
}
