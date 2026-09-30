//! Own a capture process group until its output and exit status are validated.
//! Keep the leader unreaped while cleanup is armed, so its group ID cannot be
//! reused. Successful captures release startup helpers; escaped groups and
//! executor death are outside this guard's scope. This includes bubblewrap's
//! separate session when it inherits the host PID namespace.

use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use codex_utils_pty::process_group::kill_process_group;
use tokio::process::Child;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::signal::unix::Signal;
use tokio::signal::unix::SignalKind;
use tokio::signal::unix::signal;

pub(super) struct SnapshotCapture {
    child: Child,
    sigchld: Signal,
    cleanup: bool,
    pub(super) stdout: Option<ChildStdout>,
}

impl SnapshotCapture {
    pub(super) fn spawn(command: &mut Command) -> io::Result<Self> {
        // Subscribe before spawning so a fast exit cannot lose its notification.
        let sigchld = signal(SignalKind::child())?;
        // Group cleanup must not signal the executor or other captures.
        let mut child = command.process_group(/*pgroup*/ 0).spawn()?;
        let stdout = child.stdout.take();
        Ok(Self {
            child,
            sigchld,
            cleanup: true,
            stdout,
        })
    }

    pub(super) async fn wait_for_exit(&mut self) -> io::Result<ExitStatus> {
        let pid = self
            .child
            .id()
            .ok_or_else(|| io::Error::other("missing capture PID"))?;
        loop {
            {
                let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
                // SAFETY: we own this child and provide writable storage. WNOWAIT
                // reserves its PID until validation either releases or kills the group.
                if unsafe {
                    libc::waitid(
                        libc::P_PID,
                        pid,
                        info.as_mut_ptr(),
                        libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                    )
                } == -1
                {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    if error.raw_os_error() == Some(libc::ECHILD) {
                        self.cleanup = false;
                    }
                    return Err(error);
                }
                // SAFETY: successful waitid initialized the zeroed signal information.
                let info = unsafe { info.assume_init() };
                if unsafe { info.si_pid() } != 0 {
                    let status = unsafe { info.si_status() };
                    return Ok(ExitStatus::from_raw(if info.si_code == libc::CLD_EXITED {
                        status << 8
                    } else if info.si_code == libc::CLD_DUMPED {
                        status | 0x80
                    } else {
                        status
                    }));
                }
            }
            self.sigchld
                .recv()
                .await
                .ok_or_else(|| io::Error::other("SIGCHLD stream closed"))?;
        }
    }

    pub(super) fn preserve_helpers(&mut self) {
        self.cleanup = false;
    }
}

impl Drop for SnapshotCapture {
    fn drop(&mut self) {
        if self.cleanup
            && let Some(pid) = self.child.id()
        {
            let _ = kill_process_group(pid);
        }
        // Tokio reaps the child after group cleanup, including on cancellation.
    }
}
