//! `fuselane update`: updates the Fuselane app the same way its own "Update and
//! restart" does. The app checks its signed feed, downloads the update over
//! every network, verifies the signature and restarts into it; this command asks
//! it to, shows the progress, and opens the app first if it's closed.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use fuselane_api::{client, launch};
use serde_json::{Value, json};

/// How long one request may take (a check reads the feed over the internet).
const ASK: Duration = Duration::from_secs(30);
/// How long the app may take to open, or to come back after installing.
const OPEN: Duration = Duration::from_secs(30);

async fn ask(endpoint: &Path, method: &str) -> Result<Value, String> {
    client::request(endpoint, method, json!({}), ASK).await
}

/// The running app's version, or None when it doesn't answer.
async fn running_version(endpoint: &Path) -> Option<String> {
    let reply = client::request(endpoint, "ping", json!({}), Duration::from_secs(3))
        .await
        .ok()?;
    reply["version"].as_str().map(str::to_string)
}

/// Waits up to `wait` for the app to answer; its version when it does.
async fn wait_for_app(endpoint: &Path, wait: Duration) -> Option<String> {
    let deadline = Instant::now() + wait;
    loop {
        if let Some(v) = running_version(endpoint).await {
            return Some(v);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// The running app (opened in the background if it was closed).
async fn reach_app(endpoint: &Path) -> Result<String, String> {
    if let Some(v) = running_version(endpoint).await {
        return Ok(v);
    }
    if !launch::start_installed_app() {
        return Err("Fuselane isn't installed here, or couldn't be opened. Get it at https://fuselane.app/download".into());
    }
    eprintln!("Opening Fuselane...");
    wait_for_app(endpoint, OPEN).await.ok_or_else(|| {
        "Fuselane didn't open. Open it, then run `fuselane update` again.".to_string()
    })
}

fn progress_line(state: &Value) -> Option<String> {
    match state["phase"].as_str()? {
        "checking" => Some("Getting ready...".into()),
        "downloading" => {
            let done = state["done"].as_u64().unwrap_or(0) as f64;
            let total = state["total"]
                .as_u64()
                .map_or("?".into(), |t| crate::human(t as f64));
            let rate = state["rate"].as_u64().unwrap_or(0) as f64;
            let networks = state["networks"].as_u64().unwrap_or(1);
            let over = if networks > 1 {
                format!(" over {networks} networks")
            } else {
                String::new()
            };
            Some(format!(
                "Downloading {} of {total}  {}/s{over}",
                crate::human(done),
                crate::human(rate)
            ))
        }
        "installing" => Some("Installing...".into()),
        _ => None,
    }
}

/// `check`: only say whether there's a newer version.
pub async fn update(check: bool) -> ExitCode {
    let home = match fuselane_core::home::home() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("fuselane: can't find Fuselane's data folder: {e}");
            return ExitCode::FAILURE;
        }
    };
    let endpoint = client::endpoint(&home);
    if let Err(why) = reach_app(&endpoint).await {
        eprintln!("fuselane: {why}");
        return ExitCode::FAILURE;
    }
    let found = match ask(&endpoint, "update.check").await {
        Ok(found) => found,
        Err(why) if why.contains("unknown method") => {
            eprintln!(
                "fuselane: this Fuselane is too old to update from the command line. Use Update and restart in the app once; after that `fuselane update` works."
            );
            return ExitCode::FAILURE;
        }
        Err(why) => {
            eprintln!("fuselane: couldn't check for updates: {why}");
            return ExitCode::FAILURE;
        }
    };
    let current = found["current"].as_str().unwrap_or("?").to_string();
    let Some(latest) = found["latest"].as_str().map(str::to_string) else {
        println!("Fuselane {current} is up to date.");
        return ExitCode::SUCCESS;
    };
    if check {
        println!(
            "Fuselane {latest} is available (you have {current}). Run `fuselane update` to install it."
        );
        return ExitCode::SUCCESS;
    }
    println!("Updating Fuselane {current} to {latest}. Downloads pause and carry on afterwards.");
    if let Err(why) = ask(&endpoint, "update.install").await {
        eprintln!("fuselane: the update couldn't start: {why}");
        return ExitCode::FAILURE;
    }
    // Progress until the app quits to restart into the new version.
    loop {
        let Ok(state) = ask(&endpoint, "update.state").await else {
            break;
        };
        if state["phase"] == "failed" {
            eprintln!(
                "\nfuselane: {}",
                state["error"].as_str().unwrap_or("the update failed")
            );
            return ExitCode::FAILURE;
        }
        if let Some(line) = progress_line(&state) {
            eprint!("\r{line}   ");
            let _ = std::io::stderr().flush();
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
    eprintln!();
    // The app restarts by itself; if it hasn't come back, it's opened again.
    let mut now = wait_for_app(&endpoint, OPEN).await;
    if now.is_none() && launch::start_installed_app() {
        now = wait_for_app(&endpoint, OPEN).await;
    }
    match now {
        Some(v) if v == latest => {
            println!("Fuselane is updated to {latest}.");
            ExitCode::SUCCESS
        }
        Some(v) => {
            eprintln!(
                "fuselane: Fuselane restarted, but it's still {v}. Try again, or use Update and restart in the app."
            );
            ExitCode::FAILURE
        }
        None => {
            println!("Fuselane {latest} is installed. Open Fuselane to use it.");
            ExitCode::SUCCESS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_reads_like_the_download_line() {
        let state = json!({"phase": "downloading", "done": 1_048_576, "total": 24_641_536, "rate": 5_242_880, "networks": 2});
        assert_eq!(
            progress_line(&state).unwrap(),
            "Downloading 1.0 MB of 23.5 MB  5.0 MB/s over 2 networks"
        );
        let one =
            json!({"phase": "downloading", "done": 0, "total": null, "rate": 0, "networks": 1});
        assert_eq!(progress_line(&one).unwrap(), "Downloading 0 B of ?  0 B/s");
        assert_eq!(
            progress_line(&json!({"phase": "installing"})).unwrap(),
            "Installing..."
        );
        assert_eq!(progress_line(&json!({"phase": "idle"})), None);
    }
}
