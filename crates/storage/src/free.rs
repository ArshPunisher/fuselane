//! Free space on the disk holding a folder, checked before a download starts so
//! it fails with a clear message instead of halfway through (STEPS 2.20).

use std::path::Path;

/// Bytes available to this user on the volume containing `dir`.
#[cfg(unix)]
pub fn free_space(dir: &Path) -> std::io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path contains a NUL byte")
    })?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: valid NUL-terminated path and a writable statvfs struct.
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // f_bavail: blocks free to unprivileged users (not root's reserve).
    #[allow(clippy::unnecessary_cast)]
    Ok((s.f_bavail as u64).saturating_mul(s.f_frsize as u64))
}

/// Bytes available to this user on the volume containing `dir`.
#[cfg(windows)]
pub fn free_space(dir: &Path) -> std::io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = dir
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut available: u64 = 0;
    // SAFETY: valid NUL-terminated wide path; the out pointer is a live u64.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(available)
}

#[cfg(not(any(unix, windows)))]
pub fn free_space(_dir: &Path) -> std::io::Result<u64> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "free space unknown here",
    ))
}

/// Extra room kept free beyond the download itself.
pub const MARGIN: u64 = 32 * 1024 * 1024;

/// Whether `needed` more bytes fit, keeping `MARGIN` spare. Unknown free space is
/// treated as enough: a failed check must never block a download by itself.
pub fn fits(free: Option<u64>, needed: u64) -> bool {
    free.is_none_or(|f| f >= needed.saturating_add(MARGIN))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_temp_folder_has_some_free_space() {
        let dir = tempfile::tempdir().unwrap();
        let free = free_space(dir.path()).unwrap();
        assert!(free > 1024 * 1024, "{free}");
    }

    #[test]
    fn a_missing_folder_is_an_error_not_a_guess() {
        assert!(free_space(Path::new("/definitely/not/here/fuselane")).is_err());
    }

    #[test]
    fn fitting_keeps_a_margin_and_never_overflows() {
        assert!(fits(Some(MARGIN + 100), 100));
        assert!(!fits(Some(MARGIN + 99), 100));
        assert!(!fits(Some(u64::MAX - 1), u64::MAX));
        assert!(fits(None, u64::MAX), "unknown free space doesn't block");
    }
}
