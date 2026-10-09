//! Owns the local gRPC code-mode client and host process. Sessions share a lazy
//! HTTP/2 channel; reconnecting spawns a fresh host, and dropping its I/O kills and
//! reaps the child. Session recovery and stale cell IDs use the shared gRPC client.

use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::OnceLock;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use codex_code_mode::CodeModeSessionCellExecutionLimits;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::CodeModeSessionProviderFuture;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_code_mode::ProcessOwnedCodeModeSessionProvider;
use codex_protocol::shell_environment::scrub_non_inheritable_env_vars;
use codex_utils_process::background_command;
use hyper_util::rt::TokioIo;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::BufReader;
use tokio::io::ReadBuf;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tonic::transport::Endpoint;
use tower::service_fn;

pub(crate) struct ProcessOwnedGrpcCodeModeSessionProvider {
    host_program: PathBuf,
    provider: OnceLock<GrpcCodeModeSessionProvider>,
}

impl ProcessOwnedGrpcCodeModeSessionProvider {
    pub(crate) fn with_host_program(host_program: PathBuf) -> Self {
        Self {
            host_program,
            provider: OnceLock::new(),
        }
    }
}

impl CodeModeSessionProvider for ProcessOwnedGrpcCodeModeSessionProvider {
    fn availability(&self) -> Result<(), String> {
        ProcessOwnedCodeModeSessionProvider::with_host_program(self.host_program.clone())
            .availability()
    }

    fn create_session(&self) -> CodeModeSessionProviderFuture<'_> {
        self.create_session_with_limits(CodeModeSessionCellExecutionLimits::default())
    }

    fn create_session_with_limits<'a>(
        &'a self,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async move {
            self.availability()?;
            let provider = self.provider.get_or_init(|| {
                let host_program = self.host_program.clone();
                // The URI is an HTTP/2 authority only; the connector exclusively uses pipes.
                // A live but unresponsive child can leave both pipes open. Let HTTP/2
                // detect that and drop the I/O, which reaps the child on reconnect.
                let channel = Endpoint::from_static("http://code-mode-host")
                    .http2_keep_alive_interval(Duration::from_secs(/*secs*/ 30))
                    .keep_alive_timeout(Duration::from_secs(/*secs*/ 10))
                    .keep_alive_while_idle(/*enabled*/ true)
                    .connect_with_connector_lazy(service_fn(move |_| {
                        let host_program = host_program.clone();
                        async move { HostConnection::spawn(&host_program).map(TokioIo::new) }
                    }));
                GrpcCodeModeSessionProvider::with_channel(channel)
            });
            provider.create_session_with_limits(limits).await
        })
    }
}

struct HostConnection {
    io: tokio::io::Join<ChildStdout, ChildStdin>,
    child: Option<Child>,
}

impl HostConnection {
    fn spawn(host_program: &Path) -> io::Result<Self> {
        let mut command = Command::from(background_command(host_program));
        #[cfg(unix)]
        command.process_group(/*pgroup*/ 0);
        command
            .args(["--listen", "grpc+stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(/*kill_on_drop*/ true);
        scrub_non_inheritable_env_vars(command.as_std_mut());
        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("code-mode host has no stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("code-mode host has no stdout"))?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!("code-mode host stderr: {line}");
                }
            });
        }
        Ok(Self {
            io: tokio::io::join(stdout, stdin),
            child: Some(child),
        })
    }
}

impl AsyncRead for HostConnection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for HostConnection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

impl Drop for HostConnection {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = child.wait().await;
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "code_mode_host_tests.rs"]
mod tests;
