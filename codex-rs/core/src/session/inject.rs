use super::input_queue::TurnInput;
use super::session::Session;
use super::turn_context::TurnContext;
use crate::state::ActiveTurn;
use crate::tasks::RegularTask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::models::ResponseItem;
use std::sync::Arc;

impl Session {
    /// Returns the input if there is no active turn to inject into.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub async fn inject_if_running(
        &self,
        input: Vec<ResponseItem>,
    ) -> Result<(), Vec<ResponseItem>> {
        let mut active = self.active_turn.lock().await;
        match active.as_mut() {
            Some(active_turn) => {
                self.input_queue
                    .extend_pending_input_for_turn_state(
                        active_turn.turn_state.as_ref(),
                        input.into_iter().map(TurnInput::ResponseItem).collect(),
                    )
                    .await;
                Ok(())
            }
            None => Err(input),
        }
    }

    /// Injects items into active work, or starts a regular turn with them.
    pub(crate) async fn inject_or_start_turn(self: &Arc<Self>, input: Vec<ResponseItem>) {
        if input.is_empty() {
            return;
        }
        let input = match self.inject_if_running(input).await {
            Ok(()) => return,
            Err(input) => input,
        };
        if self.collaboration_mode().await.mode == ModeKind::Plan {
            let _ = self.inject_if_running(input).await;
            return;
        }

        let (turn_state, should_start_turn) = {
            let mut active_turn = self.active_turn.lock().await;
            if let Some(active_turn) = active_turn.as_ref() {
                (Arc::clone(&active_turn.turn_state), false)
            } else {
                let active_turn = active_turn.get_or_insert_with(ActiveTurn::default);
                (Arc::clone(&active_turn.turn_state), true)
            }
        };

        self.input_queue
            .extend_pending_input_for_turn_state(
                turn_state.as_ref(),
                input.into_iter().map(TurnInput::ResponseItem).collect(),
            )
            .await;
        if !should_start_turn {
            return;
        }

        let turn_context = self
            .new_default_turn_with_sub_id(uuid::Uuid::new_v4().to_string())
            .await;
        self.maybe_emit_unknown_model_warning_for_turn(turn_context.as_ref())
            .await;
        let still_reserved = {
            let active_turn = self.active_turn.lock().await;
            active_turn.as_ref().is_some_and(|active_turn| {
                active_turn.task.is_none() && Arc::ptr_eq(&active_turn.turn_state, &turn_state)
            })
        };
        if !still_reserved {
            let mut active_turn_guard = self.active_turn.lock().await;
            if let Some(active_turn) = active_turn_guard.as_ref()
                && active_turn.task.is_none()
                && Arc::ptr_eq(&active_turn.turn_state, &turn_state)
            {
                *active_turn_guard = None;
            }
            return;
        }

        self.start_task(turn_context, Vec::new(), RegularTask::new())
            .await;
    }

    /// Injects items into active work, or records them without starting a turn.
    pub(crate) async fn inject_no_new_turn(
        &self,
        items: Vec<ResponseItem>,
        current_turn_context: Option<&TurnContext>,
    ) {
        let Err(items) = self.inject_if_running(items).await else {
            return;
        };
        let default_turn_context;
        let turn_context = match current_turn_context {
            Some(turn_context) => turn_context,
            None => {
                default_turn_context = self.new_default_turn().await;
                default_turn_context.as_ref()
            }
        };
        self.record_conversation_items(turn_context, &items).await;
    }
}
