//! Video and audio from pages (B10.4), in the queue: a choice from yt-dlp
//! becomes one download, or video and audio that ffmpeg joins into one file
//! once both have finished. The two show as one item (REVIEW-2026-10-11 B5):
//! the sound part stays out of the list, and a video never shows as done
//! without its sound unless the person chose that.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use fuselane_core::Status;

use super::{AddRequest, JobView, Service, UiError, lock};
use crate::helpers::{HelperError, Tool};
use crate::media::{self, MediaInfo, Plan};

/// Links from yt-dlp expire; a choice is used within this time or asked again.
const FRESH: Duration = Duration::from_secs(10 * 60);
/// A page looked up this recently shows its qualities again without asking
/// yt-dlp (Back, or the extension asking twice); half of FRESH, so a choice
/// made from it still has time to be downloaded.
const SHOW_AGAIN: Duration = Duration::from_secs(5 * 60);

/// Which helper programs were found.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaTools {
    pub yt_dlp: Option<String>,
    pub ffmpeg: Option<String>,
}

/// Video and audio waiting to be joined (kept across restarts).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Join {
    pub video: i64,
    pub audio: i64,
    pub name: String,
    /// Not a join: `video` becomes an MP3 (then `audio` is the same id).
    #[serde(default)]
    pub mp3: bool,
    /// Where the sound came from, to fetch it again if its part is lost.
    #[serde(default)]
    pub audio_url: Option<String>,
    #[serde(default)]
    pub audio_headers: Vec<(String, String)>,
    #[serde(default)]
    pub audio_name: Option<String>,
}

/// Where a video waiting for its sound is, as the list shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    /// The video part is done; the sound is still downloading.
    Getting,
    /// Both parts are done and ffmpeg is joining them.
    Joining,
    /// The sound part is gone or failed for good: get it again, or keep the
    /// video without sound.
    Missing,
}

impl Sound {
    pub fn word(self) -> &'static str {
        match self {
            Sound::Getting => "getting",
            Sound::Joining => "joining",
            Sound::Missing => "missing",
        }
    }
}

pub(super) type Probes = HashMap<String, (Instant, String, HashMap<String, Plan>)>;

/// How to install yt-dlp by hand, for the rare computer Fuselane can't set
/// it up on by itself.
fn by_hand() -> &'static str {
    if cfg!(target_os = "macos") {
        "Install it with Homebrew (brew install yt-dlp), then try again."
    } else if cfg!(windows) {
        "Install it with winget (winget install yt-dlp), then try again."
    } else {
        "Install it from your package manager (yt-dlp), then try again."
    }
}

/// What the window says when yt-dlp couldn't be set up. Only shown after
/// Fuselane really tried; each one offers Try again.
fn setup_error(e: &HelperError) -> UiError {
    match e {
        HelperError::NoBuild => UiError::new(
            "needs-yt-dlp",
            "Videos from pages need the free yt-dlp tool, and Fuselane can't set it up on this computer by itself.",
            Some(by_hand()),
        ),
        HelperError::Download(_) => UiError::new(
            "tools-download",
            "Fuselane couldn't download yt-dlp, the free tool it uses to read video pages.",
            Some("Check that this computer is online, then press Try again. It's needed only once."),
        ),
        HelperError::Damaged => UiError::new(
            "tools-damaged",
            "The yt-dlp download didn't match its published checksum, so Fuselane didn't use it.",
            Some("Press Try again in a few minutes. If it keeps happening, report a problem."),
        ),
        HelperError::Disk(_) => UiError::new(
            "tools-disk",
            "Fuselane couldn't save yt-dlp in its folder.",
            Some("Make some free space on this computer, then press Try again."),
        ),
        HelperError::WontRun(_) => UiError {
            code: "tools-blocked",
            message: "yt-dlp was downloaded but won't run on this computer. Security software may be blocking it.".into(),
            hint: Some(format!(
                "Allow it in your security software and press Try again. Or: {}",
                by_hand()
            )),
        },
    }
}

/// The helpers one page lookup uses.
struct VideoTools {
    yt_dlp: std::path::PathBuf,
    /// Fuselane's own copy (it can update itself and gets deno).
    managed: bool,
    ffmpeg: Option<std::path::PathBuf>,
    deno: Option<std::path::PathBuf>,
}

impl VideoTools {
    /// Options Fuselane adds for its own yt-dlp: deno for YouTube's checks.
    fn extra(&self) -> Vec<String> {
        match (&self.deno, self.managed) {
            (Some(d), true) => vec![
                "--js-runtimes".into(),
                format!("deno:{}", d.to_string_lossy()),
            ],
            _ => vec![],
        }
    }
}

impl Service {
    pub fn media_tools(&self) -> MediaTools {
        let show = |p: Option<std::path::PathBuf>| p.map(|p| p.to_string_lossy().into_owned());
        let h = self.helpers();
        MediaTools {
            yt_dlp: show(h.locate(Tool::YtDlp)),
            ffmpeg: show(h.locate(Tool::Ffmpeg)),
        }
    }

    /// The helpers in use (Fuselane's own, or ones the person installed).
    pub(crate) fn helpers(&self) -> Arc<crate::helpers::Helpers> {
        lock(&self.helpers).clone()
    }

    #[cfg(test)]
    pub(crate) fn set_helpers(&self, h: crate::helpers::Helpers) {
        *lock(&self.helpers) = Arc::new(h);
    }

    /// yt-dlp to read the page with, plus ffmpeg and deno when they can be
    /// had. Whatever is missing is fetched now, all at once; only a missing
    /// yt-dlp stops the lookup (without ffmpeg, HD isn't offered; without
    /// deno, yt-dlp still reads most pages).
    async fn video_tools(&self) -> Result<VideoTools, UiError> {
        let h = self.helpers();
        let have_yt = h.locate(Tool::YtDlp);
        // A copy the person installed is used as they set it up.
        let wants_deno = have_yt.is_none() || h.managed(Tool::YtDlp).is_some();
        let (yt, ffmpeg, deno) = tokio::join!(
            async {
                match have_yt {
                    Some(p) => Ok(p),
                    None => h.install(Tool::YtDlp).await,
                }
            },
            async {
                match h.locate(Tool::Ffmpeg) {
                    Some(p) => Some(p),
                    None => h.install(Tool::Ffmpeg).await.ok(),
                }
            },
            async {
                match h.managed(Tool::Deno) {
                    Some(p) => Some(p),
                    None if wants_deno => h.install(Tool::Deno).await.ok(),
                    None => None,
                }
            },
        );
        let yt_dlp = yt.map_err(|e| setup_error(&e))?;
        Ok(VideoTools {
            managed: h.managed(Tool::YtDlp).as_deref() == Some(yt_dlp.as_path()),
            yt_dlp,
            ffmpeg,
            deno,
        })
    }

    /// What a page offers (titles and qualities); nothing is downloaded.
    pub async fn media_info(&self, url: &str) -> Result<MediaInfo, UiError> {
        let url = url.trim();
        fuselane_core::runner::parse_link(url).map_err(|m| {
            UiError::new("bad-link", m, Some("Links start with http:// or https://."))
        })?;
        // The same page asked for twice at once runs yt-dlp once: the second ask
        // waits here, then finds the first one's answer.
        let turn = lock(&self.media_lookups)
            .entry(url.to_string())
            .or_default()
            .clone();
        let info = {
            let _turn = turn.lock().await;
            match self.media_seen(url) {
                Some(info) => Ok(info),
                None => self.look_up_media(url).await,
            }
        };
        let mut lookups = lock(&self.media_lookups);
        if Arc::strong_count(&turn) <= 2 {
            lookups.remove(url); // nobody else waiting
        }
        info
    }

    /// The qualities found for `url` a moment ago, if still fresh.
    fn media_seen(&self, url: &str) -> Option<MediaInfo> {
        let mut seen = lock(&self.media_seen);
        seen.retain(|_, (at, _)| at.elapsed() < SHOW_AGAIN);
        seen.get(url).map(|(_, info)| info.clone())
    }

    async fn look_up_media(&self, url: &str) -> Result<MediaInfo, UiError> {
        let mut tools = self.video_tools().await?;
        let json = match media::probe(&tools.yt_dlp, &tools.extra(), url).await {
            Ok(json) => Ok(json),
            // An old yt-dlp is the usual reason a video page won't read: update
            // Fuselane's own copy, or fetch one next to the person's (theirs may
            // be an old one from a package manager), then ask once more.
            Err(why) if media::may_be_stale(&why) && self.newer_yt_dlp(&tools).await => {
                tools = self.video_tools().await?;
                media::probe(&tools.yt_dlp, &tools.extra(), url).await
            }
            Err(why) => Err(why),
        }
        .map_err(|why| {
            UiError::new(
                "no-media",
                format!("No video found there: {why}."),
                Some("Paste the link of the video's own page."),
            )
        })?;
        let (info, plans) = media::plans_from(&json, tools.ffmpeg.is_some()).ok_or_else(|| {
            UiError::new(
                "no-media",
                "That page's video can't be downloaded in parts (it streams in pieces only).",
                None,
            )
        })?;
        let mut probes = lock(&self.media_probes);
        probes.retain(|_, (at, _, _)| at.elapsed() < FRESH);
        probes.insert(url.to_string(), (Instant::now(), info.title.clone(), plans));
        drop(probes);
        lock(&self.media_seen).insert(url.to_string(), (Instant::now(), info.clone()));
        Ok(info)
    }

    /// Gets a newer yt-dlp than the one that just failed. True when there is one.
    async fn newer_yt_dlp(&self, tools: &VideoTools) -> bool {
        let h = self.helpers();
        if tools.managed {
            h.update_yt_dlp().await
        } else {
            h.can_fetch(Tool::YtDlp) && h.install(Tool::YtDlp).await.is_ok()
        }
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
            if plan.mp3 {
                let id = add(&plan.video, format!("{base}.{}", plan.video.ext))?;
                self.add_join(Join {
                    video: id,
                    audio: id,
                    name: format!("{base}.mp3"),
                    mp3: true,
                    ..Join::default()
                });
                return Ok(vec![id]);
            }
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
        let audio_name = format!("{base} ({}) audio.{}", plan.label, audio.ext);
        let a = add(audio, audio_name.clone())?;
        // One item, not a group: the sound part stays out of the list.
        self.add_join(Join {
            video: v,
            audio: a,
            name: format!("{base} ({}).{}", plan.label, plan.out_ext),
            mp3: false,
            audio_url: Some(audio.url.clone()),
            audio_headers: audio.headers.clone(),
            audio_name: Some(audio_name),
        });
        self.publish_jobs();
        Ok(vec![v])
    }

    fn joins(&self) -> Vec<Join> {
        self.store
            .setting("media_joins")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn add_join(&self, j: Join) {
        let _held = lock(&self.joins_lock);
        let mut joins = self.joins();
        joins.push(j);
        self.save_joins(&joins);
    }

    /// Changes the saved joins under the lock.
    fn edit_joins<R>(&self, f: impl FnOnce(&mut Vec<Join>) -> R) -> R {
        let _held = lock(&self.joins_lock);
        let mut joins = self.joins();
        let r = f(&mut joins);
        self.save_joins(&joins);
        r
    }

    /// Video-and-sound joins still to happen, by the video's id.
    pub(super) fn sound_joins(&self) -> HashMap<i64, Join> {
        self.joins()
            .into_iter()
            .filter(|j| !j.mp3)
            .map(|j| (j.video, j))
            .collect()
    }

    /// The hidden sound part of a video, while they're still to be joined.
    pub(super) fn sound_part(&self, video: i64) -> Option<i64> {
        self.sound_joins().get(&video).map(|j| j.audio)
    }

    /// Forgets the join of `video` (removed, or kept without sound).
    pub(super) fn drop_join(&self, video: i64) -> Option<Join> {
        self.edit_joins(|joins| {
            let at = joins.iter().position(|j| j.video == video)?;
            Some(joins.remove(at))
        })
    }

    /// "Save without sound": the video part becomes the finished file, named
    /// like the joined one would have been (with the video's own extension).
    pub fn media_keep_silent(self: &Arc<Self>, video: i64) -> Result<(), UiError> {
        let job = self.store.get(video).map_err(|_| super::not_found(video))?;
        let (Status::Completed, Some(path)) = (job.status, job.final_path.clone()) else {
            return Err(UiError::new(
                "not-finished",
                "The video part hasn't finished yet.",
                None,
            ));
        };
        let Some(j) = self.drop_join(video) else {
            return Ok(());
        };
        if j.audio != video {
            let _ = self.remove(j.audio);
        }
        // "Talk (1080p).mkv" was the plan; without sound it keeps the video's type.
        let stem = j.name.rsplit_once('.').map_or(j.name.as_str(), |(s, _)| s);
        let ext = path.extension().map(|e| e.to_string_lossy().into_owned());
        let wanted = match ext {
            Some(e) => format!("{stem}.{e}"),
            None => stem.to_string(),
        };
        if let Some(dir) = path.parent()
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy() != wanted)
        {
            let to = super::free_name(dir, &wanted);
            if std::fs::rename(&path, &to).is_ok() {
                let size = std::fs::metadata(&to).map_or(0, |m| m.len());
                let _ = self.store.set_finished(video, &to, size);
            }
        }
        self.publish_jobs();
        Ok(())
    }

    /// "Get sound again": the sound part is downloaded anew from the link it
    /// came from (links from yt-dlp expire; then the page is looked up again).
    pub fn media_get_sound(self: &Arc<Self>, video: i64) -> Result<i64, UiError> {
        let j = self.sound_joins().remove(&video).ok_or_else(|| {
            UiError::new(
                "no-join",
                "This download isn't waiting for its sound.",
                None,
            )
        })?;
        let job = self.store.get(video).map_err(|_| super::not_found(video))?;
        let Some(url) = j.audio_url.clone() else {
            return Err(UiError::new(
                "media-expired",
                "The sound's link wasn't kept. Look the page up again.",
                None,
            ));
        };
        let dir = job
            .final_path
            .as_deref()
            .and_then(std::path::Path::parent)
            .map_or_else(|| job.dir.clone(), std::path::Path::to_path_buf);
        let fresh = self.add_with(
            &url,
            Some(&dir.to_string_lossy()),
            &AddRequest {
                name: j.audio_name.clone(),
                allow_duplicate: true,
                headers: j.audio_headers.clone(),
                ..AddRequest::default()
            },
        )?;
        let old = j.audio;
        self.edit_joins(|joins| {
            if let Some(x) = joins.iter_mut().find(|x| x.video == video) {
                x.audio = fresh;
            }
        });
        if old != video && self.store.get(old).is_ok() {
            let _ = self.remove(old);
        }
        self.publish_jobs();
        Ok(fresh)
    }

    fn save_joins(&self, joins: &[Join]) {
        if let Ok(json) = serde_json::to_string(joins) {
            let _ = self.store.set_setting("media_joins", &json);
        }
    }

    /// After a download finishes: joins video and audio whose both halves are
    /// done. The joined file takes the video's place in the list.
    pub(super) fn join_finished(self: &Arc<Self>) {
        // Taken out of the saved list under the lock: each join starts once.
        let _held = lock(&self.joins_lock);
        let joins = self.joins();
        if joins.is_empty() {
            return;
        }
        let mut keep = vec![];
        let mut fetching = false;
        for j in joins {
            let (v, a) = (self.store.get(j.video), self.store.get(j.audio));
            let Ok(v) = v else {
                continue; // the video was removed: nothing to join
            };
            let Ok(a) = a else {
                keep.push(j); // the sound is gone: the list offers to get it again
                continue;
            };
            if v.status != Status::Completed || a.status != Status::Completed {
                keep.push(j);
                continue;
            }
            let (Some(vp), Some(ap)) = (v.final_path.clone(), a.final_path.clone()) else {
                continue;
            };
            let helpers = self.helpers();
            let Some(ffmpeg) = helpers.locate(Tool::Ffmpeg) else {
                // Fetch ffmpeg (once), then come back to join; the item shows
                // "joining" meanwhile. Only if that fails are the parts kept apart.
                if helpers.can_fetch(Tool::Ffmpeg)
                    && !self
                        .ffmpeg_gave_up
                        .load(std::sync::atomic::Ordering::SeqCst)
                {
                    keep.push(j);
                    if !fetching {
                        fetching = true;
                        let me = self.clone();
                        tokio::spawn(async move {
                            let got = me.helpers().install(Tool::Ffmpeg).await;
                            me.ffmpeg_gave_up
                                .store(got.is_err(), std::sync::atomic::Ordering::SeqCst);
                            me.join_finished();
                            me.ffmpeg_gave_up
                                .store(false, std::sync::atomic::Ordering::SeqCst);
                            me.publish_jobs();
                        });
                    }
                    continue;
                }
                let _ = self.store.set_error(
                    v.id,
                    "Saved as separate video and audio: ffmpeg, which joins them, couldn't be set up. Check that this computer is online.",
                    "join",
                );
                continue;
            };
            let me = self.clone();
            if !j.mp3 {
                lock(&self.joining).insert(j.video, j.audio);
            }
            tokio::task::spawn_blocking(move || {
                let dir = vp
                    .parent()
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or_default();
                let out = super::free_name(&dir, &j.name);
                let made = if j.mp3 {
                    media::to_mp3(&ffmpeg, &vp, &out)
                } else {
                    media::join(&ffmpeg, &vp, &ap, &out)
                };
                match made {
                    Ok(()) => {
                        let size = std::fs::metadata(&out).map_or(0, |m| m.len());
                        let _ = me.store.set_finished(j.video, &out, size);
                        let _ = std::fs::remove_file(&vp);
                        if !j.mp3 {
                            let _ = std::fs::remove_file(&ap);
                            let _ = me.store.delete(j.audio);
                        }
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
                lock(&me.joining).remove(&j.video);
                me.publish_jobs();
            });
        }
        self.save_joins(&keep);
    }
}

/// One item for a video and its sound (B5): the final name, both parts'
/// progress, and where the sound is once the video part is done.
pub(super) fn merge_sound(v: &mut JobView, j: &Join, audio: Option<&JobView>) {
    v.name = j.name.clone();
    if let Some(a) = audio {
        v.written += a.written;
        v.total = v.total.zip(a.total).map(|(x, y)| x + y);
    }
    if v.status != "completed" {
        return; // the video part is still on its way: it speaks for both
    }
    let gone = |v: &mut JobView, why: String| {
        v.status = "failed-final";
        v.resumable = false;
        v.error = Some(why);
        v.error_action = Some("no-sound".into());
        v.sound = Some(Sound::Missing.word());
    };
    match audio {
        None => gone(
            v,
            "The sound part is gone, so this video has no sound yet.".into(),
        ),
        Some(a) if a.status == "completed" => {
            v.status = "running";
            v.sound = Some(Sound::Joining.word());
        }
        Some(a) if a.status == "failed-final" || a.status == "cancelled" => gone(
            v,
            match &a.error {
                Some(e) => format!("The sound couldn't be downloaded: {e}"),
                None => "The sound couldn't be downloaded.".into(),
            },
        ),
        Some(a) => {
            v.status = a.status;
            v.resumable = a.resumable;
            v.error = a.error.clone();
            v.error_action = a.error_action.clone();
            v.retry_in = a.retry_in;
            v.sound = Some(Sound::Getting.word());
        }
    }
}
