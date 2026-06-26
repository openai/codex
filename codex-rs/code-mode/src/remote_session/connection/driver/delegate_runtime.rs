//! Client-side delegate task and closure lifecycle.
//!
//! Cancellation suppresses the wire response but deliberately retains the task. Cell closure is
//! emitted only after every delegate task for that cell exits. On connection failure, the last
//! delegate guard synchronously emits pending closure callbacks and completes connection cleanup.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::DelegateRequest;
use codex_code_mode_protocol::host::DelegateRequestId;
use codex_code_mode_protocol::host::DelegateResponse;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::WireCellId;
use codex_code_mode_protocol::host::WireResult;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::ConnectionDriver;
use super::cleanup::ConnectionCleanup;
use super::notify_cell_closed;
use super::session_registry::CellOwner;
use super::session_registry::DelegateTarget;
use super::types::DeferredShutdown;
use super::types::DriverEvent;

const MAX_RECENT_DELEGATE_REQUEST_IDS: usize = 4096;

#[derive(Clone, Eq, Hash, PartialEq)]
struct CellKey {
    session_id: codex_code_mode_protocol::host::SessionId,
    cell_id: codex_code_mode_protocol::CellId,
}

impl CellKey {
    fn for_owner(owner: &CellOwner) -> Self {
        Self {
            session_id: owner.session_id.clone(),
            cell_id: owner.cell_id.clone(),
        }
    }
}

struct DelegateCall {
    cell: CellKey,
    cancellation: CancellationToken,
    response_suppressed: bool,
}

enum DelegateTask {
    InvokeTool(CodeModeNestedToolCall),
    Notify {
        call_id: String,
        cell_id: codex_code_mode_protocol::CellId,
        text: String,
    },
}

pub(super) struct DelegateEffects {
    pub(super) response: Option<(DelegateRequestId, Result<DelegateResponse, String>)>,
    pub(super) closed_cells: Vec<CellOwner>,
    pub(super) shutdowns: Vec<DeferredShutdown>,
}

impl DelegateEffects {
    fn empty() -> Self {
        Self {
            response: None,
            closed_cells: Vec::new(),
            shutdowns: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        debug_assert!(self.response.is_none());
        self.response = other.response.take();
        self.closed_cells.append(&mut other.closed_cells);
        self.shutdowns.append(&mut other.shutdowns);
    }
}

pub(super) struct DelegateRuntime {
    calls: HashMap<DelegateRequestId, DelegateCall>,
    seen_requests: HashSet<DelegateRequestId>,
    request_order: VecDeque<DelegateRequestId>,
    closing_cells: HashMap<CellKey, CellOwner>,
    deferred_shutdowns: Vec<DeferredShutdown>,
    event_tx: mpsc::Sender<DriverEvent>,
    cleanup: ConnectionCleanup,
}

impl DelegateRuntime {
    pub(super) fn new(event_tx: mpsc::Sender<DriverEvent>, cleanup: ConnectionCleanup) -> Self {
        Self {
            calls: HashMap::new(),
            seen_requests: HashSet::new(),
            request_order: VecDeque::new(),
            closing_cells: HashMap::new(),
            deferred_shutdowns: Vec::new(),
            event_tx,
            cleanup,
        }
    }

    pub(super) fn start(
        &mut self,
        id: DelegateRequestId,
        target: DelegateTarget,
        request: DelegateRequest,
    ) -> Result<(), String> {
        if self.calls.contains_key(&id) || self.seen_requests.contains(&id) {
            return Err(format!("duplicate code-mode delegate request ID {id:?}"));
        }
        self.remember_request(id);
        let cancellation = CancellationToken::new();
        let task_request = match request {
            DelegateRequest::InvokeTool { invocation } => {
                let mut invocation: CodeModeNestedToolCall = invocation.into();
                invocation.cell_id = target.cell_id.clone();
                DelegateTask::InvokeTool(invocation)
            }
            DelegateRequest::Notify {
                call_id,
                cell_id: _,
                text,
            } => DelegateTask::Notify {
                call_id,
                cell_id: target.cell_id.clone(),
                text,
            },
        };
        let delegate = target.delegate;
        let task_cancellation = cancellation.clone();
        let cleanup_guard = self.cleanup.delegate_guard();
        let delegate_task = tokio::spawn(async move {
            let _cleanup_guard = cleanup_guard;
            match task_request {
                DelegateTask::InvokeTool(invocation) => delegate
                    .invoke_tool(invocation, task_cancellation)
                    .await
                    .map(|result| DelegateResponse::ToolResult { result }),
                DelegateTask::Notify {
                    call_id,
                    cell_id,
                    text,
                } => delegate
                    .notify(call_id, cell_id, text, task_cancellation)
                    .await
                    .map(|()| DelegateResponse::NotificationDelivered),
            }
        });
        self.calls.insert(
            id,
            DelegateCall {
                cell: CellKey {
                    session_id: target.session_id,
                    cell_id: target.cell_id,
                },
                cancellation,
                response_suppressed: false,
            },
        );
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            let result = match delegate_task.await {
                Ok(result) => result,
                Err(err) => Err(format!("code-mode delegate task failed: {err}")),
            };
            let _ = event_tx
                .send(DriverEvent::DelegateCompleted { id, result })
                .await;
        });
        Ok(())
    }

    pub(super) fn cancel(&mut self, id: DelegateRequestId) {
        if let Some(call) = self.calls.get_mut(&id) {
            call.response_suppressed = true;
            call.cancellation.cancel();
        }
    }

    pub(super) fn complete(
        &mut self,
        id: DelegateRequestId,
        result: Result<DelegateResponse, String>,
    ) -> DelegateEffects {
        let Some(call) = self.calls.remove(&id) else {
            return DelegateEffects::empty();
        };
        let mut effects = DelegateEffects::empty();
        if !call.response_suppressed {
            effects.response = Some((id, result));
        }
        if !self.calls.values().any(|active| active.cell == call.cell)
            && let Some(closed) = self.closing_cells.remove(&call.cell)
        {
            effects.closed_cells.push(closed);
        }
        self.release_shutdown_waiters(&mut effects);
        effects
    }

    pub(super) fn close_cell(&mut self, owner: CellOwner) -> DelegateEffects {
        let key = CellKey::for_owner(&owner);
        let mut draining = false;
        for call in self.calls.values_mut().filter(|call| call.cell == key) {
            call.response_suppressed = true;
            call.cancellation.cancel();
            draining = true;
        }
        let mut effects = DelegateEffects::empty();
        if draining {
            self.closing_cells.insert(key, owner);
        } else {
            effects.closed_cells.push(owner);
        }
        self.release_shutdown_waiters(&mut effects);
        effects
    }

    pub(super) fn close_cells(&mut self, owners: Vec<CellOwner>) -> DelegateEffects {
        let mut effects = DelegateEffects::empty();
        for owner in owners {
            effects.append(self.close_cell(owner));
        }
        effects
    }

    pub(super) fn is_session_draining(
        &self,
        session_id: &codex_code_mode_protocol::host::SessionId,
    ) -> bool {
        self.closing_cells
            .keys()
            .any(|cell| &cell.session_id == session_id)
    }

    pub(super) fn defer_shutdown(&mut self, shutdown: DeferredShutdown) {
        self.deferred_shutdowns.push(shutdown);
    }

    pub(super) fn fail_all(&mut self, reason: &str, live_cells: Vec<CellOwner>) {
        for (_, call) in self.calls.drain() {
            call.cancellation.cancel();
        }
        for shutdown in self.deferred_shutdowns.drain(..) {
            let _ = shutdown.response_tx.send(Err(reason.to_string()));
        }
        let mut closed_cells = self.closing_cells.drain().collect::<HashMap<_, _>>();
        for owner in live_cells {
            closed_cells.insert(CellKey::for_owner(&owner), owner);
        }
        self.cleanup.fail(closed_cells.into_values().collect());
    }

    fn remember_request(&mut self, id: DelegateRequestId) {
        self.seen_requests.insert(id);
        self.request_order.push_back(id);
        while self.request_order.len() > MAX_RECENT_DELEGATE_REQUEST_IDS {
            if let Some(expired) = self.request_order.pop_front() {
                self.seen_requests.remove(&expired);
            }
        }
    }

    fn release_shutdown_waiters(&mut self, effects: &mut DelegateEffects) {
        let deferred_shutdowns = std::mem::take(&mut self.deferred_shutdowns);
        for shutdown in deferred_shutdowns {
            if !self.is_session_draining(&shutdown.session_id) {
                effects.shutdowns.push(shutdown);
            } else {
                self.deferred_shutdowns.push(shutdown);
            }
        }
    }
}

impl ConnectionDriver {
    pub(super) fn start_delegate(
        &mut self,
        id: DelegateRequestId,
        session_id: SessionId,
        request: DelegateRequest,
    ) -> bool {
        let wire_cell_id = match &request {
            DelegateRequest::InvokeTool { invocation } => &invocation.cell_id,
            DelegateRequest::Notify { cell_id, .. } => cell_id,
        };
        let target = match self.sessions.delegate_target(&session_id, wire_cell_id) {
            Ok(target) => target,
            Err(err) => {
                self.fail(err);
                return false;
            }
        };
        if let Err(err) = self.delegates.start(id, target, request) {
            self.fail(err);
            return false;
        }
        true
    }

    pub(super) fn complete_delegate(
        &mut self,
        id: DelegateRequestId,
        result: Result<DelegateResponse, String>,
    ) -> bool {
        let effects = self.delegates.complete(id, result);
        self.apply_delegate_effects(effects)
    }

    fn send_delegate_response(
        &mut self,
        id: DelegateRequestId,
        result: Result<DelegateResponse, String>,
    ) -> bool {
        let message = ClientToHost::DelegateResponse {
            id,
            result: WireResult::from_result(result),
        };
        let frame = match EncodedFrame::encode(&message) {
            Ok(frame) => frame,
            Err(err) => {
                let fallback = ClientToHost::DelegateResponse {
                    id,
                    result: WireResult::Err {
                        message: format!(
                            "code-mode delegate response exceeds the IPC frame limit: {err}"
                        ),
                    },
                };
                match EncodedFrame::encode(&fallback) {
                    Ok(frame) => frame,
                    Err(fallback_err) => {
                        self.fail(format!(
                            "failed to encode code-mode delegate error response: {fallback_err}"
                        ));
                        return false;
                    }
                }
            }
        };
        self.queue_frame(frame)
    }

    pub(super) fn close_cell(&mut self, session_id: SessionId, cell_id: WireCellId) -> bool {
        let owner = match self.sessions.remove_cell(&session_id, &cell_id) {
            Ok(owner) => owner,
            Err(err) => {
                self.fail(err);
                return false;
            }
        };
        let effects = self.delegates.close_cell(owner);
        self.apply_delegate_effects(effects)
    }

    pub(super) fn close_session_locally(&mut self, session_id: &SessionId) -> DelegateEffects {
        self.requests.remove_unclaimed_for_session(session_id);
        let owners = self.sessions.remove_session(session_id);
        self.delegates.close_cells(owners)
    }

    pub(super) fn apply_delegate_effects(&mut self, effects: DelegateEffects) -> bool {
        if let Some((id, result)) = effects.response
            && !self.send_delegate_response(id, result)
        {
            return false;
        }
        for closed in effects.closed_cells {
            notify_cell_closed(&closed.delegate, &closed.cell_id);
        }
        for shutdown in effects.shutdowns {
            let _ = shutdown.response_tx.send(Ok(()));
        }
        true
    }
}
