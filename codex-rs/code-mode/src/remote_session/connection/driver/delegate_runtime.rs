//! Client-side delegate task and closure lifecycle.
//!
//! Cancellation revokes the task's completion path before removing its active-call state. The
//! delegate future may finish later, but it can no longer send a response or affect cell closure.

use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;

use codex_code_mode_protocol::CodeModeNestedToolCall;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::DelegateRequest;
use codex_code_mode_protocol::host::DelegateRequestId;
use codex_code_mode_protocol::host::DelegateResponse;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::MAX_PENDING_DELEGATE_CALLS;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::WireCellId;
use codex_code_mode_protocol::host::WireResult;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::ConnectionDriver;
use super::notify_cell_closed;
use super::session_registry::CellOwner;
use super::session_registry::DelegateTarget;
use super::session_registry::FailedSession;
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
    completion_stop: CancellationToken,
}

impl DelegateCall {
    fn revoke(&self) {
        self.cancellation.cancel();
        self.completion_stop.cancel();
    }
}

enum DelegateTask {
    InvokeTool(CodeModeNestedToolCall),
    Notify {
        call_id: String,
        cell_id: codex_code_mode_protocol::CellId,
        text: String,
    },
}

enum DelegateStartError {
    Duplicate(DelegateRequestId),
    CapacityExceeded,
}

pub(super) struct DelegateEffects {
    pub(super) response: Option<(DelegateRequestId, Result<DelegateResponse, String>)>,
    pub(super) closed_cells: Vec<CellOwner>,
}

impl DelegateEffects {
    fn empty() -> Self {
        Self {
            response: None,
            closed_cells: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        debug_assert!(self.response.is_none());
        self.response = other.response.take();
        self.closed_cells.append(&mut other.closed_cells);
    }
}

pub(super) struct DelegateRuntime {
    calls: HashMap<DelegateRequestId, DelegateCall>,
    seen_requests: HashSet<DelegateRequestId>,
    request_order: VecDeque<DelegateRequestId>,
    event_tx: mpsc::Sender<DriverEvent>,
}

impl DelegateRuntime {
    pub(super) fn new(event_tx: mpsc::Sender<DriverEvent>) -> Self {
        Self {
            calls: HashMap::new(),
            seen_requests: HashSet::new(),
            request_order: VecDeque::new(),
            event_tx,
        }
    }

    fn start(
        &mut self,
        id: DelegateRequestId,
        target: DelegateTarget,
        request: DelegateRequest,
    ) -> Result<(), DelegateStartError> {
        if self.calls.contains_key(&id) || self.seen_requests.contains(&id) {
            return Err(DelegateStartError::Duplicate(id));
        }
        self.remember_request(id);
        if self.calls.len() >= MAX_PENDING_DELEGATE_CALLS {
            return Err(DelegateStartError::CapacityExceeded);
        }
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
        let delegate_task = tokio::spawn(async move {
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
        let completion_stop = CancellationToken::new();
        self.calls.insert(
            id,
            DelegateCall {
                cell: CellKey {
                    session_id: target.session_id,
                    cell_id: target.cell_id,
                },
                cancellation,
                completion_stop: completion_stop.clone(),
            },
        );
        let event_tx = self.event_tx.clone();
        tokio::spawn(async move {
            let result = tokio::select! {
                biased;
                _ = completion_stop.cancelled() => return,
                result = delegate_task => match result {
                    Ok(result) => result,
                    Err(err) => Err(format!("code-mode delegate task failed: {err}")),
                },
            };
            tokio::select! {
                biased;
                _ = completion_stop.cancelled() => {}
                _ = event_tx.send(DriverEvent::DelegateCompleted { id, result }) => {}
            }
        });
        Ok(())
    }

    pub(super) fn cancel(&mut self, id: DelegateRequestId) {
        if let Some(call) = self.calls.remove(&id) {
            call.revoke();
        }
    }

    pub(super) fn complete(
        &mut self,
        id: DelegateRequestId,
        result: Result<DelegateResponse, String>,
    ) -> DelegateEffects {
        if self.calls.remove(&id).is_none() {
            return DelegateEffects::empty();
        }
        let mut effects = DelegateEffects::empty();
        effects.response = Some((id, result));
        effects
    }

    pub(super) fn close_cell(&mut self, owner: CellOwner) -> DelegateEffects {
        let key = CellKey::for_owner(&owner);
        self.calls.retain(|_, call| {
            if call.cell != key {
                return true;
            }
            call.revoke();
            false
        });
        let mut effects = DelegateEffects::empty();
        effects.closed_cells.push(owner);
        effects
    }

    pub(super) fn close_cells(&mut self, owners: Vec<CellOwner>) -> DelegateEffects {
        let mut effects = DelegateEffects::empty();
        for owner in owners {
            effects.append(self.close_cell(owner));
        }
        effects
    }

    pub(super) fn fail_all(&mut self, failed_sessions: Vec<FailedSession>) {
        for (_, call) in self.calls.drain() {
            call.revoke();
        }
        for session in failed_sessions {
            session.cleanup.fail(session.cells);
        }
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
        match self.delegates.start(id, target, request) {
            Ok(()) => true,
            Err(DelegateStartError::Duplicate(id)) => {
                self.fail(format!("duplicate code-mode delegate request ID {id:?}"));
                false
            }
            Err(DelegateStartError::CapacityExceeded) => self.send_delegate_response(
                id,
                Err(format!(
                    "code-mode host exceeded the limit of {MAX_PENDING_DELEGATE_CALLS} pending delegate calls"
                )),
            ),
        }
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
        true
    }
}
