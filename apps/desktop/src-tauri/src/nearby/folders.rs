//! Folders over Nearby (B10.3): walking a folder into files with relative
//! names, and knowing which changed since they were last sent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Most files taken from one folder (a home folder by mistake shouldn't hang).
pub const MAX_FILES: usize = 20_000;

/// One file in a folder: where it is, its name relative to the folder's parent
/// ("Photos/2026/a.jpg"), its size and when it last changed (unix seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub rel: String,
    pub size: u64,
    pub modified: i64,
}

/// Every file under `folder`, skipping hidden files and folders (.git, .DS_Store)
/// and links, named "<folder name>/<path inside>" with `/` separators.
pub fn walk(folder: &Path) -> Vec<Entry> {
    let root = folder
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out = vec![];
    let mut stack = vec![(folder.to_path_buf(), root)];
    while let Some((dir, rel)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut items: Vec<_> = read.flatten().collect();
        items.sort_by_key(std::fs::DirEntry::file_name);
        for e in items {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = e.file_type() else { continue };
            let child = format!("{rel}/{name}");
            if kind.is_dir() {
                stack.push((e.path(), child));
            } else if kind.is_file() {
                let Ok(meta) = e.metadata() else { continue };
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs() as i64);
                out.push(Entry {
                    path: e.path(),
                    rel: child,
                    size: meta.len(),
                    modified,
                });
                if out.len() >= MAX_FILES {
                    return out;
                }
            }
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

/// The files new or changed since `sent` (what was sent last: size and time).
pub fn changed<'a>(now: &'a [Entry], sent: &HashMap<String, (u64, i64)>) -> Vec<&'a Entry> {
    now.iter()
        .filter(|e| sent.get(&e.rel) != Some(&(e.size, e.modified)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_walks_into_relative_names_without_hidden_files() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("Notes");
        std::fs::create_dir_all(root.join("2026/october")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("a.txt"), "a").unwrap();
        std::fs::write(root.join("2026/october/b.md"), "bb").unwrap();
        std::fs::write(root.join(".DS_Store"), "x").unwrap();
        std::fs::write(root.join(".git/config"), "x").unwrap();
        let rels: Vec<String> = walk(&root).into_iter().map(|e| e.rel).collect();
        assert_eq!(rels, ["Notes/2026/october/b.md", "Notes/a.txt"]);
    }

    #[test]
    fn only_new_and_changed_files_are_sent_again() {
        let e = |rel: &str, size, modified| Entry {
            path: rel.into(),
            rel: rel.into(),
            size,
            modified,
        };
        let now = [e("N/a", 1, 10), e("N/b", 2, 20), e("N/c", 3, 30)];
        let sent = HashMap::from([
            ("N/a".to_string(), (1, 10)),
            ("N/b".to_string(), (2, 19)), // edited since
        ]);
        let names: Vec<&str> = changed(&now, &sent)
            .iter()
            .map(|e| e.rel.as_str())
            .collect();
        assert_eq!(names, ["N/b", "N/c"]);
    }
}
