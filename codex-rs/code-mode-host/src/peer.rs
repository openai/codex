use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::host::DelegateRequest;
use codex_code_mode_protocol::host::DelegateRequestId;
use codex_code_mode_protocol::host::DelegateResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::SessionId;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub(super) struct HostPeer {
    outgoing_tx: mpsc::UnboundedSender<HostToClient>,
    pending: Mutex<HashMap<DelegateRequestId, oneshot::Sender<Result<DelegateResponse, String>>>>,
    next_request_id: AtomicI64,
    disconnected: CancellationToken,
}

impl HostPeer {
    pub(super) fn new(outgoing_tx: mpsc::UnboundedSender<HostToClient>) -> Self {
        Self {
            outgoing_tx,
            pending: Mutex::new(HashMap::new()),
            next_request_id: AtomicI64::new(1),
            disconnected: CancellationToken::new(),
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
        if !self.send(HostToClient::DelegateRequest {
            id,
            session_id,
            request,
        }) {
            self.pending.lock().await.remove(&id);
            pending.disarm();
            return Err("code-mode client connection closed".to_string());
        }

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
