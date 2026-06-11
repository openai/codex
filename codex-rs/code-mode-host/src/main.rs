use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_code_mode::CodeModeService;
use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::NotificationFuture;
use codex_code_mode_protocol::ToolInvocationFuture;
use codex_code_mode_protocol::wire::ClientMessage;
use codex_code_mode_protocol::wire::DelegateRequest;
use codex_code_mode_protocol::wire::DelegateRequestId;
use codex_code_mode_protocol::wire::DelegateResponse;
use codex_code_mode_protocol::wire::HostMessage;
use codex_code_mode_protocol::wire::HostRequest;
use codex_code_mode_protocol::wire::HostResponse;
use codex_code_mode_protocol::wire::SessionId;
use codex_code_mode_protocol::wire::read_frame;
use codex_code_mode_protocol::wire::write_frame;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

const IPC_CHANNEL_CAPACITY: usize = 128;

fn main() {
    let runtime = build_runtime()
        .unwrap_or_else(|err| panic!("failed to build code-mode host runtime: {err}"));
    if let Err(err) = runtime.block_on(run()) {
        eprintln!("codex-code-mode-host failed: {err}");
        std::process::exit(1);
    }
}

fn build_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
}

async fn run() -> Result<(), String> {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(IPC_CHANNEL_CAPACITY);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let state = Arc::new(HostState {
        sessions: Mutex::new(HashMap::new()),
        next_session_id: AtomicU64::new(1),
        host_id: std::process::id().to_string(),
        peer,
    });

    let writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(message) = outgoing_rx.recv().await {
            write_frame(&mut stdout, &message)
                .await
                .map_err(|err| err.to_string())?;
        }
        Ok::<(), String>(())
    });

    let mut stdin = tokio::io::stdin();
    while let Some(message) = read_frame::<_, ClientMessage>(&mut stdin)
        .await
        .map_err(|err| err.to_string())?
    {
        match message {
            ClientMessage::Request { id, request } => {
                let state = Arc::clone(&state);
                spawn_critical_request_task("request handler", async move {
                    state.handle_request(id, request).await;
                });
            }
            ClientMessage::DelegateResponse { id, response } => {
                state.peer.complete(id, response).await;
            }
        }
    }

    let sessions = state
        .sessions
        .lock()
        .await
        .drain()
        .map(|(_, session)| session)
        .collect::<Vec<_>>();
    for session in sessions {
        let _ = session.shutdown().await;
    }
    drop(state);
    writer.await.map_err(|err| err.to_string())?
}

fn spawn_critical_request_task(
    description: &'static str,
    future: impl Future<Output = ()> + Send + 'static,
) {
    let task = tokio::spawn(future);
    tokio::spawn(async move {
        if let Err(err) = task.await
            && err.is_panic()
        {
            eprintln!("code-mode host {description} panicked: {err}");
            std::process::exit(1);
        }
    });
}

struct HostState {
    sessions: Mutex<HashMap<SessionId, Arc<CodeModeService>>>,
    next_session_id: AtomicU64,
    host_id: String,
    peer: Arc<HostPeer>,
}

impl HostState {
    async fn handle_request(self: Arc<Self>, request_id: u64, request: HostRequest) {
        match request {
            HostRequest::CreateSession => {
                let session_id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
                let delegate = Arc::new(RemoteDelegate {
                    session_id,
                    peer: Arc::clone(&self.peer),
                });
                self.sessions.lock().await.insert(
                    session_id,
                    Arc::new(CodeModeService::with_delegate_and_cell_id_prefix(
                        delegate,
                        self.host_id.clone(),
                    )),
                );
                self.respond(request_id, Ok(HostResponse::SessionCreated { session_id }))
                    .await;
            }
            HostRequest::Execute {
                session_id,
                request,
            } => {
                let error = match self.session(session_id).await {
                    Ok(session) => match session.execute(request).await {
                        Ok(started) => {
                            let cell_id = started.cell_id.clone();
                            self.respond(
                                request_id,
                                Ok(HostResponse::ExecutionStarted { cell_id }),
                            )
                            .await;
                            let peer = Arc::clone(&self.peer);
                            spawn_critical_request_task("initial response handler", async move {
                                let response = started.initial_response().await;
                                peer.send(HostMessage::InitialResponse {
                                    id: request_id,
                                    response,
                                })
                                .await;
                            });
                            return;
                        }
                        Err(err) => err,
                    },
                    Err(err) => err,
                };
                self.respond(request_id, Err(error)).await;
            }
            HostRequest::Wait {
                session_id,
                request,
            } => {
                let result = match self.session(session_id).await {
                    Ok(session) => session
                        .wait(request)
                        .await
                        .map(|outcome| HostResponse::WaitCompleted { outcome }),
                    Err(err) => Err(err),
                };
                self.respond(request_id, result).await;
            }
            HostRequest::Terminate {
                session_id,
                cell_id,
            } => {
                let result = match self.session(session_id).await {
                    Ok(session) => session
                        .terminate(cell_id)
                        .await
                        .map(|outcome| HostResponse::WaitCompleted { outcome }),
                    Err(err) => Err(err),
                };
                self.respond(request_id, result).await;
            }
            HostRequest::ShutdownSession { session_id } => {
                let session = self.sessions.lock().await.remove(&session_id);
                let result = match session {
                    Some(session) => session
                        .shutdown()
                        .await
                        .map(|()| HostResponse::SessionShutdown),
                    None => Ok(HostResponse::SessionShutdown),
                };
                self.respond(request_id, result).await;
            }
        }
    }

    async fn session(&self, session_id: SessionId) -> Result<Arc<CodeModeService>, String> {
        self.sessions
            .lock()
            .await
            .get(&session_id)
            .cloned()
            .ok_or_else(|| format!("unknown code-mode session {session_id}"))
    }

    async fn respond(&self, id: u64, response: Result<HostResponse, String>) {
        self.peer.send(HostMessage::Response { id, response }).await;
    }
}

struct HostPeer {
    outgoing_tx: mpsc::Sender<HostMessage>,
    pending: Mutex<HashMap<DelegateRequestId, oneshot::Sender<Result<DelegateResponse, String>>>>,
    next_request_id: AtomicU64,
}

impl HostPeer {
    fn new(outgoing_tx: mpsc::Sender<HostMessage>) -> Self {
        Self {
            outgoing_tx,
            pending: Mutex::new(HashMap::new()),
            next_request_id: AtomicU64::new(1),
        }
    }

    async fn send(&self, message: HostMessage) {
        let _ = self.outgoing_tx.send(message).await;
    }

    async fn call(
        &self,
        session_id: SessionId,
        request: DelegateRequest,
        cancellation_token: CancellationToken,
    ) -> Result<DelegateResponse, String> {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (response_tx, response_rx) = oneshot::channel();
        self.pending.lock().await.insert(id, response_tx);
        if self
            .outgoing_tx
            .send(HostMessage::DelegateRequest {
                id,
                session_id,
                request,
            })
            .await
            .is_err()
        {
            self.pending.lock().await.remove(&id);
            return Err("code-mode client connection closed".to_string());
        }
        tokio::select! {
            response = response_rx => {
                response.map_err(|_| "code-mode client closed before returning delegate output".to_string())?
            }
            _ = cancellation_token.cancelled() => {
                self.pending.lock().await.remove(&id);
                self.send(HostMessage::CancelDelegateRequest { id }).await;
                Err("code mode delegate request cancelled".to_string())
            }
        }
    }

    async fn complete(&self, id: DelegateRequestId, response: Result<DelegateResponse, String>) {
        if let Some(sender) = self.pending.lock().await.remove(&id) {
            let _ = sender.send(response);
        }
    }
}

struct RemoteDelegate {
    session_id: SessionId,
    peer: Arc<HostPeer>,
}

impl CodeModeSessionDelegate for RemoteDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            match self
                .peer
                .call(
                    self.session_id,
                    DelegateRequest::InvokeTool(invocation),
                    cancellation_token,
                )
                .await?
            {
                DelegateResponse::ToolResult(result) => Ok(result),
                DelegateResponse::NotificationDelivered => {
                    Err("code-mode client returned an invalid tool result".to_string())
                }
            }
        })
    }

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            match self
                .peer
                .call(
                    self.session_id,
                    DelegateRequest::Notify {
                        call_id,
                        cell_id,
                        text,
                    },
                    cancellation_token,
                )
                .await?
            {
                DelegateResponse::NotificationDelivered => Ok(()),
                DelegateResponse::ToolResult(_) => {
                    Err("code-mode client returned an invalid notification result".to_string())
                }
            }
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        let peer = Arc::clone(&self.peer);
        let cell_id = cell_id.clone();
        let session_id = self.session_id;
        tokio::spawn(async move {
            peer.send(HostMessage::CellClosed {
                session_id,
                cell_id,
            })
            .await;
        });
    }
}
