use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

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

/// Connects a cell actor to session-owned callbacks and stored values.
///
/// Implementations should forward callback cancellation to downstream work.
/// Implementations must not return from `closed` until the session can no longer
/// route requests to the cell.
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

    fn commit_completion(
        &self,
        stored_value_writes: HashMap<String, JsonValue>,
        event: CellEvent,
        cell_state: Arc<CellState>,
    ) -> impl Future<Output = CompletionCommit> + Send;

    fn closed(&self) -> impl Future<Output = ()> + Send;
}

#[derive(Clone)]
pub(crate) struct CellHandle {
    command_tx: mpsc::UnboundedSender<CellCommand>,
    state: Arc<CellState>,
}

impl CellHandle {
    pub(super) fn new(
        command_tx: mpsc::UnboundedSender<CellCommand>,
        state: Arc<CellState>,
    ) -> Self {
        Self { command_tx, state }
    }

    pub(crate) fn observe(&self, mode: ObserveMode) -> CellEventFuture {
        if !self.state.accepting_observations() {
            return closed_event();
        }
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
        self.state.request_termination()
    }

    pub(crate) fn shutdown(&self) {
        self.state.cancellation_token().cancel();
    }
}

/// The single linearization point for a cell's terminal outcome.
///
/// Callback cancellation tokens are children of the cell token, so a terminal
/// decision cancels runtime work and its callbacks together.
///
/// The mutex is held only for synchronous phase transitions and terminal
/// delivery. Runtime execution, observation waits, and callbacks never run
/// while it is held.
pub(crate) struct CellState {
    phase: Mutex<CellPhase>,
    cancellation_token: CancellationToken,
}

enum CellPhase {
    Running,
    Terminating {
        response_tx: oneshot::Sender<Result<CellEvent, CellError>>,
    },
    Completed(CellEvent),
    CompletionClaimed(CellEvent),
    Tombstone,
}

pub(crate) enum CompletionDelivery {
    Delivered,
    Buffered,
    Rejected(Option<oneshot::Sender<Result<CellEvent, CellError>>>),
}

/// Result of atomically publishing a completed cell and its session side effects.
#[derive(Debug, PartialEq)]
pub(crate) enum CompletionCommit {
    Committed,
    Rejected(CellEvent),
}

pub(crate) enum ObservationDelivery {
    Running(oneshot::Sender<Result<CellEvent, CellError>>),
    Delivered,
    Buffered,
    Closed,
}

impl CellState {
    pub(crate) fn new(cancellation_token: CancellationToken) -> Self {
        Self {
            phase: Mutex::new(CellPhase::Running),
            cancellation_token,
        }
    }

    pub(crate) fn accepting_observations(&self) -> bool {
        let accepting_phase = matches!(
            *self
                .phase
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            CellPhase::Running | CellPhase::Completed(_)
        );
        accepting_phase && !self.cancellation_token.is_cancelled()
    }

    pub(crate) fn request_termination(&self) -> CellEventFuture {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match std::mem::replace(&mut *phase, CellPhase::Tombstone) {
            CellPhase::Running => {
                let (response_tx, response_rx) = oneshot::channel();
                *phase = CellPhase::Terminating { response_tx };
                self.cancellation_token.cancel();
                response_event(response_rx)
            }
            CellPhase::Terminating { response_tx } => {
                *phase = CellPhase::Terminating { response_tx };
                Box::pin(async { Err(CellError::AlreadyTerminating) })
            }
            CellPhase::Completed(event) => {
                *phase = CellPhase::CompletionClaimed(event.clone());
                self.cancellation_token.cancel();
                ready_event(event)
            }
            CellPhase::CompletionClaimed(event) => {
                *phase = CellPhase::CompletionClaimed(event);
                Box::pin(async { Err(CellError::AlreadyTerminating) })
            }
            CellPhase::Tombstone => closed_event(),
        }
    }

    pub(crate) fn commit_completion(
        &self,
        event: CellEvent,
        commit: impl FnOnce(),
    ) -> CompletionCommit {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !matches!(*phase, CellPhase::Running) || self.cancellation_token.is_cancelled() {
            return CompletionCommit::Rejected(event);
        }
        commit();
        *phase = CellPhase::Completed(event);
        CompletionCommit::Committed
    }

    pub(crate) fn deliver_completion(
        &self,
        response_tx: Option<oneshot::Sender<Result<CellEvent, CellError>>>,
    ) -> CompletionDelivery {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let event = match std::mem::replace(&mut *phase, CellPhase::Tombstone) {
            CellPhase::Completed(event) => event,
            previous => {
                *phase = previous;
                return CompletionDelivery::Rejected(response_tx);
            }
        };
        let Some(response_tx) = response_tx else {
            *phase = CellPhase::Completed(event);
            return CompletionDelivery::Buffered;
        };
        match response_tx.send(Ok(event)) {
            Ok(()) => {
                self.cancellation_token.cancel();
                CompletionDelivery::Delivered
            }
            Err(Ok(event)) => {
                *phase = CellPhase::Completed(event);
                CompletionDelivery::Buffered
            }
            Err(Err(error)) => {
                panic!("completion delivery unexpectedly carried an actor error: {error:?}")
            }
        }
    }

    pub(crate) fn route_observation(
        &self,
        response_tx: oneshot::Sender<Result<CellEvent, CellError>>,
    ) -> ObservationDelivery {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match std::mem::replace(&mut *phase, CellPhase::Tombstone) {
            CellPhase::Running => {
                *phase = CellPhase::Running;
                ObservationDelivery::Running(response_tx)
            }
            CellPhase::Completed(event) => match response_tx.send(Ok(event)) {
                Ok(()) => {
                    self.cancellation_token.cancel();
                    ObservationDelivery::Delivered
                }
                Err(Ok(event)) => {
                    *phase = CellPhase::Completed(event);
                    ObservationDelivery::Buffered
                }
                Err(Err(error)) => {
                    panic!("completion delivery unexpectedly carried an actor error: {error:?}")
                }
            },
            CellPhase::Terminating {
                response_tx: termination_tx,
            } => {
                *phase = CellPhase::Terminating {
                    response_tx: termination_tx,
                };
                let _ = response_tx.send(Err(CellError::Closed));
                ObservationDelivery::Closed
            }
            CellPhase::CompletionClaimed(event) => {
                *phase = CellPhase::CompletionClaimed(event);
                let _ = response_tx.send(Err(CellError::Closed));
                ObservationDelivery::Closed
            }
            CellPhase::Tombstone => {
                let _ = response_tx.send(Err(CellError::Closed));
                ObservationDelivery::Closed
            }
        }
    }

    pub(crate) fn finish_termination(&self, event: CellEvent) -> Option<CellEvent> {
        let mut phase = self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let observer_event = match std::mem::replace(&mut *phase, CellPhase::Tombstone) {
            CellPhase::Running => Some(event),
            CellPhase::Terminating { response_tx } => {
                let _ = response_tx.send(Ok(event.clone()));
                Some(event)
            }
            CellPhase::Completed(completed_event) => Some(completed_event),
            CellPhase::CompletionClaimed(completed_event) => Some(completed_event),
            CellPhase::Tombstone => None,
        };
        self.cancellation_token.cancel();
        observer_event
    }

    pub(crate) fn tombstone(&self) {
        *self
            .phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = CellPhase::Tombstone;
        self.cancellation_token.cancel();
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }
}

pub(super) enum CellCommand {
    Observe {
        mode: ObserveMode,
        response_tx: oneshot::Sender<Result<CellEvent, CellError>>,
    },
}

fn response_event(response_rx: oneshot::Receiver<Result<CellEvent, CellError>>) -> CellEventFuture {
    Box::pin(async move { response_rx.await.unwrap_or(Err(CellError::Closed)) })
}

fn ready_event(event: CellEvent) -> CellEventFuture {
    Box::pin(async move { Ok(event) })
}

fn closed_event() -> CellEventFuture {
    Box::pin(async { Err(CellError::Closed) })
}
