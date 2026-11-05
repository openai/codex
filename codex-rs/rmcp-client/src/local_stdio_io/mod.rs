//! Platform-specific pipe ownership beneath the shared MCP framing layer.

use std::io;

use codex_utils_pty::Child;
use codex_utils_pty::Command;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub(super) type Stdin = tokio::net::windows::named_pipe::NamedPipeServer;
#[cfg(windows)]
pub(super) type Stdout = tokio::net::windows::named_pipe::NamedPipeServer;
#[cfg(windows)]
pub(super) type Stderr = tokio::net::windows::named_pipe::NamedPipeServer;

#[cfg(not(windows))]
pub(super) type Stdin = tokio::process::ChildStdin;
#[cfg(not(windows))]
pub(super) type Stdout = tokio::process::ChildStdout;
#[cfg(not(windows))]
pub(super) type Stderr = tokio::process::ChildStderr;

pub(super) struct LocalStdio {
    pub(super) child: Child,
    pub(super) stdin: Stdin,
    pub(super) stdout: Stdout,
    pub(super) stderr: Option<Stderr>,
}

pub(super) fn spawn(command: Command) -> io::Result<LocalStdio> {
    #[cfg(windows)]
    {
        let mut command = command;
        let pipes = windows::Stdio::configure(&mut command)?;
        Ok(LocalStdio {
            child: command.spawn()?,
            stdin: pipes.stdin,
            stdout: pipes.stdout,
            stderr: Some(pipes.stderr),
        })
    }
    #[cfg(not(windows))]
    {
        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("MCP server stdin was not piped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("MCP server stdout was not piped"))?;
        let stderr = child.stderr.take();
        Ok(LocalStdio {
            child,
            stdin,
            stdout,
            stderr,
        })
    }
}
