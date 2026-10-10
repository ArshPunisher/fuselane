//! Metalink downloads (8.3): the files a Metalink lists, each with its other
//! places as mirrors and its SHA-256 checked at the end, kept together as a
//! group.

use std::sync::Arc;

use super::{AddRequest, BatchResult, Service, Skipped, UiError};
use fuselane_core::metalink;

/// Largest Metalink read.
const MAX_METALINK: usize = 4 * 1024 * 1024;

impl Service {
    /// Adds the files of the Metalink at `link`. Something that turns out not
    /// to be a Metalink is downloaded as an ordinary file.
    pub async fn add_metalink(
        self: &Arc<Self>,
        link: &str,
        dir: Option<&str>,
        later: bool,
    ) -> Result<BatchResult, UiError> {
        let link = link.trim();
        fuselane_core::runner::parse_link(link).map_err(|m| {
            UiError::new("bad-link", m, Some("Links start with http:// or https://."))
        })?;
        let (_, text) = fuselane_core::runner::fetch_text(link, MAX_METALINK)
            .await
            .map_err(|m| UiError::new("metalink-failed", m, None))?;
        if !metalink::looks_like(&text) {
            let req = AddRequest {
                later,
                ..AddRequest::default()
            };
            let id = self.add_with(link, dir, &req)?;
            return Ok(BatchResult {
                added: vec![id],
                ..BatchResult::default()
            });
        }
        self.add_metalink_text(&text, dir, later)
    }

    /// Adds the files of a Metalink document.
    pub fn add_metalink_text(
        self: &Arc<Self>,
        text: &str,
        dir: Option<&str>,
        later: bool,
    ) -> Result<BatchResult, UiError> {
        let files = metalink::parse(text).map_err(|m| {
            UiError::new(
                "metalink-bad",
                m,
                Some("Download the .meta4 or .metalink file again from its page."),
            )
        })?;
        let mut result = BatchResult::default();
        let mut links = Vec::new();
        for f in files {
            let req = AddRequest {
                name: Some(f.name.clone()),
                sha256: f.sha256.clone(),
                later,
                mirrors: f.urls[1..].to_vec(),
                ..AddRequest::default()
            };
            match self.add_with(&f.urls[0], dir, &req) {
                Ok(id) => {
                    result.added.push(id);
                    links.push(f.urls[0].clone());
                }
                Err(e) => result.skipped.push(Skipped {
                    url: f.urls[0].clone(),
                    reason: e.message,
                }),
            }
        }
        result.group = self.group_added(Some(""), &links, &result.added)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn files_come_with_mirrors_checksums_and_a_group() {
        let dir = tempfile::tempdir().unwrap();
        let store = fuselane_core::Store::open(&dir.path().join("db")).unwrap();
        let svc = Service::new(store, dir.path().to_path_buf()).unwrap();
        let sha = "c9e15763f722f23e98a29decdfae341b98d53056c9e15763f722f23e98a29dec";
        let xml = format!(
            r#"<metalink xmlns="urn:ietf:params:xml:ns:metalink">
  <file name="a.iso"><hash type="sha-256">{sha}</hash>
    <url priority="1">https://one.example/a.iso</url>
    <url priority="2">https://two.example/a.iso</url></file>
  <file name="b.iso"><url>https://one.example/b.iso</url></file>
</metalink>"#
        );
        let r = svc.add_metalink_text(&xml, None, true).unwrap();
        assert_eq!(r.added.len(), 2);
        assert!(r.group.is_some(), "two files: one group");
        let jobs = svc.jobs().unwrap();
        let a = jobs.iter().find(|j| j.name == "a.iso").unwrap();
        assert!(a.verify, "its SHA-256 is checked");
        assert_eq!(a.mirrors, ["two.example"]);
        assert_eq!(a.status, "paused", "Download later");
        // Not a Metalink: said so, with what to do.
        let e = svc.add_metalink_text("<rss/>", None, false).unwrap_err();
        assert_eq!(e.code, "metalink-bad");
    }
}
