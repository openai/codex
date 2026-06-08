use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::PoisonError;

use codex_extension_api::InitialGoalError;
use codex_extension_api::InitialGoalInput;
use codex_protocol::protocol::InitialGoal;
use tokio::sync::oneshot;

use super::session::Session;
use super::turn_context::PreparedTurn;

pub(crate) enum InitialGoalStartError {
    InvalidRequest(String),
    Internal(String),
}

#[derive(Default)]
pub(crate) struct InitialGoalStartAcks {
    senders: Mutex<HashMap<String, oneshot::Sender<Result<(), InitialGoalStartError>>>>,
}

impl InitialGoalStartAcks {
    pub(crate) fn register(
        &self,
        turn_id: String,
    ) -> oneshot::Receiver<Result<(), InitialGoalStartError>> {
        let (sender, receiver) = oneshot::channel();
        self.senders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(turn_id, sender);
        receiver
    }

    pub(crate) fn complete(&self, turn_id: &str, result: Result<(), InitialGoalStartError>) {
        if let Some(sender) = self
            .senders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(turn_id)
        {
            let _ = sender.send(result);
        }
    }

    pub(crate) fn cancel(&self, turn_id: &str) {
        self.senders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(turn_id);
    }
}

impl Session {
    pub(crate) async fn prepare_initial_goal(
        &self,
        turn_id: &str,
        prepared_turn: &PreparedTurn,
        goal: &InitialGoal,
    ) -> Result<(), InitialGoalStartError> {
        let contributor = self
            .services
            .extensions
            .initial_goal_contributor()
            .ok_or_else(|| {
                InitialGoalStartError::Internal(
                    "goal extension is unavailable for this thread".to_string(),
                )
            })?;
        contributor
            .replace_for_turn(InitialGoalInput {
                turn_id,
                goal,
                collaboration_mode: &prepared_turn.session_configuration.collaboration_mode,
                session_store: &self.services.session_extension_data,
                thread_store: &self.services.thread_extension_data,
            })
            .await
            .map_err(|err| match err {
                InitialGoalError::InvalidRequest(message) => {
                    InitialGoalStartError::InvalidRequest(message)
                }
                InitialGoalError::Internal(message) => InitialGoalStartError::Internal(message),
            })
    }

    pub(crate) fn register_initial_goal_start_ack(
        &self,
        turn_id: String,
    ) -> oneshot::Receiver<Result<(), InitialGoalStartError>> {
        self.initial_goal_start_acks.register(turn_id)
    }

    pub(crate) fn complete_initial_goal_start(
        &self,
        turn_id: &str,
        result: Result<(), InitialGoalStartError>,
    ) {
        self.initial_goal_start_acks.complete(turn_id, result);
    }

    pub(crate) fn cancel_initial_goal_start(&self, turn_id: &str) {
        self.initial_goal_start_acks.cancel(turn_id);
    }
}
