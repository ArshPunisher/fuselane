//! XDG desktop portals, for what a Flatpak can't do itself (see `flatpak.rs`):
//! start at login through the Background portal and keep the computer awake
//! through the Inhibit portal. Raw D-Bus calls with zbus (already in the build for
//! the single-instance plugin), so no new crates. Linux only; used only in a Flatpak.
//!
//! Both portals answer through a Request object: the call returns its path, and the
//! result arrives later as that object's `Response` signal. The path is predictable
//! (`.../request/<sender>/<token>`), so the signal is subscribed to before the call.

use std::collections::HashMap;
use std::time::Duration;

use futures_lite::StreamExt;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const DEST: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const REQUEST: &str = "org.freedesktop.portal.Request";

/// The Inhibit portal's flag for "suspend" (1 logout, 2 user switch, 4 suspend, 8 idle).
const INHIBIT_SUSPEND: u32 = 4;

/// How long the Background portal may take to answer: it can ask the user first.
const BACKGROUND_WAIT: Duration = Duration::from_secs(120);

/// Why a portal request didn't do what was asked.
#[derive(Debug)]
pub enum PortalError {
    /// The user (or the system's policy) said no.
    Refused,
    /// No answer in time.
    TimedOut,
    /// No portal, or a D-Bus failure.
    Failed(String),
}

impl std::fmt::Display for PortalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PortalError::Refused => f.write_str("it was refused"),
            PortalError::TimedOut => f.write_str("the system didn't answer"),
            PortalError::Failed(e) => f.write_str(e),
        }
    }
}

impl From<zbus::Error> for PortalError {
    fn from(e: zbus::Error) -> Self {
        PortalError::Failed(e.to_string())
    }
}

/// Where the portal puts the Request object for `token` asked by `unique_name`
/// (":1.42" → ".../request/1_42/<token>"), per the portal documentation.
pub fn request_path(unique_name: &str, token: &str) -> String {
    let sender = unique_name.trim_start_matches(':').replace('.', "_");
    format!("{PATH}/request/{sender}/{token}")
}

/// A fresh handle token: letters, digits and underscores only (a path element).
fn new_token() -> String {
    let mut raw = [0u8; 8];
    let _ = getrandom::fill(&mut raw);
    let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    format!("fuselane_{hex}")
}

/// The Background portal's answer to an autostart request: whether autostart is now
/// on. Response 0 is success, 1 the user cancelled, 2 anything else.
pub fn autostart_answer(
    response: u32,
    results: &HashMap<String, OwnedValue>,
    asked: bool,
) -> Result<bool, PortalError> {
    match response {
        0 => {
            let autostart = results
                .get("autostart")
                .and_then(|v| v.downcast_ref::<bool>().ok())
                .unwrap_or(false);
            // Asking to turn it on and getting "off" back is a refusal.
            if asked && !autostart {
                Err(PortalError::Refused)
            } else {
                Ok(autostart)
            }
        }
        1 => Err(PortalError::Refused),
        n => Err(PortalError::Failed(format!(
            "the background portal answered {n}"
        ))),
    }
}

/// Turns start at login on or off through the Background portal. The portal
/// writes (or removes) the autostart entry on the host, starting the app with
/// `--minimized` so it opens in the tray. Returns whether it's on now.
pub async fn set_autostart(on: bool) -> Result<bool, PortalError> {
    let conn = zbus::Connection::session().await?;
    let unique = conn
        .unique_name()
        .ok_or_else(|| PortalError::Failed("no D-Bus name".into()))?
        .to_string();
    let token = new_token();
    let path = request_path(&unique, &token);
    let request = zbus::Proxy::new(&conn, DEST, path.as_str(), REQUEST).await?;
    let mut answers = request.receive_signal("Response").await?;

    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert(
        "reason",
        Value::from("Start Fuselane when you sign in, so scheduled downloads run."),
    );
    options.insert("autostart", Value::from(on));
    options.insert(
        "commandline",
        Value::from(vec!["fuselane-desktop", "--minimized"]),
    );
    options.insert("dbus-activatable", Value::from(false));
    let reply = conn
        .call_method(
            Some(DEST),
            PATH,
            Some("org.freedesktop.portal.Background"),
            "RequestBackground",
            &("", options),
        )
        .await?;
    let handle: OwnedObjectPath = reply.body().deserialize()?;
    // Portals older than 0.9 pick their own path: listen there instead.
    if handle.as_str() != path {
        let request = zbus::Proxy::new(&conn, DEST, handle.as_str(), REQUEST).await?;
        answers = request.receive_signal("Response").await?;
    }

    let answer = tokio::time::timeout(BACKGROUND_WAIT, answers.next()).await;
    let Ok(Some(msg)) = answer else {
        // Close any dialog still open; the portal then sends no answer.
        let _ = conn
            .call_method(Some(DEST), handle.as_str(), Some(REQUEST), "Close", &())
            .await;
        return Err(PortalError::TimedOut);
    };
    let (response, results): (u32, HashMap<String, OwnedValue>) = msg.body().deserialize()?;
    autostart_answer(response, &results, on)
}

/// Keeps the computer from suspending until dropped (the Inhibit portal). The
/// session may still lock and the screen turn off, as with `systemd-inhibit` outside
/// the sandbox. The portal also lifts it if Fuselane dies, as the connection closes.
pub struct Inhibit {
    conn: zbus::blocking::Connection,
    handle: OwnedObjectPath,
}

impl std::fmt::Debug for Inhibit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inhibit")
            .field("handle", &self.handle.as_str())
            .finish()
    }
}

impl Inhibit {
    /// Asks the portal; quick (it shows nothing to the user).
    pub fn start() -> Result<Inhibit, PortalError> {
        let conn = zbus::blocking::Connection::session()?;
        let mut options: HashMap<&str, Value<'_>> = HashMap::new();
        options.insert("handle_token", Value::from(new_token()));
        options.insert("reason", Value::from("Downloading"));
        let reply = conn.call_method(
            Some(DEST),
            PATH,
            Some("org.freedesktop.portal.Inhibit"),
            "Inhibit",
            &("", INHIBIT_SUSPEND, options),
        )?;
        let handle: OwnedObjectPath = reply.body().deserialize()?;
        Ok(Inhibit { conn, handle })
    }
}

impl Drop for Inhibit {
    fn drop(&mut self) {
        let _ = self.conn.call_method(
            Some(DEST),
            self.handle.as_str(),
            Some(REQUEST),
            "Close",
            &(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_path_follows_the_portal_rules() {
        assert_eq!(
            request_path(":1.42", "fuselane_00ff"),
            "/org/freedesktop/portal/desktop/request/1_42/fuselane_00ff"
        );
    }

    #[test]
    fn tokens_are_valid_path_elements_and_differ() {
        let a = new_token();
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        assert_ne!(a, new_token());
    }

    fn results(autostart: Option<bool>) -> HashMap<String, OwnedValue> {
        let mut r = HashMap::new();
        r.insert("background".to_string(), OwnedValue::from(true));
        if let Some(a) = autostart {
            r.insert("autostart".to_string(), OwnedValue::from(a));
        }
        r
    }

    #[test]
    fn autostart_answers_are_read_plainly() {
        assert!(matches!(
            autostart_answer(0, &results(Some(true)), true),
            Ok(true)
        ));
        assert!(matches!(
            autostart_answer(0, &results(Some(false)), false),
            Ok(false)
        ));
        // Granted background but not autostart: refused.
        assert!(matches!(
            autostart_answer(0, &results(Some(false)), true),
            Err(PortalError::Refused)
        ));
        assert!(matches!(
            autostart_answer(0, &results(None), true),
            Err(PortalError::Refused)
        ));
        assert!(matches!(
            autostart_answer(1, &results(None), true),
            Err(PortalError::Refused)
        ));
        assert!(matches!(
            autostart_answer(2, &results(None), false),
            Err(PortalError::Failed(_))
        ));
    }
}
