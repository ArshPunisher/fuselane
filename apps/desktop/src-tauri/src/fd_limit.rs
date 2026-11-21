//! Room for open connections.
//!
//! A bonded download keeps several sockets per network, torrents keep many peer
//! sockets, and the extension, remote control, Nearby and streaming each listen
//! too. A Mac app started from Finder gets a soft limit of 256 open files; past
//! it, `accept` and `connect` fail with "too many open files". The soft limit is
//! raised once at start-up to the hard limit, capped at 10240 (macOS refuses more
//! than OPEN_MAX).

/// Raises this process's open-files soft limit. False if it couldn't be read
/// or changed (the app runs on regardless).
#[cfg(unix)]
pub fn raise() -> bool {
    const CAP: libc::rlim_t = 10_240;
    let mut r = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit writes into the rlimit we own and pass by pointer.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) } != 0 {
        return false;
    }
    let want = r.rlim_max.min(CAP);
    if r.rlim_cur >= want {
        return true;
    }
    let new = libc::rlimit {
        rlim_cur: want,
        rlim_max: r.rlim_max,
    };
    // SAFETY: setrlimit only reads the rlimit we pass; only the soft limit changes, and only upwards.
    unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &new) == 0 }
}

/// Windows has no such per-process limit for sockets.
#[cfg(not(unix))]
pub fn raise() -> bool {
    true
}

#[cfg(all(test, unix))]
mod tests {
    fn limit() -> libc::rlimit {
        let mut r = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) };
        r
    }

    #[test]
    fn the_soft_limit_is_raised_and_never_lowered() {
        let before = limit();
        assert!(super::raise());
        let after = limit();
        assert!(after.rlim_cur >= before.rlim_cur, "never lowered");
        assert!(
            after.rlim_cur >= before.rlim_max.min(10_240),
            "raised to the cap"
        );
    }
}
