//! LocalSend v2 messages (docs/03-architecture/NEARBY.md), with the checks every
//! incoming message goes through before anything else looks at it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The protocol version we speak.
pub const VERSION: &str = "2.1";
/// LocalSend's default port and multicast group.
pub const PORT: u16 = 53317;
pub const MULTICAST: std::net::Ipv4Addr = std::net::Ipv4Addr::new(224, 0, 0, 167);

/// Most files one request may offer.
pub const MAX_FILES: usize = 1000;
/// Largest JSON body we read (metadata only; previews are dropped).
pub const MAX_JSON: usize = 2 << 20;

/// Who a device is, as it says. Names are labels only; the fingerprint is the identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub alias: String,
    pub version: String,
    #[serde(default)]
    pub device_model: Option<String>,
    #[serde(default)]
    pub device_type: Option<String>,
    #[serde(default)]
    pub fingerprint: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default)]
    pub download: bool,
}

fn default_port() -> u16 {
    PORT
}

fn default_protocol() -> String {
    "https".into()
}

/// A multicast message: the device's info plus whether it asks for answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    #[serde(flatten)]
    pub info: DeviceInfo,
    #[serde(default)]
    pub announce: bool,
}

/// One file offered in a transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub id: String,
    pub file_name: String,
    pub size: u64,
    #[serde(default)]
    pub file_type: String,
    #[serde(default)]
    pub sha256: Option<String>,
    /// For a text message (`text/plain`), the text itself; LocalSend sends it
    /// here instead of uploading a file. Thumbnails of other files are ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// Longest text message taken (a clipboard's worth; anything longer is a file).
pub const MAX_TEXT: usize = 64 * 1024;

impl FileMeta {
    /// The text, when this "file" is a text message LocalSend-style.
    pub fn message(&self) -> Option<&str> {
        self.preview
            .as_deref()
            .filter(|t| self.file_type.starts_with("text/") && t.len() <= MAX_TEXT)
    }
}

/// `POST /api/localsend/v2/prepare-upload`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUpload {
    pub info: DeviceInfo,
    pub files: BTreeMap<String, FileMeta>,
}

/// The answer to an accepted prepare-upload: a token per accepted file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareUploadReply {
    pub session_id: String,
    pub files: BTreeMap<String, String>,
}

/// Why a message was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Invalid {
    #[error("the message isn't valid JSON for this request")]
    Json,
    #[error("the device name is empty or too long")]
    Alias,
    #[error("it offers no files")]
    NoFiles,
    #[error("it offers more than {MAX_FILES} files")]
    TooManyFiles,
    #[error("a file id doesn't match its entry")]
    FileId,
    #[error("a file name can't be used")]
    FileName,
}

fn check_info(i: &DeviceInfo) -> Result<(), Invalid> {
    let n = i.alias.trim().chars().count();
    if n == 0 || n > 64 || i.alias.chars().any(char::is_control) {
        return Err(Invalid::Alias);
    }
    Ok(())
}

/// Reads and checks a prepare-upload body: a sane name, 1 to 1000 files, ids
/// that match, and names that clean up to something usable (no folders, no
/// `..`, nothing hidden). Returns the request with each name already cleaned.
pub fn parse_prepare(body: &[u8]) -> Result<PrepareUpload, Invalid> {
    if body.len() > MAX_JSON {
        return Err(Invalid::Json);
    }
    let mut p: PrepareUpload = serde_json::from_slice(body).map_err(|_| Invalid::Json)?;
    check_info(&p.info)?;
    if p.files.is_empty() {
        return Err(Invalid::NoFiles);
    }
    if p.files.len() > MAX_FILES {
        return Err(Invalid::TooManyFiles);
    }
    for (key, f) in &mut p.files {
        if *key != f.id || key.is_empty() || key.len() > 128 {
            return Err(Invalid::FileId);
        }
        // Only the last part of a path counts; the engine's cleaning does the rest.
        let last = f.file_name.rsplit(['/', '\\']).next().unwrap_or("").trim();
        if last.is_empty() || last.starts_with('.') {
            return Err(Invalid::FileName);
        }
        let clean = fuselane_storage::names::sanitize(last);
        if clean.is_empty() || clean.starts_with('.') {
            return Err(Invalid::FileName);
        }
        f.file_name = clean;
    }
    Ok(p)
}

/// Reads an announcement or register body.
pub fn parse_info(body: &[u8]) -> Result<DeviceInfo, Invalid> {
    if body.len() > 64 * 1024 {
        return Err(Invalid::Json);
    }
    let i: DeviceInfo = serde_json::from_slice(body).map_err(|_| Invalid::Json)?;
    check_info(&i)?;
    Ok(i)
}

pub fn parse_announcement(body: &[u8]) -> Result<Announcement, Invalid> {
    if body.len() > 64 * 1024 {
        return Err(Invalid::Json);
    }
    let a: Announcement = serde_json::from_slice(body).map_err(|_| Invalid::Json)?;
    check_info(&a.info)?;
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREPARE: &str = r#"{
      "info": {"alias": "Nice Orange", "version": "2.0", "deviceModel": "Samsung",
               "deviceType": "mobile", "fingerprint": "abc", "port": 53317,
               "protocol": "https", "download": true},
      "files": {
        "f1": {"id": "f1", "fileName": "my image.png", "size": 324242,
               "fileType": "image/jpeg", "sha256": null, "preview": "AAAA"},
        "f2": {"id": "f2", "fileName": "../../etc/evil.sh", "size": 1, "fileType": "text/plain"}
      }
    }"#;

    #[test]
    fn a_localsend_request_parses_and_names_are_cleaned() {
        let p = parse_prepare(PREPARE.as_bytes()).unwrap();
        assert_eq!(p.info.alias, "Nice Orange");
        assert_eq!(p.info.device_type.as_deref(), Some("mobile"));
        assert_eq!(p.files["f1"].size, 324_242);
        assert_eq!(p.files["f2"].file_name, "evil.sh", "path parts are dropped");
    }

    #[test]
    fn bad_requests_are_refused_with_a_reason() {
        let bad = |s: &str| parse_prepare(s.as_bytes()).unwrap_err();
        assert_eq!(bad("{"), Invalid::Json);
        let info = r#""info":{"alias":"A","version":"2.0"}"#;
        assert_eq!(bad(&format!("{{{info},\"files\":{{}}}}")), Invalid::NoFiles);
        assert_eq!(
            bad(&format!(
                r#"{{{info},"files":{{"a":{{"id":"b","fileName":"x","size":1}}}}}}"#
            )),
            Invalid::FileId
        );
        assert_eq!(
            bad(&format!(
                r#"{{{info},"files":{{"a":{{"id":"a","fileName":"..","size":1}}}}}}"#
            )),
            Invalid::FileName
        );
        assert_eq!(
            bad(&format!(
                r#"{{{info},"files":{{"a":{{"id":"a","fileName":".hidden","size":1}}}}}}"#
            )),
            Invalid::FileName
        );
        let long = "x".repeat(65);
        assert_eq!(
            parse_info(format!(r#"{{"alias":"{long}","version":"2.0"}}"#).as_bytes()).unwrap_err(),
            Invalid::Alias
        );
        let many: String = (0..=MAX_FILES)
            .map(|i| format!(r#""f{i}":{{"id":"f{i}","fileName":"a{i}","size":1}}"#))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            bad(&format!("{{{info},\"files\":{{{many}}}}}")),
            Invalid::TooManyFiles
        );
    }

    #[test]
    fn announcements_round_trip() {
        let a = parse_announcement(
            br#"{"alias":"Secret Banana","version":"2.0","deviceModel":"Windows","deviceType":"desktop","fingerprint":"F","port":53317,"protocol":"https","download":false,"announce":true}"#,
        )
        .unwrap();
        assert!(a.announce);
        assert_eq!(a.info.port, 53317);
        let back = serde_json::to_string(&a).unwrap();
        assert!(back.contains("\"announce\":true") && back.contains("\"deviceType\":\"desktop\""));
    }
}
