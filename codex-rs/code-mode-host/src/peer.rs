use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::wire::CallbackId;
use codex_code_mode_protocol::wire::CallbackRequest;
use codex_code_mode_protocol::wire::CallbackResponse;
use codex_code_mode_protocol::wire::HostMessage;
use codex_code_mode_protocol::wire::SessionId;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub(super) struct HostPeer {
    outgoing_tx: mpsc::Sender<HostMessage>,
    pending_callbacks: Mutex<HashMap<CallbackId, oneshot::Sender<CallbackResponse>>>,
    next_callback_id: AtomicU64,
    connection_closed: CancellationToken,
}

impl HostPeer {
    pub(super) fn new(outgoing_tx: mpsc::Sender<HostMessage>) -> Self {
        Self {
            outgoing_tx,
            pending_callbacks: Mutex::new(HashMap::new()),
            next_callback_id: AtomicU64::new(1),
            connection_closed: CancellationToken::new(),
        }
    }

    pub(super) async fn send(&self, message: HostMessage) -> bool {
        tokio::select! {
            sent = self.outgoing_tx.send(message) => sent.is_ok(),
            _ = self.connection_closed.cancelled() => false,
        }
    }

    pub(super) fn send_nowait(self: &Arc<Self>, message: HostMessage) {
        match self.outgoing_tx.try_send(message) {
            Ok(()) | Err(mpsc::error::TrySendError::Closed(_)) => {}
            Err(mpsc::error::TrySendError::Full(message)) => {
                let peer = Arc::clone(self);
                tokio::spawn(async move {
                    peer.send(message).await;
                });
            }
        }
    }

    pub(super) async fn call(
        self: &Arc<Self>,
        session_id: SessionId,
        request: CallbackRequest,
        cancellation_token: CancellationToken,
    ) -> Result<CallbackResponse, String> {
        if self.connection_closed.is_cancelled() {
            return Err("code-mode client connection closed".to_string());
        }
        let id = self.next_callback_id.fetch_add(1, Ordering::Relaxed);
        let (response_tx, response_rx) = oneshot::channel();
        self.pending_callbacks.lock().await.insert(id, response_tx);
        let mut pending_callback = PendingCallback::new(Arc::clone(self), id);

        let request_sent = tokio::select! {
            sent = self.outgoing_tx.send(HostMessage::CallbackRequest {
                id,
                session_id,
                request,
            }) => sent.is_ok(),
            _ = cancellation_token.cancelled() => false,
            _ = self.connection_closed.cancelled() => false,
        };
        if !request_sent {
            self.pending_callbacks.lock().await.remove(&id);
            pending_callback.disarm();
            return Err(if cancellation_token.is_cancelled() {
                "code mode callback cancelled".to_string()
            } else {
                "code-mode client connection closed".to_string()
            });
        }

        tokio::select! {
            response = response_rx => {
                pending_callback.disarm();
                response.map_err(|_| {
                    "code-mode client closed before returning callback output".to_string()
                })
            },
            _ = cancellation_token.cancelled() => {
                if self.pending_callbacks.lock().await.remove(&id).is_some() {
                    self.send(HostMessage::CancelCallback { id }).await;
                }
                pending_callback.disarm();
                Err("code mode callback cancelled".to_string())
            }
            _ = self.connection_closed.cancelled() => {
                self.pending_callbacks.lock().await.remove(&id);
                pending_callback.disarm();
                Err("code-mode client connection closed".to_string())
            }
        }
    }

    pub(super) async fn complete(&self, id: CallbackId, response: CallbackResponse) {
        if let Some(response_tx) = self.pending_callbacks.lock().await.remove(&id) {
            let _ = response_tx.send(response);
        }
    }

    pub(super) async fn disconnect(&self) {
        self.connection_closed.cancel();
        self.pending_callbacks.lock().await.clear();
    }
}

struct PendingCallback {
    peer: Arc<HostPeer>,
    id: Option<CallbackId>,
}

impl PendingCallback {
    fn new(peer: Arc<HostPeer>, id: CallbackId) -> Self {
        Self { peer, id: Some(id) }
    }

    fn disarm(&mut self) {
        self.id = None;
    }
}

impl Drop for PendingCallback {
    fn drop(&mut self) {
        let Some(id) = self.id.take() else {
            return;
        };
        let peer = Arc::clone(&self.peer);
        tokio::spawn(async move {
            if peer.pending_callbacks.lock().await.remove(&id).is_some() {
                peer.send(HostMessage::CancelCallback { id }).await;
            }
        });
    }
}
