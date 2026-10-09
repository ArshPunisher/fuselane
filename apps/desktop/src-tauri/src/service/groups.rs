//! Groups (B9.2): links added together stay together. The list shows a group
//! as one row with one progress; it can be paused or resumed as a whole, and
//! one notification says when all of it is done.

use std::sync::Arc;

use fuselane_core::Status;

use super::{Service, UiError, not_found, store_error};

/// Longest group name.
const MAX_NAME: usize = 80;

/// "12 files from releases.ubuntu.com", or "12 downloads" from several sites.
pub fn default_name(links: &[String]) -> String {
    let hosts: Vec<String> = links
        .iter()
        .filter_map(|l| url::Url::parse(l).ok()?.host_str().map(str::to_string))
        .collect();
    let n = links.len();
    match hosts.first() {
        Some(h) if hosts.len() == n && hosts.iter().all(|x| x == h) => {
            format!("{n} files from {h}")
        }
        _ => format!("{n} downloads"),
    }
}

/// A name from the window: trimmed, short, no control characters.
pub fn clean_name(name: &str) -> Result<String, UiError> {
    let name = name.trim();
    let bad = |m: &str| UiError::new("bad-group-name", m, Some("Use up to 80 characters."));
    if name.chars().count() > MAX_NAME {
        return Err(bad("That group name is too long."));
    }
    if name.chars().any(char::is_control) {
        return Err(bad("Group names can't contain control characters."));
    }
    Ok(name.to_string())
}

impl Service {
    /// Puts downloads just added together into a new group; returns its id.
    pub(super) fn group_added(
        &self,
        name: Option<&str>,
        links: &[String],
        ids: &[i64],
    ) -> Result<Option<i64>, UiError> {
        if ids.len() < 2 {
            return Ok(None);
        }
        let Some(name) = name else {
            return Ok(None);
        };
        let mut name = clean_name(name)?;
        if name.is_empty() {
            name = default_name(links);
        }
        let id = self.store.create_group(&name, ids).map_err(store_error)?;
        self.publish_jobs();
        Ok(Some(id))
    }

    pub fn rename_group(&self, id: i64, name: &str) -> Result<(), UiError> {
        let name = clean_name(name)?;
        if name.is_empty() {
            return Err(UiError::new(
                "bad-group-name",
                "Give the group a name.",
                None,
            ));
        }
        self.store.rename_group(id, &name).map_err(|e| match e {
            fuselane_core::StoreError::NotFound(id) => not_found(id),
            e => store_error(e),
        })?;
        self.publish_jobs();
        Ok(())
    }

    /// The group's downloads go back to being single ones.
    pub fn ungroup(&self, id: i64) -> Result<(), UiError> {
        self.store.ungroup(id).map_err(|e| match e {
            fuselane_core::StoreError::NotFound(id) => not_found(id),
            e => store_error(e),
        })?;
        self.publish_jobs();
        Ok(())
    }

    fn members(&self, id: i64) -> Result<Vec<fuselane_core::Job>, UiError> {
        let jobs: Vec<_> = self
            .store
            .list()
            .map_err(store_error)?
            .into_iter()
            .filter(|j| j.group_id == Some(id))
            .collect();
        if jobs.is_empty() {
            return Err(not_found(id));
        }
        Ok(jobs)
    }

    /// Pauses every download in the group that is running or waiting.
    pub fn pause_group(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        for j in self.members(id)? {
            if matches!(j.status, Status::Running | Status::Queued) {
                self.pause(j.id)?;
            }
        }
        Ok(())
    }

    /// Resumes every paused (or resumable failed) download in the group.
    pub fn resume_group(self: &Arc<Self>, id: i64) -> Result<(), UiError> {
        for j in self.members(id)? {
            if matches!(
                j.status,
                Status::Paused | Status::Failed { resumable: true }
            ) {
                let _ = self.resume(j.id);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_get_a_useful_name_by_themselves() {
        let same: Vec<String> = (1..=3)
            .map(|i| format!("https://releases.example.org/part{i}.zip"))
            .collect();
        assert_eq!(default_name(&same), "3 files from releases.example.org");
        let mixed = vec![
            "https://a.example/x".to_string(),
            "https://b.example/y".into(),
        ];
        assert_eq!(default_name(&mixed), "2 downloads");
    }

    #[test]
    fn group_names_are_checked() {
        assert_eq!(clean_name("  Season 1 ").unwrap(), "Season 1");
        assert!(clean_name(&"x".repeat(81)).is_err());
        assert!(clean_name("a\u{7}b").is_err());
    }
}
