//! Video and audio from pages (B10.4). Fuselane never bundles a site-specific
//! downloader: when the person has the free yt-dlp installed, it's asked only
//! for a page's real media links; Fuselane's engine then downloads them over
//! every network. ffmpeg, when installed, joins separate video and audio into
//! one file without re-encoding.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

/// The page and the choices offered for it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub title: String,
    pub site: String,
    /// Seconds.
    pub duration: Option<f64>,
    pub options: Vec<MediaOption>,
    /// HD needs ffmpeg to join video and audio; without it, say so.
    pub hd_needs_ffmpeg: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaOption {
    pub id: String,
    /// "1080p", "Audio only".
    pub label: String,
    /// "MP4, about 412 MB".
    pub detail: String,
    pub size: Option<u64>,
}

/// One stream to download.
#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    pub url: String,
    pub ext: String,
    pub headers: Vec<(String, String)>,
    pub size: Option<u64>,
}

/// What a choice downloads: one stream, or video and audio to join.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub video: Stream,
    pub audio: Option<Stream>,
    /// The joined (or single) file's extension.
    pub out_ext: String,
    pub label: String,
    /// Converted to MP3 by ffmpeg once downloaded.
    pub mp3: bool,
}

/// Headers the engine may pass on (yt-dlp asks for a few it doesn't need).
const PASSED: &[&str] = &[
    "user-agent",
    "accept",
    "accept-language",
    "referer",
    "cookie",
];

/// Finds a program by name on the PATH or where package managers put it.
pub fn find(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local/bin"));
        dirs.push(home.join("bin"));
        dirs.push(home.join("scoop/shims"));
    }
    for d in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/snap/bin",
    ] {
        dirs.push(PathBuf::from(d));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft/WinGet/Links"));
    }
    dirs.into_iter().map(|d| d.join(&exe)).find(|p| p.is_file())
}

fn stream(f: &serde_json::Value) -> Option<Stream> {
    let url = f.get("url")?.as_str()?;
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return None;
    }
    let headers = f
        .get("http_headers")
        .and_then(|h| h.as_object())
        .map(|h| {
            h.iter()
                .filter(|(k, _)| PASSED.contains(&k.to_ascii_lowercase().as_str()))
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    Some(Stream {
        url: url.to_string(),
        ext: f.get("ext")?.as_str()?.to_string(),
        headers,
        size: f
            .get("filesize")
            .and_then(serde_json::Value::as_u64)
            .or_else(|| f.get("filesize_approx").and_then(serde_json::Value::as_u64)),
    })
}

fn codec(f: &serde_json::Value, key: &str) -> String {
    f.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("none")
        .to_string()
}

/// Only plain HTTP(S) downloads (not HLS/DASH manifests) can be split over networks.
fn direct(f: &serde_json::Value) -> bool {
    matches!(
        f.get("protocol").and_then(|p| p.as_str()),
        Some("https" | "http")
    )
}

fn about(size: Option<u64>) -> String {
    match size {
        Some(b) if b >= 1 << 30 => format!(", about {:.1} GB", b as f64 / (1u64 << 30) as f64),
        Some(b) => format!(", about {} MB", (b + (1 << 19)) >> 20),
        None => String::new(),
    }
}

/// The choices for a page from yt-dlp's JSON: each height once (best direct
/// video, preferring MP4 so the joined file plays everywhere), audio only, and
/// without ffmpeg only formats that already carry both.
pub fn plans_from(
    json: &serde_json::Value,
    ffmpeg: bool,
) -> Option<(MediaInfo, HashMap<String, Plan>)> {
    let formats = json.get("formats")?.as_array()?;
    let title = json
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("Video")
        .to_string();
    let site = json
        .get("extractor_key")
        .or_else(|| json.get("extractor"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let duration = json.get("duration").and_then(serde_json::Value::as_f64);
    let direct: Vec<&serde_json::Value> = formats.iter().filter(|f| direct(f)).collect();
    let is_video = |f: &&serde_json::Value| codec(f, "vcodec") != "none";
    let is_audio = |f: &&serde_json::Value| codec(f, "acodec") != "none";
    let height = |f: &serde_json::Value| {
        f.get("height")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0)
    };
    // Best audio: m4a first (joins into MP4), then the biggest bitrate.
    let best_audio = direct
        .iter()
        .filter(|f| is_audio(f) && !is_video(f))
        .max_by(|a, b| {
            let m4a = |f: &serde_json::Value| f.get("ext").and_then(|e| e.as_str()) == Some("m4a");
            let br = |f: &serde_json::Value| {
                f.get("abr")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0)
            };
            (m4a(a), br(a))
                .partial_cmp(&(m4a(b), br(b)))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied();
    let mut plans = HashMap::new();
    let mut options = vec![];
    let mut heights: Vec<u64> = direct
        .iter()
        .filter(|f| is_video(f))
        .map(|f| height(f))
        .filter(|h| *h > 0)
        .collect();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();
    let mut hd_needs_ffmpeg = false;
    for h in heights {
        let at: Vec<&&serde_json::Value> = direct
            .iter()
            .filter(|f| is_video(f) && height(f) == h)
            .collect();
        // Carries its own sound: works without ffmpeg.
        let combined = at.iter().find(|f| is_audio(f)).copied();
        let video_only = at
            .iter()
            .filter(|f| !is_audio(f))
            .max_by_key(|f| {
                let v = codec(f, "vcodec");
                (
                    v.starts_with("avc1") as u8 * 2 + v.starts_with("av01") as u8,
                    f.get("tbr")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0) as u64,
                )
            })
            .copied();
        let label = format!("{h}p");
        let plan = match (ffmpeg, video_only, best_audio, combined) {
            (true, Some(v), Some(a), _) => {
                let (v, a) = (stream(v)?, stream(a)?);
                let out_ext = if v.ext == "mp4" && a.ext == "m4a" {
                    "mp4"
                } else {
                    "mkv"
                }
                .to_string();
                Plan {
                    video: v,
                    audio: Some(a),
                    out_ext,
                    label: label.clone(),
                    mp3: false,
                }
            }
            (_, _, _, Some(c)) => {
                let c = stream(c)?;
                Plan {
                    out_ext: c.ext.clone(),
                    video: c,
                    audio: None,
                    label: label.clone(),
                    mp3: false,
                }
            }
            _ => {
                hd_needs_ffmpeg = true;
                continue;
            }
        };
        let size = match (&plan.video.size, plan.audio.as_ref().and_then(|a| a.size)) {
            (Some(v), Some(a)) => Some(v + a),
            (Some(v), None) if plan.audio.is_none() => Some(*v),
            _ => None,
        };
        let id = format!("v{h}");
        options.push(MediaOption {
            id: id.clone(),
            label,
            detail: format!("{}{}", plan.out_ext.to_uppercase(), about(size)),
            size,
        });
        plans.insert(id, plan);
    }
    if let Some(a) = best_audio.and_then(stream) {
        let size = a.size;
        options.push(MediaOption {
            id: "a".into(),
            label: "Audio only".into(),
            detail: format!("{}{}", a.ext.to_uppercase(), about(size)),
            size,
        });
        if ffmpeg {
            options.push(MediaOption {
                id: "mp3".into(),
                label: "Audio (MP3)".into(),
                detail: format!("MP3, plays everywhere{}", about(size)),
                size,
            });
            plans.insert(
                "mp3".into(),
                Plan {
                    out_ext: "mp3".into(),
                    video: a.clone(),
                    audio: None,
                    label: "audio".into(),
                    mp3: true,
                },
            );
        }
        plans.insert(
            "a".into(),
            Plan {
                out_ext: a.ext.clone(),
                video: a,
                audio: None,
                label: "audio".into(),
                mp3: false,
            },
        );
    }
    if options.is_empty() {
        return None;
    }
    Some((
        MediaInfo {
            title,
            site,
            duration,
            options,
            hd_needs_ffmpeg,
        },
        plans,
    ))
}

/// Asks yt-dlp about a page (never downloads with it). The link is passed
/// after `--`, so it can't be read as an option.
pub async fn probe(yt_dlp: &Path, url: &str) -> Result<serde_json::Value, String> {
    let run = tokio::process::Command::new(yt_dlp)
        .args(["-J", "--no-playlist", "--no-warnings", "--", url])
        .kill_on_drop(true)
        .output();
    let out = tokio::time::timeout(Duration::from_secs(60), run)
        .await
        .map_err(|_| "the page took too long to read".to_string())?
        .map_err(|e| format!("yt-dlp couldn't start ({e})"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        let line = why
            .lines()
            .rev()
            .find(|l| l.starts_with("ERROR"))
            .unwrap_or("it found no video there");
        return Err(line
            .trim_start_matches("ERROR:")
            .trim()
            .chars()
            .take(200)
            .collect());
    }
    serde_json::from_slice(&out.stdout).map_err(|_| "yt-dlp's answer couldn't be read".into())
}

/// Joins video and audio into `out` without re-encoding.
pub fn join(ffmpeg: &Path, video: &Path, audio: &Path, out: &Path) -> Result<(), String> {
    let status = std::process::Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(video)
        .arg("-i")
        .arg(audio)
        .args(["-map", "0:v:0", "-map", "1:a:0", "-c", "copy"])
        .arg(out)
        .status()
        .map_err(|e| format!("ffmpeg couldn't start ({e})"))?;
    if status.success() {
        Ok(())
    } else {
        Err("ffmpeg couldn't join them".into())
    }
}

/// Turns any audio (or a video's sound) into an MP3 at good quality.
pub fn to_mp3(ffmpeg: &Path, input: &Path, out: &Path) -> Result<(), String> {
    let status = std::process::Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args(["-vn", "-codec:a", "libmp3lame", "-q:a", "2"])
        .arg(out)
        .status()
        .map_err(|e| format!("ffmpeg couldn't start ({e})"))?;
    if status.success() {
        Ok(())
    } else {
        Err("ffmpeg couldn't make an MP3 (its MP3 encoder may be missing)".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> serde_json::Value {
        serde_json::json!({
            "title": "Big Buck Bunny", "extractor_key": "Youtube", "duration": 635.0,
            "formats": [
                {"format_id": "18", "ext": "mp4", "vcodec": "avc1", "acodec": "mp4a", "height": 360,
                 "protocol": "https", "url": "https://v.example/18", "filesize": 30000000,
                 "http_headers": {"User-Agent": "UA", "Sec-Fetch-Mode": "navigate"}},
                {"format_id": "140", "ext": "m4a", "vcodec": "none", "acodec": "mp4a", "abr": 129.0,
                 "protocol": "https", "url": "https://v.example/140", "filesize": 10000000},
                {"format_id": "251", "ext": "webm", "vcodec": "none", "acodec": "opus", "abr": 140.0,
                 "protocol": "https", "url": "https://v.example/251", "filesize": 11000000},
                {"format_id": "137", "ext": "mp4", "vcodec": "avc1.640028", "acodec": "none", "height": 1080,
                 "protocol": "https", "url": "https://v.example/137", "filesize": 200000000, "tbr": 4000.0},
                {"format_id": "248", "ext": "webm", "vcodec": "vp9", "acodec": "none", "height": 1080,
                 "protocol": "https", "url": "https://v.example/248", "filesize": 150000000, "tbr": 3000.0},
                {"format_id": "312", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "height": 1080,
                 "protocol": "m3u8_native", "url": "https://v.example/312.m3u8"},
                {"format_id": "sb0", "ext": "mhtml", "vcodec": "none", "acodec": "none",
                 "protocol": "mhtml", "url": "https://v.example/sb"}
            ]
        })
    }

    #[test]
    fn with_ffmpeg_hd_joins_mp4_video_and_m4a_audio() {
        let (info, plans) = plans_from(&page(), true).unwrap();
        let labels: Vec<_> = info.options.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, ["1080p", "360p", "Audio only", "Audio (MP3)"]);
        assert!(plans["mp3"].mp3);
        let hd = &plans["v1080"];
        assert_eq!(
            hd.video.url, "https://v.example/137",
            "MP4 video preferred, never HLS"
        );
        assert_eq!(
            hd.audio.as_ref().unwrap().url,
            "https://v.example/140",
            "m4a joins into MP4"
        );
        assert_eq!(hd.out_ext, "mp4");
        assert_eq!(info.options[0].detail, "MP4, about 200 MB");
        assert!(!info.hd_needs_ffmpeg);
        // Headers the engine can't pass on are dropped.
        assert_eq!(
            plans["v360"].video.headers,
            vec![("User-Agent".to_string(), "UA".to_string())]
        );
    }

    #[test]
    fn without_ffmpeg_only_formats_with_their_own_sound_are_offered() {
        let (info, plans) = plans_from(&page(), false).unwrap();
        let labels: Vec<_> = info.options.iter().map(|o| o.label.as_str()).collect();
        assert_eq!(labels, ["360p", "Audio only"]);
        assert!(info.hd_needs_ffmpeg, "say that HD needs ffmpeg");
        assert!(plans["v360"].audio.is_none());
    }

    #[test]
    fn a_page_without_direct_media_has_no_choices() {
        let j = serde_json::json!({"title": "x", "formats": [
            {"ext": "mp4", "vcodec": "avc1", "acodec": "mp4a", "height": 720, "protocol": "m3u8_native", "url": "https://a/b.m3u8"}
        ]});
        assert!(plans_from(&j, true).is_none());
        assert!(plans_from(&serde_json::json!({"title": "no formats"}), true).is_none());
    }

    #[test]
    fn programs_are_found_where_package_managers_put_them() {
        assert!(find("definitely-not-a-real-program-xyz").is_none());
        #[cfg(unix)]
        assert!(find("sh").is_some());
    }
}
