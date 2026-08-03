use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) const PROXY_SOCKET_DIR_PREFIX: &str = "codex-linux-sandbox-proxy-";

pub(crate) fn cleanup_stale_proxy_socket_dirs_in(temp_dir: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(temp_dir)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(owner_pid) = parse_proxy_socket_dir_owner_pid(file_name.as_ref()) else {
            continue;
        };
        if is_pid_alive(owner_pid) {
            continue;
        }

        let _ = cleanup_proxy_socket_dir(entry.path().as_path());
    }

    Ok(())
}

fn parse_proxy_socket_dir_owner_pid(file_name: &str) -> Option<u32> {
    let suffix = file_name.strip_prefix(PROXY_SOCKET_DIR_PREFIX)?;
    let (pid_raw, _) = suffix.split_once('-')?;
    pid_raw.parse::<u32>().ok().filter(|pid| *pid != 0)
}

fn is_pid_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    is_pid_alive_raw(pid)
}

fn is_pid_alive_raw(pid: libc::pid_t) -> bool {
    is_pid_alive_in_proc_root(pid, Path::new("/proc"))
}

fn is_pid_alive_in_proc_root(pid: libc::pid_t, proc_root: &Path) -> bool {
    let status = unsafe { libc::kill(pid, 0) };
    if status != 0 {
        let err = io::Error::last_os_error();
        return !matches!(err.raw_os_error(), Some(libc::ESRCH));
    }

    match std::fs::read_to_string(proc_root.join(format!("{pid}/stat"))) {
        Ok(status) => !matches!(
            status
                .rsplit_once(") ")
                .and_then(|(_, fields)| fields.chars().next()),
            Some('Z')
        ),
        Err(err) if err.kind() == io::ErrorKind::NotFound => !proc_root.is_dir(),
        Err(_) => true,
    }
}

pub(crate) fn spawn_proxy_socket_dir_cleanup_worker(
    socket_dir: PathBuf,
    host_bridge_pids: Vec<libc::pid_t>,
) -> io::Result<()> {
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }

    if pid == 0 {
        if detach_bridge_stdio().is_err() {
            unsafe { libc::_exit(1) };
        }

        loop {
            if host_bridge_pids
                .iter()
                .copied()
                .all(|bridge_pid| !is_pid_alive_raw(bridge_pid))
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        let _ = cleanup_proxy_socket_dir(socket_dir.as_path());
        unsafe { libc::_exit(0) };
    }

    Ok(())
}

fn cleanup_proxy_socket_dir(socket_dir: &Path) -> io::Result<()> {
    for _ in 0..20 {
        match std::fs::remove_dir_all(socket_dir) {
            Ok(()) => return Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }

    match std::fs::remove_dir_all(socket_dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

pub(crate) fn harden_bridge_process() -> io::Result<()> {
    detach_bridge_stdio()?;
    set_parent_death_signal()?;
    codex_process_hardening::disable_process_dumping()
}

fn set_parent_death_signal() -> io::Result<()> {
    let res = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) };
    if res != 0 {
        Err(io::Error::last_os_error())
    } else if unsafe { libc::getppid() } == 1 {
        Err(io::Error::other("parent process already exited"))
    } else {
        Ok(())
    }
}

fn detach_bridge_stdio() -> io::Result<()> {
    let null_read_fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_RDONLY) };
    if null_read_fd < 0 {
        let err = io::Error::last_os_error();
        if unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_GETFD) } < 0 {
            return Err(err);
        }
        return redirect_bridge_output(libc::STDIN_FILENO);
    }

    let null_write_fd = unsafe { libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY) };
    if null_write_fd < 0 {
        if unsafe { libc::dup2(null_read_fd, libc::STDIN_FILENO) } < 0 {
            let err = io::Error::last_os_error();
            if null_read_fd > libc::STDERR_FILENO {
                let _ = close_fd(null_read_fd);
            }
            return Err(err);
        }
        let result = redirect_bridge_output(null_read_fd);
        if null_read_fd > libc::STDERR_FILENO {
            let _ = close_fd(null_read_fd);
        }
        return result;
    }

    for (source_fd, stream_fd) in [
        (null_read_fd, libc::STDIN_FILENO),
        (null_write_fd, libc::STDOUT_FILENO),
        (null_write_fd, libc::STDERR_FILENO),
    ] {
        if unsafe { libc::dup2(source_fd, stream_fd) } < 0 {
            let err = io::Error::last_os_error();
            if null_read_fd > libc::STDERR_FILENO {
                let _ = close_fd(null_read_fd);
            }
            if null_write_fd > libc::STDERR_FILENO {
                let _ = close_fd(null_write_fd);
            }
            return Err(err);
        }
    }

    if null_read_fd > libc::STDERR_FILENO {
        close_fd(null_read_fd)?;
    }
    if null_write_fd > libc::STDERR_FILENO {
        close_fd(null_write_fd)?;
    }

    Ok(())
}

fn redirect_bridge_output(source_fd: libc::c_int) -> io::Result<()> {
    for stream_fd in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        if unsafe { libc::dup2(source_fd, stream_fd) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

pub(crate) fn create_ready_pipe() -> io::Result<(libc::c_int, libc::c_int)> {
    let mut pipe_fds = [0; 2];
    let res = unsafe { libc::pipe2(pipe_fds.as_mut_ptr(), libc::O_CLOEXEC) };
    if res != 0 {
        return Err(io::Error::last_os_error());
    }

    let read_fd = match move_fd_above_stdio(pipe_fds[0]) {
        Ok(fd) => fd,
        Err(err) => {
            let _ = close_fd(pipe_fds[0]);
            let _ = close_fd(pipe_fds[1]);
            return Err(err);
        }
    };
    let write_fd = match move_fd_above_stdio(pipe_fds[1]) {
        Ok(fd) => fd,
        Err(err) => {
            let _ = close_fd(read_fd);
            let _ = close_fd(pipe_fds[1]);
            return Err(err);
        }
    };

    Ok((read_fd, write_fd))
}

fn move_fd_above_stdio(fd: libc::c_int) -> io::Result<libc::c_int> {
    if fd > libc::STDERR_FILENO {
        return Ok(fd);
    }

    let relocated_fd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, libc::STDERR_FILENO + 1) };
    if relocated_fd < 0 {
        return Err(io::Error::last_os_error());
    }

    if let Err(err) = close_fd(fd) {
        let _ = close_fd(relocated_fd);
        return Err(err);
    }

    Ok(relocated_fd)
}

pub(crate) fn close_fd(fd: libc::c_int) -> io::Result<()> {
    let res = unsafe { libc::close(fd) };
    if res < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
#[path = "proxy_lifecycle_tests.rs"]
mod tests;
