//! Early executable dispatch for Linux pipe and PTY spawns. All child setup happens
//! after exec, before application initialization or runtime threads exist.
//! A report prefix precedes target exec; failures append errno before exit.

use crate::spawn_helper::MAX_ENV_BYTES;
use crate::spawn_helper::REPORT_PREFIX;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Complete setup in the fresh, single-threaded image, then replace it with the target.
pub(super) fn dispatch(mut args: impl Iterator<Item = std::ffi::OsString>) -> ! {
    let Some(control_fd) = args
        .next()
        .and_then(|arg| arg.to_str()?.parse::<i32>().ok())
        .filter(|fd| *fd > 2 && unsafe { libc::fcntl(*fd, libc::F_GETFD) } != -1)
    else {
        std::process::exit(127);
    };
    // The parent passes ownership of this descriptor through posix_spawn.
    let mut control = unsafe { File::from_raw_fd(control_fd) };
    let mut reported = false;
    let result = (|| -> io::Result<()> {
        let parent_pid = args
            .next()
            .and_then(|arg| arg.to_str()?.parse::<libc::pid_t>().ok())
            .filter(|pid| *pid > 0)
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
        let setup = args
            .next()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
        let setup = match setup.to_str() {
            Some("pipe") => crate::spawn_helper::Setup::Pipe,
            Some("pty") => crate::spawn_helper::Setup::Pty,
            _ => return Err(io::Error::from_raw_os_error(libc::EINVAL)),
        };
        let preserved = args
            .next()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
        let preserved = preserved
            .to_str()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
        let inherited_fds = preserved
            .split(',')
            .filter(|fd| !fd.is_empty())
            .map(str::parse::<i32>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
        let cwd = args
            .next()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
        let program = args
            .next()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
        let arg0 = args
            .next()
            .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
        let mut command = Command::new(program);
        command.current_dir(cwd).arg0(arg0).args(args).env_clear();
        if unsafe { libc::fcntl(control_fd, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            return Err(io::Error::last_os_error());
        }
        match setup {
            crate::spawn_helper::Setup::Pipe => {
                crate::process_group::detach_from_tty()?;
                crate::process_group::set_parent_death_signal(parent_pid)?;
            }
            crate::spawn_helper::Setup::Pty => crate::pty::configure_child_terminal()?,
        }
        // Allocation is safe in this fresh, single-threaded image. CLOEXEC
        // leaves the report socket open until the target actually execs.
        crate::pty::close_inherited_fds_except(&inherited_fds);
        #[cfg(test)]
        crate::spawn_helper_tests::pause_handshake("environment")?;
        let mut size = [0; 4];
        control.read_exact(&mut size)?;
        let size = u32::from_le_bytes(size) as usize;
        if size > MAX_ENV_BYTES {
            return Err(io::Error::from_raw_os_error(libc::E2BIG));
        }
        let mut environment = vec![0; size];
        control.read_exact(&mut environment)?;
        for entry in environment
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
        {
            let separator = entry
                .iter()
                .position(|byte| *byte == b'=')
                .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
            command.env(
                std::ffi::OsStr::from_bytes(&entry[..separator]),
                std::ffi::OsStr::from_bytes(&entry[separator + 1..]),
            );
        }
        #[cfg(test)]
        crate::spawn_helper_tests::pause_handshake("report")?;
        control.write_all(&[REPORT_PREFIX])?;
        reported = true;
        #[cfg(test)]
        crate::spawn_helper_tests::pause_handshake("exec")?;
        Err(command.exec())
    })();
    if !reported {
        let _ = control.write_all(&[REPORT_PREFIX]);
    }
    let errno = result
        .err()
        .and_then(|error| error.raw_os_error())
        .unwrap_or(libc::EIO);
    let _ = control.write_all(&errno.to_le_bytes());
    // No diagnostics: program arguments and environment may contain secrets.
    std::process::exit(127);
}
