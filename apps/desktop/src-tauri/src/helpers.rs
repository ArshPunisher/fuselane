//! The helper programs behind videos from pages (B10.4): yt-dlp reads the
//! page, ffmpeg joins video and sound (and makes MP3s), deno lets yt-dlp
//! solve YouTube's JavaScript checks. People shouldn't have to install them by
//! hand (a Windows user met "install it with winget", 2026-10-14), so
//! Fuselane fetches them itself the first time they're needed:
//!
//! - from each project's own release, at a **pinned version with a pinned
//!   SHA-256** (checked by the engine while downloading, and again here before
//!   anything is unpacked or run);
//! - over every network, with Fuselane's own engine, showing progress;
//! - into the app-data folder (`<home>/tools`), never into the app bundle, so
//!   the signed and notarized app stays untouched and the same code works for
//!   installers, the portable build, Homebrew and Flatpak;
//! - unpacked to a staging name, test-run, then renamed into place, so a cut
//!   download or a crash never leaves a half-written program behind.
//!
//! A copy the person installed themselves (Homebrew, winget, apt) is still
//! used when there's no managed one. yt-dlp goes stale as sites change, so a
//! managed copy updates itself with `yt-dlp -U` (its own checked updater) when
//! a page fails to read, at most once an hour (see `media_jobs`).
//!
//! Licences: yt-dlp is Unlicense, deno MIT, ffmpeg GPL (these static builds
//! include x264 and LAME). Fuselane doesn't ship them; each person's computer
//! fetches them from the project's own release. See NOTICE.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// A helper program Fuselane can fetch for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    YtDlp,
    Ffmpeg,
    Deno,
}

impl Tool {
    pub const ALL: [Tool; 3] = [Tool::YtDlp, Tool::Ffmpeg, Tool::Deno];

    /// The program's own name, as on the PATH.
    pub fn name(self) -> &'static str {
        match self {
            Tool::YtDlp => "yt-dlp",
            Tool::Ffmpeg => "ffmpeg",
            Tool::Deno => "deno",
        }
    }

    /// The file name it has in the tools folder.
    pub fn file_name(self) -> String {
        if cfg!(windows) {
            format!("{}.exe", self.name())
        } else {
            self.name().to_string()
        }
    }

    /// What a quick run that proves the program works looks like.
    fn version_arg(self) -> &'static str {
        match self {
            Tool::Ffmpeg => "-version",
            Tool::YtDlp | Tool::Deno => "--version",
        }
    }
}

/// How the program sits in its download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Archive {
    /// The download is the program itself.
    Raw,
    /// A zip with the program at this path inside it.
    Zip(&'static str),
}

/// Where one helper comes from, for this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub version: &'static str,
    pub url: &'static str,
    /// Hex SHA-256 of the download (published by the project; checked by us).
    pub sha256: &'static str,
    /// Bytes, for the progress bar before the server says.
    pub size: u64,
    pub archive: Archive,
}

const YT_DLP: &str = "2026.08.19";
const DENO: &str = "2.9.7";
const FFMPEG: &str = "9.0.2";

macro_rules! yt {
    ($asset:literal, $sha:literal, $size:literal) => {
        Source {
            version: YT_DLP,
            url: concat!(
                "https://github.com/yt-dlp/yt-dlp/releases/download/2026.08.19/",
                $asset
            ),
            sha256: $sha,
            size: $size,
            archive: Archive::Raw,
        }
    };
}

macro_rules! deno {
    ($target:literal, $sha:literal, $size:literal, $exe:literal) => {
        Source {
            version: DENO,
            url: concat!(
                "https://github.com/denoland/deno/releases/download/v2.9.7/deno-",
                $target,
                ".zip"
            ),
            sha256: $sha,
            size: $size,
            archive: Archive::Zip($exe),
        }
    };
}

/// Static ffmpeg release builds by Martin Riedl (macOS and Linux; the macOS
/// ones are signed and notarized by him).
macro_rules! riedl {
    ($path:literal, $sha:literal, $size:literal) => {
        Source {
            version: FFMPEG,
            url: concat!(
                "https://ffmpeg.martin-riedl.de/download/",
                $path,
                "/ffmpeg.zip"
            ),
            sha256: $sha,
            size: $size,
            archive: Archive::Zip("ffmpeg"),
        }
    };
}

/// The pinned download for `tool` on this computer, if there is one.
pub fn source(tool: Tool) -> Option<Source> {
    source_for(tool, std::env::consts::OS, std::env::consts::ARCH)
}

/// The table itself, by OS and CPU, so every row can be tested on any machine.
pub fn source_for(tool: Tool, os: &str, arch: &str) -> Option<Source> {
    Some(match (tool, os, arch) {
        // yt-dlp_macos is one universal program for Intel and Apple silicon.
        (Tool::YtDlp, "macos", _) => yt!(
            "yt-dlp_macos",
            "0f192b7ec147ab6288885d6351d9ab67367640029b4377576ef46dd79cf7b202",
            37_146_048
        ),
        (Tool::YtDlp, "windows", "aarch64") => yt!(
            "yt-dlp_arm64.exe",
            "05b438997bafc3affdfda9d041353c9d73e04dc842207254b655b0887c4445b0",
            21_159_286
        ),
        (Tool::YtDlp, "windows", _) => yt!(
            "yt-dlp.exe",
            "66674953fe251b89f4d08c5f0e35e0728679bd67ab3d7d05c0562af101dd3e7a",
            17_840_399
        ),
        (Tool::YtDlp, "linux", "x86_64") => yt!(
            "yt-dlp_linux",
            "58162f9bfdc27458ea47bfcb311cf47028f17d8154a8bf7d689861d46399230a",
            40_446_224
        ),
        (Tool::YtDlp, "linux", "aarch64") => yt!(
            "yt-dlp_linux_aarch64",
            "b16e4dab368a816cd05d477d698a605a6ae87ccee1c8ffd38fa21d7254141fcc",
            40_167_448
        ),
        (Tool::Ffmpeg, "macos", "aarch64") => riedl!(
            "macos/arm64/1789931890_9.0.2",
            "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924",
            28_395_699
        ),
        (Tool::Ffmpeg, "macos", "x86_64") => riedl!(
            "macos/amd64/1789931006_9.0.2",
            "7c6b4125b191cbf773832dc51f424cf2b6bb7da43007d1e066f95909e47cacd4",
            33_816_391
        ),
        (Tool::Ffmpeg, "linux", "x86_64") => riedl!(
            "linux/amd64/1789931100_9.0.2",
            "fa8ecf4abbd290d98f7d188b8649cc6b391ae209a98452be955a15aab1909d7f",
            33_326_445
        ),
        (Tool::Ffmpeg, "linux", "aarch64") => riedl!(
            "linux/arm64/1789931697_9.0.2",
            "93a76ae90db5474eecdf951a729857c64f3de23567228d6a7d5e6e8e3cd1021b",
            28_753_826
        ),
        // Gyan Doshi's "essentials" build, from its GitHub mirror (x64; Windows
        // on Arm runs it emulated). The hash matches the one gyan.dev publishes.
        (Tool::Ffmpeg, "windows", _) => Source {
            version: FFMPEG,
            url: "https://github.com/GyanD/codexffmpeg/releases/download/9.0.2/ffmpeg-9.0.2-essentials_build.zip",
            sha256: "60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba",
            size: 114_768_076,
            archive: Archive::Zip("ffmpeg-9.0.2-essentials_build/bin/ffmpeg.exe"),
        },
        (Tool::Deno, "macos", "aarch64") => deno!(
            "aarch64-apple-darwin",
            "5cd46d6268f6f78f5d88bdc7159d20bd44cdaa4b3303474839f87ec6fe7ae25c",
            38_469_316,
            "deno"
        ),
        (Tool::Deno, "macos", "x86_64") => deno!(
            "x86_64-apple-darwin",
            "95daaff11c116a52ad54785e7914c8e9c9cdcaba793c5ed929c74ca2d8e6259a",
            42_295_422,
            "deno"
        ),
        (Tool::Deno, "windows", "aarch64") => deno!(
            "aarch64-pc-windows-msvc",
            "c4c4ac8bfdaa37814bda5c05fc9cdf2154904e2ef8277673a30bdceaaa649807",
            40_847_024,
            "deno.exe"
        ),
        (Tool::Deno, "windows", _) => deno!(
            "x86_64-pc-windows-msvc",
            "a0c3101b4158d1dfb7d6a78a7bf0f3de80c96bb423c152beec8beb22786f2238",
            42_630_221,
            "deno.exe"
        ),
        (Tool::Deno, "linux", "x86_64") => deno!(
            "x86_64-unknown-linux-gnu",
            "c6527f24f4b16031d3ae4fa9f658d5f11534c8d84ce7dc8502420280919c3490",
            41_596_794,
            "deno"
        ),
        (Tool::Deno, "linux", "aarch64") => deno!(
            "aarch64-unknown-linux-gnu",
            "c832298b1ad4422481334855f6003e0f54145762c5a134f20a489511d2f65bbf",
            39_821_318,
            "deno"
        ),
        _ => return None,
    })
}

/// Why a helper couldn't be set up. Each one has its own message (media_jobs).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HelperError {
    /// No build of it for this computer (an unusual OS or CPU).
    #[error("no build for this computer")]
    NoBuild,
    /// The download didn't arrive: offline, a site down, or cancelled.
    #[error("couldn't download it ({0})")]
    Download(String),
    /// What arrived isn't what was pinned: never unpacked, never run.
    #[error("the download didn't match its checksum")]
    Damaged,
    /// The folder couldn't be written (full disk, permissions).
    #[error("couldn't save it ({0})")]
    Disk(String),
    /// It arrived intact but won't run here (blocked by security software,
    /// or a CPU it doesn't support).
    #[error("it won't run on this computer ({0})")]
    WontRun(String),
}

/// What the download hands back: its bytes so far and total (if known).
pub type Progress = Arc<dyn Fn(u64, Option<u64>) + Send + Sync>;

/// One download for [`Fetch`]: `url` into the folder `dir` as `name`.
pub struct FetchJob {
    pub url: String,
    pub dir: PathBuf,
    pub name: String,
    pub sha256: [u8; 32],
    pub progress: Progress,
}

/// Fetches a file. The app uses the engine (every network); tests pass a fake.
pub type Fetch = Arc<
    dyn Fn(FetchJob) -> Pin<Box<dyn Future<Output = Result<PathBuf, String>> + Send>> + Send + Sync,
>;

/// Finds a program the person installed.
pub type Finder = Arc<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;
/// Where each helper comes from.
pub type Table = Arc<dyn Fn(Tool) -> Option<Source> + Send + Sync>;

/// What the window shows while helpers are being set up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupView {
    /// Being fetched right now ("yt-dlp", "ffmpeg", "deno").
    pub tools: Vec<&'static str>,
    pub done: u64,
    pub total: u64,
}

/// Told about setup progress (the service turns it into an event).
pub type Notify = Arc<dyn Fn(SetupView) + Send + Sync>;

/// How often the window hears about progress.
const NOTIFY_EVERY: Duration = Duration::from_millis(250);
/// How long a new program gets to answer its version (yt-dlp_macos unpacks
/// itself on its first run, which can take a while on a slow disk).
const FIRST_RUN: Duration = Duration::from_secs(90);
/// yt-dlp updates itself at most this often.
const UPDATE_EVERY: Duration = Duration::from_secs(60 * 60);

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The managed helpers: where they live, which are being fetched, and progress.
pub struct Helpers {
    dir: PathBuf,
    fetch: Fetch,
    /// Looks a program up on the PATH (tests turn it off).
    find: Finder,
    /// The table of downloads (tests point it at their own files).
    sources: Table,
    /// One install of each tool at a time; a second ask waits, then finds it.
    turns: HashMap<Tool, tokio::sync::Mutex<()>>,
    /// Shared with the downloads' progress callbacks.
    shared: Arc<Shared>,
    last_update: Mutex<Option<Instant>>,
}

/// Bytes so far per tool being fetched, and who to tell.
#[derive(Default)]
struct Shared {
    progress: Mutex<HashMap<Tool, (u64, u64)>>,
    last_notify: Mutex<Option<Instant>>,
    notify: Mutex<Option<Notify>>,
}

impl Shared {
    fn set(&self, tool: Tool, done: u64, total: u64) {
        lock(&self.progress).insert(tool, (done, total));
        self.tell(false);
    }

    fn clear(&self, tool: Tool) {
        lock(&self.progress).remove(&tool);
        self.tell(true);
    }

    /// Tells the window where setup stands (at most every 250 ms unless `now`).
    fn tell(&self, now: bool) {
        {
            let mut last = lock(&self.last_notify);
            if !now && last.is_some_and(|t| t.elapsed() < NOTIFY_EVERY) {
                return;
            }
            *last = Some(Instant::now());
        }
        let Some(f) = lock(&self.notify).clone() else {
            return;
        };
        let v = view(&lock(&self.progress));
        f(v);
    }
}

impl std::fmt::Debug for Helpers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Helpers")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

impl Helpers {
    /// Helpers kept in `dir`, downloaded with `fetch`.
    pub fn new(dir: PathBuf, fetch: Fetch) -> Helpers {
        Helpers {
            dir,
            fetch,
            find: Arc::new(crate::media::find),
            sources: Arc::new(source),
            turns: Tool::ALL
                .into_iter()
                .map(|t| (t, tokio::sync::Mutex::new(())))
                .collect(),
            shared: Arc::default(),
            last_update: Mutex::default(),
        }
    }

    /// Helpers in `<home>/tools`, fetched over every network by the engine.
    #[cfg_attr(test, allow(dead_code))]
    pub fn for_app() -> Helpers {
        let dir = fuselane_core::home::home()
            .unwrap_or_else(|_| std::env::temp_dir().join("fuselane"))
            .join("tools");
        Helpers::new(dir, engine_fetch())
    }

    #[cfg(test)]
    pub fn with_table(mut self, sources: Table, find: Finder) -> Helpers {
        self.sources = sources;
        self.find = find;
        self
    }

    /// Sends setup progress to `f` from now on.
    pub fn on_progress(&self, f: Notify) {
        *lock(&self.shared.notify) = Some(f);
    }

    /// Fuselane's own copy, if it has one.
    pub fn managed(&self, tool: Tool) -> Option<PathBuf> {
        let p = self.dir.join(tool.file_name());
        p.is_file().then_some(p)
    }

    /// The copy to use now: Fuselane's own, else one the person installed.
    pub fn locate(&self, tool: Tool) -> Option<PathBuf> {
        self.managed(tool).or_else(|| (self.find)(tool.name()))
    }

    /// Whether Fuselane can fetch `tool` on this computer.
    pub fn can_fetch(&self, tool: Tool) -> bool {
        (self.sources)(tool).is_some()
    }

    /// Fetches Fuselane's own copy of `tool` (once, however many ask at once).
    pub async fn install(&self, tool: Tool) -> Result<PathBuf, HelperError> {
        let _turn = self.turns[&tool].lock().await;
        if let Some(p) = self.managed(tool) {
            return Ok(p); // fetched while this ask waited its turn
        }
        let src = (self.sources)(tool).ok_or(HelperError::NoBuild)?;
        let made = self.install_from(tool, src).await;
        self.shared.clear(tool);
        if let Err(e) = &made {
            crate::reports::log(&format!("helper {} not set up: {e}", tool.name()));
        }
        made
    }

    async fn install_from(&self, tool: Tool, src: Source) -> Result<PathBuf, HelperError> {
        let sha = hex32(src.sha256).ok_or(HelperError::Damaged)?;
        std::fs::create_dir_all(&self.dir).map_err(|e| HelperError::Disk(e.to_string()))?;
        let staging = self
            .dir
            .join(format!(".{}-{}", tool.name(), std::process::id()));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| HelperError::Disk(e.to_string()))?;
        let made = self.fetch_and_place(tool, src, sha, &staging).await;
        let _ = std::fs::remove_dir_all(&staging);
        made
    }

    async fn fetch_and_place(
        &self,
        tool: Tool,
        src: Source,
        sha: [u8; 32],
        staging: &Path,
    ) -> Result<PathBuf, HelperError> {
        self.shared.set(tool, 0, src.size);
        self.shared.tell(true);
        let progress: Progress = {
            let (size, shared) = (src.size, self.shared.clone());
            Arc::new(move |done, total| shared.set(tool, done, total.unwrap_or(size)))
        };
        let name = src.url.rsplit('/').next().unwrap_or("download").to_string();
        let got = (self.fetch)(FetchJob {
            url: src.url.to_string(),
            dir: staging.to_path_buf(),
            name,
            sha256: sha,
            progress,
        })
        .await
        .map_err(HelperError::Download)?;
        let staged = staging.join(format!("{}.new", tool.file_name()));
        let (got2, staged2, archive) = (got.clone(), staged.clone(), src.archive);
        tokio::task::spawn_blocking(move || unpack(&got2, sha, archive, &staged2))
            .await
            .map_err(|e| HelperError::Disk(e.to_string()))??;
        // It must run before it takes the real name.
        runs(&staged, tool).await?;
        let to = self.dir.join(tool.file_name());
        replace(&staged, &to).map_err(|e| HelperError::Disk(e.to_string()))?;
        Ok(to)
    }

    /// Lets a managed yt-dlp update itself (sites change, old versions stop
    /// reading them). True when it changed. At most once an hour; a copy the
    /// person installed is theirs to update.
    pub async fn update_yt_dlp(&self) -> bool {
        let Some(yt) = self.managed(Tool::YtDlp) else {
            return false;
        };
        {
            let mut last = lock(&self.last_update);
            if last.is_some_and(|t| t.elapsed() < UPDATE_EVERY) {
                return false;
            }
            *last = Some(Instant::now());
        }
        let _turn = self.turns[&Tool::YtDlp].lock().await;
        let before = version(&yt, Tool::YtDlp).await.ok();
        let mut cmd = tokio::process::Command::new(&yt);
        cmd.args(["-U", "--no-config"]).kill_on_drop(true);
        no_window(&mut cmd);
        let ran = tokio::time::timeout(Duration::from_secs(180), cmd.output()).await;
        if !matches!(&ran, Ok(Ok(o)) if o.status.success()) {
            crate::reports::log("yt-dlp couldn't update itself");
            return false;
        }
        let after = version(&yt, Tool::YtDlp).await.ok();
        after.is_some() && after != before
    }
}

/// The window's view of what's being fetched.
fn view(progress: &HashMap<Tool, (u64, u64)>) -> SetupView {
    let mut tools: Vec<Tool> = progress.keys().copied().collect();
    tools.sort_by_key(|t| Tool::ALL.iter().position(|x| x == t));
    SetupView {
        tools: tools.iter().map(|t| t.name()).collect(),
        done: progress.values().map(|p| p.0).sum(),
        total: progress.values().map(|p| p.1.max(p.0)).sum(),
    }
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    if s.len() != 64 {
        return None;
    }
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn sha256_file(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(h.finalize().into())
}

/// Checks the download against its pinned hash, then writes the program to
/// `out` (straight from a zip entry, or the download itself), executable.
fn unpack(got: &Path, sha: [u8; 32], archive: Archive, out: &Path) -> Result<(), HelperError> {
    let disk = |e: std::io::Error| HelperError::Disk(e.to_string());
    if sha256_file(got).map_err(disk)? != sha {
        return Err(HelperError::Damaged);
    }
    match archive {
        Archive::Raw => {
            std::fs::rename(got, out)
                .or_else(|_| std::fs::copy(got, out).map(|_| ()))
                .map_err(disk)?;
        }
        Archive::Zip(entry) => {
            let file = std::fs::File::open(got).map_err(disk)?;
            let mut zip = zip::ZipArchive::new(file).map_err(|_| HelperError::Damaged)?;
            // Only the one named file is read; nothing else in the zip is written.
            let mut inner = zip.by_name(entry).map_err(|_| HelperError::Damaged)?;
            let mut to = std::fs::File::create(out).map_err(disk)?;
            std::io::copy(&mut inner, &mut to).map_err(disk)?;
            to.sync_all().map_err(disk)?;
        }
    }
    let _ = std::fs::remove_file(got);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o755)).map_err(disk)?;
    }
    Ok(())
}

/// Puts the new program in place of any old one.
fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        // Windows won't rename over a file in some cases: move the old one away.
        Err(_) if to.exists() => {
            let old = to.with_extension("old");
            let _ = std::fs::remove_file(&old);
            std::fs::rename(to, &old)?;
            std::fs::rename(from, to)?;
            let _ = std::fs::remove_file(&old);
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Runs `program --version` and returns its first line.
async fn version(program: &Path, tool: Tool) -> Result<String, String> {
    let mut cmd = tokio::process::Command::new(program);
    cmd.arg(tool.version_arg()).kill_on_drop(true);
    no_window(&mut cmd);
    let out = tokio::time::timeout(FIRST_RUN, cmd.output())
        .await
        .map_err(|_| "it didn't answer".to_string())?
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("it stopped with {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string())
}

async fn runs(program: &Path, tool: Tool) -> Result<(), HelperError> {
    version(program, tool)
        .await
        .map(|_| ())
        .map_err(HelperError::WontRun)
}

/// Windows: helpers are console programs; without this each run flashes a
/// black window over Fuselane.
pub fn no_window(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// The same for blocking commands.
pub fn no_window_std(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Downloads with Fuselane's engine over every network, checking the SHA-256.
#[cfg_attr(test, allow(dead_code))]
fn engine_fetch() -> Fetch {
    Arc::new(|job: FetchJob| {
        Box::pin(async move {
            use fuselane_core::runner::{RunOptions, fetch};
            use fuselane_engine_http::download::ProgressFn;
            let progress = job.progress.clone();
            let report = fetch(
                &job.url,
                job.dir.clone(),
                RunOptions {
                    sha256: Some(job.sha256),
                    filename: Some(job.name.clone()),
                    progress: Some(ProgressFn(Arc::new(move |done, total| {
                        progress(done, total)
                    }))),
                    ..RunOptions::default()
                },
            )
            .await?;
            Ok(report.path)
        })
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A fake fetch that "downloads" `file` and counts how often it's asked.
    pub(crate) fn serve(file: PathBuf, count: Arc<AtomicUsize>) -> Fetch {
        Arc::new(move |job: FetchJob| {
            let (file, count) = (file.clone(), count.clone());
            Box::pin(async move {
                count.fetch_add(1, Ordering::SeqCst);
                assert!(job.url.starts_with("https://") && job.sha256 != [0; 32]);
                // Slow enough that two asks overlap.
                tokio::time::sleep(Duration::from_millis(50)).await;
                (job.progress)(5, Some(10));
                let to = job.dir.join(&job.name);
                std::fs::copy(&file, &to).map_err(|e| e.to_string())?;
                Ok(to)
            })
        })
    }

    fn hex(b: [u8; 32]) -> &'static str {
        Box::leak(
            b.iter()
                .map(|x| format!("{x:02x}"))
                .collect::<String>()
                .into_boxed_str(),
        )
    }

    /// A table where every tool comes from `file` (its real hash), as `archive`.
    pub(crate) fn table(file: &Path, archive: Archive) -> Table {
        let sha = hex(sha256_file(file).unwrap());
        Arc::new(move |_| {
            Some(Source {
                version: "1",
                url: "https://example.invalid/tool",
                sha256: sha,
                size: 10,
                archive,
            })
        })
    }

    pub(crate) fn nothing_installed() -> Finder {
        Arc::new(|_| None)
    }

    /// A program that answers `--version` (and `-U`, by changing its version).
    #[cfg(unix)]
    pub(crate) fn script(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("tool.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        p
    }

    #[test]
    fn every_supported_computer_has_a_pinned_checked_download_of_each_helper() {
        for (os, arch) in [
            ("macos", "aarch64"),
            ("macos", "x86_64"),
            ("windows", "x86_64"),
            ("windows", "aarch64"),
            ("linux", "x86_64"),
            ("linux", "aarch64"),
        ] {
            for tool in Tool::ALL {
                let s = source_for(tool, os, arch)
                    .unwrap_or_else(|| panic!("no {} for {os}/{arch}", tool.name()));
                assert!(s.url.starts_with("https://"), "{}", s.url);
                assert!(hex32(s.sha256).is_some(), "bad hash for {}", s.url);
                assert_eq!(s.sha256, s.sha256.to_ascii_lowercase());
                assert!(s.size > 1_000_000, "{}", s.url);
                if let Archive::Zip(entry) = s.archive {
                    assert!(entry.ends_with(&if os == "windows" {
                        format!("{}.exe", tool.name())
                    } else {
                        tool.name().to_string()
                    }));
                }
            }
        }
        assert!(source_for(Tool::YtDlp, "freebsd", "x86_64").is_none());
        // This computer (CI runs all three) has them.
        assert!(Tool::ALL.iter().all(|t| source(*t).is_some()));
    }

    #[test]
    fn a_download_that_does_not_match_its_hash_is_never_unpacked() {
        let dir = tempfile::tempdir().unwrap();
        let got = dir.path().join("got");
        std::fs::write(&got, b"tampered").unwrap();
        let out = dir.path().join("out");
        assert_eq!(
            unpack(&got, [0; 32], Archive::Raw, &out),
            Err(HelperError::Damaged)
        );
        assert!(!out.exists());
    }

    #[test]
    fn only_the_named_program_comes_out_of_a_zip() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let got = dir.path().join("pack.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&got).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("pack/../../evil", opts).unwrap();
        zip.write_all(b"no").unwrap();
        zip.start_file("pack/bin/ffmpeg.exe", opts).unwrap();
        zip.write_all(b"the program").unwrap();
        zip.finish().unwrap();
        let sha = sha256_file(&got).unwrap();
        let out = dir.path().join("ffmpeg.new");
        unpack(&got, sha, Archive::Zip("pack/bin/ffmpeg.exe"), &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"the program");
        assert!(!dir.path().parent().unwrap().join("evil").exists());
        // A zip without the program is refused.
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&got).unwrap());
        zip.start_file("other", opts).unwrap();
        zip.finish().unwrap();
        let sha = sha256_file(&got).unwrap();
        assert_eq!(
            unpack(&got, sha, Archive::Zip("pack/bin/ffmpeg.exe"), &out),
            Err(HelperError::Damaged)
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_missing_helper_is_fetched_once_checked_and_put_in_place() {
        let src = tempfile::tempdir().unwrap();
        let file = script(src.path(), "echo 1.0");
        let tools = tempfile::tempdir().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let h = Helpers::new(tools.path().join("tools"), serve(file, count.clone())).with_table(
            table(&src.path().join("tool.sh"), Archive::Raw),
            nothing_installed(),
        );
        let seen = Arc::new(Mutex::new(Vec::<SetupView>::new()));
        let s = seen.clone();
        h.on_progress(Arc::new(move |v| lock(&s).push(v)));
        assert!(h.locate(Tool::YtDlp).is_none());
        // Two asks at once share one download.
        let (a, b) = tokio::join!(h.install(Tool::YtDlp), h.install(Tool::YtDlp));
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_eq!(a, b);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(h.locate(Tool::YtDlp), Some(a.clone()));
        assert_eq!(version(&a, Tool::YtDlp).await.unwrap(), "1.0");
        // Nothing half-made is left beside it.
        let left: Vec<_> = std::fs::read_dir(tools.path().join("tools"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, vec![std::ffi::OsString::from("yt-dlp")]);
        let seen = lock(&seen);
        assert!(seen.iter().any(|v| v.tools == ["yt-dlp"] && v.total == 10));
        assert_eq!(
            seen.last().unwrap(),
            &SetupView::default(),
            "ends with nothing to show"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_helper_that_will_not_run_is_not_kept() {
        let src = tempfile::tempdir().unwrap();
        let file = script(src.path(), "exit 3");
        let tools = tempfile::tempdir().unwrap();
        let h = Helpers::new(
            tools.path().to_path_buf(),
            serve(file.clone(), Arc::default()),
        )
        .with_table(table(&file, Archive::Raw), nothing_installed());
        assert!(matches!(
            h.install(Tool::Ffmpeg).await,
            Err(HelperError::WontRun(_))
        ));
        assert!(h.managed(Tool::Ffmpeg).is_none());
    }

    #[tokio::test]
    async fn offline_says_so_and_a_copy_the_person_installed_still_counts() {
        let tools = tempfile::tempdir().unwrap();
        let offline: Fetch = Arc::new(|_| Box::pin(async { Err("no network".to_string()) }));
        let h = Helpers::new(tools.path().to_path_buf(), offline.clone()).with_table(
            Arc::new(|t| source_for(t, "linux", "x86_64")),
            nothing_installed(),
        );
        assert_eq!(
            h.install(Tool::YtDlp).await,
            Err(HelperError::Download("no network".into()))
        );
        let theirs = tools.path().join("their-yt-dlp");
        let h = Helpers::new(tools.path().join("x"), offline)
            .with_table(Arc::new(|_| None), Arc::new(move |_| Some(theirs.clone())));
        assert!(h.locate(Tool::YtDlp).is_some());
        assert!(!h.can_fetch(Tool::YtDlp));
        assert_eq!(h.install(Tool::Deno).await, Err(HelperError::NoBuild));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fuselanes_own_yt_dlp_updates_itself_at_most_once_an_hour() {
        let src = tempfile::tempdir().unwrap();
        let file = script(
            src.path(),
            r#"d=$(dirname "$0"); case "$1" in -U) echo 2.0 > "$d/v";; *) cat "$d/v" 2>/dev/null || echo 1.0;; esac"#,
        );
        let tools = tempfile::tempdir().unwrap();
        let h = Helpers::new(
            tools.path().to_path_buf(),
            serve(file.clone(), Arc::default()),
        )
        .with_table(table(&file, Archive::Raw), nothing_installed());
        assert!(!h.update_yt_dlp().await, "nothing to update yet");
        h.install(Tool::YtDlp).await.unwrap();
        assert!(h.update_yt_dlp().await, "1.0 became 2.0");
        assert!(!h.update_yt_dlp().await, "not again within the hour");
    }

    /// The real downloads for this computer, over the real network, then a
    /// real page read with them. Run by hand on each OS (it fetches ~100 MB):
    /// `cargo test -p fuselane-desktop -- --ignored the_real_helpers`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "downloads the real helpers"]
    async fn the_real_helpers_download_check_and_read_a_video_page() {
        let tools = tempfile::tempdir().unwrap();
        let h = Helpers::new(tools.path().to_path_buf(), engine_fetch())
            .with_table(Arc::new(source), nothing_installed());
        let (yt, ff, deno) = tokio::join!(
            h.install(Tool::YtDlp),
            h.install(Tool::Ffmpeg),
            h.install(Tool::Deno)
        );
        let (yt, ff, deno) = (yt.unwrap(), ff.unwrap(), deno.unwrap());
        assert!(
            version(&ff, Tool::Ffmpeg)
                .await
                .unwrap()
                .starts_with("ffmpeg")
        );
        let extra = vec![
            "--js-runtimes".to_string(),
            format!("deno:{}", deno.to_string_lossy()),
        ];
        let json = crate::media::probe(&yt, &extra, "https://www.youtube.com/watch?v=aqz-KE-bpKQ")
            .await
            .unwrap();
        let (info, _) = crate::media::plans_from(&json, true).unwrap();
        assert!(info.options.iter().any(|o| o.label == "1080p"), "{info:?}");
    }
}
