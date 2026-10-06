//! Unix process setup specific to a managed app-server daemon.

use std::io;

const NOFILE_LIMIT: libc::rlim_t = 4096;

pub(crate) fn raise_nofile_limit() -> io::Result<()> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit initializes the stack-owned resource-limit structure.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) } == -1 {
        return Err(io::Error::last_os_error());
    }
    let target = limit.rlim_cur.max(limit.rlim_max.min(NOFILE_LIMIT));
    if target == limit.rlim_cur {
        return Ok(());
    }
    limit.rlim_cur = target;
    // SAFETY: the requested soft limit does not exceed the inherited hard limit.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
