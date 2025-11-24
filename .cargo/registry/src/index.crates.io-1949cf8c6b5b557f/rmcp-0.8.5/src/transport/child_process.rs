use std::process::Stdio;

use futures::future::Future;
use process_wrap::tokio::{TokioChildWrapper, TokioCommandWrap};
use tokio::{
    io::AsyncRead,
    process::{ChildStderr, ChildStdin, ChildStdout},
};

use super::{RxJsonRpcMessage, Transport, TxJsonRpcMessage, async_rw::AsyncRwTransport};
use crate::RoleClient;

const MAX_WAIT_ON_DROP_SECS: u64 = 3;
/// The parts of a child process.
type ChildProcessParts = (
    Box<dyn TokioChildWrapper>,
    ChildStdout,
    ChildStdin,
    Option<ChildStderr>,
);

/// Extract the stdio handles from a spawned child.
/// Returns `(child, stdout, stdin, stderr)` where `stderr` is `Some` only
/// if the process was spawned with `Stdio::piped()`.
#[inline]
fn child_process(mut child: Box<dyn TokioChildWrapper>) -> std::io::Result<ChildProcessParts> {
    let child_stdin = match child.inner_mut().stdin().take() {
        Some(stdin) => stdin,
        None => return Err(std::io::Error::other("stdin was already taken")),
    };
    let child_stdout = match child.inner_mut().stdout().take() {
        Some(stdout) => stdout,
        None => return Err(std::io::Error::other("stdout was already taken")),
    };
    let child_stderr = child.inner_mut().stderr().take();
    Ok((child, child_stdout, child_stdin, child_stderr))
}

pub struct TokioChildProcess {
    child: ChildWithCleanup,
    transport: AsyncRwTransport<RoleClient, ChildStdout, ChildStdin>,
}

pub struct ChildWithCleanup {
    inner: Option<Box<dyn TokioChildWrapper>>,
}

impl Drop for ChildWithCleanup {
    fn drop(&mut self) {
        // We should not use start_kill(), instead we should use kill() to avoid zombies
        if let Some(mut inner) = self.inner.take() {
            // We don't care about the result, just try to kill it
            tokio::spawn(async move {
                if let Err(e) = Box::into_pin(inner.kill()).await {
                    tracing::warn!("Error killing child process: {}", e);
                }
            });
        }
    }
}

// we hold the child process with stdout, for it's easier to implement AsyncRead
pin_project_lite::pin_project! {
    pub struct TokioChildProcessOut {
        child: ChildWithCleanup,
        #[pin]
        child_stdout: ChildStdout,
    }
}

impl TokioChildProcessOut {
    /// Get the process ID of the child process.
    pub fn id(&self) -> Option<u32> {
        self.child.inner.as_ref()?.id()
    }
}

impl AsyncRead for TokioChildProcessOut {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        self.project().child_stdout.poll_read(cx, buf)
    }
}

impl TokioChildProcess {
    /// Convenience: spawn with default `piped` stdio
    pub fn new(command: impl Into<TokioCommandWrap>) -> std::io::Result<Self> {
        let (proc, _ignored) = TokioChildProcessBuilder::new(command).spawn()?;
        Ok(proc)
    }

    /// Builder entry-point allowing fine-grained stdio control.
    pub fn builder(command: impl Into<TokioCommandWrap>) -> TokioChildProcessBuilder {
        TokioChildProcessBuilder::new(command)
    }

    /// Get the process ID of the child process.
    pub fn id(&self) -> Option<u32> {
        self.child.inner.as_ref()?.id()
    }

    /// Gracefully shutdown the child process
    ///
    /// This will first close the transport to the child process (the server),
    /// and wait for the child process to exit normally with a timeout.
    /// If the child process doesn't exit within the timeout, it will be killed.
    pub async fn graceful_shutdown(&mut self) -> std::io::Result<()> {
        if let Some(mut child) = self.child.inner.take() {
            self.transport.close().await?;

            let wait_fut = Box::into_pin(child.wait());
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(MAX_WAIT_ON_DROP_SECS)) => {
                    if let Err(e) = Box::into_pin(child.kill()).await {
                        tracing::warn!("Error killing child: {e}");
                        return Err(e);
                    }
                },
                res = wait_fut => {
                    match res {
                        Ok(status) => {
                            tracing::info!("Child exited gracefully {}", status);
                        }
                        Err(e) => {
                            tracing::warn!("Error waiting for child: {e}");
                            return Err(e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Take ownership of the inner child process
    pub fn into_inner(mut self) -> Option<Box<dyn TokioChildWrapper>> {
        self.child.inner.take()
    }

    /// Split this helper into a reader (stdout) and writer (stdin).
    #[deprecated(
        since = "0.5.0",
        note = "use the Transport trait implementation instead"
    )]
    pub fn split(self) -> (TokioChildProcessOut, ChildStdin) {
        unimplemented!("This method is deprecated, use the Transport trait implementation instead");
    }
}

/// Builder for `TokioChildProcess` allowing custom `Stdio` configuration.
pub struct TokioChildProcessBuilder {
    cmd: TokioCommandWrap,
    stdin: Stdio,
    stdout: Stdio,
    stderr: Stdio,
}

impl TokioChildProcessBuilder {
    fn new(cmd: impl Into<TokioCommandWrap>) -> Self {
        Self {
            cmd: cmd.into(),
            stdin: Stdio::piped(),
            stdout: Stdio::piped(),
            stderr: Stdio::inherit(),
        }
    }

    /// Override the child stdin configuration.
    pub fn stdin(mut self, io: impl Into<Stdio>) -> Self {
        self.stdin = io.into();
        self
    }
    /// Override the child stdout configuration.
    pub fn stdout(mut self, io: impl Into<Stdio>) -> Self {
        self.stdout = io.into();
        self
    }
    /// Override the child stderr configuration.
    pub fn stderr(mut self, io: impl Into<Stdio>) -> Self {
        self.stderr = io.into();
        self
    }

    /// Spawn the child process. Returns the transport plus an optional captured stderr handle.
    pub fn spawn(mut self) -> std::io::Result<(TokioChildProcess, Option<ChildStderr>)> {
        self.cmd
            .command_mut()
            .stdin(self.stdin)
            .stdout(self.stdout)
            .stderr(self.stderr);

        let (child, stdout, stdin, stderr_opt) = child_process(self.cmd.spawn()?)?;

        let transport = AsyncRwTransport::new(stdout, stdin);
        let proc = TokioChildProcess {
            child: ChildWithCleanup { inner: Some(child) },
            transport,
        };
        Ok((proc, stderr_opt))
    }
}

impl Transport<RoleClient> for TokioChildProcess {
    type Error = std::io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        self.transport.send(item)
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        self.transport.receive()
    }

    fn close(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.graceful_shutdown()
    }
}

pub trait ConfigureCommandExt {
    fn configure(self, f: impl FnOnce(&mut Self)) -> Self;
}

impl ConfigureCommandExt for tokio::process::Command {
    fn configure(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use tokio::process::Command;

    use super::*;

    #[tokio::test]
    async fn test_tokio_child_process_drop() {
        let r = TokioChildProcess::new(Command::new("sleep").configure(|cmd| {
            cmd.arg("30");
        }));
        assert!(r.is_ok());
        let child_process = r.unwrap();
        let id = child_process.id();
        assert!(id.is_some());
        let id = id.unwrap();
        // Drop the child process
        drop(child_process);
        // Wait a moment to allow the cleanup task to run
        tokio::time::sleep(std::time::Duration::from_secs(MAX_WAIT_ON_DROP_SECS + 1)).await;
        // Check if the process is still running
        let status = Command::new("ps")
            .arg("-p")
            .arg(id.to_string())
            .status()
            .await;
        match status {
            Ok(status) => {
                assert!(
                    !status.success(),
                    "Process with PID {} is still running",
                    id
                );
            }
            Err(e) => {
                panic!("Failed to check process status: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_tokio_child_process_graceful_shutdown() {
        let r = TokioChildProcess::new(Command::new("sleep").configure(|cmd| {
            cmd.arg("30");
        }));
        assert!(r.is_ok());
        let mut child_process = r.unwrap();
        let id = child_process.id();
        assert!(id.is_some());
        let id = id.unwrap();
        child_process.graceful_shutdown().await.unwrap();
        // Wait a moment to allow the cleanup task to run
        tokio::time::sleep(std::time::Duration::from_secs(MAX_WAIT_ON_DROP_SECS + 1)).await;
        // Check if the process is still running
        let status = Command::new("ps")
            .arg("-p")
            .arg(id.to_string())
            .status()
            .await;
        match status {
            Ok(status) => {
                assert!(
                    !status.success(),
                    "Process with PID {} is still running",
                    id
                );
            }
            Err(e) => {
                panic!("Failed to check process status: {}", e);
            }
        }
    }
}
