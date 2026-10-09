//! Torrent path safety (L-68). librqbit already refuses `..`, separators inside a
//! name and empty names. Before anything is written, this check also refuses what
//! would land somewhere unexpected on the user's disk: names differing only by
//! case, a file that is also a folder, names Windows can't hold (on Windows), and
//! any symlink already sitting in the way under the download folder.

use std::collections::HashMap;
use std::path::Path;

const MAX_COMPONENT: usize = 255;
const MAX_DEPTH: usize = 64;
const MAX_PATH: usize = 4096;

/// Which filesystem rules apply. Windows rules are only enforced on Windows, so a
/// Linux user can still download a file named `a:b`.
#[derive(Debug, Clone, Copy)]
pub struct Rules {
    pub windows: bool,
}

impl Rules {
    pub fn native() -> Self {
        Self {
            windows: cfg!(windows),
        }
    }
}

/// One file the torrent wants to write, as path components under its folder.
#[derive(Debug, Clone)]
pub struct Planned {
    pub parts: Vec<String>,
    pub len: u64,
    /// BEP 47 padding files are never written, but their names are still checked.
    pub padding: bool,
}

fn show(parts: &[String]) -> String {
    parts.join("/")
}

fn windows_reserved(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ((stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.len() == 4
        && matches!(stem.chars().nth(3), Some('0'..='9' | '¹' | '²' | '³')))
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(&stem[3..], "¹" | "²" | "³"))
}

fn check_component(c: &str, rules: Rules) -> Result<(), String> {
    if c.is_empty() || c == "." || c == ".." {
        return Err(format!("has an empty or \"{c}\" folder name"));
    }
    if c.len() > MAX_COMPONENT {
        return Err(format!("has a name longer than {MAX_COMPONENT} bytes"));
    }
    if c.chars()
        .any(|ch| ch == '/' || ch == '\\' || ch.is_control())
    {
        return Err("has a slash or control character inside a name".into());
    }
    if rules.windows {
        if c.chars()
            .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        {
            return Err(format!("has a name Windows can't store (\"{c}\")"));
        }
        if c.ends_with('.') || c.ends_with(' ') {
            return Err(format!(
                "has a name ending in a dot or space (\"{c}\"), which Windows drops"
            ));
        }
        if windows_reserved(c) {
            return Err(format!("uses \"{c}\", a reserved device name on Windows"));
        }
    }
    Ok(())
}

/// Checks every planned path, then what is already on disk under `base`, where the
/// torrent writes into `base/folder_parts/...`.
/// The message says which path is wrong and why, ready for the user.
pub fn check(
    base: &Path,
    folder_parts: &[String],
    files: &[Planned],
    rules: Rules,
) -> Result<(), String> {
    for c in folder_parts {
        check_component(c, rules).map_err(|why| format!("The torrent's folder name {why}."))?;
    }
    // Lower-cased path -> (original, is a file).
    let mut seen: HashMap<String, (String, bool)> = HashMap::new();
    for f in files {
        let shown = show(&f.parts);
        if f.parts.is_empty() || f.parts.len() > MAX_DEPTH || shown.len() > MAX_PATH {
            return Err(format!("\"{shown}\" is empty or nested too deeply."));
        }
        for c in &f.parts {
            check_component(c, rules).map_err(|why| format!("\"{shown}\" {why}."))?;
        }
        for depth in 1..=f.parts.len() {
            let is_file = depth == f.parts.len();
            let key = show(&f.parts[..depth]).to_lowercase();
            let here = show(&f.parts[..depth]);
            match seen.get(&key) {
                None => {
                    seen.insert(key, (here, is_file));
                }
                Some((other, other_is_file)) => {
                    if is_file || *other_is_file {
                        return Err(if *other == here {
                            format!("\"{here}\" appears twice, or is both a file and a folder.")
                        } else {
                            format!(
                                "\"{here}\" and \"{other}\" differ only by upper/lower case and would overwrite each other."
                            )
                        });
                    }
                    if *other != here {
                        return Err(format!(
                            "The folders \"{here}\" and \"{other}\" differ only by upper/lower case."
                        ));
                    }
                }
            }
        }
    }
    // Nothing already on disk may redirect a write: no symlink anywhere on the way.
    let mut folder = base.to_path_buf();
    for part in folder_parts {
        folder.push(part);
        refuse_link(&folder)?;
    }
    for f in files.iter().filter(|f| !f.padding) {
        let mut p = folder.to_path_buf();
        for part in &f.parts {
            p.push(part);
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(format!(
                        "\"{}\" is a link on your disk; Fuselane won't write through it.",
                        p.display()
                    ));
                }
                Ok(_) => {}
                Err(_) => break, // nothing there yet: deeper parts can't exist either
            }
        }
    }
    Ok(())
}

/// What a cleanup did. Anything skipped is left on disk and named here.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Cleanup {
    pub removed: usize,
    pub skipped: Vec<String>,
}

/// Deletes these files under `folder`, never following a symlink and never
/// deleting anything that isn't a regular file. Then removes folders left empty,
/// up to `folder` itself when the torrent owns it. A folder swapped for a link
/// between checking and deleting is caught by re-checking each step.
pub fn remove(folder: &Path, own_folder: bool, files: &[&Planned]) -> Cleanup {
    remove_with(folder, own_folder, files, |p| std::fs::remove_file(p))
}

/// [`remove`], with the caller choosing how each checked file goes away (the
/// desktop app moves them to the Trash so a mistaken removal can be undone).
pub fn remove_with(
    folder: &Path,
    own_folder: bool,
    files: &[&Planned],
    mut remove_file: impl FnMut(&Path) -> std::io::Result<()>,
) -> Cleanup {
    let mut out = Cleanup::default();
    if refuse_link(folder).is_err() {
        out.skipped.push(folder.display().to_string());
        return out;
    }
    let mut dirs = std::collections::BTreeSet::new();
    'file: for f in files.iter().filter(|f| !f.padding) {
        let mut p = folder.to_path_buf();
        for (i, part) in f.parts.iter().enumerate() {
            p.push(part);
            let last = i + 1 == f.parts.len();
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.file_type().is_symlink() => {
                    out.skipped.push(p.display().to_string());
                    continue 'file;
                }
                Ok(m) if last && !m.is_file() => {
                    out.skipped.push(p.display().to_string());
                    continue 'file;
                }
                Ok(m) if !last && !m.is_dir() => continue 'file,
                Ok(_) => {}
                Err(_) => continue 'file, // already gone
            }
            if !last {
                dirs.insert(p.clone());
            }
        }
        match remove_file(&p) {
            Ok(()) => out.removed += 1,
            Err(_) => out.skipped.push(p.display().to_string()),
        }
    }
    // Deepest first; remove_dir only succeeds on empty folders.
    let mut dirs: Vec<_> = dirs.into_iter().collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for d in dirs {
        if refuse_link(&d).is_ok() {
            let _ = std::fs::remove_dir(&d);
        }
    }
    if own_folder {
        let _ = std::fs::remove_dir(folder);
    }
    out
}

fn refuse_link(p: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.file_type().is_symlink() => Err(format!(
            "\"{}\" is a link on your disk; Fuselane won't write through it.",
            p.display()
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(paths: &[&str]) -> Vec<Planned> {
        paths
            .iter()
            .map(|p| Planned {
                parts: p.split('/').map(String::from).collect(),
                len: 1,
                padding: false,
            })
            .collect()
    }
    const UNIX: Rules = Rules { windows: false };
    const WIN: Rules = Rules { windows: true };

    fn run(paths: &[&str], rules: Rules) -> Result<(), String> {
        let dir = tempfile::tempdir().unwrap();
        check(dir.path(), &["T".into()], &files(paths), rules)
    }

    #[test]
    fn ordinary_torrents_pass() {
        assert_eq!(
            run(
                &["a.txt", "sub/b.txt", "sub/deeper/c.bin", "Sub2/a.txt"],
                WIN
            ),
            Ok(())
        );
        assert_eq!(
            run(&["weird: name?.txt"], UNIX),
            Ok(()),
            "Unix can store this"
        );
    }

    #[test]
    fn case_only_differences_are_refused() {
        let e = run(&["Readme.txt", "README.TXT"], UNIX).unwrap_err();
        assert!(e.contains("upper/lower case"), "{e}");
        assert!(
            run(&["Docs/a", "docs/b"], UNIX)
                .unwrap_err()
                .contains("folders")
        );
        assert!(run(&["Ä/x", "ä/y"], UNIX).is_err(), "non-ASCII case too");
    }

    #[test]
    fn a_file_that_is_also_a_folder_is_refused() {
        assert!(run(&["a", "a/b"], UNIX).is_err());
        assert!(run(&["a/b", "a"], UNIX).is_err());
        assert!(run(&["x/A", "x/a/b"], UNIX).is_err());
        assert!(run(&["same", "same"], UNIX).is_err());
    }

    #[test]
    fn bad_components_are_refused() {
        for bad in [
            "",
            ".",
            "..",
            "a\u{0}b",
            "line\nbreak",
            "tab\there",
            &"x".repeat(256),
        ] {
            assert!(run(&[&format!("ok/{bad}")], UNIX).is_err(), "{bad:?}");
        }
        assert!(run(&[&vec!["d"; 65].join("/")], UNIX).is_err(), "too deep");
    }

    #[test]
    fn windows_names_are_refused_only_on_windows() {
        for bad in [
            "CON",
            "con.txt",
            "Aux.tar.gz",
            "nul",
            "COM1",
            "lpt9.log",
            "COM¹",
            "a:b",
            "q?",
            "star*",
            "trail.",
            "space ",
            "pipe|",
        ] {
            assert!(run(&[bad], WIN).is_err(), "{bad}");
            if !matches!(bad, "trail." | "space ") {
                assert_eq!(run(&[bad], UNIX), Ok(()), "{bad}");
            }
        }
        for fine in ["CONSOLE", "com10", "lpt", "auxiliary.txt", "nul_byte.txt"] {
            assert_eq!(run(&[fine], WIN), Ok(()), "{fine}");
        }
        assert!(
            check(Path::new("/x"), &["CON".into()], &files(&["a"]), WIN).is_err(),
            "folder name too"
        );
    }

    #[test]
    fn removal_deletes_only_the_listed_files_and_empty_folders() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("T");
        std::fs::create_dir_all(folder.join("sub/deep")).unwrap();
        std::fs::write(folder.join("a"), b"1").unwrap();
        std::fs::write(folder.join("sub/deep/b"), b"2").unwrap();
        std::fs::write(folder.join("sub/mine.txt"), b"user file").unwrap();
        let fs = files(&["a", "sub/deep/b", "missing/c"]);
        let r = remove(&folder, true, &fs.iter().collect::<Vec<_>>());
        assert_eq!(
            r,
            Cleanup {
                removed: 2,
                skipped: vec![]
            }
        );
        assert!(!folder.join("sub/deep").exists(), "empty folder removed");
        assert!(
            folder.join("sub/mine.txt").exists(),
            "a file not in the torrent stays"
        );
        assert!(folder.exists(), "folder isn't empty, so it stays");
    }

    #[test]
    fn a_custom_remover_gets_only_checked_files() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("T");
        std::fs::create_dir_all(folder.join("sub")).unwrap();
        std::fs::write(folder.join("a"), b"1").unwrap();
        std::fs::write(folder.join("sub/b"), b"2").unwrap();
        std::fs::create_dir(folder.join("dir-not-file")).unwrap();
        let fs = files(&["a", "sub/b", "dir-not-file", "gone"]);
        let mut seen = vec![];
        // Stands in for "move to the Trash": records, then takes the file away.
        let r = remove_with(&folder, true, &fs.iter().collect::<Vec<_>>(), |p| {
            seen.push(p.strip_prefix(&folder).unwrap().to_path_buf());
            std::fs::remove_file(p)
        });
        assert_eq!(
            seen,
            vec![
                std::path::PathBuf::from("a"),
                std::path::PathBuf::from("sub/b")
            ]
        );
        assert_eq!(r.removed, 2);
        assert_eq!(r.skipped.len(), 1, "the folder posing as a file is skipped");
        // A remover that fails is reported, not counted.
        std::fs::write(folder.join("a"), b"1").unwrap();
        let fs = files(&["a"]);
        let r = remove_with(&folder, true, &fs.iter().collect::<Vec<_>>(), |_| {
            Err(std::io::Error::other("no trash here"))
        });
        assert_eq!((r.removed, r.skipped.len()), (0, 1));
        assert!(folder.join("a").exists());
    }

    #[test]
    fn removal_skips_a_folder_where_a_file_should_be() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/inner")).unwrap();
        let fs = files(&["a"]);
        let r = remove(dir.path(), false, &fs.iter().collect::<Vec<_>>());
        assert_eq!(r.removed, 0);
        assert_eq!(r.skipped.len(), 1);
        assert!(dir.path().join("a/inner").exists());
    }

    #[cfg(unix)]
    #[test]
    fn removal_never_follows_a_swapped_in_link() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("b"), b"precious").unwrap();
        let folder = dir.path().join("T");
        std::fs::create_dir(&folder).unwrap();
        // The torrent had sub/b; someone replaced sub with a link to another folder.
        std::os::unix::fs::symlink(outside.path(), folder.join("sub")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("b"), folder.join("direct")).unwrap();
        let fs = files(&["sub/b", "direct"]);
        let r = remove(&folder, true, &fs.iter().collect::<Vec<_>>());
        assert_eq!(r.removed, 0);
        assert_eq!(r.skipped.len(), 2);
        assert_eq!(
            std::fs::read(outside.path().join("b")).unwrap(),
            b"precious"
        );
        // The whole torrent folder swapped for a link.
        let dir2 = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir2.path().join("T")).unwrap();
        let fs = files(&["b"]);
        let r = remove(&dir2.path().join("T"), true, &fs.iter().collect::<Vec<_>>());
        assert_eq!(r.removed, 0);
        assert!(outside.path().join("b").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_on_disk_are_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let folder = dir.path().join("T");
        std::fs::create_dir(&folder).unwrap();
        std::os::unix::fs::symlink(outside.path(), folder.join("sub")).unwrap();
        let e = check(dir.path(), &["T".into()], &files(&["sub/evil.txt"]), UNIX).unwrap_err();
        assert!(e.contains("link"), "{e}");
        // The torrent folder itself being a link.
        let dir2 = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir2.path().join("T")).unwrap();
        assert!(check(dir2.path(), &["T".into()], &files(&["a"]), UNIX).is_err());
    }
}
