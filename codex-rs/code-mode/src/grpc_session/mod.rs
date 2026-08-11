use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::CodeModeSessionProviderFuture;
use codex_code_mode_protocol::CodeModeSessionResultFuture;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::grpc::code_mode_host_client::CodeModeHostClient;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tonic::transport::Channel;

use self::operations::WaitSlot;
use self::state::SessionState;
use self::transport::GrpcTransport;
use self::transport::SharedTransport;
use crate::remote_session::ShutdownResultReceiver;
use crate::remote_session::wait_for_watch;

mod callbacks;
mod completion;
mod conversion;
mod deadline;
mod operations;
mod state;
mod transport;

type GrpcClient = CodeModeHostClient<GrpcTransport>;

/// Creates code-mode sessions over an HTTP/2 gRPC connection.
#[derive(Clone)]
pub struct GrpcCodeModeSessionProvider {
    transport: Arc<SharedTransport>,
}

impl GrpcCodeModeSessionProvider {
    /// Connects lazily to an `http://` or `https://` gRPC endpoint.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self::with_http_client_factory(
            endpoint,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
    }

    /// Connects using the application's resolved outbound proxy and custom CA policy.
    pub fn with_http_client_factory(
        endpoint: impl Into<String>,
        http_client_factory: HttpClientFactory,
    ) -> Self {
        Self::from_transport(SharedTransport::new(endpoint.into(), http_client_factory))
    }

    /// Uses an existing channel, including channels backed by custom transports.
    pub fn with_channel(channel: Channel) -> Self {
        Self::from_transport(SharedTransport::with_channel(channel))
    }

    fn from_transport(transport: SharedTransport) -> Self {
        Self {
            transport: Arc::new(transport),
        }
    }

    async fn open_binding(
        &self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> Result<Arc<GrpcCodeModeSession>, String> {
        let mut client = deadline::startup("transport connection", self.transport.client()).await?;
        let limits = grpc::SessionCellExecutionLimits {
            max_yield_time_ms: limits.max_yield_time_ms,
            max_heap_size_bytes: limits
                .max_heap_size_bytes
                .map(u64::try_from)
                .transpose()
                .map_err(|error| format!("invalid code-mode heap size limit: {error}"))?,
        };
        let cell_execution_limits = (limits.max_yield_time_ms.is_some()
            || limits.max_heap_size_bytes.is_some())
        .then_some(limits);
        let mut lease = deadline::startup(
            "session opening",
            client.open_session(grpc::OpenSessionRequest {
                cell_execution_limits,
            }),
        )
        .await?
        .into_inner();
        let first = deadline::startup("session lease opening", lease.message())
            .await?
            .ok_or_else(|| "gRPC code-mode session lease ended before opening".to_string())?;
        let Some(grpc::session_event::Event::Opened(opened)) = first.event else {
            return Err("gRPC code-mode session lease omitted its opening event".to_string());
        };
        validate_identifier(&opened.session_id, "session ID")?;

        let inner = Arc::new(SessionInner {
            id: opened.session_id,
            client,
            delegate,
            runtime: tokio::runtime::Handle::current(),
            state: Mutex::new(SessionState::default()),
            wait_slots: Mutex::new(HashMap::new()),
            shutdown_requested: AtomicBool::new(false),
            shutdown_result: Mutex::new(None),
            stopped: CancellationToken::new(),
            stream_tasks: TaskTracker::new(),
            _transport: Arc::clone(&self.transport),
        });
        let mut opening = OpeningSession {
            inner: Some(Arc::clone(&inner)),
        };
        inner.spawn_session_events(lease);

        let request = grpc::SubscribeToToolCallsRequest {
            session_id: inner.id.clone(),
            tool_names: Vec::new(),
        };
        let mut client = inner.client();
        let response =
            match deadline::startup("tool subscription", client.subscribe_to_tool_calls(request))
                .await
            {
                Ok(response) => response,
                Err(error) => {
                    let _ = wait_for_watch(inner.request_shutdown()).await;
                    return Err(error);
                }
            };
        inner.spawn_tool_subscription(response.into_inner());
        inner.require_open()?;
        opening.inner = None;
        Ok(Arc::new(GrpcCodeModeSession { inner }))
    }
}

impl CodeModeSessionProvider for GrpcCodeModeSessionProvider {
    fn create_session<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a> {
        self.create_session_with_limits(delegate, CodeModeSessionCellExecutionLimits::default())
    }

    fn create_session_with_limits<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> CodeModeSessionProviderFuture<'a> {
        Box::pin(async move {
            self.open_binding(delegate, limits)
                .await
                .map(|session| session as _)
        })
    }
}

struct GrpcCodeModeSession {
    inner: Arc<SessionInner>,
}

struct OpeningSession {
    inner: Option<Arc<SessionInner>>,
}

impl Drop for OpeningSession {
    fn drop(&mut self) {
        let Some(inner) = self.inner.take() else {
            return;
        };
        if tokio::runtime::Handle::try_current().is_ok() {
            inner.request_shutdown();
        } else {
            inner.close_state(/*failure*/ None);
        }
    }
}

impl CodeModeSession for GrpcCodeModeSession {
    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
    ) -> CodeModeSessionResultFuture<'a, StartedCell> {
        Box::pin(self.inner.execute(request))
    }

    fn wait<'a>(&'a self, request: WaitRequest) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(self.inner.wait(request))
    }

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(self.inner.terminate(cell_id))
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(wait_for_watch(self.inner.request_shutdown()))
    }
}

impl Drop for GrpcCodeModeSession {
    fn drop(&mut self) {
        self.inner.request_shutdown();
    }
}

pub(super) struct SessionInner {
    pub(super) id: String,
    pub(super) client: GrpcClient,
    pub(super) delegate: Arc<dyn CodeModeSessionDelegate>,
    runtime: tokio::runtime::Handle,
    state: Mutex<SessionState>,
    wait_slots: Mutex<HashMap<CellId, Weak<WaitSlot>>>,
    shutdown_requested: AtomicBool,
    shutdown_result: Mutex<Option<ShutdownResultReceiver>>,
    pub(super) stopped: CancellationToken,
    stream_tasks: TaskTracker,
    _transport: Arc<SharedTransport>,
}

impl SessionInner {
    pub(super) fn client(&self) -> GrpcClient {
        self.client.clone()
    }

    pub(super) fn require_open(&self) -> Result<(), String> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .require_open()?;
        if self.shutdown_requested.load(Ordering::Acquire) {
            return Err("code mode session is shutting down".to_string());
        }
        Ok(())
    }

    pub(super) fn report_closed_cell(&self, cell_id: Option<CellId>) {
        if let Some(cell_id) = cell_id {
            self.wait_slots
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&cell_id);
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                self.delegate.cell_closed(&cell_id);
            }));
        }
    }

    pub(super) fn fail(&self, reason: String) {
        self.close_state(Some(reason));
    }

    fn close_state(&self, failure: Option<String>) {
        let cells = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .close(failure);
        self.stopped.cancel();
        self.stream_tasks.close();
        for cell_id in cells {
            self.report_closed_cell(Some(cell_id));
        }
    }

    fn request_shutdown(self: &Arc<Self>) -> ShutdownResultReceiver {
        self.shutdown_requested.store(true, Ordering::Release);
        let mut result = self
            .shutdown_result
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(receiver) = result.as_ref() {
            return receiver.clone();
        }
        let (sender, receiver) = watch::channel(None);
        *result = Some(receiver.clone());
        let inner = Arc::clone(self);
        self.runtime.spawn(async move {
            let result = inner.drive_shutdown().await;
            sender.send_replace(Some(result));
        });
        receiver
    }

    async fn drive_shutdown(&self) -> Result<(), String> {
        let is_open = self
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .require_open()
            .is_ok();
        let result = if is_open {
            let mut client = self.client();
            deadline::request(
                self,
                "session shutdown",
                Duration::ZERO,
                client.close_session(grpc::CloseSessionRequest {
                    session_id: self.id.clone(),
                }),
            )
            .await
            .map(|_| ())
        } else {
            Ok(())
        };
        self.close_state(/*failure*/ None);
        self.stream_tasks.wait().await;
        result
    }
}

fn validate_identifier(value: &str, field: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("gRPC code-mode host returned an empty {field}"));
    }
    if value.len() > grpc::MAX_IDENTIFIER_BYTES {
        return Err(format!(
            "gRPC code-mode host returned {field} exceeding {} bytes",
            grpc::MAX_IDENTIFIER_BYTES
        ));
    }
    Ok(())
}
