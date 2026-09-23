//! Local child ownership and platform selection, independent of process transports.
//!
//! Every child exposes Tokio stdio handles. Reap-only children retain ownership until
//! reaped, including when a wait is cancelled or the async runtime shuts down.

use std::io;
use std::process::ExitStatus;

use tokio::io::AsyncReadExt;
use tokio::process::ChildStderr;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[path = "posix_child.rs"]
pub(super) mod posix;

#[cfg(unix)]
#[path = "child_reaper.rs"]
pub(super) mod reaper;

/// A local subprocess with owned stdio and cancellation-safe exit handling.
pub struct Child {
    pub(super) inner: ChildKind,
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
}

pub(super) enum ChildKind {
    Tokio(tokio::process::Child),
    #[cfg(unix)]
    TokioReapOnly {
        child: Option<tokio::process::Child>,
        reaper: std::sync::mpsc::Sender<reaper::ChildToReap>,
    },
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Native(posix::NativeChild),
}

impl Child {
    pub fn id(&self) -> Option<u32> {
        match &self.inner {
            ChildKind::Tokio(child) => child.id(),
            #[cfg(unix)]
            ChildKind::TokioReapOnly { child, .. } => {
                child.as_ref().and_then(tokio::process::Child::id)
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            ChildKind::Native(child) => child.id(),
        }
    }

    /// Close retained stdin and wait without giving up ownership on cancellation.
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.stdin.take();
        match &mut self.inner {
            ChildKind::Tokio(child) => child.wait().await,
            #[cfg(unix)]
            ChildKind::TokioReapOnly { child, .. } => {
                child
                    .as_mut()
                    .ok_or_else(|| io::Error::other("child already transferred to reaper"))?
                    .wait()
                    .await
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            ChildKind::Native(child) => child.wait().await,
        }
    }

    /// Drain both output pipes while waiting, retaining the configured drop policy on cancellation.
    pub async fn wait_with_output(mut self) -> io::Result<std::process::Output> {
        let mut stdout = self.stdout.take();
        let mut stderr = self.stderr.take();
        let mut output = Vec::new();
        let mut diagnostic = Vec::new();
        let (status, _, _) = tokio::try_join!(
            self.wait(),
            async {
                if let Some(stdout) = stdout.as_mut() {
                    stdout.read_to_end(&mut output).await?;
                }
                Ok::<_, io::Error>(())
            },
            async {
                if let Some(stderr) = stderr.as_mut() {
                    stderr.read_to_end(&mut diagnostic).await?;
                }
                Ok::<_, io::Error>(())
            },
        )?;
        // Keep the pipes open until the child exits, even after EOF, matching Tokio.
        // See https://github.com/tokio-rs/tokio/issues/4309.
        drop(stdout);
        drop(stderr);
        Ok(std::process::Output {
            status,
            stdout: output,
            stderr: diagnostic,
        })
    }

    /// Kill the direct child and reap it. Process-tree policy belongs to the caller.
    pub async fn kill(&mut self) -> io::Result<()> {
        self.stdin.take();
        match &mut self.inner {
            ChildKind::Tokio(child) => child.kill().await,
            #[cfg(unix)]
            ChildKind::TokioReapOnly { child, .. } => {
                child
                    .as_mut()
                    .ok_or_else(|| io::Error::other("child already transferred to reaper"))?
                    .kill()
                    .await
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            ChildKind::Native(child) => child.kill().await,
        }
    }
}

#[cfg(all(test, unix))]
#[path = "child_tests.rs"]
mod tests;

impl Drop for ChildKind {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Self::TokioReapOnly { child, reaper } = self
            && let Some(child) = child.take()
            && child.id().is_some()
        {
            // Transfer the handle, not only its PID: Tokio must not enqueue it
            // into a stopped runtime's orphan queue before our worker reaps it.
            let _ = reaper.send(reaper::ChildToReap::Tokio(child));
        }
    }
}
