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

/// What to start for the app at `exe`: `open <bundle>` on macOS (Launch Services
/// brings it to the front), else the executable itself.
pub fn command_for(exe: &Path) -> Command {
    if cfg!(target_os = "macos")
        && let Some(bundle) = bundle_of(exe)
    {
        let mut c = Command::new("/usr/bin/open");
        c.arg(bundle);
        return c;
    }
    Command::new(exe)
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
    spawn_detached(command_for(&exe)).is_ok()
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
        let c = command_for(Path::new(
            "/Applications/Fuselane.app/Contents/MacOS/fuselane",
        ));
        assert_eq!(c.get_program(), "/usr/bin/open");
        assert_eq!(
            c.get_args().collect::<Vec<_>>(),
            vec![std::ffi::OsStr::new("/Applications/Fuselane.app")]
        );
        // A development build has no bundle: the binary itself starts.
        let dev = command_for(Path::new("/tmp/target/debug/fuselane-desktop"));
        assert_eq!(dev.get_program(), "/tmp/target/debug/fuselane-desktop");
    }
}
