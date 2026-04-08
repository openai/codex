//! File descriptor hygiene before entering the sandboxed command.

/// Mark helper-inherited descriptors close-on-exec unless they are standard
/// input/output/error.
///
/// The sandboxed command can still create allowed local IPC after exec, but it
/// must not inherit an already-connected network socket from the launcher.
pub(crate) fn mark_inherited_fds_cloexec() {
    match set_cloexec_with_close_range() {
        Ok(()) => return,
        Err(err) if can_fallback_from_close_range(&err) => {}
        Err(err) => panic!("failed to mark inherited file descriptors close-on-exec: {err}"),
    }

    let fds = match non_stdio_fds_from_proc() {
        Ok(fds) => fds,
        Err(err) => panic!("failed to enumerate inherited file descriptors: {err}"),
    };
    for fd in fds {
        set_fd_cloexec_ignoring_badf(fd);
    }
}

fn set_cloexec_with_close_range() -> std::io::Result<()> {
    let result = unsafe {
        libc::syscall(
            libc::SYS_close_range,
            (libc::STDERR_FILENO + 1) as libc::c_uint,
            u32::MAX as libc::c_uint,
            libc::CLOSE_RANGE_CLOEXEC,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn can_fallback_from_close_range(err: &std::io::Error) -> bool {
    matches!(
        err.raw_os_error(),
        Some(code) if code == libc::ENOSYS || code == libc::EPERM || code == libc::EINVAL
    )
}

fn non_stdio_fds_from_proc() -> std::io::Result<Vec<libc::c_int>> {
    let mut fds = Vec::new();
    for entry in std::fs::read_dir("/proc/self/fd")? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        let Ok(fd) = name.parse::<libc::c_int>() else {
            continue;
        };
        if fd > libc::STDERR_FILENO {
            fds.push(fd);
        }
    }
    Ok(fds)
}

fn set_fd_cloexec_ignoring_badf(fd: libc::c_int) {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags == -1 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EBADF) {
            panic!("failed to inspect inherited file descriptor {fd}: {err}");
        }
        return;
    }

    let result = unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) };
    if result == -1 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EBADF) {
            panic!("failed to mark inherited file descriptor {fd} close-on-exec: {err}");
        }
    }
}
