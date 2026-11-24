//! Open-file (socket) limits. A scan keeps hundreds of sockets open at once; macOS starts
//! apps from the Finder with a soft limit of only 256, which made probes fail and hosts
//! go missing. No administrator rights are needed for any of this.

use std::io;

/// `rlim_t` is not 64 bit on every Unix target.
#[cfg(unix)]
#[allow(clippy::unnecessary_cast)]
fn to_u64(v: libc::rlim_t) -> u64 {
    v as u64
}

/// Raise the soft limit for open files towards the hard limit (Unix). Returns the limit
/// in effect afterwards, or `None` where sockets are not limited this way (Windows).
pub fn raise_open_file_limit() -> Option<u64> {
    #[cfg(unix)]
    {
        // macOS refuses values above OPEN_MAX (10240) even when the hard limit is unlimited.
        const WANTED: u64 = 10240;
        let mut lim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        // SAFETY: getrlimit/setrlimit only read and write the passed struct.
        unsafe {
            if libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) != 0 {
                return None;
            }
            let target = WANTED.min(to_u64(lim.rlim_max));
            if to_u64(lim.rlim_cur) < target {
                let raised = libc::rlimit { rlim_cur: target as libc::rlim_t, rlim_max: lim.rlim_max };
                if libc::setrlimit(libc::RLIMIT_NOFILE, &raised) == 0 {
                    return Some(target);
                }
            }
        }
        Some(to_u64(lim.rlim_cur))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Current soft limit for open files, `None` where there is no such small limit (Windows).
pub fn open_file_limit() -> Option<u64> {
    #[cfg(unix)]
    {
        let mut lim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        // SAFETY: getrlimit only writes the passed struct.
        (unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) } == 0).then_some(to_u64(lim.rlim_cur))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// The OS ran out of sockets/file handles for the moment (too many open files, no buffer
/// space). Worth a short wait and retry: the host was not checked, it is not "dead".
pub fn is_exhausted(e: &io::Error) -> bool {
    #[cfg(unix)]
    let codes = [libc::EMFILE, libc::ENFILE, libc::ENOBUFS];
    // WSAEMFILE, WSAENOBUFS
    #[cfg(windows)]
    let codes = [10024, 10055];
    e.raw_os_error().is_some_and(|c| codes.contains(&c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhausted_codes() {
        #[cfg(unix)]
        assert!(is_exhausted(&io::Error::from_raw_os_error(libc::EMFILE)));
        #[cfg(windows)]
        assert!(is_exhausted(&io::Error::from_raw_os_error(10024)));
        assert!(!is_exhausted(&io::Error::from(io::ErrorKind::ConnectionRefused)));
    }

    #[cfg(unix)]
    #[test]
    fn raise_never_lowers() {
        let before = open_file_limit().unwrap();
        let after = raise_open_file_limit().unwrap();
        assert!(after >= before);
        assert_eq!(open_file_limit(), Some(after));
    }
}
