//! Child process helpers that keep process lifetime ownership explicit.

use std::ffi::OsString;
use std::io;
use std::ops::Deref;
use std::ops::DerefMut;
use std::process::Child;
use std::process::Command;
use std::process::Output;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use tracing::trace;
use tracing::warn;

const DROP_WAIT_TIMEOUT: Duration = Duration::from_secs(1);
const DROP_WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Extends [`Command`] with Codex-owned child process spawning.
///
/// Implementations must return a child handle that makes ownership behavior explicit instead of
/// relying on [`Child`]'s no-op drop behavior.
pub trait CommandExt {
    /// Spawn the command and return a managed direct-child handle.
    fn spawn_managed(&mut self) -> io::Result<ManagedChild>;
}

impl CommandExt for Command {
    fn spawn_managed(&mut self) -> io::Result<ManagedChild> {
        let program = self.get_program().to_os_string();
        #[allow(
            clippy::disallowed_methods,
            reason = "ManagedChild wraps the raw child handle here."
        )]
        self.spawn().map(|child| ManagedChild {
            child: Some(child),
            program,
        })
    }
}

/// A [`Child`] that best-effort terminates and reaps its direct child process on drop.
///
/// This handle manages only the direct process represented by [`Child`]. Transitive children need
/// their own process-group or process-tree lifetime policy.
#[derive(Debug)]
pub struct ManagedChild {
    child: Option<Child>,
    program: OsString,
}

impl ManagedChild {
    /// Wait for this child to exit and collect its captured output.
    #[expect(clippy::expect_used)]
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        self.child
            .take()
            .expect("managed child is present until consumed")
            .wait_with_output()
    }
}

impl Deref for ManagedChild {
    type Target = Child;

    #[expect(clippy::expect_used)]
    fn deref(&self) -> &Self::Target {
        self.child
            .as_ref()
            .expect("managed child is present until consumed")
    }
}

impl DerefMut for ManagedChild {
    #[expect(clippy::expect_used)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.child
            .as_mut()
            .expect("managed child is present until consumed")
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let Some(child) = self.child.as_mut() else {
            // `wait_with_output` takes ownership of the child before this destructor runs.
            return;
        };
        let pid = child.id();
        if let Err(error) = child.kill() {
            warn!(
                pid,
                program = ?self.program,
                reason = %error,
                "failed to kill managed child process during drop"
            );
            return;
        }

        match wait_for_exit(child, DROP_WAIT_TIMEOUT) {
            Ok(true) => trace!(
                pid,
                program = ?self.program,
                "managed child process exited during drop"
            ),
            Ok(false) => warn!(
                pid,
                program = ?self.program,
                "timed out waiting for managed child process to exit during drop"
            ),
            Err(error) => warn!(
                pid,
                program = ?self.program,
                reason = %error,
                "failed to wait for managed child process during drop"
            ),
        }
    }
}

/// Wait up to `timeout` for a child to exit and be reaped.
///
/// Returns `Ok(false)` if the child is still running at the deadline.
fn wait_for_exit(child: &mut Child, timeout: Duration) -> io::Result<bool> {
    // `Child::try_wait` reaps a finished child but never blocks, so enforce the timeout here.
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait()?.is_some() {
            return Ok(true);
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(false);
        }
        thread::sleep(DROP_WAIT_POLL_INTERVAL.min(remaining));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::process::Stdio;

    use super::*;

    #[test]
    fn waits_for_short_lived_managed_child() -> io::Result<()> {
        let status = short_lived_command().spawn_managed()?.wait()?;

        assert!(status.success());
        Ok(())
    }

    #[test]
    fn drop_terminates_direct_child() -> io::Result<()> {
        let child = long_lived_command().spawn_managed()?;
        let pid = child.id();

        drop(child);

        assert!(!process_exists(pid));
        Ok(())
    }

    #[test]
    fn wait_timeout_path_returns_without_hanging() -> io::Result<()> {
        let mut child = long_lived_command().spawn_managed()?;
        let exited = wait_for_exit(&mut child, Duration::ZERO)?;

        assert!(!exited);
        Ok(())
    }

    fn short_lived_command() -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 0"]);
        command
    }

    fn long_lived_command() -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30"]);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
        command
    }

    fn process_exists(pid: u32) -> bool {
        // SAFETY: `kill` with signal 0 performs existence/permission checks only.
        unsafe { libc::kill(pid.cast_signed(), 0) == 0 }
    }
}
