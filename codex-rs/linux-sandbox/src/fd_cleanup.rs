//! File descriptor hygiene before entering the sandboxed command.

/// Close helper-inherited descriptors unless they are standard input/output/error
/// or already close-on-exec.
///
/// The sandboxed command can still create allowed local IPC after exec, but it
/// must not inherit an already-connected network socket from the launcher.
pub(crate) fn close_inherited_exec_fds() {
    let fds = match non_stdio_fds_from_proc() {
        Ok(fds) => fds,
        Err(err) => panic!("failed to enumerate inherited file descriptors: {err}"),
    };
    for fd in fds {
        close_fd_if_inheritable(fd);
    }
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

fn close_fd_if_inheritable(fd: libc::c_int) {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags == -1 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EBADF) {
            panic!("failed to inspect inherited file descriptor {fd}: {err}");
        }
        return;
    }
    if flags & libc::FD_CLOEXEC != 0 {
        return;
    }

    let result = unsafe { libc::close(fd) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() != Some(libc::EBADF) {
            panic!("failed to close inherited file descriptor {fd}: {err}");
        }
    }
}
