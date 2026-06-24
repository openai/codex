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

/// Multiplexes host-to-client traffic for all sessions on one connection.
///
/// It correlates reverse delegate calls with their responses and routes each
/// cell's initial response, delegate requests, and closure notification through
/// a single cell driver.
pub(super) struct HostPeer {
    outgoing_tx: mpsc::UnboundedSender<HostToClient>,
    pending: Mutex<HashMap<DelegateRequestId, oneshot::Sender<Result<DelegateResponse, String>>>>,
    next_request_id: AtomicI64,
    disconnected: CancellationToken,
    cell_routes: StdMutex<HashMap<(SessionId, CellId), mpsc::UnboundedSender<CellMessage>>>,
}

/// Registers a cell's message channel before its runtime task can emit callbacks.
///
/// Dropping the registration removes the route, including when runtime admission
/// fails before the cell driver starts.
pub(super) struct CellRegistration {
    peer: Arc<HostPeer>,
    key: (SessionId, CellId),
    messages_rx: mpsc::UnboundedReceiver<CellMessage>,
}

/// An event emitted by a hosted session for its per-cell driver to serialize.
enum CellMessage {
    Delegate {
        id: DelegateRequestId,
        request: DelegateRequest,
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
        if !self.route_cell_message(
            (session_id.clone(), cell_id.clone().into()),
            CellMessage::Delegate { id, request },
        ) {
            self.pending.lock().await.remove(&id);
            pending.disarm();
            return Err(format!(
                "code-mode cell {} is not registered for session {session_id}",
                cell_id.as_str()
            ));
        }

        tokio::select! {
            response = response_rx => {
                pending.disarm();
                response.map_err(|_| {
                    "code-mode client closed before returning delegate output".to_string()
                })?
            }
            _ = cancellation_token.cancelled() => {
                let mut pending_calls = self.pending.lock().await;
                if pending_calls.remove(&id).is_some() {
                    self.send(HostToClient::CancelDelegateRequest { id });
                }
                drop(pending_calls);
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

    pub(super) fn register_cell(
        self: &Arc<Self>,
        session_id: SessionId,
        cell_id: CellId,
    ) -> Result<CellRegistration, String> {
        use std::collections::hash_map::Entry;

        let key = (session_id, cell_id);
        let (messages_tx, messages_rx) = mpsc::unbounded_channel();
        match self
            .cell_routes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(key.clone())
        {
            Entry::Occupied(_) => {
                return Err(format!(
                    "code-mode cell {} is already registered for session {}",
                    key.1, key.0
                ));
            }
            Entry::Vacant(entry) => {
                entry.insert(messages_tx);
            }
        }
        Ok(CellRegistration {
            peer: Arc::clone(self),
            key,
            messages_rx,
        })
    }

    pub(super) fn start_cell(
        self: &Arc<Self>,
        registration: CellRegistration,
        request_id: RequestId,
        started: StartedCell,
    ) {
        debug_assert!(Arc::ptr_eq(self, &registration.peer));
        debug_assert_eq!(registration.key.1, started.cell_id);
        tokio::spawn(async move {
            drive_cell(registration, request_id, started).await;
        });
    }

    pub(super) fn close_cell(&self, session_id: SessionId, cell_id: CellId) {
        let _ = self.route_cell_message((session_id, cell_id), CellMessage::Closed);
    }

    fn route_cell_message(&self, key: (SessionId, CellId), message: CellMessage) -> bool {
        let routes = self
            .cell_routes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(sender) = routes.get(&key) else {
            return false;
        };
        sender.send(message).is_ok()
    }

    async fn send_delegate_if_pending(
        &self,
        id: DelegateRequestId,
        session_id: SessionId,
        request: DelegateRequest,
    ) {
        // Keep the pending entry locked through enqueueing the request so a
        // concurrent cancellation can only be sent after it.
        let pending_calls = self.pending.lock().await;
        if !pending_calls.contains_key(&id) {
            return;
        }
        self.send(HostToClient::DelegateRequest {
            id,
            session_id,
            request,
        });
        drop(pending_calls);
    }
}

async fn drive_cell(
    mut registration: CellRegistration,
    request_id: RequestId,
    started: StartedCell,
) {
    let peer = Arc::clone(&registration.peer);
    let key = registration.key.clone();
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
            message = registration.messages_rx.recv() => match message {
                Some(CellMessage::Delegate { id, request }) => {
                    peer.send_delegate_if_pending(id, key.0.clone(), request).await;
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
                message = registration.messages_rx.recv() => match message {
                    Some(CellMessage::Delegate { id, request }) => {
                        peer.send_delegate_if_pending(id, key.0.clone(), request).await;
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
}

impl Drop for CellRegistration {
    fn drop(&mut self) {
        self.peer
            .cell_routes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
    }
}

/// Cancels a reverse delegate request if its waiting future is dropped.
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
            let mut pending_calls = peer.pending.lock().await;
            if pending_calls.remove(&id).is_some() {
                peer.send(HostToClient::CancelDelegateRequest { id });
            }
        });
    }
}

#[cfg(test)]
#[path = "peer_tests.rs"]
mod tests;
