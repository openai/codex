use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::session_runtime::CellEvent;
use crate::session_runtime::ObserveMode;
use crate::session_runtime::ToolKind;
use crate::session_runtime::ToolName;

pub(crate) type CellEventFuture =
    Pin<Box<dyn Future<Output = Result<CellEvent, CellError>> + Send + 'static>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CellError {
    Busy,
    AlreadyTerminating,
    Closed,
}

pub(crate) struct CellToolCall {
    pub(crate) id: String,
    pub(crate) name: ToolName,
    pub(crate) kind: ToolKind,
    pub(crate) input: Option<JsonValue>,
}

/// Connects a cell actor to session-owned callbacks and lifecycle state.
///
/// Implementations must honor callback cancellation and must not return from
/// `closed` until the session can no longer route requests to the cell.
pub(crate) trait CellHost: Send + Sync + 'static {
    fn invoke_tool(
        &self,
        invocation: CellToolCall,
        cancellation_token: CancellationToken,
    ) -> impl Future<Output = Result<JsonValue, String>> + Send;

    fn notify(
        &self,
        call_id: String,
        text: String,
        cancellation_token: CancellationToken,
    ) -> impl Future<Output = Result<(), String>> + Send;

    fn commit_stored_values(
        &self,
        stored_value_writes: HashMap<String, JsonValue>,
    ) -> impl Future<Output = ()> + Send;

    fn closed(&self) -> impl Future<Output = ()> + Send;
}

#[derive(Clone)]
pub(crate) struct CellHandle {
    command_tx: mpsc::UnboundedSender<CellCommand>,
    cancellation_token: CancellationToken,
    termination_requested: Arc<AtomicBool>,
}

impl CellHandle {
    pub(super) fn new(
        command_tx: mpsc::UnboundedSender<CellCommand>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            command_tx,
            cancellation_token,
            termination_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn observe(&self, mode: ObserveMode) -> CellEventFuture {
        let (response_tx, response_rx) = oneshot::channel();
        if self
            .command_tx
            .send(CellCommand::Observe { mode, response_tx })
            .is_err()
        {
            return closed_event();
        }
        response_event(response_rx)
    }

    pub(crate) fn terminate(&self) -> CellEventFuture {
        if self
            .termination_requested
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            return Box::pin(async { Err(CellError::AlreadyTerminating) });
        }
        let (response_tx, response_rx) = oneshot::channel();
        if self
            .command_tx
            .send(CellCommand::Terminate {
                response_tx: Some(response_tx),
            })
            .is_err()
        {
            self.termination_requested.store(false, Ordering::Relaxed);
            return closed_event();
        }
        response_event(response_rx)
    }

    pub(crate) fn shutdown(&self) {
        self.termination_requested.store(true, Ordering::Relaxed);
        self.cancellation_token.cancel();
        let _ = self
            .command_tx
            .send(CellCommand::Terminate { response_tx: None });
    }
}

pub(super) enum CellCommand {
    Observe {
        mode: ObserveMode,
        response_tx: oneshot::Sender<Result<CellEvent, CellError>>,
    },
    Terminate {
        response_tx: Option<oneshot::Sender<Result<CellEvent, CellError>>>,
    },
}

fn response_event(response_rx: oneshot::Receiver<Result<CellEvent, CellError>>) -> CellEventFuture {
    Box::pin(async move { response_rx.await.unwrap_or(Err(CellError::Closed)) })
}

fn closed_event() -> CellEventFuture {
    Box::pin(async { Err(CellError::Closed) })
}
