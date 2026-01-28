//! Turn-scoped state and active turn metadata scaffolding.

use indexmap::IndexMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::request_user_input::RequestUserInputAnswer;
use codex_protocol::request_user_input::RequestUserInputResponse;
use tokio::sync::oneshot;

use crate::codex::TurnContext;
use crate::protocol::ReviewDecision;
use crate::tasks::SessionTask;

/// Metadata about the currently running turn.
pub(crate) struct ActiveTurn {
    pub(crate) tasks: IndexMap<String, RunningTask>,
    pub(crate) turn_state: Arc<Mutex<TurnState>>,
}

impl Default for ActiveTurn {
    fn default() -> Self {
        Self {
            tasks: IndexMap::new(),
            turn_state: Arc::new(Mutex::new(TurnState::default())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TaskKind {
    Regular,
    Review,
    Compact,
}

pub(crate) struct RunningTask {
    pub(crate) done: Arc<Notify>,
    pub(crate) kind: TaskKind,
    pub(crate) task: Arc<dyn SessionTask>,
    pub(crate) cancellation_token: CancellationToken,
    pub(crate) handle: Arc<AbortOnDropHandle<()>>,
    pub(crate) turn_context: Arc<TurnContext>,
    // Timer recorded when the task drops to capture the full turn duration.
    pub(crate) _timer: Option<codex_otel::Timer>,
}

impl ActiveTurn {
    pub(crate) fn add_task(&mut self, task: RunningTask) {
        let sub_id = task.turn_context.sub_id.clone();
        self.tasks.insert(sub_id, task);
    }

    pub(crate) fn remove_task(&mut self, sub_id: &str) -> bool {
        self.tasks.swap_remove(sub_id);
        self.tasks.is_empty()
    }

    pub(crate) fn drain_tasks(&mut self) -> Vec<RunningTask> {
        self.tasks.drain(..).map(|(_, task)| task).collect()
    }
}

/// Mutable state for a single turn.
#[derive(Default)]
pub(crate) struct TurnState {
    pending_approvals: HashMap<String, oneshot::Sender<ReviewDecision>>,
    pending_user_input: HashMap<String, PendingUserInput>,
    pending_dynamic_tools: HashMap<String, oneshot::Sender<DynamicToolResponse>>,
    pending_input: Vec<ResponseInputItem>,
}

pub(crate) struct PendingUserInput {
    call_id: String,
    question_ids: HashSet<String>,
    answers: HashMap<String, RequestUserInputAnswer>,
    tx: oneshot::Sender<RequestUserInputResponse>,
}

pub(crate) struct PendingUserInputUpdate {
    pub(crate) call_id: String,
    pub(crate) merged: RequestUserInputResponse,
    pub(crate) is_complete: bool,
    pub(crate) tx: Option<oneshot::Sender<RequestUserInputResponse>>,
}

impl PendingUserInput {
    pub(crate) fn new(
        call_id: String,
        question_ids: HashSet<String>,
        tx: oneshot::Sender<RequestUserInputResponse>,
    ) -> Self {
        Self {
            call_id,
            question_ids,
            answers: HashMap::new(),
            tx,
        }
    }

    pub(crate) fn apply_update(
        &mut self,
        update: RequestUserInputResponse,
    ) -> RequestUserInputResponse {
        let RequestUserInputResponse { answers } = update;
        self.answers = answers;
        RequestUserInputResponse {
            answers: self.answers.clone(),
        }
    }

    pub(crate) fn is_complete(&self, response: &RequestUserInputResponse) -> bool {
        self.question_ids
            .iter()
            .all(|id| response.answers.contains_key(id))
    }
}

impl TurnState {
    pub(crate) fn insert_pending_approval(
        &mut self,
        key: String,
        tx: oneshot::Sender<ReviewDecision>,
    ) -> Option<oneshot::Sender<ReviewDecision>> {
        self.pending_approvals.insert(key, tx)
    }

    pub(crate) fn remove_pending_approval(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<ReviewDecision>> {
        self.pending_approvals.remove(key)
    }

    pub(crate) fn clear_pending(&mut self) {
        self.pending_approvals.clear();
        self.pending_user_input.clear();
        self.pending_dynamic_tools.clear();
        self.pending_input.clear();
    }

    pub(crate) fn insert_pending_user_input(
        &mut self,
        key: String,
        pending: PendingUserInput,
    ) -> Option<PendingUserInput> {
        self.pending_user_input.insert(key, pending)
    }

    pub(crate) fn update_pending_user_input(
        &mut self,
        key: &str,
        update: RequestUserInputResponse,
    ) -> Option<PendingUserInputUpdate> {
        let pending = self.pending_user_input.get_mut(key)?;
        let merged = pending.apply_update(update);
        let is_complete = pending.is_complete(&merged);
        let call_id = pending.call_id.clone();
        if !is_complete {
            return Some(PendingUserInputUpdate {
                call_id,
                merged,
                is_complete,
                tx: None,
            });
        }
        let pending = self.pending_user_input.remove(key)?;
        Some(PendingUserInputUpdate {
            call_id,
            merged,
            is_complete,
            tx: Some(pending.tx),
        })
    }

    pub(crate) fn cancel_pending_user_input(&mut self, key: &str) -> Option<String> {
        self.pending_user_input
            .remove(key)
            .map(|pending| pending.call_id)
    }

    pub(crate) fn insert_pending_dynamic_tool(
        &mut self,
        key: String,
        tx: oneshot::Sender<DynamicToolResponse>,
    ) -> Option<oneshot::Sender<DynamicToolResponse>> {
        self.pending_dynamic_tools.insert(key, tx)
    }

    pub(crate) fn remove_pending_dynamic_tool(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<DynamicToolResponse>> {
        self.pending_dynamic_tools.remove(key)
    }

    pub(crate) fn push_pending_input(&mut self, input: ResponseInputItem) {
        self.pending_input.push(input);
    }

    pub(crate) fn take_pending_input(&mut self) -> Vec<ResponseInputItem> {
        if self.pending_input.is_empty() {
            Vec::with_capacity(0)
        } else {
            let mut ret = Vec::new();
            std::mem::swap(&mut ret, &mut self.pending_input);
            ret
        }
    }

    pub(crate) fn has_pending_input(&self) -> bool {
        !self.pending_input.is_empty()
    }
}

impl ActiveTurn {
    /// Clear any pending approvals and input buffered for the current turn.
    pub(crate) async fn clear_pending(&self) {
        let mut ts = self.turn_state.lock().await;
        ts.clear_pending();
    }
}
