//! File descriptor hygiene before entering the sandboxed command.

use std::io::ErrorKind;

const ESCALATE_SOCKET_ENV_VAR: &str = "CODEX_ESCALATE_SOCKET";

/// Close helper-inherited descriptors unless they are standard input/output/error,
/// already close-on-exec, or known helper IPC.
///
/// The sandboxed command can still create allowed local IPC after exec, but it
/// must not inherit an already-connected network socket from the launcher.
pub(crate) fn close_inherited_exec_fds() {
    let preserved_fd = inherited_fd_to_preserve();
    let fds = match non_stdio_fds_from_proc() {
        Ok(fds) => fds,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            mark_inherited_exec_fds_cloexec(preserved_fd);
            return;
        }
        Err(err) => panic!("failed to enumerate inherited file descriptors: {err}"),
    };
    for fd in fds {
        if Some(fd) == preserved_fd {
            continue;
        }
        close_fd_if_inheritable(fd);
    }
}

fn inherited_fd_to_preserve() -> Option<libc::c_int> {
    std::env::var(ESCALATE_SOCKET_ENV_VAR)
        .ok()
        .and_then(|fd| fd.parse::<libc::c_int>().ok())
        .filter(|fd| *fd > libc::STDERR_FILENO)
}

fn mark_inherited_exec_fds_cloexec(preserved_fd: Option<libc::c_int>) {
    let start = (libc::STDERR_FILENO + 1) as libc::c_uint;
    let Some(preserved_fd) = preserved_fd
        .and_then(|fd| u32::try_from(fd).ok())
        .filter(|fd| *fd >= start)
    else {
        mark_fd_range_cloexec(start, u32::MAX);
        return;
    };

    if preserved_fd > start {
        mark_fd_range_cloexec(start, preserved_fd - 1);
    }
    if preserved_fd < u32::MAX {
        mark_fd_range_cloexec(preserved_fd + 1, u32::MAX);
    }
}

fn mark_fd_range_cloexec(first: libc::c_uint, last: libc::c_uint) {
    let result = unsafe {
        libc::syscall(
            libc::SYS_close_range,
            first,
            last,
            libc::CLOSE_RANGE_CLOEXEC,
        )
    };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        panic!("failed to mark inherited file descriptors close-on-exec: {err}");
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
