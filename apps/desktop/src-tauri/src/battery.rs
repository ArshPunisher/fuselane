//! The computer's battery (B9.10): how full it is and whether it's charging,
//! read the OS's own way (`pmset` on macOS, `/sys/class/power_supply` on
//! Linux, `GetSystemPowerStatus` on Windows). A desktop without a battery
//! reads as None, and nothing changes for it.

/// The battery as Fuselane needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    /// Plugged in (charging or full).
    pub plugged_in: bool,
}

/// At or below this, on battery, counts as low.
pub const LOW_PERCENT: u8 = 20;

impl Battery {
    pub fn low(&self) -> bool {
        !self.plugged_in && self.percent <= LOW_PERCENT
    }
}

/// Reads the battery now; None without one (or when the OS won't say).
pub fn read() -> Option<Battery> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/bin/pmset")
            .args(["-g", "batt"])
            .output()
            .ok()?;
        parse_pmset(&String::from_utf8_lossy(&out.stdout))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = std::fs::read_dir("/sys/class/power_supply").ok()?;
        for e in dir.flatten() {
            let p = e.path();
            let kind = std::fs::read_to_string(p.join("type")).unwrap_or_default();
            if kind.trim() != "Battery" {
                continue;
            }
            let cap = std::fs::read_to_string(p.join("capacity")).unwrap_or_default();
            let status = std::fs::read_to_string(p.join("status")).unwrap_or_default();
            if let Some(b) = parse_sysfs(&cap, &status) {
                return Some(b);
            }
        }
        None
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        // SAFETY: GetSystemPowerStatus only writes into the struct we pass.
        let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
            return None;
        }
        parse_windows(s.ACLineStatus, s.BatteryFlag, s.BatteryLifePercent)
    }
}

/// `pmset -g batt`: "Now drawing from 'Battery Power'" then
/// " -InternalBattery-0 (id=…) 18%; discharging; 1:02 remaining present: true" (a tab before the percent).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn parse_pmset(text: &str) -> Option<Battery> {
    let line = text.lines().find(|l| l.contains("InternalBattery"))?;
    let percent = line
        .split(|c: char| c.is_whitespace() || c == ';')
        .find_map(|w| w.strip_suffix('%'))
        .and_then(|n| n.parse::<u8>().ok())?;
    let on_battery = text.contains("'Battery Power'") || line.contains("discharging");
    Some(Battery {
        percent: percent.min(100),
        plugged_in: !on_battery,
    })
}

/// Linux sysfs: `capacity` "18", `status` "Discharging" / "Charging" / "Full" / "Not charging".
#[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
pub fn parse_sysfs(capacity: &str, status: &str) -> Option<Battery> {
    let percent = capacity.trim().parse::<u8>().ok()?;
    Some(Battery {
        percent: percent.min(100),
        plugged_in: !status.trim().eq_ignore_ascii_case("discharging"),
    })
}

/// Windows SYSTEM_POWER_STATUS: AC line 1 = plugged in; battery flag 128 = no
/// battery, 255 = unknown; life 255 = unknown.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn parse_windows(ac_line: u8, flag: u8, life: u8) -> Option<Battery> {
    if flag == 128 || flag == 255 || life == 255 {
        return None;
    }
    Some(Battery {
        percent: life.min(100),
        plugged_in: ac_line == 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_battery_output_is_read() {
        let on_battery = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=4653155)\t18%; discharging; 1:02 remaining present: true\n";
        let b = parse_pmset(on_battery).unwrap();
        assert_eq!(
            b,
            Battery {
                percent: 18,
                plugged_in: false
            }
        );
        assert!(b.low());
        let charging = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=4653155)\t12%; charging; 2:10 remaining present: true\n";
        assert!(
            !parse_pmset(charging).unwrap().low(),
            "plugged in is never low"
        );
        assert_eq!(
            parse_pmset("Now drawing from 'AC Power'\n"),
            None,
            "a desktop Mac"
        );
    }

    #[test]
    fn linux_and_windows_batteries_are_read() {
        assert_eq!(
            parse_sysfs("19\n", "Discharging\n"),
            Some(Battery {
                percent: 19,
                plugged_in: false
            })
        );
        assert!(!parse_sysfs("19", "Charging").unwrap().low());
        assert!(!parse_sysfs("80", "Discharging").unwrap().low());
        assert_eq!(parse_sysfs("", "Full"), None);
        assert_eq!(
            parse_windows(0, 2, 15),
            Some(Battery {
                percent: 15,
                plugged_in: false
            })
        );
        assert_eq!(parse_windows(1, 128, 255), None, "no battery");
        assert_eq!(parse_windows(0, 255, 50), None, "unknown");
    }
}
