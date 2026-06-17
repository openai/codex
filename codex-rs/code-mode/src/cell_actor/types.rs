use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

pub(crate) type CellEventFuture =
    Pin<Box<dyn Future<Output = Result<CellEvent, CellError>> + Send + 'static>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObserveMode {
    YieldAfter(Duration),
    PendingFrontier,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CellEvent {
    Yielded {
        content_items: Vec<CellOutputItem>,
    },
    Pending {
        content_items: Vec<CellOutputItem>,
        pending_tool_call_ids: Vec<String>,
    },
    Completed {
        content_items: Vec<CellOutputItem>,
        error_text: Option<String>,
    },
    Terminated {
        content_items: Vec<CellOutputItem>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CellError {
    Busy,
    AlreadyTerminating,
    Closed,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CellOutputItem {
    Text {
        text: String,
    },
    Image {
        image_url: String,
        detail: Option<CellImageDetail>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CellImageDetail {
    Auto,
    Low,
    High,
    Original,
}

pub(crate) struct CellRequest {
    pub(crate) tool_call_id: String,
    pub(crate) enabled_tools: Vec<CellToolDefinition>,
    pub(crate) source: String,
}

pub(crate) struct CellToolDefinition {
    pub(crate) name: String,
    pub(crate) tool_name: CellToolName,
    pub(crate) description: String,
    pub(crate) kind: CellToolKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CellToolName {
    pub(crate) name: String,
    pub(crate) namespace: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CellToolKind {
    Function,
    Freeform,
}

pub(crate) struct CellToolCall {
    pub(crate) id: String,
    pub(crate) name: CellToolName,
    pub(crate) kind: CellToolKind,
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
