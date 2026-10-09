//! Already downloaded (B9.8): before downloading a file again, say so when a
//! finished download with the same name and size is still on disk, so a big
//! file isn't fetched twice by accident.

use serde::Serialize;

use fuselane_core::Status;

use super::{Service, UiError, store_error};

/// A finished download that looks like the file about to be downloaded.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HaveView {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub size: u64,
    /// When it finished (unix seconds).
    pub finished_at: i64,
}

impl Service {
    /// The newest finished download named `name` whose file is still there
    /// with exactly `size` bytes. Without a size nothing matches: a name alone
    /// ("setup.exe") says too little.
    pub fn already_have(&self, name: &str, size: Option<u64>) -> Result<Option<HaveView>, UiError> {
        let (Some(size), name) = (size, name.trim()) else {
            return Ok(None);
        };
        if name.is_empty() {
            return Ok(None);
        }
        let mut jobs = self.store.list().map_err(store_error)?;
        jobs.sort_by_key(|j| std::cmp::Reverse(j.updated_at));
        Ok(jobs.into_iter().find_map(|j| {
            if j.status != Status::Completed {
                return None;
            }
            let path = j.final_path?;
            let file = path.file_name()?.to_str()?;
            // Finished copies may be "name (2).iso"; the size still has to match.
            let same = file.eq_ignore_ascii_case(name) || stem_copy(file, name);
            let on_disk = std::fs::metadata(&path).ok()?;
            (same && on_disk.is_file() && on_disk.len() == size).then(|| HaveView {
                id: j.id,
                name: file.to_string(),
                path: path.to_string_lossy().into_owned(),
                size,
                finished_at: j.updated_at,
            })
        }))
    }
}

/// `os (2).iso` is a kept-both copy of `os.iso`.
fn stem_copy(file: &str, name: &str) -> bool {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let Some(rest) = file.strip_prefix(stem) else {
        return false;
    };
    let rest = rest.strip_suffix(&format!(".{ext}")).unwrap_or(rest);
    rest.strip_prefix(" (")
        .and_then(|r| r.strip_suffix(')'))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::stem_copy;

    #[test]
    fn kept_both_copies_count_as_the_same_file() {
        assert!(stem_copy("os (2).iso", "os.iso"));
        assert!(stem_copy("README (12)", "README"));
        assert!(!stem_copy("os (x).iso", "os.iso"));
        assert!(!stem_copy("os-2.iso", "os.iso"));
        assert!(!stem_copy("other (2).iso", "os.iso"));
    }
}
