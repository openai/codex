use std::sync::Arc;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::host::DelegateRequestId;
use codex_code_mode_protocol::host::DelegateResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::WireCellId;
use codex_code_mode_protocol::host::WireWaitRequest;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::cleanup::SessionCleanup;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::remote_session) struct RemoteSession {
    pub(in crate::remote_session) id: SessionId,
    pub(in crate::remote_session) generation: u64,
}

pub(in crate::remote_session::connection) enum DriverCommand {
    OpenSession {
        session: RemoteSession,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        cleanup: SessionCleanup,
        caller_cancellation: CancellationToken,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    Execute {
        session: RemoteSession,
        request: ExecuteRequest,
        caller_cancellation: CancellationToken,
        response_tx: oneshot::Sender<Result<DeliveredExecute, String>>,
    },
    Wait {
        session: RemoteSession,
        request: WaitRequest,
        caller_cancellation: CancellationToken,
        response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
    },
    Terminate {
        session: RemoteSession,
        cell_id: CellId,
        response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
    },
    ShutdownSession {
        session: RemoteSession,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}

pub(in crate::remote_session::connection) enum DriverEvent {
    HostMessage(HostToClient),
    DelegateCompleted {
        id: DelegateRequestId,
        result: Result<DelegateResponse, String>,
    },
    RequestCancelled(RequestId),
    Failed(String),
}

pub(super) struct CancellableRequest {
    caller_cancellation: CancellationToken,
    watcher_stop: CancellationToken,
    reported: bool,
}

impl CancellableRequest {
    pub(super) fn new(caller_cancellation: CancellationToken) -> Self {
        Self {
            caller_cancellation,
            watcher_stop: CancellationToken::new(),
            reported: false,
        }
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.caller_cancellation.is_cancelled()
    }

    pub(super) fn mark_reported(&mut self) -> bool {
        if self.reported {
            return false;
        }
        self.reported = true;
        true
    }

    pub(super) fn spawn_watcher(&self, id: RequestId, event_tx: mpsc::Sender<DriverEvent>) {
        let caller_cancellation = self.caller_cancellation.clone();
        let watcher_stop = self.watcher_stop.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = caller_cancellation.cancelled() => {
                    let _ = event_tx.send(DriverEvent::RequestCancelled(id)).await;
                }
                _ = watcher_stop.cancelled() => {}
            }
        });
    }
}

impl Drop for CancellableRequest {
    fn drop(&mut self) {
        self.watcher_stop.cancel();
    }
}

pub(super) struct InitialResponse {
    pub(super) generation: u64,
    pub(super) cell_id: WireCellId,
    pub(super) response_tx: oneshot::Sender<Result<RuntimeResponse, String>>,
}

pub(in crate::remote_session::connection) struct DeliveredExecute {
    pub(in crate::remote_session::connection) request_id: RequestId,
    pub(in crate::remote_session::connection) started: StartedCell,
}

pub(super) struct UnclaimedExecute {
    pub(super) session: RemoteSession,
    pub(super) cell_id: WireCellId,
    pub(super) cancellation: CancellableRequest,
}

pub(super) enum PendingRequest {
    OpenSession {
        session: RemoteSession,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        cleanup: SessionCleanup,
        cancellation: CancellableRequest,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    Execute {
        session: RemoteSession,
        response_tx: oneshot::Sender<Result<DeliveredExecute, String>>,
        initial_response_tx: oneshot::Sender<Result<RuntimeResponse, String>>,
        initial_response_rx: oneshot::Receiver<Result<RuntimeResponse, String>>,
        cancellation: CancellableRequest,
    },
    Wait {
        session: RemoteSession,
        cell_id: WireCellId,
        cancellation: CancellableRequest,
        response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
    },
    Terminate {
        session: RemoteSession,
        cell_id: WireCellId,
        response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
    },
    ShutdownSession {
        session: RemoteSession,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
}

pub(super) struct DeferredWait {
    pub(super) session: RemoteSession,
    pub(super) request: WireWaitRequest,
    pub(super) caller_cancellation: CancellationToken,
    pub(super) response_tx: oneshot::Sender<Result<WaitOutcome, String>>,
}

impl PendingRequest {
    pub(super) fn cancellation_mut(&mut self) -> Option<&mut CancellableRequest> {
        match self {
            Self::OpenSession { cancellation, .. }
            | Self::Execute { cancellation, .. }
            | Self::Wait { cancellation, .. } => Some(cancellation),
            Self::Terminate { .. } | Self::ShutdownSession { .. } => None,
        }
    }

    pub(super) fn fail(self, reason: String) {
        match self {
            Self::OpenSession { response_tx, .. } | Self::ShutdownSession { response_tx, .. } => {
                let _ = response_tx.send(Err(reason));
            }
            Self::Execute { response_tx, .. } => {
                let _ = response_tx.send(Err(reason));
            }
            Self::Wait { response_tx, .. } | Self::Terminate { response_tx, .. } => {
                let _ = response_tx.send(Err(reason));
            }
        }
    }
}
