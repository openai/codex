//! Attempts one new continuation turn immediately after daemon thread restoration.
//! Completed or superseded work is discarded; recovery never waits for a later idle event.
//! Permissions and the saved local executor selection must match the resumed runtime.

use super::ThreadRequestProcessor;
use codex_app_server_protocol::ThreadEnvironment;
use codex_app_server_transport::daemon_recovery::InterruptedTurn;
use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_core::TurnStartOptions;
use codex_core::context::ContextualUserFragment;
use codex_core::context::InternalContextSource;
use codex_core::context::InternalModelContextFragment;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::TurnAbortReason;
use codex_protocol::protocol::TurnAbortedEvent;
use codex_rollout::RolloutItem;

impl ThreadRequestProcessor {
    pub(crate) async fn continue_daemon_turn(&self, thread_id: &str, saved: InterruptedTurn) {
        let Ok(thread_id) = ThreadId::from_string(thread_id) else {
            return;
        };
        let Ok(thread) = self.thread_manager.get_thread(thread_id).await else {
            return;
        };
        let Some(path) = thread.rollout_path() else {
            return;
        };
        let history = match codex_rollout::RolloutRecorder::load_rollout_items(&path).await {
            Ok((history, _, _)) => history,
            Err(err) => {
                tracing::warn!(%thread_id, %err, "failed to read interrupted turn history");
                return;
            }
        };
        // The snapshot has complete recorded input, but the old process may have
        // completed or replaced its turn during the shutdown grace period.
        let Some(start) = history
            .iter()
            .rposition(|item| matches!(item, RolloutItem::EventMsg(EventMsg::TurnStarted(_))))
        else {
            return;
        };
        if !matches!(&history[start], RolloutItem::EventMsg(EventMsg::TurnStarted(event))
            if event.turn_id == saved.turn_id)
            || history[start..].iter().any(|item| match item {
                RolloutItem::EventMsg(EventMsg::TurnComplete(event)) => {
                    event.turn_id == saved.turn_id
                }
                RolloutItem::EventMsg(EventMsg::TurnAborted(event)) => event
                    .turn_id
                    .as_ref()
                    .is_none_or(|turn_id| turn_id == &saved.turn_id),
                _ => false,
            })
        {
            return;
        }
        let previous_context = history.iter().rev().find_map(|item| match item {
            RolloutItem::TurnContext(context)
                if context.turn_id.as_ref() == Some(&saved.turn_id) =>
            {
                Some(context)
            }
            _ => None,
        });
        let Some(previous_context) = previous_context else {
            return;
        };
        let config = thread.config_snapshot().await;
        let [environment] = config.environment_selections() else {
            return;
        };
        if environment.environment_id != codex_exec_server::LOCAL_ENVIRONMENT_ID
            || environment.config != EnvironmentConfigState::FromThread
            || saved.local_environment.as_ref() != Some(&ThreadEnvironment::from(environment))
        {
            return;
        }
        // Recovery must not override a stricter saved or newly configured policy.
        if previous_context.permission_profile() != config.permission_profile {
            return;
        }
        let continuation = ContextualUserFragment::into(InternalModelContextFragment::new(
            InternalContextSource::from_static("daemon_recovery"),
            "The server restarted and interrupted the previous turn. Continue the unfinished work from the saved conversation. Check the current state before repeating actions that may already have completed.",
        ));
        let request = TurnInputRequest::new(TurnInput::ResponseItem(continuation)).on_start(
            TurnStartOptions {
                turn_trigger: Some("daemon_recovery".to_string()),
                final_output_json_schema: saved.output_schema,
                service_tier: saved.service_tier,
                cyber_access_program: saved.cyber_access_program,
                root_turn_id: previous_context.root_turn_id.clone(),
                ..Default::default()
            },
        );
        // Close the old turn in persisted history before exposing a new running turn.
        if let Err(err) = thread
            .append_rollout_items(&[RolloutItem::EventMsg(EventMsg::TurnAborted(
                TurnAbortedEvent {
                    turn_id: Some(saved.turn_id.clone()),
                    reason: TurnAbortReason::Interrupted,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                },
            ))])
            .await
        {
            tracing::warn!(%thread_id, %err, "failed to close interrupted turn history");
            return;
        }
        match thread.continue_turn_if_idle(request, saved.turn_id).await {
            Ok(TurnInputSubmission::Started { .. }) => {
                crate::extensions::send_thread_warning(
                    &self.outgoing,
                    &self.thread_state_manager,
                    thread_id,
                    "Resuming interrupted work".to_string(),
                )
                .await;
            }
            Ok(TurnInputSubmission::NotSubmitted { reason }) => {
                tracing::debug!(%thread_id, ?reason, "recovery continuation was not started");
            }
            Ok(TurnInputSubmission::Steered { .. }) => unreachable!("continuation cannot steer"),
            Err(err) => tracing::warn!(%thread_id, %err, "recovery continuation failed"),
        }
    }
}
