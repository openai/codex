use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::host::Capability;
use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::FramedWriter;
use codex_code_mode_protocol::host::HandshakeRejectReason;
use codex_code_mode_protocol::host::HostHello;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::HostResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SESSION_RESOURCE_LIMITS_CAPABILITY;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::SupportedProtocolVersions;
use codex_code_mode_protocol::host::WireWaitOutcome;
use codex_code_mode_runtime::InProcessCodeModeSession;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::TryAcquireError;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use self::delegate::RemoteDelegate;
use self::peer::HostPeer;

pub use self::grpc::GrpcCodeModeHost;
pub use self::transport::DEFAULT_LISTEN_URL;

mod delegate;
mod grpc;
mod grpc_transport;
mod peer;
mod transport;

const MAX_IN_FLIGHT_REQUESTS: usize = 256;
const MAX_ACTIVE_CELLS: usize = 128;
const MAX_RECENT_REQUEST_IDS: usize = 4096;
const MAX_RECENT_SESSION_IDS: usize = 4096;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const OUTGOING_CHANNEL_CAPACITY: usize = 128;

enum NegotiatedConnection {
    Rejected,
    Accepted,
}

struct HostLimits {
    request_permits: Arc<Semaphore>,
    active_cell_permits: Arc<Semaphore>,
}

impl HostLimits {
    fn new() -> Self {
        Self {
            request_permits: Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS)),
            active_cell_permits: Arc::new(Semaphore::new(MAX_ACTIVE_CELLS)),
        }
    }

    fn request_permit(&self) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        Arc::clone(&self.request_permits).try_acquire_owned()
    }

    fn cell_permit(&self) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        Arc::clone(&self.active_cell_permits).try_acquire_owned()
    }
}

/// Runs the code-mode host on its configured stdio or gRPC transport.
#[tracing::instrument(
    name = "code_mode_host.run_main",
    level = "info",
    skip_all,
    fields(otel.name = "code_mode_host.run_main")
)]
pub async fn run_main(listen_url: &str) -> Result<()> {
    transport::run_transport(listen_url).await
}

/// Runs one code-mode host connection over the process standard streams.
pub async fn run_stdio() -> Result<()> {
    run(tokio::io::stdin(), tokio::io::stdout()).await
}

/// Runs one code-mode host connection over an ordered input/output pair.
async fn run<R, W>(reader: R, writer: W) -> Result<()>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let mut reader = FramedReader::new(reader);
    let mut writer = FramedWriter::new(writer);
    match negotiate(&mut reader, &mut writer).await? {
        NegotiatedConnection::Rejected => return Ok(()),
        NegotiatedConnection::Accepted => {}
    }
    let (outgoing_tx, outgoing_rx) = mpsc::channel::<EncodedFrame>(OUTGOING_CHANNEL_CAPACITY);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let state = Arc::new(HostState {
        sessions: Mutex::new(HashMap::new()),
        limits: Arc::new(HostLimits::new()),
        seen_session_ids: Mutex::new(SeenSessionIds::default()),
        requests: Mutex::new(RequestRegistry::default()),
        request_tasks: TaskTracker::new(),
        closing: AtomicBool::new(false),
        peer: Arc::clone(&peer),
    });
    let writer_disconnected = peer.disconnection_token();
    let writer_task =
        tokio::spawn(async move { drive_writer(writer, outgoing_rx, writer_disconnected).await });
    let writer_peer = Arc::clone(&peer);
    let writer_supervisor = tokio::spawn(async move {
        match writer_task.await {
            Ok(Ok(())) if !writer_peer.is_disconnected() => {
                writer_peer.fail("code-mode writer task exited unexpectedly".to_string());
            }
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                writer_peer.fail(format!("code-mode writer task failed: {err:#}"));
            }
            Err(err) => {
                writer_peer.fail(format!("code-mode writer task failed: {err}"));
            }
        }
    });

    let input_result = async {
        loop {
            let message = tokio::select! {
                biased;
                _ = peer.disconnected() => break,
                message = reader.read() => message.context("failed to read code-mode client message")?,
            };
            let Some(message) = message else {
                break;
            };
            match message {
                ClientToHost::ClientHello(_) => {
                    anyhow::bail!("received a second code-mode client hello");
                }
                ClientToHost::Request { id, request } => {
                    state.spawn_request(id, request)?;
                }
                ClientToHost::CancelRequest { id } => {
                    state.cancel_request(id);
                }
                ClientToHost::DelegateResponse { id, result } => {
                    peer.complete(id, result.into_result()).await;
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    peer.disconnect();
    if tokio::time::timeout(SHUTDOWN_TIMEOUT, state.disconnect())
        .await
        .is_err()
    {
        peer.fail("timed out shutting down code-mode host state".to_string());
    }
    drop(state);
    tokio::time::timeout(SHUTDOWN_TIMEOUT, writer_supervisor)
        .await
        .context("timed out supervising code-mode writer task")?
        .context("code-mode writer supervisor task failed")?;
    let failure = peer.failure();
    drop(peer);
    input_result?;
    if let Some(failure) = failure {
        anyhow::bail!(failure);
    }
    Ok(())
}

async fn drive_writer<W: AsyncWrite + Unpin>(
    mut writer: FramedWriter<W>,
    mut outgoing: mpsc::Receiver<EncodedFrame>,
    disconnected: CancellationToken,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = disconnected.cancelled() => return Ok(()),
            frame = outgoing.recv() => {
                let Some(frame) = frame else {
                    return Ok(());
                };
                tokio::select! {
                    _ = disconnected.cancelled() => return Ok(()),
                    result = writer.write_frame(&frame) => {
                        result.context("failed to write code-mode host message")?;
                    }
                }
            }
        }
    }
}

async fn negotiate<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    reader: &mut FramedReader<R>,
    writer: &mut FramedWriter<W>,
) -> Result<NegotiatedConnection> {
    let Some(first_message) = reader
        .read()
        .await
        .context("failed to read code-mode client hello")?
    else {
        return Ok(NegotiatedConnection::Rejected);
    };
    let ClientToHost::ClientHello(client_hello) = first_message else {
        writer
            .write(&HostToClient::HandshakeRejected {
                reason: HandshakeRejectReason::InvalidHello {
                    message: "first message must be connection/hello".to_string(),
                },
            })
            .await
            .context("failed to reject invalid code-mode client hello")?;
        return Ok(NegotiatedConnection::Rejected);
    };

    let supported_versions = SupportedProtocolVersions::try_new([ProtocolVersion::V1])?;
    if !client_hello
        .supported_versions()
        .contains(ProtocolVersion::V1)
    {
        writer
            .write(&HostToClient::HandshakeRejected {
                reason: HandshakeRejectReason::NoCompatibleVersion { supported_versions },
            })
            .await
            .context("failed to reject incompatible code-mode client")?;
        return Ok(NegotiatedConnection::Rejected);
    }

    let resource_limits_capability = Capability::new(SESSION_RESOURCE_LIMITS_CAPABILITY)?;
    let resource_limits_requested = client_hello
        .required_capabilities()
        .contains(&resource_limits_capability)
        || client_hello
            .optional_capabilities()
            .contains(&resource_limits_capability);
    let host_capabilities =
        CapabilitySet::try_new(resource_limits_requested.then_some(resource_limits_capability))?;
    if let Some(capability) = client_hello
        .required_capabilities()
        .iter()
        .find(|capability| !host_capabilities.contains(capability))
    {
        writer
            .write(&HostToClient::HandshakeRejected {
                reason: HandshakeRejectReason::MissingRequiredCapability {
                    capability: capability.clone(),
                },
            })
            .await
            .context("failed to reject unsupported code-mode capability")?;
        return Ok(NegotiatedConnection::Rejected);
    }

    let hello = HostHello::new(ProtocolVersion::V1, host_capabilities);
    writer
        .write(&HostToClient::HostHello(hello))
        .await
        .context("failed to write code-mode host hello")?;
    Ok(NegotiatedConnection::Accepted)
}

struct HostState {
    sessions: Mutex<HashMap<SessionId, Arc<InProcessCodeModeSession>>>,
    limits: Arc<HostLimits>,
    seen_session_ids: Mutex<SeenSessionIds>,
    requests: Mutex<RequestRegistry>,
    request_tasks: TaskTracker,
    closing: AtomicBool,
    peer: Arc<HostPeer>,
}

impl HostState {
    fn spawn_request(
        self: &Arc<Self>,
        request_id: RequestId,
        request: HostRequest,
    ) -> Result<(), anyhow::Error> {
        // Include host admission and scheduling before the runtime observes the request.
        let received_at = Instant::now();
        let request_kind = RequestKind::from(&request);
        let cancellation = self
            .requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .start(request_id, request_kind)?;
        let Ok(permit) = self.limits.request_permit() else {
            self.respond(
                request_id,
                Err("code-mode host has too many in-flight requests".to_string()),
            );
            self.finish_request(request_id);
            return Ok(());
        };
        let state = Arc::clone(self);
        let request_task = self.request_tasks.spawn(async move {
            let _permit = permit;
            state
                .handle_request(request_id, request, cancellation, received_at)
                .await;
            state.finish_request(request_id);
        });
        self.supervise_request_task(request_task);
        Ok(())
    }

    fn supervise_request_task(&self, task: tokio::task::JoinHandle<()>) {
        let peer = Arc::clone(&self.peer);
        tokio::spawn(async move {
            if let Err(err) = task.await {
                peer.fail(format!("code-mode request task failed: {err}"));
            }
        });
    }

    #[tracing::instrument(
        name = "code_mode_host.request",
        level = "info",
        skip_all,
        fields(
            otel.name = "code_mode_host.request",
            request.kind = RequestKind::from(&request).as_str(),
            request.id = ?request_id,
            call_id = tracing::field::Empty,
        )
    )]
    async fn handle_request(
        &self,
        request_id: RequestId,
        request: HostRequest,
        cancellation: CancellationToken,
        received_at: Instant,
    ) {
        if let HostRequest::Execute { request, .. } = &request {
            tracing::Span::current().record("call_id", request.tool_call_id.as_str());
        }
        if self.closing.load(Ordering::Acquire) {
            self.respond(
                request_id,
                Err("code-mode host is shutting down".to_string()),
            );
            return;
        }
        match request {
            HostRequest::OpenSession {
                session_id,
                cell_execution_limits,
            } => {
                let result = CodeModeSessionCellExecutionLimits::try_from(
                    cell_execution_limits.unwrap_or_default(),
                )
                .map_err(|error| format!("invalid code-mode session execution limits: {error}"))
                .and_then(|limits| self.open_session(session_id.clone(), limits))
                .map(|()| HostResponse::SessionReady { session_id });
                self.respond(request_id, result);
            }
            HostRequest::Execute {
                session_id,
                request,
            } => {
                if cancellation.is_cancelled() {
                    self.respond(request_id, Err("code-mode request cancelled".to_string()));
                    return;
                }
                let request = match request.try_into() {
                    Ok(request) => request,
                    Err(err) => {
                        self.respond(
                            request_id,
                            Err(format!("invalid code-mode execute request: {err}")),
                        );
                        return;
                    }
                };
                let session = match self.session(&session_id) {
                    Ok(session) => session,
                    Err(err) => {
                        self.respond(request_id, Err(err));
                        return;
                    }
                };
                let Ok(active_cell_permit) = self.limits.cell_permit() else {
                    self.respond(
                        request_id,
                        Err("code-mode host has too many active cells".to_string()),
                    );
                    return;
                };
                let delegate = Arc::new(RemoteDelegate::new(
                    session_id.clone(),
                    Arc::clone(&self.peer),
                ));
                let result = session.execute(request, delegate).await;
                match result {
                    Ok(started) => {
                        let cell_id = started.cell_id.clone();
                        self.respond(
                            request_id,
                            Ok(HostResponse::ExecutionStarted {
                                cell_id: cell_id.into(),
                            }),
                        );
                        let initial_response_sent = self.peer.start_cell(
                            session_id,
                            request_id,
                            started,
                            active_cell_permit,
                            received_at,
                        );
                        let _ = initial_response_sent.await;
                    }
                    Err(err) => self.respond(request_id, Err(err)),
                }
            }
            HostRequest::Wait {
                session_id,
                request,
            } => {
                let result = match self.session(&session_id) {
                    Ok(session) => {
                        tokio::select! {
                            biased;
                            _ = cancellation.cancelled() => {
                                Err("code-mode request cancelled".to_string())
                            }
                            result = session.wait(request.into()) => result.and_then(|outcome| {
                                let outcome = outcome.with_code_mode_host_duration(received_at.elapsed());
                                Ok(HostResponse::WaitCompleted {
                                    outcome: WireWaitOutcome::try_from(outcome)
                                        .map_err(|error| error.to_string())?,
                                })
                            }),
                        }
                    }
                    Err(err) => Err(err),
                };
                self.respond(request_id, result);
            }
            HostRequest::Terminate {
                session_id,
                cell_id,
            } => {
                let result = match self.session(&session_id) {
                    Ok(session) => session.terminate(cell_id.into()).await.and_then(|outcome| {
                        let outcome = outcome.with_code_mode_host_duration(received_at.elapsed());
                        Ok(HostResponse::WaitCompleted {
                            outcome: WireWaitOutcome::try_from(outcome)
                                .map_err(|error| error.to_string())?,
                        })
                    }),
                    Err(err) => Err(err),
                };
                self.respond(request_id, result);
            }
            HostRequest::ShutdownSession { session_id } => {
                let session = self
                    .sessions
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&session_id);
                let result = match session {
                    Some(session) => match session.shutdown().await {
                        Ok(()) => {
                            self.peer.wait_for_session_cells(&session_id).await;
                            Ok(HostResponse::SessionClosed { session_id })
                        }
                        Err(err) => Err(err),
                    },
                    None => Err(format!("unknown code-mode session {session_id}")),
                };
                self.respond(request_id, result);
            }
        }
    }

    fn open_session(
        &self,
        session_id: SessionId,
        cell_execution_limits: CodeModeSessionCellExecutionLimits,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        if sessions.contains_key(&session_id) {
            return Err(format!(
                "code-mode session ID `{session_id}` is already open"
            ));
        }
        if self.closing.load(Ordering::Acquire) {
            return Err("code-mode host is shutting down".to_string());
        }
        if !self
            .seen_session_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remember(session_id.clone())
        {
            return Err(format!("code-mode session ID `{session_id}` was reused"));
        }
        let peer = Arc::downgrade(&self.peer);
        let task_failure_handler = Arc::new(move |reason| {
            if let Some(peer) = peer.upgrade() {
                peer.fail(reason);
            }
        });
        sessions.insert(
            session_id,
            Arc::new(InProcessCodeModeSession::with_task_failure_handler(
                task_failure_handler,
                cell_execution_limits,
            )),
        );
        Ok(())
    }

    fn session(&self, session_id: &SessionId) -> Result<Arc<InProcessCodeModeSession>, String> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("unknown code-mode session {session_id}"))
    }

    fn respond(&self, id: RequestId, result: Result<HostResponse, String>) {
        self.peer.respond(id, result);
    }

    fn cancel_request(&self, request_id: RequestId) {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cancel(request_id);
    }

    fn finish_request(&self, request_id: RequestId) {
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .finish(request_id);
    }

    async fn disconnect(&self) {
        self.closing.store(true, Ordering::Release);
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cancel_all();
        self.request_tasks.close();
        self.request_tasks.wait().await;
        let sessions = self
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain()
            .map(|(_, session)| session)
            .collect::<Vec<_>>();
        for session in sessions {
            let _ = session.shutdown().await;
        }
    }
}

#[derive(Clone, Copy)]
enum RequestKind {
    OpenSession,
    Execute,
    Wait,
    Terminate,
    ShutdownSession,
}

impl RequestKind {
    fn from(request: &HostRequest) -> Self {
        match request {
            HostRequest::OpenSession { .. } => Self::OpenSession,
            HostRequest::Execute { .. } => Self::Execute,
            HostRequest::Wait { .. } => Self::Wait,
            HostRequest::Terminate { .. } => Self::Terminate,
            HostRequest::ShutdownSession { .. } => Self::ShutdownSession,
        }
    }

    fn is_cancellable(self) -> bool {
        matches!(self, Self::Execute | Self::Wait)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::OpenSession => "open_session",
            Self::Execute => "execute",
            Self::Wait => "wait",
            Self::Terminate => "terminate",
            Self::ShutdownSession => "shutdown_session",
        }
    }
}

struct ActiveRequest {
    kind: RequestKind,
    cancellation: CancellationToken,
}

#[derive(Default)]
struct RequestRegistry {
    active: HashMap<RequestId, ActiveRequest>,
    recent: HashSet<RequestId>,
    recent_order: VecDeque<RequestId>,
}

impl RequestRegistry {
    fn start(
        &mut self,
        request_id: RequestId,
        kind: RequestKind,
    ) -> Result<CancellationToken, anyhow::Error> {
        if self.active.contains_key(&request_id) || self.recent.contains(&request_id) {
            anyhow::bail!("duplicate code-mode request ID {request_id:?}");
        }
        let cancellation = CancellationToken::new();
        self.active.insert(
            request_id,
            ActiveRequest {
                kind,
                cancellation: cancellation.clone(),
            },
        );
        Ok(cancellation)
    }

    fn cancel(&mut self, request_id: RequestId) {
        if let Some(request) = self.active.get(&request_id)
            && request.kind.is_cancellable()
        {
            request.cancellation.cancel();
        }
    }

    fn finish(&mut self, request_id: RequestId) {
        if self.active.remove(&request_id).is_none() {
            return;
        }
        self.recent.insert(request_id);
        self.recent_order.push_back(request_id);
        while self.recent_order.len() > MAX_RECENT_REQUEST_IDS {
            if let Some(expired) = self.recent_order.pop_front() {
                self.recent.remove(&expired);
            }
        }
    }

    fn cancel_all(&self) {
        for request in self.active.values() {
            request.cancellation.cancel();
        }
    }
}

#[derive(Default)]
struct SeenSessionIds {
    ids: HashSet<SessionId>,
    order: VecDeque<SessionId>,
}

impl SeenSessionIds {
    fn remember(&mut self, session_id: SessionId) -> bool {
        if !self.ids.insert(session_id.clone()) {
            return false;
        }
        self.order.push_back(session_id);
        while self.order.len() > MAX_RECENT_SESSION_IDS {
            if let Some(expired) = self.order.pop_front() {
                self.ids.remove(&expired);
            }
        }
        true
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
