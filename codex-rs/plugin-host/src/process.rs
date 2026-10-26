//! Owns the helper process group and preserves bounded diagnostics when it exits.

use futures::Sink;
use futures::Stream;
use futures::StreamExt;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio_util::bytes::Bytes;
use tokio_util::bytes::BytesMut;
use tokio_util::codec::FramedRead;
use tokio_util::codec::FramedWrite;
use tokio_util::codec::LengthDelimitedCodec;

use crate::HELPER_ARGUMENT;
use crate::READY;
use crate::framing;

const STDERR_TAIL_BYTES: usize = 16 * 1024;

/// Process diagnostics. Raw stderr is untrusted and deliberately omitted from Debug.
pub struct PluginExit {
    pub status: ExitStatus,
    pub group_termination_error: Option<io::Error>,
    /// At most the last 16 KiB, which may start mid-character or contain terminal escapes.
    pub stderr_tail: Vec<u8>,
    pub stderr_read_error: Option<io::ErrorKind>,
}

impl fmt::Debug for PluginExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginExit")
            .field("status", &self.status)
            .field("group_termination_error", &self.group_termination_error)
            .field("stderr_tail_bytes", &self.stderr_tail.len())
            .field("stderr_read_error", &self.stderr_read_error)
            .finish()
    }
}

/// A launch failure, or a readiness failure together with the outcome of closing the helper.
#[derive(Debug)]
pub enum PluginProcessError {
    Launch(io::Error),
    Handshake {
        source: io::Error,
        close_result: io::Result<Arc<PluginExit>>,
    },
}

impl From<io::Error> for PluginProcessError {
    fn from(error: io::Error) -> Self {
        Self::Launch(error)
    }
}

impl fmt::Display for PluginProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch(error) => fmt::Display::fmt(error, f),
            Self::Handshake {
                source,
                close_result: Ok(exit),
            } => {
                write!(f, "{source} ({})", exit.status)?;
                if let Some(error) = &exit.group_termination_error {
                    write!(f, "; process-group cleanup failed: {error}")?;
                }
                Ok(())
            }
            Self::Handshake {
                source,
                close_result: Err(error),
            } => write!(f, "{source}; process cleanup failed: {error}"),
        }
    }
}

impl std::error::Error for PluginProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Launch(source) | Self::Handshake { source, .. } => Some(source),
        }
    }
}

pub struct PluginProcess {
    child: Child,
    stderr: Option<tokio::task::JoinHandle<(Vec<u8>, Option<io::ErrorKind>)>>,
    exit: Option<Arc<PluginExit>>,
    #[cfg(windows)]
    job: codex_utils_pty::JobObject,
    input: FramedWrite<ChildStdin, LengthDelimitedCodec>,
    output: FramedRead<ChildStdout, LengthDelimitedCodec>,
}

impl PluginProcess {
    /// Borrow both pipes so callers can drive reads and writes concurrently.
    pub fn pipes(
        &mut self,
    ) -> (
        &mut (impl Sink<Bytes, Error = io::Error> + Unpin),
        &mut (impl Stream<Item = io::Result<BytesMut>> + Unpin),
    ) {
        (&mut self.input, &mut self.output)
    }

    /// Return a ready process, or reap it before returning a startup failure.
    /// The caller owns the startup deadline.
    /// Cancel through `stop` and await completion when teardown must finish before proceeding.
    pub async fn spawn(
        program: &Path,
        stop: &tokio_util::sync::CancellationToken,
    ) -> Result<Self, PluginProcessError> {
        if !program.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "plugin helper must be absolute",
            )
            .into());
        }
        let program = std::fs::canonicalize(program)?;
        let mut command = codex_utils_process::background_command(&program);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(/*pgroup*/ 0);
        }
        command
            .arg(HELPER_ARGUMENT)
            .env_clear()
            .current_dir(
                program
                    .parent()
                    .ok_or_else(|| io::Error::other("invalid helper path"))?,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut command = tokio::process::Command::from(command);
        command.kill_on_drop(/*kill_on_drop*/ true);
        let canceled = || io::Error::new(io::ErrorKind::Interrupted, "plugin startup canceled");
        if stop.is_cancelled() {
            return Err(canceled().into());
        }
        #[cfg(windows)]
        let (mut child, job) = {
            let job = codex_utils_pty::JobObject::create_without_breakaway()?;
            let child = job.spawn_contained(&mut command)?;
            (child, job)
        };
        #[cfg(not(windows))]
        let mut child = command.spawn()?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("missing helper stdin"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing helper stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("missing helper stderr"))?;
        // Drain from startup so a full diagnostic pipe cannot block the handshake or exit.
        let stderr = tokio::spawn(async move {
            let mut tail = Vec::with_capacity(STDERR_TAIL_BYTES);
            let mut buffer = [0; 4096];
            loop {
                match stderr.read(&mut buffer).await {
                    Ok(0) => return (tail, None),
                    Ok(length) => {
                        let overflow = (tail.len() + length).saturating_sub(STDERR_TAIL_BYTES);
                        tail.drain(..overflow);
                        tail.extend_from_slice(&buffer[..length]);
                    }
                    Err(error) => return (tail, Some(error.kind())),
                }
            }
        });
        let mut input = FramedWrite::new(input, framing::codec());
        // Keep only one accepted frame buffered when the child stops reading.
        input.set_backpressure_boundary(/*boundary*/ 1);
        let mut process = Self {
            child,
            stderr: Some(stderr),
            exit: None,
            #[cfg(windows)]
            job,
            input,
            output: FramedRead::new(output, framing::codec()),
        };
        let ready = async {
            let bytes = process
                .output
                .next()
                .await
                .ok_or(io::ErrorKind::UnexpectedEof)??;
            if bytes.as_ref() != READY {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid plugin handshake",
                ));
            }
            Ok(())
        };
        let result = tokio::select! {
            biased;
            _ = stop.cancelled() => Err(canceled()),
            result = ready => result,
        };
        if let Err(error) = result {
            let close_result = process.close().await;
            return Err(PluginProcessError::Handshake {
                source: error,
                close_result,
            });
        }
        Ok(process)
    }

    /// Terminate the group, reap the child, and collect its exit status and diagnostic tail.
    /// Group termination errors are retained in the report without losing the child's exit status.
    /// The helper must not pass its stderr pipe to subprocesses; a surviving writer could make
    /// diagnostic collection wait indefinitely after the helper exits.
    /// Repeated calls after completion return the same report. A canceled call can be retried,
    /// but the retry may lose a group-termination error from the canceled attempt.
    pub async fn close(&mut self) -> io::Result<Arc<PluginExit>> {
        if let Some(exit) = &self.exit {
            return Ok(exit.clone());
        }
        let terminated = self.terminate_group();
        let reaped = self.child.kill().await;
        reaped?;
        let status = self.child.wait().await?;
        // Borrow the task across await so canceling close does not detach the diagnostic owner.
        let stderr = self
            .stderr
            .as_mut()
            .ok_or_else(|| io::Error::other("missing stderr capture"))?;
        let (stderr_tail, stderr_read_error) = stderr
            .await
            .unwrap_or_else(|_| (Vec::new(), Some(io::ErrorKind::Other)));
        self.stderr.take();
        let exit = Arc::new(PluginExit {
            status,
            group_termination_error: terminated.err(),
            stderr_tail,
            stderr_read_error,
        });
        self.exit = Some(exit.clone());
        Ok(exit)
    }

    fn terminate_group(&self) -> io::Result<()> {
        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            codex_utils_pty::process_group::kill_process_group(pid)?;
        }
        #[cfg(windows)]
        self.job.terminate()?;
        Ok(())
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        let _ = self.terminate_group();
        if let Some(stderr) = &self.stderr {
            stderr.abort();
        }
    }
}
