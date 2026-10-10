//! Opens one particular browser at a page: Fuselane's page in that browser's
//! extension store ("Add to Chrome"), or the browser's own extensions page to
//! turn the extension back on. The system's default browser might be another
//! one, so this starts the named browser itself.
//!
//! Only pages from `fuselane_api::browsers` are ever opened (fixed strings, no
//! user input), and the browser is started without a shell.

use std::process::Command;

use fuselane_api::browsers::Known;

/// The ways to start a browser on this OS, most direct first.
pub fn commands(known: &Known, page: &str) -> Vec<Command> {
    let mut out = Vec::new();
    #[cfg(target_os = "macos")]
    {
        let (app, exe) = match known.id {
            "chrome" => ("Google Chrome", "Google Chrome"),
            "edge" => ("Microsoft Edge", "Microsoft Edge"),
            "brave" => ("Brave Browser", "Brave Browser"),
            "vivaldi" => ("Vivaldi", "Vivaldi"),
            "chromium" => ("Chromium", "Chromium"),
            _ => ("Firefox", "firefox"),
        };
        // The browser's own binary takes chrome:// and about: pages too (`open`
        // won't pass those); a running browser opens it in its current window.
        for root in ["/Applications".into(), dirs_home_apps()] {
            let bin = format!("{root}/{app}.app/Contents/MacOS/{exe}");
            if std::path::Path::new(&bin).exists() {
                let mut c = Command::new(bin);
                c.arg(page);
                out.push(c);
            }
        }
        let mut c = Command::new("open");
        c.args(["-a", app, page]);
        out.push(c);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let exe = match known.id {
            "chrome" | "chromium" => "chrome.exe",
            "edge" => "msedge.exe",
            "brave" => "brave.exe",
            "vivaldi" => "vivaldi.exe",
            _ => "firefox.exe",
        };
        // `start` finds browsers through App Paths, where every installer registers.
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", exe, page])
            .creation_flags(CREATE_NO_WINDOW);
        out.push(c);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let names: &[&str] = match known.id {
            "chrome" => &["google-chrome", "google-chrome-stable"],
            "edge" => &["microsoft-edge", "microsoft-edge-stable"],
            "brave" => &["brave-browser", "brave"],
            "vivaldi" => &["vivaldi", "vivaldi-stable"],
            "chromium" => &["chromium", "chromium-browser"],
            _ => &["firefox"],
        };
        for n in names {
            let mut c = Command::new(n);
            c.arg(page);
            out.push(c);
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn dirs_home_apps() -> String {
    dirs::home_dir()
        .map(|h| h.join("Applications").to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Starts the first way that works. The browser keeps running on its own.
pub fn open(known: &Known, page: &str) -> Result<(), String> {
    for mut c in commands(known, page) {
        c.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if c.spawn().is_ok() {
            return Ok(());
        }
    }
    Err(format!("{} couldn't be started.", known.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fuselane_api::browsers::known;

    #[test]
    fn every_browser_has_a_way_to_start_and_the_page_is_passed_as_one_argument() {
        for id in ["chrome", "edge", "brave", "vivaldi", "chromium", "firefox"] {
            let k = known(id).unwrap();
            let cmds = commands(k, k.store.url());
            assert!(!cmds.is_empty(), "{id}");
            for c in &cmds {
                let args: Vec<_> = c
                    .get_args()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect();
                assert_eq!(args.last().map(String::as_str), Some(k.store.url()), "{id}");
            }
        }
    }
}
