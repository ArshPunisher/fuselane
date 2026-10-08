//! OS power actions: keep the computer awake while something downloads, and sleep
//! or shut down once everything has finished (automation::WhenDone).
//!
//! Each OS's own tool: `caffeinate` on macOS, `SetThreadExecutionState` on Windows,
//! `systemd-inhibit` on Linux. No admin rights are needed for any of them.

use std::io;

/// Holds the computer awake until dropped. The display may still sleep.
#[derive(Debug)]
pub struct KeepAwake {
    #[cfg(not(windows))]
    child: std::process::Child,
    #[cfg(windows)]
    stop: std::sync::mpsc::Sender<()>,
}

impl KeepAwake {
    /// None when this system has no way to do it (a minimal Linux without systemd).
    pub fn start() -> Option<KeepAwake> {
        #[cfg(target_os = "macos")]
        {
            // -i: no idle sleep; -w: also ends if Fuselane dies without dropping this.
            let child = std::process::Command::new("/usr/bin/caffeinate")
                .args(["-i", "-w", &std::process::id().to_string()])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok()?;
            Some(KeepAwake { child })
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            let child = std::process::Command::new("systemd-inhibit")
                .args([
                    "--what=idle:sleep",
                    "--who=Fuselane",
                    "--why=Downloading",
                    "--mode=block",
                    "sleep",
                    "infinity",
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok()?;
            Some(KeepAwake { child })
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Power::{
                ES_CONTINUOUS, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
            };
            // The request belongs to the thread that makes it, so one thread holds it.
            let (stop, wait) = std::sync::mpsc::channel::<()>();
            std::thread::Builder::new()
                .name("keep-awake".into())
                .spawn(move || {
                    // SAFETY: plain flag arguments; no pointers involved.
                    unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
                    let _ = wait.recv();
                    // SAFETY: as above; clears this thread's request.
                    unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
                })
                .ok()?;
            Some(KeepAwake { stop })
        }
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        #[cfg(not(windows))]
        {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        #[cfg(windows)]
        {
            let _ = self.stop.send(());
        }
    }
}

fn run(program: &str, args: &[&str]) -> io::Result<()> {
    let status = std::process::Command::new(program).args(args).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{program} exited with {status}")))
    }
}

/// Puts the computer to sleep now.
pub fn sleep_now() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        run("/usr/bin/pmset", &["sleepnow"])
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        run("systemctl", &["suspend"])
    }
    #[cfg(windows)]
    {
        run("rundll32.exe", &["powrprof.dll,SetSuspendState", "0,1,0"])
    }
}

/// Shuts the computer down the normal way (apps are asked to quit first).
pub fn shut_down() -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        run(
            "/usr/bin/osascript",
            &["-e", "tell application \"System Events\" to shut down"],
        )
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        run("systemctl", &["poweroff"])
    }
    #[cfg(windows)]
    {
        run("shutdown.exe", &["/s", "/t", "0"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    fn caffeinate_running() -> bool {
        let me = std::process::id().to_string();
        std::process::Command::new("/usr/bin/pgrep")
            .args(["-f", &format!("caffeinate -i -w {me}")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn keeping_awake_starts_and_stops_cleanly() {
        let guard = KeepAwake::start();
        #[cfg(target_os = "macos")]
        {
            assert!(guard.is_some(), "macOS always has caffeinate");
            assert!(caffeinate_running());
        }
        drop(guard);
        #[cfg(target_os = "macos")]
        {
            // Killed and reaped on drop.
            let t = std::time::Instant::now();
            while caffeinate_running() {
                assert!(
                    t.elapsed() < std::time::Duration::from_secs(5),
                    "still running"
                );
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }

    #[test]
    fn a_failing_command_is_an_error_not_a_panic() {
        assert!(run("/definitely/not/a/program", &[]).is_err());
        #[cfg(unix)]
        assert!(run("false", &[]).is_err());
    }
}
