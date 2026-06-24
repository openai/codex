use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::fd::RawFd;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;

pub(super) const BWRAP_CLEANUP_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) struct SandboxInit {
    pidfd: OwnedFd,
}

impl SandboxInit {
    pub(super) fn from_bwrap_info(info: &str, launcher_pid: u32) -> Result<Self> {
        let child_pid: libc::pid_t = info
            .lines()
            .find_map(|line| line.trim().strip_prefix("\"child-pid\":"))
            .map(|value| value.trim().trim_end_matches(',').parse())
            .transpose()
            .context("parse bwrap child PID")?
            .context("bwrap child information omitted child-pid")?;
        anyhow::ensure!(
            child_pid > 0,
            "bwrap reported invalid child PID {child_pid}"
        );

        // SAFETY: pidfd_open does not dereference userspace pointers. The
        // returned descriptor is uniquely owned when the syscall succeeds.
        let raw_pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, child_pid, 0) };
        if raw_pidfd == -1 {
            return Err(io::Error::last_os_error()).context("open bwrap child pidfd");
        }
        // SAFETY: a successful pidfd_open returns a new owned descriptor.
        let pidfd = unsafe { OwnedFd::from_raw_fd(raw_pidfd as i32) };
        // Validate the numeric PID before retaining the pidfd. If setup failed
        // and Linux already reused the PID, it cannot still name the launcher's
        // direct child.
        let status = fs::read_to_string(format!("/proc/{child_pid}/status"))
            .context("read bwrap child status")?;
        let parent_pid: u32 = status
            .lines()
            .find_map(|line| line.strip_prefix("PPid:"))
            .map(str::trim)
            .context("bwrap child status omitted PPid")?
            .parse()
            .context("parse bwrap child parent PID")?;
        anyhow::ensure!(
            parent_pid == launcher_pid,
            "bwrap child PID {child_pid} belongs to parent {parent_pid}, not launcher {launcher_pid}"
        );
        Ok(Self { pidfd })
    }

    pub(super) fn start_kill(&self) -> io::Result<()> {
        // SAFETY: pidfd_send_signal only reads the supplied null siginfo
        // pointer, and pidfd remains owned by self for the syscall's duration.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.pidfd.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        Ok(())
    }

    pub(super) async fn wait(&self) -> io::Result<()> {
        let pidfd = self.pidfd.try_clone()?;
        let wait = tokio::task::spawn_blocking(move || {
            wait_for_pidfd_exit(pidfd.as_raw_fd(), BWRAP_CLEANUP_TIMEOUT)
        });
        match tokio::time::timeout(BWRAP_CLEANUP_TIMEOUT, wait).await {
            Ok(result) => result.map_err(io::Error::other)?,
            Err(_) => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out scheduling wait for bwrap PID namespace init",
            )),
        }
    }

    pub(super) fn wait_blocking(&self) -> io::Result<()> {
        wait_for_pidfd_exit(self.pidfd.as_raw_fd(), BWRAP_CLEANUP_TIMEOUT)
    }
}

fn wait_for_pidfd_exit(pidfd: RawFd, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    let mut poll_fd = libc::pollfd {
        fd: pidfd,
        events: libc::POLLIN,
        revents: 0,
    };
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for bwrap PID namespace init",
            ));
        }
        let timeout_millis = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
        poll_fd.revents = 0;
        // SAFETY: poll_fd points to one initialized pollfd for the duration of
        // the call.
        let result = unsafe {
            libc::poll(&mut poll_fd, /*nfds*/ 1, timeout_millis)
        };
        if result == 0 {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "timed out waiting for bwrap PID namespace init",
            ));
        }
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if poll_fd.revents & libc::POLLIN != 0 {
            return Ok(());
        }
        return Err(io::Error::other(format!(
            "unexpected pidfd poll events: {:#x}",
            poll_fd.revents
        )));
    }
}
