//! Turn-scoped state and active turn metadata scaffolding.

use indexmap::IndexMap;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tokio_util::task::AbortOnDropHandle;

use codex_protocol::dynamic_tools::DynamicToolResponse;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::request_user_input::RequestUserInputResponse;
use tokio::sync::oneshot;

use crate::codex::TurnContext;
use crate::protocol::ReviewDecision;
use crate::tasks::SessionTask;
use codex_protocol::ThreadId;

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
    pending_user_input: HashMap<String, oneshot::Sender<RequestUserInputResponse>>,
    pending_dynamic_tools: HashMap<String, oneshot::Sender<DynamicToolResponse>>,
    pending_input: Vec<ResponseInputItem>,
    active_waits: HashMap<String, HashMap<ThreadId, usize>>,
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
        self.active_waits.clear();
    }

    pub(crate) fn insert_pending_user_input(
        &mut self,
        key: String,
        tx: oneshot::Sender<RequestUserInputResponse>,
    ) -> Option<oneshot::Sender<RequestUserInputResponse>> {
        self.pending_user_input.insert(key, tx)
    }

    pub(crate) fn remove_pending_user_input(
        &mut self,
        key: &str,
    ) -> Option<oneshot::Sender<RequestUserInputResponse>> {
        self.pending_user_input.remove(key)
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

    pub(crate) fn begin_wait(&mut self, turn_id: &str, agent_ids: &[ThreadId]) {
        if agent_ids.is_empty() {
            return;
        }
        let waits = self.active_waits.entry(turn_id.to_string()).or_default();
        for agent_id in agent_ids {
            *waits.entry(*agent_id).or_default() += 1;
        }
    }

    pub(crate) fn end_wait(&mut self, turn_id: &str, agent_ids: &[ThreadId]) {
        if agent_ids.is_empty() {
            return;
        }
        let mut remove_turn = false;
        if let Some(waits) = self.active_waits.get_mut(turn_id) {
            for agent_id in agent_ids {
                if let Some(count) = waits.get_mut(agent_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        waits.remove(agent_id);
                    }
                }
            }
            remove_turn = waits.is_empty();
        }
        if remove_turn {
            self.active_waits.remove(turn_id);
        }
    }

    pub(crate) fn is_waiting_on(&self, turn_id: &str, agent_id: ThreadId) -> bool {
        self.active_waits
            .get(turn_id)
            .is_some_and(|waits| waits.contains_key(&agent_id))
    }
}

impl ActiveTurn {
    /// Clear any pending approvals and input buffered for the current turn.
    pub(crate) async fn clear_pending(&self) {
        let mut ts = self.turn_state.lock().await;
        ts.clear_pending();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn thread_id(s: &str) -> ThreadId {
        ThreadId::from_string(s).expect("valid thread id")
    }

    #[test]
    fn wait_tracking_is_turn_scoped_and_reference_counted() {
        let mut state = TurnState::default();
        let turn_a = "turn-a";
        let turn_b = "turn-b";
        let agent = thread_id("00000000-0000-7000-0000-000000000001");

        state.begin_wait(turn_a, &[agent]);
        state.begin_wait(turn_a, &[agent]);
        state.begin_wait(turn_b, &[agent]);

        assert_eq!(state.is_waiting_on(turn_a, agent), true);
        assert_eq!(state.is_waiting_on(turn_b, agent), true);

        state.end_wait(turn_a, &[agent]);
        assert_eq!(state.is_waiting_on(turn_a, agent), true);

        state.end_wait(turn_a, &[agent]);
        assert_eq!(state.is_waiting_on(turn_a, agent), false);
        assert_eq!(state.is_waiting_on(turn_b, agent), true);
    }
}
