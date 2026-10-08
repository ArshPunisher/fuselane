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
