use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::host::DelegateRequest;
use codex_code_mode_protocol::host::DelegateRequestId;
use codex_code_mode_protocol::host::DelegateResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::WireResult;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub(super) struct HostPeer {
    outgoing_tx: mpsc::UnboundedSender<HostToClient>,
    pending: Mutex<HashMap<DelegateRequestId, oneshot::Sender<Result<DelegateResponse, String>>>>,
    next_request_id: AtomicI64,
    disconnected: CancellationToken,
    cell_routes: StdMutex<HashMap<(SessionId, CellId), CellRoute>>,
}

enum CellRoute {
    Pending(Vec<CellMessage>),
    Active(mpsc::UnboundedSender<CellMessage>),
}

enum CellMessage {
    Delegate {
        id: DelegateRequestId,
        message: HostToClient,
    },
    Closed,
}

impl HostPeer {
    pub(super) fn new(outgoing_tx: mpsc::UnboundedSender<HostToClient>) -> Self {
        Self {
            outgoing_tx,
            pending: Mutex::new(HashMap::new()),
            next_request_id: AtomicI64::new(1),
            disconnected: CancellationToken::new(),
            cell_routes: StdMutex::new(HashMap::new()),
        }
    }

    pub(super) fn send(&self, message: HostToClient) -> bool {
        self.outgoing_tx.send(message).is_ok()
    }

    pub(super) async fn call(
        self: &Arc<Self>,
        session_id: SessionId,
        request: DelegateRequest,
        cancellation_token: CancellationToken,
    ) -> Result<DelegateResponse, String> {
        if self.disconnected.is_cancelled() {
            return Err("code-mode client connection closed".to_string());
        }
        let id = DelegateRequestId::new(self.next_request_id.fetch_add(1, Ordering::Relaxed));
        let (response_tx, response_rx) = oneshot::channel();
        self.pending.lock().await.insert(id, response_tx);
        let mut pending = PendingDelegateRequest::new(Arc::clone(self), id);
        let cell_id = match &request {
            DelegateRequest::InvokeTool { invocation } => invocation.cell_id.clone(),
            DelegateRequest::Notify { cell_id, .. } => cell_id.clone(),
        };
        self.route_cell_message(
            (session_id.clone(), cell_id.into()),
            CellMessage::Delegate {
                id,
                message: HostToClient::DelegateRequest {
                    id,
                    session_id,
                    request,
                },
            },
        );

        tokio::select! {
            response = response_rx => {
                pending.disarm();
                response.map_err(|_| {
                    "code-mode client closed before returning delegate output".to_string()
                })?
            }
            _ = cancellation_token.cancelled() => {
                if self.pending.lock().await.remove(&id).is_some() {
                    self.send(HostToClient::CancelDelegateRequest { id });
                }
                pending.disarm();
                Err("code mode delegate request cancelled".to_string())
            }
            _ = self.disconnected.cancelled() => {
                self.pending.lock().await.remove(&id);
                pending.disarm();
                Err("code-mode client connection closed".to_string())
            }
        }
    }

    pub(super) async fn complete(
        &self,
        id: DelegateRequestId,
        response: Result<DelegateResponse, String>,
    ) {
        if let Some(sender) = self.pending.lock().await.remove(&id) {
            let _ = sender.send(response);
        }
    }

    pub(super) fn disconnect(&self) {
        self.disconnected.cancel();
    }

    pub(super) fn start_cell(
        self: &Arc<Self>,
        session_id: SessionId,
        request_id: RequestId,
        started: StartedCell,
    ) {
        let key = (session_id, started.cell_id.clone());
        let (messages_tx, messages_rx) = mpsc::unbounded_channel();
        let pending = self
            .cell_routes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(key.clone(), CellRoute::Active(messages_tx.clone()));
        if let Some(CellRoute::Pending(messages)) = pending {
            for message in messages {
                let _ = messages_tx.send(message);
            }
        }
        let peer = Arc::clone(self);
        tokio::spawn(async move {
            drive_cell(peer, key, request_id, started, messages_rx).await;
        });
    }

    pub(super) fn close_cell(&self, session_id: SessionId, cell_id: CellId) {
        self.route_cell_message((session_id, cell_id), CellMessage::Closed);
    }

    fn route_cell_message(&self, key: (SessionId, CellId), message: CellMessage) {
        use std::collections::hash_map::Entry;

        match self
            .cell_routes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(key)
        {
            Entry::Occupied(mut entry) => match entry.get_mut() {
                CellRoute::Pending(messages) => messages.push(message),
                CellRoute::Active(sender) => {
                    let _ = sender.send(message);
                }
            },
            Entry::Vacant(entry) => {
                entry.insert(CellRoute::Pending(vec![message]));
            }
        }
    }

    async fn send_delegate_if_pending(&self, id: DelegateRequestId, message: HostToClient) {
        if self.pending.lock().await.contains_key(&id) {
            self.send(message);
        }
    }
}

async fn drive_cell(
    peer: Arc<HostPeer>,
    key: (SessionId, CellId),
    request_id: RequestId,
    started: StartedCell,
    mut messages_rx: mpsc::UnboundedReceiver<CellMessage>,
) {
    let initial_response = started.initial_response();
    tokio::pin!(initial_response);
    let closed = loop {
        tokio::select! {
            biased;
            result = &mut initial_response => {
                peer.send(HostToClient::InitialResponse {
                    id: request_id,
                    result: WireResult::from_result(result.map(Into::into)),
                });
                break false;
            }
            message = messages_rx.recv() => match message {
                Some(CellMessage::Delegate { id, message }) => {
                    peer.send_delegate_if_pending(id, message).await;
                }
                Some(CellMessage::Closed) | None => break true,
            },
            _ = peer.disconnected.cancelled() => return,
        }
    };

    if closed {
        peer.send(HostToClient::InitialResponse {
            id: request_id,
            result: WireResult::from_result(initial_response.await.map(Into::into)),
        });
    } else {
        loop {
            tokio::select! {
                message = messages_rx.recv() => match message {
                    Some(CellMessage::Delegate { id, message }) => {
                        peer.send_delegate_if_pending(id, message).await;
                    }
                    Some(CellMessage::Closed) | None => break,
                },
                _ = peer.disconnected.cancelled() => return,
            }
        }
    }
    peer.send(HostToClient::CellClosed {
        session_id: key.0.clone(),
        cell_id: key.1.clone().into(),
    });
    peer.cell_routes
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&key);
}

struct PendingDelegateRequest {
    peer: Arc<HostPeer>,
    id: Option<DelegateRequestId>,
}

impl PendingDelegateRequest {
    fn new(peer: Arc<HostPeer>, id: DelegateRequestId) -> Self {
        Self { peer, id: Some(id) }
    }

    fn disarm(&mut self) {
        self.id = None;
    }
}

impl Drop for PendingDelegateRequest {
    fn drop(&mut self) {
        let Some(id) = self.id.take() else {
            return;
        };
        let peer = Arc::clone(&self.peer);
        tokio::spawn(async move {
            if peer.pending.lock().await.remove(&id).is_some() {
                peer.send(HostToClient::CancelDelegateRequest { id });
            }
        });
    }
}
