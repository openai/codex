use std::collections::HashMap;
use std::future::Future;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::wire;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::warn;

use crate::CodeModeHostCommand;
use crate::convert;

const IPC_CHANNEL_CAPACITY: usize = 128;

#[derive(Clone, Debug)]
pub(super) enum RequestError {
    Host(wire::Error),
    Transport(String),
}

impl RequestError {
    pub(super) fn into_message(self) -> String {
        match self {
            Self::Host(error) => convert::wire_error_message(&error),
            Self::Transport(message) => message,
        }
    }
}

pub(super) struct Connection {
    state: Arc<ConnectionState>,
    cancellation: CancellationToken,
}

impl Connection {
    pub(super) async fn spawn(command: &CodeModeHostCommand) -> Result<Self, String> {
        let mut child = host_process(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| {
                format!(
                    "failed to spawn code-mode host {}: {err}",
                    command.program.display()
                )
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "spawned code-mode host has no stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "spawned code-mode host has no stdout".to_string())?;
        let stderr = child.stderr.take();
        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(IPC_CHANNEL_CAPACITY);
        let cancellation = CancellationToken::new();
        let state = Arc::new(ConnectionState::new(outgoing_tx));

        let writer_state = Arc::downgrade(&state);
        let writer_cancellation = cancellation.clone();
        spawn_critical_connection_task("writer", Arc::downgrade(&state), async move {
            let mut stdin = stdin;
            loop {
                tokio::select! {
                    _ = writer_cancellation.cancelled() => break,
                    message = outgoing_rx.recv() => {
                        let Some(message) = message else {
                            break;
                        };
                        if let Err(err) = wire::write_frame(&mut stdin, &message).await {
                            if let Some(state) = writer_state.upgrade() {
                                state
                                    .fail(format!(
                                        "failed to write code-mode host message: {err}"
                                    ))
                                    .await;
                            }
                            break;
                        }
                    }
                }
            }
        });

        let reader_cancellation = cancellation.clone();
        spawn_critical_connection_task(
            "reader",
            Arc::downgrade(&state),
            drive_reader(stdout, Arc::downgrade(&state), reader_cancellation),
        );

        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => debug!("code-mode host stderr: {line}"),
                        Ok(None) => break,
                        Err(err) => {
                            warn!("failed to read code-mode host stderr: {err}");
                            break;
                        }
                    }
                }
            });
        }

        let supervisor_state = Arc::downgrade(&state);
        let supervisor_cancellation = cancellation.clone();
        spawn_critical_connection_task("supervisor", Arc::downgrade(&state), async move {
            tokio::select! {
                result = child.wait() => {
                    let reason = match result {
                        Ok(status) => format!("code-mode host exited with status {status}"),
                        Err(err) => format!("failed waiting for code-mode host: {err}"),
                    };
                    if let Some(state) = supervisor_state.upgrade() {
                        state.fail(reason).await;
                    }
                }
                _ = supervisor_cancellation.cancelled() => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    if let Some(state) = supervisor_state.upgrade() {
                        state
                            .fail("code-mode host connection closed".to_string())
                            .await;
                    }
                }
            }
        });

        Ok(Self {
            state,
            cancellation,
        })
    }

    pub(super) fn is_alive(&self) -> bool {
        self.state.alive.load(Ordering::Acquire)
    }

    pub(super) async fn request(
        &self,
        request: wire::HostRequest,
    ) -> Result<wire::HostResponse, RequestError> {
        let id = self.state.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (response_tx, response_rx) = oneshot::channel();
        self.state.pending.lock().await.insert(id, response_tx);
        let mut pending_request = PendingRequest::new(Arc::clone(&self.state), id);
        if let Err(err) = self
            .state
            .send(wire::ClientMessage::Request { id, request })
            .await
        {
            pending_request.cleanup().await;
            return Err(RequestError::Transport(err));
        }
        pending_request.mark_request_sent();
        let response = response_rx
            .await
            .map_err(|_| RequestError::Transport(self.state.failure_message()))?;
        pending_request.disarm();
        response
    }

    pub(super) async fn register_delegate(
        &self,
        session_id: wire::SessionId,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) {
        self.state
            .delegates
            .lock()
            .await
            .insert(session_id, delegate);
    }

    pub(super) async fn remove_delegate(&self, session_id: wire::SessionId) {
        self.state.delegates.lock().await.remove(&session_id);
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

struct ConnectionState {
    outgoing_tx: mpsc::Sender<wire::ClientMessage>,
    pending:
        Mutex<HashMap<wire::RequestId, oneshot::Sender<Result<wire::HostResponse, RequestError>>>>,
    delegates: Mutex<HashMap<wire::SessionId, Arc<dyn CodeModeSessionDelegate>>>,
    callback_cancellations: Mutex<HashMap<wire::CallbackId, CancellationToken>>,
    next_request_id: AtomicU64,
    alive: AtomicBool,
    failure: std::sync::Mutex<Option<String>>,
}

impl ConnectionState {
    fn new(outgoing_tx: mpsc::Sender<wire::ClientMessage>) -> Self {
        Self {
            outgoing_tx,
            pending: Mutex::new(HashMap::new()),
            delegates: Mutex::new(HashMap::new()),
            callback_cancellations: Mutex::new(HashMap::new()),
            next_request_id: AtomicU64::new(1),
            alive: AtomicBool::new(true),
            failure: std::sync::Mutex::new(None),
        }
    }

    async fn send(&self, message: wire::ClientMessage) -> Result<(), String> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(self.failure_message());
        }
        self.outgoing_tx
            .send(message)
            .await
            .map_err(|_| self.failure_message())
    }

    fn failure_message(&self) -> String {
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| "code-mode host connection closed".to_string())
    }

    async fn fail(&self, reason: String) {
        if !self.alive.swap(false, Ordering::AcqRel) {
            return;
        }
        warn!(%reason, "code-mode host connection failed");
        *self.failure.lock().unwrap_or_else(PoisonError::into_inner) = Some(reason.clone());
        for (_, sender) in self.pending.lock().await.drain() {
            let _ = sender.send(Err(RequestError::Transport(reason.clone())));
        }
        for (_, cancellation) in self.callback_cancellations.lock().await.drain() {
            cancellation.cancel();
        }
        self.delegates.lock().await.clear();
    }
}

struct PendingRequest {
    cleanup: Option<PendingRequestCleanup>,
}

struct PendingRequestCleanup {
    state: Arc<ConnectionState>,
    id: wire::RequestId,
    request_sent: bool,
}

impl PendingRequest {
    fn new(state: Arc<ConnectionState>, id: wire::RequestId) -> Self {
        Self {
            cleanup: Some(PendingRequestCleanup {
                state,
                id,
                request_sent: false,
            }),
        }
    }

    fn mark_request_sent(&mut self) {
        if let Some(cleanup) = self.cleanup.as_mut() {
            cleanup.request_sent = true;
        }
    }

    fn disarm(&mut self) {
        self.cleanup = None;
    }

    async fn cleanup(&mut self) {
        if let Some(task) = self.spawn_cleanup() {
            let _ = task.await;
        }
    }

    fn spawn_cleanup(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        let handle = tokio::runtime::Handle::try_current().ok()?;
        let cleanup = self.cleanup.take()?;
        Some(handle.spawn(cleanup.run()))
    }
}

impl Drop for PendingRequest {
    fn drop(&mut self) {
        std::mem::drop(self.spawn_cleanup());
    }
}

impl PendingRequestCleanup {
    async fn run(self) {
        self.state.pending.lock().await.remove(&self.id);
        if self.request_sent {
            let _ = self
                .state
                .send(wire::ClientMessage::CancelRequest { id: self.id })
                .await;
        }
    }
}

async fn drive_reader(
    mut stdout: tokio::process::ChildStdout,
    state: Weak<ConnectionState>,
    cancellation: CancellationToken,
) {
    loop {
        let message = tokio::select! {
            _ = cancellation.cancelled() => return,
            result = wire::read_frame::<_, wire::HostMessage>(&mut stdout) => result,
        };
        match message {
            Ok(Some(message)) => {
                let Some(state) = state.upgrade() else {
                    return;
                };
                handle_host_message(state, message).await;
            }
            Ok(None) => {
                if let Some(state) = state.upgrade() {
                    state
                        .fail("code-mode host closed its stdout".to_string())
                        .await;
                }
                return;
            }
            Err(err) => {
                if let Some(state) = state.upgrade() {
                    state
                        .fail(format!("failed to read code-mode host message: {err}"))
                        .await;
                }
                return;
            }
        }
    }
}

async fn handle_host_message(state: Arc<ConnectionState>, message: wire::HostMessage) {
    match message {
        wire::HostMessage::Response { id, result } => {
            if let Some(sender) = state.pending.lock().await.remove(&id) {
                let result = match result {
                    wire::WireResult::Ok { value } => Ok(value),
                    wire::WireResult::Err { error } => Err(RequestError::Host(error)),
                };
                let _ = sender.send(result);
            }
        }
        wire::HostMessage::CellClosed {
            session_id,
            cell_id,
        } => {
            if let Some(delegate) = state.delegates.lock().await.get(&session_id).cloned() {
                delegate.cell_closed(&convert::protocol_cell_id(&cell_id));
            }
        }
        wire::HostMessage::CallbackRequest {
            id,
            session_id,
            request,
        } => {
            start_callback(state, id, session_id, request).await;
        }
        wire::HostMessage::CancelCallback { id } => {
            if let Some(cancellation) = state.callback_cancellations.lock().await.remove(&id) {
                cancellation.cancel();
            }
        }
    }
}

async fn start_callback(
    state: Arc<ConnectionState>,
    id: wire::CallbackId,
    session_id: wire::SessionId,
    request: wire::CallbackRequest,
) {
    let delegate = state.delegates.lock().await.get(&session_id).cloned();
    let Some(delegate) = delegate else {
        let response = match request {
            wire::CallbackRequest::InvokeTool { .. } => wire::CallbackResponse::ToolError {
                error_text: format!("code-mode session {session_id} not found"),
            },
            wire::CallbackRequest::Notify { .. } => wire::CallbackResponse::NotificationError {
                error_text: format!("code-mode session {session_id} not found"),
            },
        };
        let _ = state
            .send(wire::ClientMessage::CallbackResponse { id, response })
            .await;
        return;
    };

    let cancellation = CancellationToken::new();
    state
        .callback_cancellations
        .lock()
        .await
        .insert(id, cancellation.clone());
    tokio::spawn(async move {
        let response = match request {
            wire::CallbackRequest::InvokeTool { invocation } => delegate
                .invoke_tool(convert::nested_tool_call(invocation), cancellation)
                .await
                .map_or_else(
                    |error_text| wire::CallbackResponse::ToolError { error_text },
                    |result| wire::CallbackResponse::ToolResult { result },
                ),
            wire::CallbackRequest::Notify {
                call_id,
                cell_id,
                text,
            } => delegate
                .notify(
                    call_id,
                    convert::protocol_cell_id(&cell_id),
                    text,
                    cancellation,
                )
                .await
                .map_or_else(
                    |error_text| wire::CallbackResponse::NotificationError { error_text },
                    |()| wire::CallbackResponse::NotificationDelivered,
                ),
        };
        state.callback_cancellations.lock().await.remove(&id);
        let _ = state
            .send(wire::ClientMessage::CallbackResponse { id, response })
            .await;
    });
}

fn spawn_critical_connection_task(
    description: &'static str,
    state: Weak<ConnectionState>,
    future: impl Future<Output = ()> + Send + 'static,
) {
    let task = tokio::spawn(future);
    tokio::spawn(async move {
        if let Err(err) = task.await
            && let Some(state) = state.upgrade()
        {
            state
                .fail(format!("code-mode host {description} task failed: {err}"))
                .await;
        }
    });
}

fn host_process(command: &CodeModeHostCommand) -> Command {
    let mut process = Command::new(&command.program);
    process.args(&command.args);
    #[cfg(unix)]
    process.process_group(0);
    process
}
