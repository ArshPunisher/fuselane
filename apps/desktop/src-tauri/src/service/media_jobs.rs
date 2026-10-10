//! Video and audio from pages (B10.4), in the queue: a choice from yt-dlp
//! becomes one download, or video and audio as a two-item group that ffmpeg
//! joins into one file once both have finished.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use fuselane_core::Status;

use super::{AddRequest, Service, UiError, lock};
use crate::media::{self, MediaInfo, Plan};

/// Links from yt-dlp expire; a choice is used within this time or asked again.
const FRESH: Duration = Duration::from_secs(10 * 60);

/// Which helper programs were found.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaTools {
    pub yt_dlp: Option<String>,
    pub ffmpeg: Option<String>,
}

/// Video and audio waiting to be joined (kept across restarts).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Join {
    pub video: i64,
    pub audio: i64,
    pub name: String,
}

pub(super) type Probes = HashMap<String, (Instant, String, HashMap<String, Plan>)>;

fn need_yt_dlp() -> UiError {
    UiError::new(
        "needs-yt-dlp",
        "Videos from pages need the free yt-dlp tool, which isn't installed.",
        Some(if cfg!(target_os = "macos") {
            "Install it with Homebrew (brew install yt-dlp), then try again."
        } else if cfg!(windows) {
            "Install it with winget (winget install yt-dlp), then try again."
        } else {
            "Install it from your package manager (yt-dlp), then try again."
        }),
    )
}

impl Service {
    pub fn media_tools(&self) -> MediaTools {
        let show = |p: Option<std::path::PathBuf>| p.map(|p| p.to_string_lossy().into_owned());
        MediaTools {
            yt_dlp: show(media::find("yt-dlp")),
            ffmpeg: show(media::find("ffmpeg")),
        }
    }

    /// What a page offers (titles and qualities); nothing is downloaded.
    pub async fn media_info(&self, url: &str) -> Result<MediaInfo, UiError> {
        let url = url.trim();
        fuselane_core::runner::parse_link(url).map_err(|m| {
            UiError::new("bad-link", m, Some("Links start with http:// or https://."))
        })?;
        let yt = media::find("yt-dlp").ok_or_else(need_yt_dlp)?;
        let json = media::probe(&yt, url).await.map_err(|why| {
            UiError::new(
                "no-media",
                format!("No video found there: {why}."),
                Some("Paste the link of the video's own page."),
            )
        })?;
        let (info, plans) =
            media::plans_from(&json, media::find("ffmpeg").is_some()).ok_or_else(|| {
                UiError::new(
                    "no-media",
                    "That page's video can't be downloaded in parts (it streams in pieces only).",
                    None,
                )
            })?;
        let mut probes = lock(&self.media_probes);
        probes.retain(|_, (at, _, _)| at.elapsed() < FRESH);
        probes.insert(url.to_string(), (Instant::now(), info.title.clone(), plans));
        Ok(info)
    }

    /// Adds the chosen quality: one download, or video and audio as a group
    /// that's joined when both finish. Returns the new downloads' ids.
    pub fn media_add(
        self: &Arc<Self>,
        url: &str,
        option: &str,
        dir: Option<&str>,
    ) -> Result<Vec<i64>, UiError> {
        let (title, plan) = lock(&self.media_probes)
            .get(url.trim())
            .filter(|(at, _, _)| at.elapsed() < FRESH)
            .and_then(|(_, t, plans)| plans.get(option).map(|p| (t.clone(), p.clone())))
            .ok_or_else(|| {
                UiError::new(
                    "media-expired",
                    "That choice has expired. Look the page up again.",
                    None,
                )
            })?;
        let base = fuselane_storage::names::sanitize(&title);
        let add = |s: &media::Stream, name: String| {
            self.add_with(
                &s.url,
                dir,
                &AddRequest {
                    name: Some(name),
                    allow_duplicate: true,
                    headers: s.headers.clone(),
                    ..AddRequest::default()
                },
            )
        };
        let Some(audio) = &plan.audio else {
            let suffix = if plan.label == "audio" {
                String::new()
            } else {
                format!(" ({})", plan.label)
            };
            return Ok(vec![add(
                &plan.video,
                format!("{base}{suffix}.{}", plan.out_ext),
            )?]);
        };
        let v = add(
            &plan.video,
            format!("{base} ({}) video.{}", plan.label, plan.video.ext),
        )?;
        let a = add(
            audio,
            format!("{base} ({}) audio.{}", plan.label, audio.ext),
        )?;
        let _ = self
            .store
            .create_group(&title.chars().take(80).collect::<String>(), &[v, a]);
        let mut joins = self.joins();
        joins.push(Join {
            video: v,
            audio: a,
            name: format!("{base} ({}).{}", plan.label, plan.out_ext),
        });
        self.save_joins(&joins);
        self.publish_jobs();
        Ok(vec![v, a])
    }

    fn joins(&self) -> Vec<Join> {
        self.store
            .setting("media_joins")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save_joins(&self, joins: &[Join]) {
        if let Ok(json) = serde_json::to_string(joins) {
            let _ = self.store.set_setting("media_joins", &json);
        }
    }

    /// After a download finishes: joins video and audio whose both halves are
    /// done. The joined file takes the video's place in the list.
    pub(super) fn join_finished(self: &Arc<Self>) {
        let joins = self.joins();
        if joins.is_empty() {
            return;
        }
        let mut keep = vec![];
        for j in joins {
            let (v, a) = (self.store.get(j.video), self.store.get(j.audio));
            let (Ok(v), Ok(a)) = (v, a) else {
                continue; // one was removed: nothing to join
            };
            if v.status != Status::Completed || a.status != Status::Completed {
                keep.push(j);
                continue;
            }
            let (Some(vp), Some(ap)) = (v.final_path.clone(), a.final_path.clone()) else {
                continue;
            };
            let Some(ffmpeg) = media::find("ffmpeg") else {
                let _ = self.store.set_error(
                    v.id,
                    "Saved as separate video and audio: install ffmpeg to join them next time.",
                    "join",
                );
                continue;
            };
            let me = self.clone();
            tokio::task::spawn_blocking(move || {
                let dir = vp
                    .parent()
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or_default();
                let out = super::free_name(&dir, &j.name);
                match media::join(&ffmpeg, &vp, &ap, &out) {
                    Ok(()) => {
                        let size = std::fs::metadata(&out).map_or(0, |m| m.len());
                        let _ = me.store.set_finished(j.video, &out, size);
                        let _ = std::fs::remove_file(&vp);
                        let _ = std::fs::remove_file(&ap);
                        let _ = me.store.delete(j.audio);
                        if let Some(g) = v.group_id {
                            let _ = me.store.ungroup(g);
                        }
                    }
                    Err(why) => {
                        let _ = me.store.set_error(
                            j.video,
                            &format!("{why}; the video and audio are kept as they are."),
                            "join",
                        );
                        crate::reports::log(&format!("join failed for download #{}", j.video));
                    }
                }
                me.publish_jobs();
            });
        }
        self.save_joins(&keep);
    }
}
