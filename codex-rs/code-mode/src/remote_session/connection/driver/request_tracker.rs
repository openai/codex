use std::collections::HashMap;
use std::collections::VecDeque;

use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::WireCellId;
use tokio::sync::mpsc;

use super::types::DeferredWait;
use super::types::DriverEvent;
use super::types::InitialResponse;
use super::types::PendingRequest;
use super::types::RemoteSession;
use super::types::UnclaimedExecute;

pub(super) enum CancellationAction {
    Send(RequestId),
    Terminate {
        request_id: RequestId,
        execute: UnclaimedExecute,
    },
}

pub(super) struct RequestTracker {
    pending: HashMap<RequestId, PendingRequest>,
    unclaimed_executes: HashMap<RequestId, UnclaimedExecute>,
    initial_responses: HashMap<RequestId, InitialResponse>,
    deferred_waits: VecDeque<DeferredWait>,
    next_request_id: i64,
}

impl RequestTracker {
    pub(super) fn new() -> Self {
        Self {
            pending: HashMap::new(),
            unclaimed_executes: HashMap::new(),
            initial_responses: HashMap::new(),
            deferred_waits: VecDeque::new(),
            next_request_id: 1,
        }
    }

    pub(super) fn contains_pending_open(&self, session: &RemoteSession) -> bool {
        self.pending.values().any(|pending| {
            matches!(
                pending,
                PendingRequest::OpenSession {
                    session: pending_session,
                    ..
                } if pending_session.id == session.id
            )
        })
    }

    pub(super) fn allocate_id(&mut self) -> Result<RequestId, String> {
        let id = self.next_request_id;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or_else(|| "code-mode host request ID space exhausted".to_string())?;
        Ok(RequestId::new(id))
    }

    pub(super) fn insert_pending(
        &mut self,
        id: RequestId,
        pending: PendingRequest,
        event_tx: &mpsc::Sender<DriverEvent>,
    ) {
        self.pending.insert(id, pending);
        if let Some(cancellation) = self
            .pending
            .get_mut(&id)
            .and_then(PendingRequest::cancellation_mut)
        {
            cancellation.spawn_watcher(id, event_tx.clone());
        }
    }

    pub(super) fn remove_pending(&mut self, id: RequestId) -> Option<PendingRequest> {
        self.pending.remove(&id)
    }

    pub(super) fn insert_initial_response(&mut self, id: RequestId, response: InitialResponse) {
        self.initial_responses.insert(id, response);
    }

    pub(super) fn remove_initial_response(&mut self, id: RequestId) -> Option<InitialResponse> {
        self.initial_responses.remove(&id)
    }

    pub(super) fn insert_unclaimed_execute(&mut self, id: RequestId, execute: UnclaimedExecute) {
        self.unclaimed_executes.insert(id, execute);
    }

    pub(super) fn claim_execute(&mut self, id: RequestId) {
        self.unclaimed_executes.remove(&id);
    }

    pub(super) fn collect_cancellations(&mut self) -> Vec<CancellationAction> {
        let mut actions = self
            .pending
            .iter_mut()
            .filter_map(|(id, pending)| {
                let cancellation = pending.cancellation_mut()?;
                (cancellation.is_cancelled() && cancellation.mark_reported())
                    .then_some(CancellationAction::Send(*id))
            })
            .collect::<Vec<_>>();
        actions.extend(
            self.unclaimed_executes
                .extract_if(|_, execute| {
                    execute.cancellation.is_cancelled() && execute.cancellation.mark_reported()
                })
                .map(|(request_id, execute)| CancellationAction::Terminate {
                    request_id,
                    execute,
                }),
        );
        actions
    }

    pub(super) fn mark_cancelled(&mut self, id: RequestId) -> Option<CancellationAction> {
        if let Some(cancellation) = self
            .pending
            .get_mut(&id)
            .and_then(PendingRequest::cancellation_mut)
        {
            return cancellation
                .mark_reported()
                .then_some(CancellationAction::Send(id));
        }
        let execute = self.unclaimed_executes.get_mut(&id)?;
        if !execute.cancellation.mark_reported() {
            return None;
        }
        self.unclaimed_executes
            .remove(&id)
            .map(|execute| CancellationAction::Terminate {
                request_id: id,
                execute,
            })
    }

    pub(super) fn has_cancelled_wait(&self, session: &RemoteSession, cell_id: &WireCellId) -> bool {
        self.pending.values().any(|pending| {
            matches!(
                pending,
                PendingRequest::Wait {
                    session: pending_session,
                    cell_id: pending_cell_id,
                    cancellation,
                    ..
                } if pending_session == session
                    && pending_cell_id == cell_id
                    && cancellation.is_cancelled()
            )
        })
    }

    pub(super) fn push_deferred_wait(&mut self, wait: DeferredWait) {
        self.deferred_waits.push_back(wait);
    }

    pub(super) fn take_deferred_waits(&mut self) -> VecDeque<DeferredWait> {
        std::mem::take(&mut self.deferred_waits)
    }

    pub(super) fn remove_unclaimed_for_session(&mut self, session_id: &SessionId) {
        self.unclaimed_executes
            .retain(|_, execute| &execute.session.id != session_id);
    }

    pub(super) fn fail_all(&mut self, reason: &str) {
        for (_, pending) in self.pending.drain() {
            pending.fail(reason.to_string());
        }
        self.unclaimed_executes.clear();
        for (_, initial) in self.initial_responses.drain() {
            let _ = initial.response_tx.send(Err(reason.to_string()));
        }
        for wait in self.deferred_waits.drain(..) {
            let _ = wait.response_tx.send(Err(reason.to_string()));
        }
    }
}
