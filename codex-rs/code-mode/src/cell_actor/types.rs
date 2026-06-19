use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
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
/// Implementations should forward callback cancellation to downstream work.
/// The actor stops awaiting callbacks once cancellation begins. Implementations
/// must not return from `closed` until the session can no longer route requests
/// to the cell.
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
        lifecycle: Arc<CellLifecycle>,
    ) -> impl Future<Output = bool> + Send;

    fn closed(&self) -> impl Future<Output = ()> + Send;
}

#[derive(Clone)]
pub(crate) struct CellHandle {
    command_tx: mpsc::UnboundedSender<CellCommand>,
    lifecycle: Arc<CellLifecycle>,
    terminal_event_rx: watch::Receiver<Option<CellEvent>>,
}

impl CellHandle {
    pub(super) fn new(
        command_tx: mpsc::UnboundedSender<CellCommand>,
        lifecycle: Arc<CellLifecycle>,
        terminal_event_rx: watch::Receiver<Option<CellEvent>>,
    ) -> Self {
        Self {
            command_tx,
            lifecycle,
            terminal_event_rx,
        }
    }

    pub(crate) fn observe(&self, mode: ObserveMode) -> CellEventFuture {
        if !self.lifecycle.accepting_requests() {
            return closed_event();
        }
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
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
        match self.lifecycle.request_termination() {
            TerminationRequest::Terminate => {}
            TerminationRequest::ConsumeCompletion => {}
            TerminationRequest::AlreadyTerminating => {
                return Box::pin(async { Err(CellError::AlreadyTerminating) });
            }
            TerminationRequest::Closed => return closed_event(),
        }
        if self.command_tx.send(CellCommand::Terminate).is_err() {
            self.lifecycle.close();
            return closed_event();
        }
        terminal_event(self.terminal_event_rx.clone())
    }
}

pub(crate) struct CellLifecycle {
    state: Mutex<CellLifecycleState>,
    session_shutdown_token: CancellationToken,
    termination_token: CancellationToken,
    work_cancellation_token: CancellationToken,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CellLifecycleState {
    Running,
    TerminationRequested,
    CompletionCommitted,
    CompletionBuffered,
    CompletionConsumptionRequested,
    Closed,
}

enum TerminationRequest {
    Terminate,
    ConsumeCompletion,
    AlreadyTerminating,
    Closed,
}

impl CellLifecycle {
    pub(crate) fn new(session_shutdown_token: CancellationToken) -> Self {
        let work_cancellation_token = session_shutdown_token.child_token();
        Self {
            state: Mutex::new(CellLifecycleState::Running),
            session_shutdown_token,
            termination_token: CancellationToken::new(),
            work_cancellation_token,
        }
    }

    pub(crate) fn accepting_requests(&self) -> bool {
        let accepting_state = matches!(
            *self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            CellLifecycleState::Running
                | CellLifecycleState::CompletionCommitted
                | CellLifecycleState::CompletionBuffered
        );
        accepting_state && !self.session_shutdown_token.is_cancelled()
    }

    fn request_termination(&self) -> TerminationRequest {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match *state {
            CellLifecycleState::Running => {
                *state = CellLifecycleState::TerminationRequested;
                self.termination_token.cancel();
                self.work_cancellation_token.cancel();
                TerminationRequest::Terminate
            }
            CellLifecycleState::TerminationRequested => TerminationRequest::AlreadyTerminating,
            CellLifecycleState::CompletionCommitted | CellLifecycleState::CompletionBuffered => {
                *state = CellLifecycleState::CompletionConsumptionRequested;
                TerminationRequest::ConsumeCompletion
            }
            CellLifecycleState::CompletionConsumptionRequested => {
                TerminationRequest::AlreadyTerminating
            }
            CellLifecycleState::Closed => TerminationRequest::Closed,
        }
    }

    pub(crate) fn commit_completion(&self, commit: impl FnOnce()) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *state != CellLifecycleState::Running || self.session_shutdown_token.is_cancelled() {
            return false;
        }
        commit();
        *state = CellLifecycleState::CompletionCommitted;
        true
    }

    pub(crate) fn buffer_completion(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *state == CellLifecycleState::CompletionCommitted {
            *state = CellLifecycleState::CompletionBuffered;
        }
    }

    pub(crate) fn close(&self) {
        *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = CellLifecycleState::Closed;
    }

    pub(crate) fn session_shutdown_token(&self) -> CancellationToken {
        self.session_shutdown_token.clone()
    }

    pub(crate) fn termination_token(&self) -> CancellationToken {
        self.termination_token.clone()
    }

    pub(crate) fn work_cancellation_token(&self) -> CancellationToken {
        self.work_cancellation_token.clone()
    }
}

pub(super) enum CellCommand {
    Observe {
        mode: ObserveMode,
        response_tx: oneshot::Sender<Result<CellEvent, CellError>>,
    },
    Terminate,
}

fn response_event(
    response_rx: tokio::sync::oneshot::Receiver<Result<CellEvent, CellError>>,
) -> CellEventFuture {
    Box::pin(async move { response_rx.await.unwrap_or(Err(CellError::Closed)) })
}

fn terminal_event(mut terminal_event_rx: watch::Receiver<Option<CellEvent>>) -> CellEventFuture {
    Box::pin(async move {
        loop {
            if let Some(event) = terminal_event_rx.borrow_and_update().clone() {
                return Ok(event);
            }
            if terminal_event_rx.changed().await.is_err() {
                return Err(CellError::Closed);
            }
        }
    })
}

fn closed_event() -> CellEventFuture {
    Box::pin(async { Err(CellError::Closed) })
}
