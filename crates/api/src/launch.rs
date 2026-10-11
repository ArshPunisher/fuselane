//! Starting the Fuselane app from its native-messaging host, so a download the
//! extension sends while the app is closed opens Fuselane and starts there
//! (REVIEW-2026-10-11 B1, D).
//!
//! The host is the app's own executable started by the browser, so the app to
//! start is this executable (or, on macOS, the `.app` bundle around it). The new
//! process must not inherit the browser's pipe: anything it wrote to stdout would
//! corrupt the message stream, so all three standard streams are closed.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The `.app` bundle that holds `exe`, if there is one (macOS).
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")))
        .map(Path::to_path_buf)
}

/// What to start for the app at `exe`: `open <bundle>` on macOS, else the
/// executable itself. It starts in the background (`--minimized`, to the tray):
/// a download from the browser carries on without the window jumping up; what
/// needs the person (a video's qualities, a feed) brings the window up itself.
/// `home` is a data folder of its own (FUSELANE_HOME), kept for the new process.
pub fn command_for(exe: &Path, home: Option<&std::ffi::OsStr>) -> Command {
    if cfg!(target_os = "macos")
        && let Some(bundle) = bundle_of(exe)
    {
        let mut c = Command::new("/usr/bin/open");
        // -g: not brought to the front. Launch Services doesn't pass this
        // process's environment on, so a data folder of its own goes as --env.
        c.arg("-g");
        if let Some(home) = home {
            c.arg("--env")
                .arg(format!("FUSELANE_HOME={}", home.to_string_lossy()));
        }
        c.arg(bundle).args(["--args", "--minimized"]);
        return c;
    }
    let mut c = Command::new(exe);
    c.arg("--minimized");
    c
}

/// Starts the app, detached from the browser that started this host. False when
/// it couldn't be started (the caller then answers as if the app were closed).
pub fn start_app() -> bool {
    let Ok(current) = std::env::current_exe() else {
        return false;
    };
    // An AppImage runs from a mount that disappears with this process.
    let exe = crate::hosts::host_exe(current, std::env::var_os("APPIMAGE").map(Into::into));
    if is_cli(&exe) {
        return false;
    }
    let home = std::env::var_os("FUSELANE_HOME");
    spawn_detached(command_for(&exe, home.as_deref())).is_ok()
}

/// The command-line tool (`fuselane`) can be the host too, for people without
/// the app; it has no window to open, so nothing is started for it.
pub fn is_cli(exe: &Path) -> bool {
    bundle_of(exe).is_none() && exe.file_stem().is_some_and(|s| s == "fuselane")
}

fn spawn_detached(mut c: Command) -> std::io::Result<()> {
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own process group: closing the browser's host doesn't stop the app.
        c.process_group(0);
        c.spawn().map(|_| ())
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        // Chrome runs hosts in a job that ends with them; leave it when allowed.
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
        match c.spawn() {
            Ok(_) => Ok(()),
            Err(_) => {
                c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
                c.spawn().map(|_| ())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundle_around_the_executable_is_found() {
        let exe = Path::new("/Applications/Fuselane.app/Contents/MacOS/fuselane");
        assert_eq!(
            bundle_of(exe),
            Some(PathBuf::from("/Applications/Fuselane.app"))
        );
        assert_eq!(bundle_of(Path::new("/usr/bin/fuselane")), None);
    }

    #[test]
    fn the_command_line_tool_is_not_an_app_to_start() {
        assert!(is_cli(Path::new("/usr/local/bin/fuselane")));
        assert!(is_cli(Path::new(r"C:\Tools\fuselane.exe")) || cfg!(not(windows)));
        assert!(!is_cli(Path::new("/usr/bin/fuselane-desktop")));
        assert!(!is_cli(Path::new(
            "/Applications/Fuselane.app/Contents/MacOS/fuselane"
        )));
        assert!(!is_cli(Path::new("/opt/Fuselane.AppImage")));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn on_macos_the_bundle_is_opened_not_the_binary() {
        let args = |c: &Command| {
            c.get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        let app = Path::new("/Applications/Fuselane.app/Contents/MacOS/fuselane");
        let c = command_for(app, None);
        assert_eq!(c.get_program(), "/usr/bin/open");
        // In the background, to the tray: the window doesn't jump up.
        assert_eq!(
            args(&c),
            ["-g", "/Applications/Fuselane.app", "--args", "--minimized"]
        );
        // A copy on its own data folder is started on that folder.
        let test = command_for(app, Some(std::ffi::OsStr::new("/Users/me/fuselane-test")));
        assert_eq!(
            args(&test),
            [
                "-g",
                "--env",
                "FUSELANE_HOME=/Users/me/fuselane-test",
                "/Applications/Fuselane.app",
                "--args",
                "--minimized"
            ]
        );
        // A development build has no bundle: the binary itself starts.
        let dev = command_for(Path::new("/tmp/target/debug/fuselane-desktop"), None);
        assert_eq!(dev.get_program(), "/tmp/target/debug/fuselane-desktop");
        assert_eq!(args(&dev), ["--minimized"]);
    }
}
