//! Watching the clipboard for download links (opt-in): which copied text counts.
//!
//! Only a single copied link counts, and only one that looks like a file to
//! download (by its extension) or a magnet. Pages, searches and everything else
//! copied are ignored, so the watcher never interrupts ordinary copying.

/// Copied text longer than this is never a link someone meant to download.
pub const MAX_LEN: usize = 4096;

/// File types worth offering to download. Lower case, without the dot.
#[rustfmt::skip]
const EXTENSIONS: &[&str] = &[
    // Archives and disk images
    "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "lz", "lzma",
    "cab", "iso", "img", "dmg", "vhd", "vhdx", "qcow2", "vmdk", "ova", "wim",
    // Installers and packages
    "exe", "msi", "msix", "appx", "pkg", "deb", "rpm", "appimage", "flatpak", "snap", "apk",
    "aab", "ipa", "jar",
    // Video and audio
    "mp4", "m4v", "mkv", "avi", "mov", "webm", "wmv", "flv", "mpg", "mpeg", "ts", "3gp", "mp3",
    "m4a", "aac", "flac", "wav", "ogg", "opus", "wma",
    // Documents and data
    "pdf", "epub", "mobi", "djvu", "psd", "ai", "csv", "parquet", "sqlite", "db", "bin", "dat",
    "torrent", "safetensors", "gguf", "ckpt", "pt", "onnx", "npz",
];

/// The link to offer when `text` is a single download link, else `None`.
pub fn download_link(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t.len() > MAX_LEN || t.chars().any(char::is_whitespace) {
        return None;
    }
    if t.len() >= 8 && t[..8].eq_ignore_ascii_case("magnet:?") {
        return t
            .to_ascii_lowercase()
            .contains("xt=urn:btih:")
            .then(|| t.to_owned());
    }
    let lower = t.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return None;
    }
    let url = url::Url::parse(t).ok()?;
    url.host_str()?;
    let last = url.path_segments()?.next_back()?;
    let ext = last.rsplit_once('.')?.1.to_ascii_lowercase();
    // Split archives: file.7z.001, file.part2.rar, movie.r00.
    let split = ext.len() == 3 && ext.bytes().all(|b| b.is_ascii_digit())
        || ext.len() == 3 && ext.starts_with('r') && ext[1..].bytes().all(|b| b.is_ascii_digit());
    (EXTENSIONS.contains(&ext.as_str()) || split).then(|| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_links_and_magnets_count() {
        for link in [
            "https://releases.ubuntu.com/26.04/ubuntu-26.04-desktop-amd64.iso",
            "  http://example.com/files/Setup.EXE  ",
            "https://cdn.example.com/a/b/movie.mkv?token=abc&exp=1",
            "https://example.com/archive.7z.001",
            "https://example.com/old.r01",
            "https://huggingface.co/x/y/resolve/main/model.safetensors",
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567&dn=x",
        ] {
            assert!(download_link(link).is_some(), "{link}");
        }
        assert_eq!(
            download_link(" https://example.com/a.zip\n").as_deref(),
            Some("https://example.com/a.zip")
        );
    }

    #[test]
    fn pages_searches_and_other_text_dont() {
        for text in [
            "",
            "hello world",
            "https://example.com/",
            "https://example.com/blog/post",
            "https://example.com/page.html",
            "https://www.google.com/search?q=file.zip",
            "https://example.com/a.zip https://example.com/b.zip",
            "Download https://example.com/a.zip now",
            "ftp://example.com/a.zip",
            "file:///Users/me/a.zip",
            "javascript:alert(1)//a.zip",
            "magnet:?dn=no-hash",
        ] {
            assert_eq!(download_link(text), None, "{text:?}");
        }
        let long = format!("https://example.com/{}.zip", "a".repeat(MAX_LEN));
        assert_eq!(download_link(&long), None);
    }
}
