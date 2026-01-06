use crate::CodexConversation;
use crate::agent::AgentStatus;
use crate::conversation_manager::ConversationManagerState;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use codex_protocol::ConversationId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use std::sync::Arc;
use std::sync::Weak;

/// Control-plane handle for multi-agent operations.
/// `AgentControl` is held by each session (via `SessionServices`). It provides capability to
/// spawn new agents and the inter-agent communication layer.
#[derive(Clone, Default)]
pub(crate) struct AgentControl {
    /// Weak handle back to the global conversation registry/state.
    /// This is `Weak` to avoid reference cycles and shadow persistence of the form
    /// `ConversationManagerState -> CodexConversation -> Session -> SessionServices -> ConversationManagerState`.
    manager: Weak<ConversationManagerState>,
}

impl AgentControl {
    /// Construct a new `AgentControl` that can spawn/message agents via the given manager state.
    pub(crate) fn new(manager: Weak<ConversationManagerState>) -> Self {
        Self { manager }
    }

    #[allow(dead_code)] // Used by upcoming multi-agent tooling.
    /// Spawn a new agent conversation and submit the initial prompt.
    ///
    /// If `headless` is true, a background drain task is spawned to prevent unbounded event growth
    /// of the channel queue when there is no client actively reading the conversation events.
    pub(crate) async fn spawn_agent(
        &self,
        config: crate::config::Config,
        prompt: String,
        headless: bool,
    ) -> CodexResult<ConversationId> {
        let state = self.upgrade()?;
        let new_conversation = state.spawn_new_conversation(config, self.clone()).await?;

        if headless {
            spawn_headless_drain(Arc::clone(&new_conversation.conversation));
        }

        self.send_prompt(new_conversation.conversation_id, prompt)
            .await?;

        Ok(new_conversation.conversation_id)
    }

    #[allow(dead_code)] // Used by upcoming multi-agent tooling.
    /// Send a `user` prompt to an existing agent conversation.
    pub(crate) async fn send_prompt(
        &self,
        agent_id: ConversationId,
        prompt: String,
    ) -> CodexResult<String> {
        let state = self.upgrade()?;
        state
            .send_op(
                agent_id,
                Op::UserInput {
                    items: vec![UserInput::Text { text: prompt }],
                    final_output_json_schema: None,
                },
            )
            .await
    }

    #[allow(dead_code)] // Used by upcoming multi-agent tooling.
    /// Fetch the last known status for `agent_id`, returning `NotFound` when unavailable.
    pub(crate) async fn get_status(&self, agent_id: ConversationId) -> AgentStatus {
        let Ok(state) = self.upgrade() else {
            // No agent available if upgrade fails.
            return AgentStatus::NotFound;
        };
        let Ok(conversation) = state.get_conversation(agent_id).await else {
            return AgentStatus::NotFound;
        };
        conversation.agent_status().await
    }

    fn upgrade(&self) -> CodexResult<Arc<ConversationManagerState>> {
        self.manager.upgrade().ok_or_else(|| {
            CodexErr::UnsupportedOperation("conversation manager dropped".to_string())
        })
    }
}

/// When an agent is spawned "headless" (no UI/view attached), there may be no consumer polling
/// `CodexConversation::next_event()`. The underlying event channel is unbounded, so the producer can
/// accumulate events indefinitely. This drain task prevents that memory growth by polling and
/// discarding events until shutdown.
fn spawn_headless_drain(conversation: Arc<CodexConversation>) {
    tokio::spawn(async move {
        loop {
            match conversation.next_event().await {
                Ok(event) => {
                    if matches!(event.msg, EventMsg::ShutdownComplete) {
                        break;
                    }
                }
                Err(err) => {
                    tracing::warn!("failed to receive event from agent: {err:?}");
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::agent_status_from_event;
    use codex_protocol::protocol::ErrorEvent;
    use codex_protocol::protocol::TaskCompleteEvent;
    use codex_protocol::protocol::TaskStartedEvent;
    use codex_protocol::protocol::TurnAbortReason;
    use codex_protocol::protocol::TurnAbortedEvent;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn send_prompt_errors_when_manager_dropped() {
        let control = AgentControl::default();
        let err = control
            .send_prompt(ConversationId::new(), "hello".to_string())
            .await
            .expect_err("send_prompt should fail without a manager");
        assert_eq!(
            err.to_string(),
            "unsupported operation: conversation manager dropped"
        );
    }

    #[tokio::test]
    async fn get_status_returns_not_found_without_manager() {
        let control = AgentControl::default();
        let got = control.get_status(ConversationId::new()).await;
        assert_eq!(got, AgentStatus::NotFound);
    }

    #[tokio::test]
    async fn on_event_updates_status_from_task_started() {
        let status = agent_status_from_event(&EventMsg::TaskStarted(TaskStartedEvent {
            model_context_window: None,
        }));
        assert_eq!(status, Some(AgentStatus::Running));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_task_complete() {
        let status = agent_status_from_event(&EventMsg::TaskComplete(TaskCompleteEvent {
            last_agent_message: Some("done".to_string()),
        }));
        let expected = AgentStatus::Completed(Some("done".to_string()));
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_error() {
        let status = agent_status_from_event(&EventMsg::Error(ErrorEvent {
            message: "boom".to_string(),
            codex_error_info: None,
        }));

        let expected = AgentStatus::Errored("boom".to_string());
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_turn_aborted() {
        let status = agent_status_from_event(&EventMsg::TurnAborted(TurnAbortedEvent {
            reason: TurnAbortReason::Interrupted,
        }));

        let expected = AgentStatus::Errored("Interrupted".to_string());
        assert_eq!(status, Some(expected));
    }

    #[tokio::test]
    async fn on_event_updates_status_from_shutdown_complete() {
        let status = agent_status_from_event(&EventMsg::ShutdownComplete);
        assert_eq!(status, Some(AgentStatus::Shutdown));
    }
}
