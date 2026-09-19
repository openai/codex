//! Local child ownership and platform selection, independent of process transports.
//!
//! Every child exposes Tokio stdio handles. Native children retain their PID until
//! reaped, including when a wait is cancelled or the async runtime shuts down.

use std::io;
use std::process::ExitStatus;

use tokio::process::ChildStderr;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;

#[cfg(target_os = "macos")]
#[path = "macos_child.rs"]
pub(super) mod macos;

/// A local subprocess with owned stdio and cancellation-safe exit handling.
pub struct Child {
    pub(super) inner: ChildKind,
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
}

pub(super) enum ChildKind {
    Tokio(tokio::process::Child),
    #[cfg(target_os = "macos")]
    Native(macos::NativeChild),
}

impl Child {
    pub fn id(&self) -> Option<u32> {
        match &self.inner {
            ChildKind::Tokio(child) => child.id(),
            #[cfg(target_os = "macos")]
            ChildKind::Native(child) => child.id(),
        }
    }

    /// Close retained stdin and wait without giving up ownership on cancellation.
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.stdin.take();
        match &mut self.inner {
            ChildKind::Tokio(child) => child.wait().await,
            #[cfg(target_os = "macos")]
            ChildKind::Native(child) => child.wait().await,
        }
    }

    /// Kill the direct child and reap it. Process-tree policy belongs to the caller.
    pub async fn kill(&mut self) -> io::Result<()> {
        self.stdin.take();
        match &mut self.inner {
            ChildKind::Tokio(child) => child.kill().await,
            #[cfg(target_os = "macos")]
            ChildKind::Native(child) => child.kill().await,
        }
    }
}
