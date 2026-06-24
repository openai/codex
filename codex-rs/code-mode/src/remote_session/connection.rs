use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientHello;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::DelegateRequestId;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::FramedWriter;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::HostResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::SupportedProtocolVersions;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::warn;

use self::reader::drive_reader;

mod reader;

const IPC_CHANNEL_CAPACITY: usize = 128;

pub(super) struct Connection {
    state: Arc<ConnectionState>,
    cancellation: CancellationToken,
}

impl Connection {
    pub(super) async fn spawn(host_program: &Path) -> Result<Self, String> {
        let mut command = Command::new(host_program);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| {
                format!(
                    "failed to spawn code-mode host {}: {err}",
                    host_program.display()
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
        let mut reader = FramedReader::new(stdout);
        let mut writer = FramedWriter::new(stdin);

        let hello = ClientHello::new(
            SupportedProtocolVersions::try_new([ProtocolVersion::V1])
                .map_err(|err| err.to_string())?,
            CapabilitySet::empty(),
            CapabilitySet::empty(),
        )
        .map_err(|err| err.to_string())?;
        writer
            .write(&ClientToHost::ClientHello(hello))
            .await
            .map_err(|err| format!("failed to write code-mode host hello: {err}"))?;
        match reader
            .read::<HostToClient>()
            .await
            .map_err(|err| format!("failed to read code-mode host hello: {err}"))?
        {
            Some(HostToClient::HostHello(hello))
                if hello.selected_version() == ProtocolVersion::V1 => {}
            Some(HostToClient::HandshakeRejected { reason }) => {
                return Err(format!("code-mode host rejected the handshake: {reason:?}"));
            }
            Some(message) => {
                return Err(format!(
                    "code-mode host returned an invalid handshake response: {message:?}"
                ));
            }
            None => return Err("code-mode host exited during handshake".to_string()),
        }

        let (outgoing_tx, mut outgoing_rx) = mpsc::channel(IPC_CHANNEL_CAPACITY);
        let cancellation = CancellationToken::new();
        let state = Arc::new(ConnectionState::new(outgoing_tx, cancellation.clone()));

        let writer_state = Arc::downgrade(&state);
        let writer_cancellation = cancellation.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = writer_cancellation.cancelled() => break,
                    message = outgoing_rx.recv() => {
                        let Some(message) = message else {
                            break;
                        };
                        if let Err(err) = writer.write(&message).await {
                            if let Some(state) = writer_state.upgrade() {
                                state.fail(format!(
                                    "failed to write code-mode host message: {err}"
                                )).await;
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
            drive_reader(reader, reader_state, reader_cancellation).await;
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

    pub(super) async fn open_session(
        &self,
        session_id: SessionId,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> Result<(), String> {
        self.state
            .delegates
            .lock()
            .await
            .insert(session_id.clone(), delegate);
        let response = self
            .request(HostRequest::OpenSession {
                session_id: session_id.clone(),
            })
            .await;
        match response {
            Ok(HostResponse::SessionReady {
                session_id: ready_session_id,
            }) if ready_session_id == session_id => Ok(()),
            Ok(_) => {
                self.state.delegates.lock().await.remove(&session_id);
                Err("code-mode host returned an invalid open-session response".to_string())
            }
            Err(err) => {
                self.state.delegates.lock().await.remove(&session_id);
                Err(err)
            }
        }
    }

    pub(super) async fn execute(
        &self,
        session_id: SessionId,
        request: ExecuteRequest,
    ) -> Result<StartedCell, String> {
        let request = request
            .try_into()
            .map_err(|err| format!("failed to encode code-mode execute request: {err}"))?;
        let id = RequestId::new(self.state.next_request_id.fetch_add(1, Ordering::Relaxed));
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
            .send(ClientToHost::Request {
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
            HostResponse::ExecutionStarted { cell_id } => Ok(StartedCell::from_result_receiver(
                cell_id.into(),
                initial_rx,
            )),
            _ => {
                self.state.initial_responses.lock().await.remove(&id);
                Err("code-mode host returned an invalid execute response".to_string())
            }
        }
    }

    pub(super) async fn wait(
        &self,
        session_id: SessionId,
        request: WaitRequest,
    ) -> Result<WaitOutcome, String> {
        match self
            .request(HostRequest::Wait {
                session_id,
                request: request.into(),
            })
            .await?
        {
            HostResponse::WaitCompleted { outcome } => Ok(outcome.into()),
            _ => Err("code-mode host returned an invalid wait response".to_string()),
        }
    }

    pub(super) async fn terminate(
        &self,
        session_id: SessionId,
        cell_id: CellId,
    ) -> Result<WaitOutcome, String> {
        match self
            .request(HostRequest::Terminate {
                session_id,
                cell_id: cell_id.into(),
            })
            .await?
        {
            HostResponse::WaitCompleted { outcome } => Ok(outcome.into()),
            _ => Err("code-mode host returned an invalid terminate response".to_string()),
        }
    }

    pub(super) async fn shutdown_session(&self, session_id: SessionId) -> Result<(), String> {
        let response = self
            .request(HostRequest::ShutdownSession {
                session_id: session_id.clone(),
            })
            .await;
        self.state.delegates.lock().await.remove(&session_id);
        match response? {
            HostResponse::SessionClosed {
                session_id: closed_session_id,
            } if closed_session_id == session_id => Ok(()),
            _ => Err("code-mode host returned an invalid shutdown response".to_string()),
        }
    }

    async fn request(&self, request: HostRequest) -> Result<HostResponse, String> {
        let id = RequestId::new(self.state.next_request_id.fetch_add(1, Ordering::Relaxed));
        let (response_tx, response_rx) = oneshot::channel();
        self.state.pending.lock().await.insert(id, response_tx);
        if let Err(err) = self.state.send(ClientToHost::Request { id, request }).await {
            self.state.pending.lock().await.remove(&id);
            return Err(err);
        }
        response_rx
            .await
            .map_err(|_| self.state.failure_message())?
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

struct ConnectionState {
    outgoing_tx: mpsc::Sender<ClientToHost>,
    pending: Mutex<HashMap<RequestId, oneshot::Sender<Result<HostResponse, String>>>>,
    initial_responses: Mutex<HashMap<RequestId, oneshot::Sender<Result<RuntimeResponse, String>>>>,
    delegates: Mutex<HashMap<SessionId, Arc<dyn CodeModeSessionDelegate>>>,
    delegate_cancellations: Mutex<HashMap<DelegateRequestId, CancellationToken>>,
    next_request_id: AtomicI64,
    alive: AtomicBool,
    failure: std::sync::Mutex<Option<String>>,
    cancellation: CancellationToken,
}

impl ConnectionState {
    fn new(outgoing_tx: mpsc::Sender<ClientToHost>, cancellation: CancellationToken) -> Self {
        Self {
            outgoing_tx,
            pending: Mutex::new(HashMap::new()),
            initial_responses: Mutex::new(HashMap::new()),
            delegates: Mutex::new(HashMap::new()),
            delegate_cancellations: Mutex::new(HashMap::new()),
            next_request_id: AtomicI64::new(1),
            alive: AtomicBool::new(true),
            failure: std::sync::Mutex::new(None),
            cancellation,
        }
    }

    async fn send(&self, message: ClientToHost) -> Result<(), String> {
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
        if let Ok(mut failure) = self.failure.lock() {
            *failure = Some(reason.clone());
        }
        self.cancellation.cancel();
        for (_, sender) in self.pending.lock().await.drain() {
            let _ = sender.send(Err(reason.clone()));
        }
        for (_, sender) in self.initial_responses.lock().await.drain() {
            let _ = sender.send(Err(reason.clone()));
        }
        self.delegates.lock().await.clear();
        for (_, cancellation) in self.delegate_cancellations.lock().await.drain() {
            cancellation.cancel();
        }
    }
}
