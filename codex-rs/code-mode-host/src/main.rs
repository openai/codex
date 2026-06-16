use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::wire::ClientMessage;
use codex_code_mode_protocol::wire::Error;
use codex_code_mode_protocol::wire::HostMessage;
use codex_code_mode_protocol::wire::HostRequest;
use codex_code_mode_protocol::wire::HostResponse;
use codex_code_mode_protocol::wire::RequestId;
use codex_code_mode_protocol::wire::SessionId;
use codex_code_mode_protocol::wire::WireResult;
use codex_code_mode_protocol::wire::read_frame;
use codex_code_mode_protocol::wire::write_frame;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod convert;
mod peer;
mod session;

use peer::HostPeer;
use session::HostSession;

const IPC_CHANNEL_CAPACITY: usize = 128;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("codex-code-mode-host failed: {err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(IPC_CHANNEL_CAPACITY);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let state = Arc::new(HostState {
        sessions: StdMutex::new(HashMap::new()),
        pending_requests: Mutex::new(HashMap::new()),
        next_session_id: AtomicU64::new(1),
        closing: AtomicBool::new(false),
        peer: Arc::clone(&peer),
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

    let input_result = async {
        let mut stdin = tokio::io::stdin();
        while let Some(message) = read_frame::<_, ClientMessage>(&mut stdin)
            .await
            .map_err(|err| err.to_string())?
        {
            match message {
                ClientMessage::Request { id, request } => {
                    state.spawn_request(id, request).await;
                }
                ClientMessage::CancelRequest { id } => state.cancel_request(id).await,
                ClientMessage::CallbackResponse { id, response } => {
                    peer.complete(id, response).await;
                }
            }
        }
        Ok::<(), String>(())
    }
    .await;

    peer.disconnect().await;
    state.disconnect().await;
    drop(state);
    drop(peer);
    let writer_result = writer.await.map_err(|err| err.to_string())?;
    input_result?;
    writer_result
}

struct HostState {
    sessions: StdMutex<HashMap<SessionId, HostSession>>,
    pending_requests: Mutex<HashMap<RequestId, CancellationToken>>,
    next_session_id: AtomicU64,
    closing: AtomicBool,
    peer: Arc<HostPeer>,
}

impl HostState {
    async fn spawn_request(self: &Arc<Self>, request_id: RequestId, request: HostRequest) {
        if self.closing.load(Ordering::Acquire) {
            self.send_response(request_id, Err(Error::ShuttingDown), None)
                .await;
            return;
        }
        let cancellation_token = CancellationToken::new();
        let duplicate = {
            let mut pending_requests = self.pending_requests.lock().await;
            if let std::collections::hash_map::Entry::Vacant(e) = pending_requests.entry(request_id)
            {
                e.insert(cancellation_token.clone());
                false
            } else {
                true
            }
        };
        if duplicate {
            self.send_response(
                request_id,
                Err(Error::InvalidRequest {
                    message: format!("duplicate request id {request_id}"),
                }),
                None,
            )
            .await;
            return;
        }

        let state = Arc::clone(self);
        spawn_critical_request_task(async move {
            state
                .handle_request(request_id, request, cancellation_token)
                .await;
            state.pending_requests.lock().await.remove(&request_id);
        });
    }

    async fn cancel_request(&self, request_id: RequestId) {
        if let Some(cancellation_token) = self.pending_requests.lock().await.get(&request_id) {
            cancellation_token.cancel();
        }
    }

    async fn handle_request(
        &self,
        request_id: RequestId,
        request: HostRequest,
        cancellation_token: CancellationToken,
    ) {
        match request {
            HostRequest::CreateSession => {
                if cancellation_token.is_cancelled() || self.closing.load(Ordering::Acquire) {
                    return;
                }
                let session_id = self.next_session_id.fetch_add(1, Ordering::Relaxed);
                let session = HostSession::new(session_id, Arc::clone(&self.peer));
                let inserted = {
                    let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
                    if self.closing.load(Ordering::Acquire) {
                        false
                    } else {
                        sessions.insert(session_id, session);
                        true
                    }
                };
                if !inserted {
                    return;
                }
                self.send_response(
                    request_id,
                    Ok(HostResponse::SessionCreated { session_id }),
                    Some(&cancellation_token),
                )
                .await;
            }
            HostRequest::ShutdownSession { session_id } => {
                let session = self
                    .sessions
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&session_id);
                let result = match session {
                    Some(session) => session
                        .shutdown()
                        .await
                        .map(|()| HostResponse::SessionShutdown)
                        .map_err(convert::runtime_error),
                    None => Err(Error::MissingSession { session_id }),
                };
                self.send_response(request_id, result, Some(&cancellation_token))
                    .await;
            }
            HostRequest::CreateCell {
                session_id,
                request,
            } => {
                let Some(session) = self.session(session_id) else {
                    self.send_response(
                        request_id,
                        Err(Error::MissingSession { session_id }),
                        Some(&cancellation_token),
                    )
                    .await;
                    return;
                };
                let result = session
                    .create_cell(convert::create_cell_request(request))
                    .await
                    .map(|cell_id| HostResponse::CellCreated {
                        cell_id: convert::wire_cell_id(&cell_id),
                    })
                    .map_err(convert::runtime_error);
                self.send_response(request_id, result, Some(&cancellation_token))
                    .await;
            }
            HostRequest::Observe {
                session_id,
                cell_id,
                mode,
            } => {
                let Some(session) = self.session(session_id) else {
                    self.send_response(
                        request_id,
                        Err(Error::MissingSession { session_id }),
                        Some(&cancellation_token),
                    )
                    .await;
                    return;
                };
                let runtime_cell_id = convert::runtime_cell_id(&cell_id);
                let result = session
                    .observe(&runtime_cell_id, convert::observe_mode(mode))
                    .await
                    .map(convert::cell_event)
                    .map(|event| HostResponse::Observed { event })
                    .map_err(convert::runtime_error);
                self.send_response(request_id, result, Some(&cancellation_token))
                    .await;
            }
            HostRequest::Terminate {
                session_id,
                cell_id,
            } => {
                let Some(session) = self.session(session_id) else {
                    self.send_response(
                        request_id,
                        Err(Error::MissingSession { session_id }),
                        Some(&cancellation_token),
                    )
                    .await;
                    return;
                };
                let runtime_cell_id = convert::runtime_cell_id(&cell_id);
                let result = session
                    .terminate(&runtime_cell_id)
                    .await
                    .map(convert::cell_event)
                    .map(|event| HostResponse::Observed { event })
                    .map_err(convert::runtime_error);
                self.send_response(request_id, result, Some(&cancellation_token))
                    .await;
            }
        }
    }

    fn session(&self, session_id: SessionId) -> Option<HostSession> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&session_id)
            .cloned()
    }

    async fn send_response(
        &self,
        request_id: RequestId,
        result: Result<HostResponse, Error>,
        cancellation_token: Option<&CancellationToken>,
    ) {
        if cancellation_token.is_some_and(CancellationToken::is_cancelled) {
            return;
        }
        self.peer
            .send(HostMessage::Response {
                id: request_id,
                result: WireResult::from_result(result),
            })
            .await;
    }

    async fn disconnect(&self) {
        self.closing.store(true, Ordering::Release);
        for cancellation_token in self.pending_requests.lock().await.values() {
            cancellation_token.cancel();
        }
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
        while !self.pending_requests.lock().await.is_empty() {
            tokio::task::yield_now().await;
        }
    }
}

fn spawn_critical_request_task(future: impl Future<Output = ()> + Send + 'static) {
    let task = tokio::spawn(future);
    tokio::spawn(async move {
        if let Err(err) = task.await
            && err.is_panic()
        {
            eprintln!("code-mode host request task panicked: {err}");
            std::process::exit(1);
        }
    });
}
