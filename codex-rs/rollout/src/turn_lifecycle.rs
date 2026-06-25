use codex_protocol::items::parse_hook_prompt_message;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::RolloutItem;

/// Whether the current explicit turn is still running or has reached a terminal state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplicitTurnState {
    InProgress,
    Terminal,
}

/// The explicit turn currently open at the end of the observed rollout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentExplicitTurn {
    /// Identifier carried by the `TurnStarted` event.
    pub turn_id: String,
    /// Zero-based index of that event in the raw rollout stream.
    pub rollout_start_index: usize,
    /// Whether the turn is still active or has reached a terminal boundary.
    pub state: ExplicitTurnState,
}

/// Tracks turn lifecycle boundaries in persisted rollout order.
///
/// This intentionally models only lifecycle state. Implicit turns are retained as
/// placeholders so rollback counts and late explicit turn IDs remain correlated
/// without reconstructing an app-server presentation history.
#[derive(Debug, Default)]
pub struct RolloutTurnLifecycleTracker {
    finished_turns: Vec<FinishedTurn>,
    current_turn: Option<CurrentTurn>,
    next_rollout_index: usize,
}

#[derive(Debug)]
enum CurrentTurn {
    Explicit(CurrentExplicitTurn),
    Implicit(ImplicitTurnState),
}

#[derive(Debug)]
enum FinishedTurn {
    Explicit(String),
    Implicit,
}

#[derive(Debug)]
enum ImplicitTurnState {
    CompactionOnly,
    Materialized,
}

impl RolloutTurnLifecycleTracker {
    /// Create an empty lifecycle tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe one rollout item in persisted order.
    pub fn handle_rollout_item(&mut self, item: &RolloutItem) {
        let rollout_index = self.next_rollout_index;
        self.next_rollout_index += 1;

        if matches!(item, RolloutItem::Compacted(_)) {
            self.handle_compacted();
            return;
        }

        if let RolloutItem::ResponseItem(codex_protocol::models::ResponseItem::Message {
            role,
            content,
            id,
            ..
        }) = item
            && role == "user"
            && parse_hook_prompt_message(id.as_ref(), content).is_some()
        {
            self.materialize_implicit_turn();
            return;
        }

        let RolloutItem::EventMsg(event) = item else {
            return;
        };

        match event {
            EventMsg::UserMessage(_) => self.handle_implicit_user_turn(),
            EventMsg::AgentMessage(event) if !event.message.is_empty() => {
                self.materialize_implicit_turn();
            }
            EventMsg::AgentReasoning(event) if !event.text.is_empty() => {
                self.materialize_implicit_turn();
            }
            EventMsg::AgentReasoningRawContent(event) if !event.text.is_empty() => {
                self.materialize_implicit_turn();
            }
            EventMsg::PatchApplyEnd(event) if event.turn_id.is_empty() => {
                self.materialize_implicit_turn();
            }
            EventMsg::DynamicToolCallRequest(event) if event.turn_id.is_empty() => {
                self.materialize_implicit_turn();
            }
            EventMsg::DynamicToolCallResponse(event) if event.turn_id.is_empty() => {
                self.materialize_implicit_turn();
            }
            EventMsg::GuardianAssessment(event)
                if event.turn_id.is_empty()
                    && event.status != GuardianAssessmentStatus::Approved
                    && event.target_item_id.is_some()
                    && matches!(
                        &event.action,
                        GuardianAssessmentAction::Command { .. }
                            | GuardianAssessmentAction::Execve { .. }
                    ) =>
            {
                self.materialize_implicit_turn();
            }
            EventMsg::ContextCompacted(_)
            | EventMsg::EnteredReviewMode(_)
            | EventMsg::ExitedReviewMode(_)
            | EventMsg::McpToolCallEnd(_)
            | EventMsg::WebSearchEnd(_)
            | EventMsg::ImageGenerationEnd(_)
            | EventMsg::SubAgentActivity(_)
            | EventMsg::ViewImageToolCall(_)
            | EventMsg::CollabAgentSpawnEnd(_)
            | EventMsg::CollabAgentInteractionEnd(_)
            | EventMsg::CollabWaitingEnd(_)
            | EventMsg::CollabCloseEnd(_)
            | EventMsg::CollabResumeEnd(_) => self.materialize_implicit_turn(),
            EventMsg::TurnStarted(event) => {
                self.finish_current_turn();
                self.current_turn = Some(CurrentTurn::Explicit(CurrentExplicitTurn {
                    turn_id: event.turn_id.clone(),
                    rollout_start_index: rollout_index,
                    state: ExplicitTurnState::InProgress,
                }));
            }
            EventMsg::TurnComplete(event) => self.handle_turn_complete(&event.turn_id),
            EventMsg::TurnAborted(event) => self.handle_turn_aborted(event.turn_id.as_deref()),
            EventMsg::Error(event) if event.affects_turn_status() => {
                self.mark_current_explicit_turn_terminal();
            }
            EventMsg::ThreadRolledBack(event) => {
                self.finish_current_turn();
                let num_turns = usize::try_from(event.num_turns).unwrap_or(usize::MAX);
                self.finished_turns
                    .truncate(self.finished_turns.len().saturating_sub(num_turns));
            }
            _ => {}
        }
    }

    /// Return the explicitly opened turn that remains current, if any.
    pub fn current_explicit_turn(&self) -> Option<&CurrentExplicitTurn> {
        match self.current_turn.as_ref() {
            Some(CurrentTurn::Explicit(turn)) => Some(turn),
            Some(CurrentTurn::Implicit(_)) | None => None,
        }
    }

    fn handle_implicit_user_turn(&mut self) {
        if matches!(self.current_turn.as_ref(), Some(CurrentTurn::Explicit(_))) {
            return;
        }
        if matches!(
            self.current_turn.as_ref(),
            Some(CurrentTurn::Implicit(ImplicitTurnState::CompactionOnly))
        ) {
            self.current_turn = Some(CurrentTurn::Implicit(ImplicitTurnState::Materialized));
            return;
        }
        if self.current_turn.is_some() {
            self.finish_current_turn();
        }
        self.current_turn = Some(CurrentTurn::Implicit(ImplicitTurnState::Materialized));
    }

    fn handle_compacted(&mut self) {
        if self.current_turn.is_none() {
            self.current_turn = Some(CurrentTurn::Implicit(ImplicitTurnState::CompactionOnly));
        }
    }

    fn materialize_implicit_turn(&mut self) {
        match self.current_turn.as_mut() {
            Some(CurrentTurn::Explicit(_)) => {}
            Some(CurrentTurn::Implicit(state)) => *state = ImplicitTurnState::Materialized,
            None => {
                self.current_turn = Some(CurrentTurn::Implicit(ImplicitTurnState::Materialized));
            }
        }
    }

    fn handle_turn_complete(&mut self, turn_id: &str) {
        if self.current_explicit_turn_has_id(turn_id) {
            self.finish_current_turn();
            return;
        }

        if self.finished_turn_has_id(turn_id) {
            return;
        }

        self.finish_current_turn();
    }

    fn handle_turn_aborted(&mut self, turn_id: Option<&str>) {
        if turn_id.is_some_and(|turn_id| self.current_explicit_turn_has_id(turn_id)) {
            self.mark_current_explicit_turn_terminal();
            return;
        }

        if turn_id.is_some_and(|turn_id| self.finished_turn_has_id(turn_id)) {
            return;
        }

        self.mark_current_explicit_turn_terminal();
    }

    fn mark_current_explicit_turn_terminal(&mut self) {
        if let Some(CurrentTurn::Explicit(turn)) = self.current_turn.as_mut() {
            turn.state = ExplicitTurnState::Terminal;
        }
    }

    fn current_explicit_turn_has_id(&self, turn_id: &str) -> bool {
        self.current_explicit_turn()
            .is_some_and(|turn| turn.turn_id == turn_id)
    }

    fn finished_turn_has_id(&self, turn_id: &str) -> bool {
        self.finished_turns
            .iter()
            .any(|turn| matches!(turn, FinishedTurn::Explicit(id) if id == turn_id))
    }

    fn finish_current_turn(&mut self) {
        let Some(turn) = self.current_turn.take() else {
            return;
        };
        self.finished_turns.push(match turn {
            CurrentTurn::Explicit(turn) => FinishedTurn::Explicit(turn.turn_id),
            CurrentTurn::Implicit(_) => FinishedTurn::Implicit,
        });
    }
}

#[cfg(test)]
#[path = "turn_lifecycle_tests.rs"]
mod tests;
