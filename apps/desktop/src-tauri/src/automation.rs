//! Download-manager automation, as pure decisions (IDM parity, PARITY-CHECKLIST
//! §3): a daily schedule window, what to do once everything has finished, keeping
//! the computer awake, and sorting finished files into folders by type.

use serde::{Deserialize, Serialize};

/// Only download between `start` and `stop` (local time) on the chosen days. A
/// window that ends before it starts runs overnight: Monday 23:00–07:00 means
/// Monday 23:00 to Tuesday 07:00.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub enabled: bool,
    /// Minutes after midnight, 0–1439.
    pub start: u16,
    pub stop: u16,
    /// Monday first.
    pub days: [bool; 7],
}

impl Default for Schedule {
    fn default() -> Self {
        // A common choice: overnight, when data is cheap and nobody is waiting.
        Schedule {
            enabled: false,
            start: 60,
            stop: 7 * 60,
            days: [true; 7],
        }
    }
}

/// What to do once every download has finished.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WhenDone {
    #[default]
    Nothing,
    Sleep,
    ShutDown,
    Quit,
}

/// What to do when a file with the same name is already in the folder (B8.6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NameTaken {
    /// The New download dialog asks; downloads added elsewhere keep both.
    #[default]
    Ask,
    /// Save as "name (1).ext" next to the old one.
    KeepBoth,
    /// Move the old file to the Trash once the new one is complete.
    Replace,
}

/// What happens to each download once it has finished (B8.7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AfterDownload {
    #[default]
    Nothing,
    /// Open it with its usual app.
    Open,
    /// Unpack zip and tar archives into a folder next to them.
    Unpack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Automation {
    pub schedule: Schedule,
    pub when_done: WhenDone,
    /// Stop the computer sleeping while something downloads.
    pub keep_awake: bool,
    /// Move finished files into Video, Music, Documents… under the downloads folder.
    pub sort_by_type: bool,
    pub name_taken: NameTaken,
    pub after_download: AfterDownload,
}

impl Default for Automation {
    fn default() -> Self {
        Automation {
            schedule: Schedule::default(),
            when_done: WhenDone::Nothing,
            keep_awake: true,
            sort_by_type: false,
            name_taken: NameTaken::Ask,
            after_download: AfterDownload::Nothing,
        }
    }
}

impl Automation {
    pub fn validated(self) -> Result<Automation, String> {
        let s = &self.schedule;
        if s.start >= 1440 || s.stop >= 1440 {
            return Err("Times must be between 00:00 and 23:59.".into());
        }
        if s.enabled && s.start == s.stop {
            return Err(
                "The schedule starts and stops at the same time. Pick a stop time after the start."
                    .into(),
            );
        }
        if s.enabled && !s.days.iter().any(|d| *d) {
            return Err("Pick at least one day for the schedule.".into());
        }
        Ok(self)
    }
}

/// A moment in local time: weekday (0 = Monday) and minutes after midnight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    pub weekday: u8,
    pub minute: u16,
}

impl Moment {
    pub fn now() -> Moment {
        use chrono::{Datelike, Timelike};
        let t = chrono::Local::now();
        Moment {
            weekday: t.weekday().num_days_from_monday() as u8,
            minute: (t.hour() * 60 + t.minute()) as u16,
        }
    }
}

/// May downloads run at `at`?
pub fn allowed(s: &Schedule, at: Moment) -> bool {
    if !s.enabled {
        return true;
    }
    let day = |d: u8| s.days[usize::from(d % 7)];
    if s.start < s.stop {
        day(at.weekday) && at.minute >= s.start && at.minute < s.stop
    } else {
        // Overnight: the evening part belongs to today, the morning part to yesterday.
        (day(at.weekday) && at.minute >= s.start) || (day(at.weekday + 6) && at.minute < s.stop)
    }
}

/// Minutes until `allowed` next changes (at most a week ahead), for "starts at 01:00".
pub fn minutes_until_change(s: &Schedule, at: Moment) -> Option<u32> {
    if !s.enabled {
        return None;
    }
    let now = allowed(s, at);
    (1..=7 * 1440u32).find(|&m| {
        let total = u32::from(at.minute) + m;
        let next = Moment {
            weekday: ((u32::from(at.weekday) + total / 1440) % 7) as u8,
            minute: (total % 1440) as u16,
        };
        allowed(s, next) != now
    })
}

pub fn clock(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// The folder a finished file goes to when sorting by type, or None to leave it.
pub fn category(file_name: &str) -> Option<&'static str> {
    let ext = file_name.rsplit_once('.')?.1.to_ascii_lowercase();
    // `.tar.gz` and friends: the last part decides, and it's Compressed either way.
    Some(match ext.as_str() {
        "mp4" | "mkv" | "avi" | "mov" | "webm" | "m4v" | "wmv" | "flv" | "mpg" | "mpeg" | "ts"
        | "3gp" => "Video",
        "mp3" | "flac" | "wav" | "aac" | "m4a" | "ogg" | "opus" | "wma" | "aiff" | "alac" => {
            "Music"
        }
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "svg" | "bmp" | "tif" | "tiff"
        | "raw" => "Pictures",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "rtf" | "odt"
        | "ods" | "odp" | "epub" | "csv" | "md" | "pages" | "numbers" | "key" => "Documents",
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "lz" | "cab" => {
            "Compressed"
        }
        "iso" | "img" | "dmg" | "vhd" | "vhdx" | "qcow2" => "Disk Images",
        "exe" | "msi" | "pkg" | "deb" | "rpm" | "appimage" | "apk" | "msix" | "flatpak" => {
            "Programs"
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(weekday: u8, h: u16, m: u16) -> Moment {
        Moment {
            weekday,
            minute: h * 60 + m,
        }
    }

    fn sched(start: u16, stop: u16, days: [bool; 7]) -> Schedule {
        Schedule {
            enabled: true,
            start,
            stop,
            days,
        }
    }

    #[test]
    fn a_disabled_schedule_always_allows() {
        let s = Schedule::default();
        assert!(!s.enabled);
        assert!(allowed(&s, at(3, 12, 0)));
        assert_eq!(minutes_until_change(&s, at(3, 12, 0)), None);
    }

    #[test]
    fn a_daytime_window_includes_its_start_and_excludes_its_stop() {
        let s = sched(
            9 * 60,
            17 * 60,
            [true, true, true, true, true, false, false],
        );
        assert!(!allowed(&s, at(0, 8, 59)));
        assert!(allowed(&s, at(0, 9, 0)));
        assert!(allowed(&s, at(0, 16, 59)));
        assert!(!allowed(&s, at(0, 17, 0)));
        assert!(!allowed(&s, at(5, 12, 0)), "Saturday isn't chosen");
    }

    #[test]
    fn an_overnight_window_belongs_to_the_evening_it_starts() {
        // Only Friday night: Friday 23:00 to Saturday 07:00.
        let mut days = [false; 7];
        days[4] = true;
        let s = sched(23 * 60, 7 * 60, days);
        assert!(!allowed(&s, at(4, 22, 59)));
        assert!(allowed(&s, at(4, 23, 30)), "Friday evening");
        assert!(allowed(&s, at(5, 6, 59)), "Saturday early morning");
        assert!(!allowed(&s, at(5, 7, 0)));
        assert!(!allowed(&s, at(5, 23, 30)), "Saturday night isn't chosen");
        assert!(
            !allowed(&s, at(4, 3, 0)),
            "Friday morning belongs to Thursday"
        );
        // Sunday night wraps to Monday morning.
        let mut sun = [false; 7];
        sun[6] = true;
        let s = sched(22 * 60, 2 * 60, sun);
        assert!(allowed(&s, at(0, 1, 0)), "Monday 01:00 is Sunday night");
    }

    #[test]
    fn the_next_change_is_found_across_days() {
        let s = sched(60, 7 * 60, [true; 7]);
        assert_eq!(
            minutes_until_change(&s, at(2, 0, 30)),
            Some(30),
            "starts at 01:00"
        );
        assert_eq!(
            minutes_until_change(&s, at(2, 6, 0)),
            Some(60),
            "stops at 07:00"
        );
        assert_eq!(
            minutes_until_change(&s, at(2, 7, 0)),
            Some(18 * 60),
            "tomorrow 01:00"
        );
        let mut weekend = [false; 7];
        weekend[5] = true;
        let s = sched(10 * 60, 11 * 60, weekend);
        assert_eq!(
            minutes_until_change(&s, at(0, 10, 0)),
            Some(5 * 1440),
            "Saturday"
        );
    }

    #[test]
    fn nonsense_schedules_are_refused_with_a_reason() {
        let bad_time = Automation {
            schedule: sched(1440, 60, [true; 7]),
            ..Automation::default()
        };
        assert!(bad_time.validated().is_err());
        let same = Automation {
            schedule: sched(60, 60, [true; 7]),
            ..Automation::default()
        };
        assert!(same.validated().unwrap_err().contains("same time"));
        let no_days = Automation {
            schedule: sched(60, 120, [false; 7]),
            ..Automation::default()
        };
        assert!(no_days.validated().unwrap_err().contains("day"));
        // A disabled schedule may hold anything sensible.
        let off = Automation {
            schedule: Schedule {
                enabled: false,
                start: 60,
                stop: 60,
                days: [false; 7],
            },
            ..Automation::default()
        };
        assert!(off.validated().is_ok());
    }

    #[test]
    fn saved_settings_round_trip_and_fill_in_new_fields() {
        let a = Automation {
            when_done: WhenDone::ShutDown,
            sort_by_type: true,
            ..Automation::default()
        };
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains(r#""whenDone":"shut-down""#), "{json}");
        assert_eq!(serde_json::from_str::<Automation>(&json).unwrap(), a);
        // From an older version that only knew some fields.
        let old: Automation = serde_json::from_str(r#"{"sortByType":true}"#).unwrap();
        assert!(old.sort_by_type && old.keep_awake && old.when_done == WhenDone::Nothing);
    }

    #[test]
    fn files_sort_into_the_folder_people_expect() {
        for (name, folder) in [
            ("movie.MKV", Some("Video")),
            ("song.flac", Some("Music")),
            ("photo.HEIC", Some("Pictures")),
            ("report.pdf", Some("Documents")),
            ("backup.tar.gz", Some("Compressed")),
            ("ubuntu.iso", Some("Disk Images")),
            ("setup.exe", Some("Programs")),
            ("Fuselane.AppImage", Some("Programs")),
            ("README", None),
            ("data.bin", None),
            (".bashrc", None),
        ] {
            assert_eq!(category(name), folder, "{name}");
        }
        assert_eq!(clock(65), "01:05");
        assert_eq!(clock(1439), "23:59");
    }
}
