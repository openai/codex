use std::ffi::OsString;
use std::io;
use std::ops::Deref;
use std::ops::DerefMut;
use std::panic::Location;
use std::process::ExitStatus;
use std::process::Output;

use ::tokio::process::Child;
use ::tokio::process::Command;

use crate::drop_bomb::DebugDropBomb;

/// Extends Tokio [`Command`] with Codex-owned child process spawning.
///
/// Implementations must return a child handle that requires callers to explicitly wait for or
/// terminate the spawned process.
pub trait TokioCommandExt {
    /// Spawn the command and return a managed direct-child handle.
    #[track_caller]
    fn spawn_managed(&mut self) -> io::Result<ManagedTokioChild>;
}

impl TokioCommandExt for Command {
    #[track_caller]
    fn spawn_managed(&mut self) -> io::Result<ManagedTokioChild> {
        let program = self.as_std().get_program().to_os_string();
        let spawn_location = Location::caller();
        // Prefer the explicit terminal methods below; this is only a fallback if ownership escapes.
        self.kill_on_drop(true);
        #[allow(
            clippy::disallowed_methods,
            reason = "ManagedTokioChild wraps the raw child handle here."
        )]
        self.spawn()
            .map(|child| ManagedTokioChild::new(child, program, spawn_location))
    }
}

/// A Tokio [`Child`] that requires explicit asynchronous teardown via [`Self::wait`],
/// [`Self::wait_with_output`], or [`Self::kill_and_wait`].
///
/// Violating this requirement panics in debug builds.
#[derive(Debug)]
pub struct ManagedTokioChild {
    // This only becomes `None` in consuming terminal methods such as `wait_with_output`.
    child: Option<Child>,
    drop_bomb: DebugDropBomb,
}

impl ManagedTokioChild {
    fn new(child: Child, program: OsString, spawn_location: &'static Location<'static>) -> Self {
        Self {
            child: Some(child),
            drop_bomb: DebugDropBomb::new(program, spawn_location),
        }
    }

    /// Wait for this child to exit.
    #[expect(clippy::expect_used)]
    pub async fn wait(mut self) -> io::Result<ExitStatus> {
        let result = self
            .child
            .as_mut()
            .expect("managed Tokio child is present until consumed")
            .wait()
            .await;
        self.drop_bomb.defuse();
        result
    }

    /// Wait for this child to exit and collect its captured output.
    #[expect(clippy::expect_used)]
    pub async fn wait_with_output(mut self) -> io::Result<Output> {
        let result = self
            .child
            .take()
            .expect("managed Tokio child is present until consumed")
            .wait_with_output()
            .await;
        self.drop_bomb.defuse();
        result
    }

    /// Kill this child and wait for it to exit.
    #[expect(clippy::expect_used)]
    pub async fn kill_and_wait(mut self) -> io::Result<()> {
        let result = self
            .child
            .as_mut()
            .expect("managed Tokio child is present until consumed")
            .kill()
            .await;
        self.drop_bomb.defuse();
        result
    }
}

impl Deref for ManagedTokioChild {
    type Target = Child;

    #[expect(clippy::expect_used)]
    fn deref(&self) -> &Self::Target {
        self.child
            .as_ref()
            .expect("managed Tokio child is present until consumed")
    }
}

impl DerefMut for ManagedTokioChild {
    #[expect(clippy::expect_used)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.child
            .as_mut()
            .expect("managed Tokio child is present until consumed")
    }
}

// FIXME: Expand these process tests to cover Windows.
#[cfg(all(test, unix))]
mod tests {
    use std::process::Stdio;

    use ::tokio::time::Duration;
    use ::tokio::time::sleep;

    use super::*;

    #[tokio::test]
    async fn waits_for_short_lived_managed_child() -> io::Result<()> {
        let status = short_lived_command().spawn_managed()?.wait().await?;

        assert!(status.success());
        Ok(())
    }

    #[tokio::test]
    async fn waits_for_managed_child_output() -> io::Result<()> {
        let output = output_command().spawn_managed()?.wait_with_output().await?;

        assert_eq!(output.stdout, b"managed\n");
        Ok(())
    }

    #[tokio::test]
    async fn kill_and_wait_terminates_direct_child() -> io::Result<()> {
        let child = long_lived_command().spawn_managed()?;
        let pid = child.id();

        child.kill_and_wait().await?;

        assert!(!process_exists(pid.expect("child should have a PID")));
        Ok(())
    }

    #[tokio::test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "dropped without explicit teardown")]
    async fn drop_without_teardown_panics_in_debug() {
        drop(long_lived_command().spawn_managed().expect("spawn child"));
    }

    #[tokio::test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "dropped without explicit teardown")]
    async fn cancelled_wait_panics_in_debug() {
        let child = long_lived_command().spawn_managed().expect("spawn child");
        let wait = child.wait();
        ::tokio::pin!(wait);
        ::tokio::select! {
            _ = sleep(Duration::from_millis(10)) => {}
            _ = &mut wait => panic!("long-lived child should not exit first"),
        }
    }

    fn short_lived_command() -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "exit 0"]);
        command
    }

    fn output_command() -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf 'managed\n'"]);
        command.stdout(Stdio::piped());
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
