//! File descriptor hygiene before entering the sandboxed command.

/// Close helper-inherited descriptors that are not standard input/output/error.
///
/// The sandboxed command can still create allowed local IPC after exec, but it
/// must not inherit an already-connected network socket from the launcher.
pub(crate) fn close_inherited_fds() {
    match close_fds_with_close_range() {
        Ok(()) => return,
        Err(err) if can_fallback_from_close_range(&err) => {}
        Err(err) => panic!("failed to close inherited file descriptors: {err}"),
    }

    let fds = match non_stdio_fds_from_proc() {
        Ok(fds) => fds,
        Err(err) => panic!("failed to enumerate inherited file descriptors: {err}"),
    };
    for fd in fds {
        close_fd_ignoring_badf(fd);
    }
}

fn close_fds_with_close_range() -> std::io::Result<()> {
    let result = unsafe {
        libc::syscall(
            libc::SYS_close_range,
            (libc::STDERR_FILENO + 1) as libc::c_uint,
            u32::MAX as libc::c_uint,
            0 as libc::c_uint,
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
        Some(code) if code == libc::ENOSYS || code == libc::EPERM
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

fn close_fd_ignoring_badf(fd: libc::c_int) {
    let result = unsafe { libc::close(fd) };
    if result == 0 {
        return;
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() != Some(libc::EBADF) {
        panic!("failed to close inherited file descriptor {fd}: {err}");
    }
}
