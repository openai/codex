use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Weak;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::wire::ClientMessage;
use codex_code_mode_protocol::wire::DelegateRequest;
use codex_code_mode_protocol::wire::DelegateRequestId;
use codex_code_mode_protocol::wire::DelegateResponse;
use codex_code_mode_protocol::wire::HostMessage;
use codex_code_mode_protocol::wire::HostRequest;
use codex_code_mode_protocol::wire::HostResponse;
use codex_code_mode_protocol::wire::RequestId;
use codex_code_mode_protocol::wire::SessionId;
use codex_code_mode_protocol::wire::read_frame;
use codex_code_mode_protocol::wire::write_frame;
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

const IPC_CHANNEL_CAPACITY: usize = 128;

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
        tokio::spawn(async move {
            let mut stdin = stdin;
            loop {
                tokio::select! {
                    _ = writer_cancellation.cancelled() => break,
                    message = outgoing_rx.recv() => {
                        let Some(message) = message else {
                            break;
                        };
                        if let Err(err) = write_frame(&mut stdin, &message).await {
                            warn!("failed to write code-mode host message: {err}");
                            if let Some(state) = writer_state.upgrade() {
                                state
                                    .fail(format!("failed to write code-mode host message: {err}"))
                                    .await;
                            }
                            break;
                        }
                    }
                }
            }
        });

        let reader_state = Arc::downgrade(&state);
        let reader_cancellation = cancellation.clone();
        tokio::spawn(async move {
            drive_reader(stdout, reader_state, reader_cancellation).await;
        });

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
        tokio::spawn(async move {
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

    pub(super) async fn request(&self, request: HostRequest) -> Result<HostResponse, String> {
        let id = self.state.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (response_tx, response_rx) = oneshot::channel();
        self.state.pending.lock().await.insert(id, response_tx);
        if let Err(err) = self
            .state
            .send(ClientMessage::Request { id, request })
            .await
        {
            self.state.pending.lock().await.remove(&id);
            return Err(err);
        }
        response_rx
            .await
            .map_err(|_| self.state.failure_message())?
    }

    pub(super) async fn execute(
        &self,
        session_id: SessionId,
        request: ExecuteRequest,
    ) -> Result<StartedCell, String> {
        let id = self.state.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (response_tx, response_rx) = oneshot::channel();
        let (initial_tx, initial_rx) = oneshot::channel();
        self.state.pending.lock().await.insert(id, response_tx);
        self.state
            .initial_responses
            .lock()
            .await
            .insert(id, initial_tx);
        if let Err(err) = self
            .state
            .send(ClientMessage::Request {
                id,
                request: HostRequest::Execute {
                    session_id,
                    request,
                },
            })
            .await
        {
            self.state.pending.lock().await.remove(&id);
            self.state.initial_responses.lock().await.remove(&id);
            return Err(err);
        }
        let response = match response_rx.await {
            Ok(Ok(response)) => response,
            Ok(Err(err)) => {
                self.state.initial_responses.lock().await.remove(&id);
                return Err(err);
            }
            Err(_) => {
                self.state.initial_responses.lock().await.remove(&id);
                return Err(self.state.failure_message());
            }
        };
        match response {
            HostResponse::ExecutionStarted { cell_id } => {
                Ok(StartedCell::from_result_receiver(cell_id, initial_rx))
            }
            _ => {
                self.state.initial_responses.lock().await.remove(&id);
                Err("code-mode host returned an invalid execute response".to_string())
            }
        }
    }

    pub(super) async fn register_delegate(
        &self,
        session_id: SessionId,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) {
        self.state
            .delegates
            .lock()
            .await
            .insert(session_id, delegate);
    }

    pub(super) async fn remove_delegate(&self, session_id: SessionId) {
        self.state.delegates.lock().await.remove(&session_id);
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

struct ConnectionState {
    outgoing_tx: mpsc::Sender<ClientMessage>,
    pending: Mutex<HashMap<RequestId, oneshot::Sender<Result<HostResponse, String>>>>,
    initial_responses: Mutex<
        HashMap<
            RequestId,
            oneshot::Sender<Result<codex_code_mode_protocol::RuntimeResponse, String>>,
        >,
    >,
    delegates: Mutex<HashMap<SessionId, Arc<dyn CodeModeSessionDelegate>>>,
    delegate_cancellations: Mutex<HashMap<DelegateRequestId, CancellationToken>>,
    next_request_id: AtomicU64,
    alive: AtomicBool,
    failure: std::sync::Mutex<Option<String>>,
}

impl ConnectionState {
    fn new(outgoing_tx: mpsc::Sender<ClientMessage>) -> Self {
        Self {
            outgoing_tx,
            pending: Mutex::new(HashMap::new()),
            initial_responses: Mutex::new(HashMap::new()),
            delegates: Mutex::new(HashMap::new()),
            delegate_cancellations: Mutex::new(HashMap::new()),
            next_request_id: AtomicU64::new(1),
            alive: AtomicBool::new(true),
            failure: std::sync::Mutex::new(None),
        }
    }

    async fn send(&self, message: ClientMessage) -> Result<(), String> {
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
            .ok()
            .and_then(|failure| failure.clone())
            .unwrap_or_else(|| "code-mode host connection closed".to_string())
    }

    async fn fail(&self, reason: String) {
        if !self.alive.swap(false, Ordering::AcqRel) {
            return;
        }
        warn!(%reason, "code-mode host connection failed");
        if let Ok(mut failure) = self.failure.lock() {
            *failure = Some(reason.clone());
        }
        for (_, sender) in self.pending.lock().await.drain() {
            let _ = sender.send(Err(reason.clone()));
        }
        self.initial_responses.lock().await.clear();
        for (_, cancellation) in self.delegate_cancellations.lock().await.drain() {
            cancellation.cancel();
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
            result = read_frame::<_, HostMessage>(&mut stdout) => result,
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

async fn handle_host_message(state: Arc<ConnectionState>, message: HostMessage) {
    match message {
        HostMessage::Response { id, response } => {
            if let Some(sender) = state.pending.lock().await.remove(&id) {
                let _ = sender.send(response);
            }
        }
        HostMessage::InitialResponse { id, response } => {
            if let Some(sender) = state.initial_responses.lock().await.remove(&id) {
                let _ = sender.send(response);
            }
        }
        HostMessage::DelegateRequest {
            id,
            session_id,
            request,
        } => {
            let delegate = state.delegates.lock().await.get(&session_id).cloned();
            let Some(delegate) = delegate else {
                let _ = state
                    .send(ClientMessage::DelegateResponse {
                        id,
                        response: Err(format!("unknown code-mode session {session_id}")),
                    })
                    .await;
                return;
            };
            let cancellation = CancellationToken::new();
            state
                .delegate_cancellations
                .lock()
                .await
                .insert(id, cancellation.clone());
            tokio::spawn(async move {
                let response = match request {
                    DelegateRequest::InvokeTool(invocation) => delegate
                        .invoke_tool(invocation, cancellation)
                        .await
                        .map(DelegateResponse::ToolResult),
                    DelegateRequest::Notify {
                        call_id,
                        cell_id,
                        text,
                    } => delegate
                        .notify(call_id, cell_id, text, cancellation)
                        .await
                        .map(|()| DelegateResponse::NotificationDelivered),
                };
                state.delegate_cancellations.lock().await.remove(&id);
                let _ = state
                    .send(ClientMessage::DelegateResponse { id, response })
                    .await;
            });
        }
        HostMessage::CancelDelegateRequest { id } => {
            if let Some(cancellation) = state.delegate_cancellations.lock().await.remove(&id) {
                cancellation.cancel();
            }
        }
        HostMessage::CellClosed {
            session_id,
            cell_id,
        } => {
            if let Some(delegate) = state.delegates.lock().await.get(&session_id).cloned() {
                delegate.cell_closed(&cell_id);
            }
        }
    }
}

fn host_process(command: &CodeModeHostCommand) -> Command {
    let mut process = Command::new(&command.program);
    process.args(&command.args);
    #[cfg(unix)]
    process.process_group(0);
    process
}
