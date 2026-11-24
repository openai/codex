//! Owns Core's local code-mode output rollout and gRPC host. Sessions share a lazy
//! HTTP/2 connection. Only session admission can replace a dead host; metrics
//! use the current generation and cannot reconnect. Once session and operation
//! streams drain, we briefly retain the host for reuse before dropping its I/O.

use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use codex_code_mode::CodeModeOutputTruncation;
use codex_code_mode::CodeModeProcessMetricsSample;
use codex_code_mode::CodeModeSessionCellExecutionLimits;
use codex_code_mode::CodeModeSessionOptions;
use codex_code_mode::CodeModeSessionProvider;
use codex_code_mode::CodeModeSessionProviderFuture;
use codex_code_mode::CodeModeSessionResultFuture;
use codex_code_mode::GrpcCallKind;
use codex_code_mode::GrpcCodeModeSessionProvider;
use codex_protocol::shell_environment::scrub_non_inheritable_env_vars;
use codex_utils_process::background_command;
use http_body_util::BodyExt;
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
use tokio::sync::oneshot;
use tokio_util::task::AbortOnDropHandle;
use tonic::Status;
use tonic::body::Body;
use tonic::transport::Channel;
use tonic::transport::Endpoint;
use tower::ServiceExt;
use tower::service_fn;

const HOST_IDLE_GRACE: Duration = Duration::from_secs(/*secs*/ 5);
const MAX_DISPLAYED_HOST_PROGRAM_BYTES: usize = 512;

/// Only wraps Core's built-in providers; injected providers own their session settings.
pub(crate) struct LocalServerOutputProvider {
    pub(crate) inner: Arc<dyn CodeModeSessionProvider>,
}

impl CodeModeSessionProvider for LocalServerOutputProvider {
    fn availability(&self) -> Result<(), String> {
        self.inner.availability()
    }

    fn sample_process_metrics(
        &self,
    ) -> CodeModeSessionResultFuture<'_, CodeModeProcessMetricsSample> {
        self.inner.sample_process_metrics()
    }

    fn create_session(&self) -> CodeModeSessionProviderFuture<'_> {
        self.create_session_with_limits(CodeModeSessionCellExecutionLimits::default())
    }

    fn default_session_options(&self) -> CodeModeSessionOptions {
        CodeModeSessionOptions {
            output_truncation: CodeModeOutputTruncation::Server,
            ..CodeModeSessionOptions::default()
        }
    }

    fn create_session_with_limits<'a>(
        &'a self,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> CodeModeSessionProviderFuture<'a> {
        self.inner
            .create_session_with_options(CodeModeSessionOptions {
                cell_execution_limits: limits,
                ..self.default_session_options()
            })
    }

    fn create_session_with_options<'a>(
        &'a self,
        options: CodeModeSessionOptions,
    ) -> CodeModeSessionProviderFuture<'a> {
        self.inner.create_session_with_options(options)
    }
}

pub struct ProcessOwnedGrpcCodeModeSessionProvider {
    host_program: PathBuf,
    provider: OnceLock<GrpcCodeModeSessionProvider>,
    current: Arc<Mutex<Option<HostChannel>>>,
}

impl ProcessOwnedGrpcCodeModeSessionProvider {
    pub fn with_host_program(host_program: PathBuf) -> Self {
        Self {
            host_program,
            provider: OnceLock::new(),
            current: Arc::new(Mutex::new(None)),
        }
    }
}

impl CodeModeSessionProvider for ProcessOwnedGrpcCodeModeSessionProvider {
    fn availability(&self) -> Result<(), String> {
        if self.host_program.is_file() {
            return Ok(());
        }
        // Spawn errors reach tool output. Keep the executable-bearing suffix of long paths.
        let path = self.host_program.to_string_lossy();
        let path = if path.len() > MAX_DISPLAYED_HOST_PROGRAM_BYTES {
            let suffix_bytes = MAX_DISPLAYED_HOST_PROGRAM_BYTES - "...".len();
            let start = path.ceil_char_boundary(path.len() - suffix_bytes);
            format!("...{}", &path[start..])
        } else {
            path.into_owned()
        };
        Err(format!(
            "failed to spawn code-mode host {path}: host executable was not found"
        ))
    }

    fn sample_process_metrics(
        &self,
    ) -> CodeModeSessionResultFuture<'_, CodeModeProcessMetricsSample> {
        // Absence is local lifecycle state, not a meaning assigned to any remote
        // gRPC error. A child dying after this observation produces a failed sample.
        if !self
            .current
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|host| host.alive.load(Ordering::Acquire))
        {
            return Box::pin(async { Ok(CodeModeProcessMetricsSample::NotRunning) });
        }
        match self.provider.get() {
            Some(provider) => provider.sample_process_metrics(),
            None => Box::pin(async { Ok(CodeModeProcessMetricsSample::NotRunning) }),
        }
    }

    fn create_session(&self) -> CodeModeSessionProviderFuture<'_> {
        self.create_session_with_options(CodeModeSessionOptions::default())
    }

    fn create_session_with_limits<'a>(
        &'a self,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> CodeModeSessionProviderFuture<'a> {
        self.create_session_with_options(CodeModeSessionOptions {
            cell_execution_limits: limits,
            ..CodeModeSessionOptions::default()
        })
    }

    fn create_session_with_options<'a>(
        &'a self,
        options: CodeModeSessionOptions,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async move {
            self.availability()?;
            let provider = self.provider.get_or_init(|| {
                let transport = Arc::new(HostTransport {
                    host_program: self.host_program.clone(),
                    current: self.current.clone(),
                });
                GrpcCodeModeSessionProvider::with_transport(service_fn(
                    move |request: http::Request<Body>| {
                        let transport = transport.clone();
                        async move { transport.select(request.extensions())?.send(request).await }
                    },
                ))
            });
            provider.create_session_with_options(options).await
        })
    }
}

// Every channel gets exactly one child. Tonic's internal reconnect loop cannot
// spawn a process, including if the child dies between a metrics liveness check
// and dispatch. Session admission selects a new generation during recovery.
struct HostTransport {
    host_program: PathBuf,
    current: Arc<Mutex<Option<HostChannel>>>,
}

struct HostChannel {
    channel: Channel,
    alive: Arc<AtomicBool>,
    activity: Weak<oneshot::Sender<()>>,
    retirement: Option<AbortOnDropHandle<()>>,
}

struct HostRpc {
    channel: Channel,
    activity: Option<Arc<oneshot::Sender<()>>>,
}

impl HostTransport {
    fn select(&self, extensions: &http::Extensions) -> Result<HostRpc, Status> {
        let kind = extensions.get::<GrpcCallKind>().copied();
        let mut current = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let host = match current
            .as_mut()
            .filter(|host| host.alive.load(Ordering::Acquire))
        {
            Some(host) => host,
            None if kind == Some(GrpcCallKind::Admission) => {
                current.insert(HostChannel::spawn(&self.host_program)?)
            }
            None => return Err(Status::unavailable("code-mode host is not running")),
        };
        let activity = match kind {
            Some(GrpcCallKind::Observation) => None,
            Some(GrpcCallKind::Admission) | None => {
                Some(host.acquire(Arc::downgrade(&self.current)))
            }
        };
        Ok(HostRpc {
            channel: host.channel.clone(),
            activity,
        })
    }
}

impl HostChannel {
    fn spawn(host_program: &Path) -> Result<Self, Status> {
        let connection = HostConnection::spawn(host_program)
            .map_err(|error| Status::unavailable(error.to_string()))?;
        let alive = connection.alive.clone();
        let connection = Arc::new(Mutex::new(Some(connection)));
        // Preserve execution liveness detection independently of metrics cadence.
        let channel = Endpoint::from_static("http://code-mode-host")
            .http2_keep_alive_interval(Duration::from_secs(/*secs*/ 30))
            .keep_alive_timeout(Duration::from_secs(/*secs*/ 10))
            .keep_alive_while_idle(/*enabled*/ true)
            .connect_with_connector_lazy(service_fn(move |_| {
                let connection = connection
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take();
                async move {
                    connection.map(TokioIo::new).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::NotConnected,
                            "code-mode host connection ended",
                        )
                    })
                }
            }));
        Ok(Self {
            channel,
            alive,
            activity: Weak::new(),
            retirement: None,
        })
    }

    /// Overlapping RPCs share a sender. Dropping the last sender starts the grace
    /// period. A new RPC during that grace gets a distinct sender under the same
    /// lock used by the timer, so an expiring timer can only retire its own host.
    fn acquire(&mut self, current: Weak<Mutex<Option<Self>>>) -> Arc<oneshot::Sender<()>> {
        if let Some(active) = self.activity.upgrade() {
            return active;
        }
        let (sender, idle) = oneshot::channel();
        let active = Arc::new(sender);
        self.activity = Arc::downgrade(&active);
        let activity = self.activity.clone();
        self.retirement = Some(AbortOnDropHandle::new(tokio::spawn(async move {
            let _ = idle.await;
            tokio::time::sleep(HOST_IDLE_GRACE).await;
            if let Some(current) = current.upgrade() {
                let mut current = current.lock().unwrap_or_else(PoisonError::into_inner);
                if current
                    .as_ref()
                    .is_some_and(|host| host.activity.ptr_eq(&activity))
                {
                    current.take();
                }
            }
        })));
        active
    }
}

impl HostRpc {
    async fn send(self, request: http::Request<Body>) -> Result<http::Response<Body>, Status> {
        let response = self.channel.oneshot(request).await.map_err(|error| {
            // Channel errors are transport failures, not host RPC statuses.
            // In particular a child exiting during handshake can surface as
            // BrokenPipe, which generic tonic conversion calls Unknown and
            // would prevent the existing bounded OpenSession retry policy.
            let mut status = Status::unavailable("code-mode host transport failed");
            status.set_source(Arc::new(error));
            status
        })?;
        // OpenSession's response body is the session lease. Keep each RPC
        // active from selection through the lifetime of its response stream.
        Ok(response.map(|body| {
            Body::new(body.map_frame(move |frame| {
                let _activity = &self.activity;
                frame
            }))
        }))
    }
}

struct HostConnection {
    io: tokio::io::Join<ChildStdout, ChildStdin>,
    child: Option<Child>,
    alive: Arc<AtomicBool>,
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
            alive: Arc::new(AtomicBool::new(true)),
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
        self.alive.store(false, Ordering::Release);
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
