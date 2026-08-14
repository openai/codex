use std::borrow::Borrow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::Weak;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::grpc as proto;
use codex_code_mode_protocol::host::MAX_PENDING_DELEGATE_CALLS;
use codex_code_mode_runtime::InProcessCodeModeSession;
use serde_json::Value as JsonValue;
use tokio::sync::Notify;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::Status;
use uuid::Uuid;

use super::GrpcStream;
use super::delegate::GrpcDelegate;
use super::events::EventSender;
use super::validation;
use super::waits::ActiveWait;
use crate::HostLimits;
use crate::MAX_ACTIVE_CELLS;
use crate::MAX_IN_FLIGHT_REQUESTS;
use crate::MAX_RECENT_REQUEST_IDS;
use crate::OUTGOING_CHANNEL_CAPACITY;

pub(super) struct GrpcHostState {
    sessions: Mutex<HashMap<Uuid, Arc<GrpcSession>>>,
    limits: HostLimits,
    delegate_permits: Arc<Semaphore>,
    control_permits: Arc<Semaphore>,
}

pub(super) struct GrpcSession {
    pub(super) id: Uuid,
    pub(super) runtime: Arc<InProcessCodeModeSession>,
    pub(super) closed: CancellationToken,
    pub(super) state: Mutex<SessionState>,
    events: EventSender,
    cells_changed: Notify,
    delegate_permits: Arc<Semaphore>,
}

#[derive(Default)]
pub(super) struct SessionState {
    pub(super) cells: HashMap<String, ExecutionState>,
    pending_executions: HashSet<String>,
    pending_closures: HashSet<String>,
    seen_executions: BoundedIds,
    pub(super) subscriptions: Vec<ToolSubscription>,
    pub(super) next_subscription: usize,
    pub(super) pending_invocations: HashMap<Uuid, PendingInvocation>,
    pub(super) seen_invocations: BoundedIds<Uuid>,
    pub(super) waits: HashMap<String, ActiveWait>,
    pub(super) seen_waits: BoundedIds,
    pub(super) cancelled_waits: BoundedIds,
}

pub(super) struct ExecutionState {
    pub(super) execution_id: String,
    pub(super) tool_call_sequence: u64,
    permit: OwnedSemaphorePermit,
}

pub(super) struct ToolSubscription {
    pub(super) id: Uuid,
    pub(super) filters: Vec<proto::ToolName>,
    pub(super) sender: mpsc::Sender<Result<proto::ToolCall, Status>>,
}

pub(super) struct PendingInvocation {
    pub(super) subscription_id: Uuid,
    pub(super) response: oneshot::Sender<Result<JsonValue, String>>,
}

#[derive(Default)]
pub(super) struct BoundedIds<T = String> {
    ids: HashSet<T>,
    order: VecDeque<T>,
}

impl GrpcHostState {
    pub(super) fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            limits: HostLimits::new(),
            delegate_permits: Arc::new(Semaphore::new(MAX_PENDING_DELEGATE_CALLS)),
            control_permits: Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS)),
        }
    }

    pub(super) fn open_session(
        self: &Arc<Self>,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> Result<GrpcStream<proto::SessionEvent>, Status> {
        let id = Uuid::new_v4();
        let (events, receiver) = mpsc::channel(OUTGOING_CHANNEL_CAPACITY);
        let closed = CancellationToken::new();
        let event_sender = EventSender::new(events.clone(), closed.clone());
        let session = GrpcSession::new(
            id,
            event_sender,
            closed,
            Arc::clone(&self.delegate_permits),
            limits,
        );
        events
            .try_send(Ok(proto::SessionEvent {
                event: Some(proto::session_event::Event::Opened(proto::SessionOpened {
                    session_id: id.to_string(),
                })),
            }))
            .map_err(|_| Status::internal("failed to publish the opened code-mode session"))?;
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        sessions.insert(id, Arc::clone(&session));
        drop(sessions);

        let host = Arc::downgrade(self);
        tokio::spawn(async move {
            tokio::select! {
                _ = events.closed() => {}
                _ = session.closed.cancelled() => {}
            }
            if let Some(host) = host.upgrade() {
                host.close_lease(id, &session).await;
            } else {
                let _ = session.shutdown().await;
            }
        });

        Ok(Box::pin(ReceiverStream::new(receiver)))
    }

    pub(super) fn session(&self, id: &str) -> Result<Arc<GrpcSession>, Status> {
        let session_id = validation::uuid(id, "session ID")?;
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&session_id)
            .cloned()
            .ok_or_else(|| Status::not_found(format!("unknown code-mode session {id}")))
    }

    pub(super) async fn close_session(&self, id: &str) -> Result<(), Status> {
        let session_id = validation::uuid(id, "session ID")?;
        let session = self
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&session_id)
            .ok_or_else(|| Status::not_found(format!("unknown code-mode session {id}")))?;
        session.shutdown().await
    }

    async fn close_lease(&self, id: Uuid, expected: &Arc<GrpcSession>) {
        let session = {
            let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            if sessions
                .get(&id)
                .is_some_and(|session| Arc::ptr_eq(session, expected))
            {
                sessions.remove(&id)
            } else {
                None
            }
        };
        if let Some(session) = session {
            let _ = session.shutdown().await;
        }
    }

    pub(super) fn request_permit(&self) -> Result<OwnedSemaphorePermit, Status> {
        self.limits.request_permit().map_err(|_| {
            Status::resource_exhausted("code-mode host has too many in-flight requests")
        })
    }

    pub(super) fn cell_permit(&self) -> Result<OwnedSemaphorePermit, Status> {
        self.limits
            .cell_permit()
            .map_err(|_| Status::resource_exhausted("code-mode host has too many active cells"))
    }

    pub(super) fn control_permit(&self) -> Result<OwnedSemaphorePermit, Status> {
        Arc::clone(&self.control_permits)
            .try_acquire_owned()
            .map_err(|_| {
                Status::resource_exhausted("code-mode host has too many in-flight control requests")
            })
    }
}

impl GrpcSession {
    fn new(
        id: Uuid,
        events: EventSender,
        closed: CancellationToken,
        delegate_permits: Arc<Semaphore>,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> Arc<Self> {
        Arc::new_cyclic(|weak: &Weak<Self>| {
            let delegate = Arc::new(GrpcDelegate::new(weak.clone()));
            let failure_session = weak.clone();
            let failure_handler = Arc::new(move |reason: String| {
                if let Some(session) = failure_session.upgrade() {
                    tracing::warn!(session_id = %session.id, "code-mode host session failed: {reason}");
                    session.closed.cancel();
                }
            });
            Self {
                id,
                runtime: Arc::new(
                    InProcessCodeModeSession::with_delegate_and_task_failure_handler(
                        delegate,
                        failure_handler,
                        limits,
                    ),
                ),
                closed,
                state: Mutex::new(SessionState::default()),
                events,
                cells_changed: Notify::new(),
                delegate_permits,
            }
        })
    }

    async fn shutdown(&self) -> Result<(), Status> {
        self.closed.cancel();
        {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            for wait in state.waits.values() {
                wait.cancellation.cancel();
            }
            state.pending_invocations.clear();
            state.subscriptions.clear();
        }
        let result = self.runtime.shutdown().await.map_err(Status::internal);
        self.events.shutdown().await;
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .cells
            .clear();
        result
    }

    pub(super) async fn terminate(&self, cell_id: CellId) -> Result<WaitOutcome, Status> {
        tokio::select! {
            biased;
            _ = self.closed.cancelled() => {
                Err(Status::cancelled("code-mode session is closed"))
            }
            result = self.runtime.terminate(cell_id) => {
                result.map_err(Status::failed_precondition)
            }
        }
    }

    pub(super) fn reserve_execution(&self, execution_id: &str) -> Result<(), Status> {
        validation::identifier(execution_id, "execution ID")?;
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if self.closed.is_cancelled() {
            return Err(Status::cancelled("code-mode session is closed"));
        }
        if state.pending_executions.contains(execution_id)
            || state
                .cells
                .values()
                .any(|execution| execution.execution_id == execution_id)
            || !state.seen_executions.remember(execution_id.to_string())
        {
            return Err(Status::already_exists(format!(
                "code-mode execution ID `{execution_id}` was reused"
            )));
        }
        state.pending_executions.insert(execution_id.to_string());
        Ok(())
    }

    pub(super) fn admit_execution(
        &self,
        execution_id: String,
        cell_id: String,
        permit: OwnedSemaphorePermit,
    ) -> Result<(), Status> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.pending_executions.remove(&execution_id) {
            return Err(Status::cancelled("code-mode execution was abandoned"));
        }
        let Entry::Vacant(entry) = state.cells.entry(cell_id.clone()) else {
            return Err(Status::internal(
                "code-mode runtime reused an active cell ID",
            ));
        };
        entry.insert(ExecutionState {
            execution_id,
            tool_call_sequence: 0,
            permit,
        });
        let closed = state.pending_closures.remove(&cell_id);
        let closed_execution = closed.then(|| state.cells.remove(&cell_id)).flatten();
        drop(state);
        self.cells_changed.notify_waiters();
        if let Some(execution) = closed_execution {
            self.send_cell_closed(&cell_id, execution);
        }
        Ok(())
    }

    pub(super) fn abandon_execution(self: &Arc<Self>, execution_id: &str) {
        let cell_id = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            state.pending_executions.remove(execution_id);
            state
                .cells
                .iter()
                .find(|(_, execution)| execution.execution_id == execution_id)
                .map(|(cell_id, _)| cell_id.clone())
        };
        if let Some(cell_id) = cell_id {
            let session = Arc::clone(self);
            tokio::spawn(async move {
                let _ = session.terminate(CellId::new(cell_id)).await;
            });
        }
    }

    pub(super) async fn execution_id(
        &self,
        cell_id: &str,
        cancellation: &CancellationToken,
    ) -> Result<String, String> {
        loop {
            let changed = self.cells_changed.notified();
            if let Some(execution_id) = self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .cells
                .get(cell_id)
                .map(|execution| execution.execution_id.clone())
            {
                return Ok(execution_id);
            }
            tokio::select! {
                _ = self.closed.cancelled() => {
                    return Err("code-mode session closed before cell admission".to_string());
                }
                _ = cancellation.cancelled() => {
                    return Err("code-mode callback was cancelled before cell admission".to_string());
                }
                _ = changed => {}
            }
        }
    }

    pub(super) fn close_cell(&self, cell_id: &str) {
        let execution = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            match state.cells.remove(cell_id) {
                Some(execution) => Some(execution),
                None if state.pending_closures.len() < MAX_ACTIVE_CELLS => {
                    state.pending_closures.insert(cell_id.to_string());
                    None
                }
                None => {
                    self.closed.cancel();
                    None
                }
            }
        };
        if let Some(execution) = execution {
            self.send_cell_closed(cell_id, execution);
        }
    }

    fn send_cell_closed(&self, cell_id: &str, execution: ExecutionState) {
        let _ = self.send_event_now(
            proto::session_event::Event::CellClosed(proto::CellClosed {
                execution_id: execution.execution_id,
                cell_id: cell_id.to_string(),
                final_tool_call_sequence: execution.tool_call_sequence,
            }),
            Some(execution.permit),
        );
    }

    pub(super) fn delegate_permit(&self) -> Result<OwnedSemaphorePermit, String> {
        Arc::clone(&self.delegate_permits)
            .try_acquire_owned()
            .map_err(|_| "code-mode host has too many pending delegate calls".to_string())
    }

    pub(super) async fn send_event(
        &self,
        event: proto::session_event::Event,
        cancellation: &CancellationToken,
    ) -> Result<(), String> {
        self.events.send(event, cancellation).await
    }

    pub(super) fn send_event_now(
        &self,
        event: proto::session_event::Event,
        cell_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<(), String> {
        self.events.send_now(event, cell_permit)
    }
}

impl<T> BoundedIds<T>
where
    T: Clone + Eq + Hash,
{
    pub(super) fn remember(&mut self, id: T) -> bool {
        if !self.ids.insert(id.clone()) {
            return false;
        }
        self.order.push_back(id);
        while self.order.len() > MAX_RECENT_REQUEST_IDS {
            if let Some(expired) = self.order.pop_front() {
                self.ids.remove(&expired);
            }
        }
        true
    }

    pub(super) fn contains<Q>(&self, id: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.ids.contains(id)
    }

    pub(super) fn remove<Q>(&mut self, id: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.ids.remove(id)
    }
}
