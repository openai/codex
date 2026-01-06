use crate::CodexConversation;
use crate::agent::AgentBus;
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
    /// Shared agent status store updated from emitted events.
    pub(crate) bus: AgentBus,
}

impl AgentControl {
    /// Construct a new `AgentControl` that can spawn/message agents via the given manager state.
    pub(crate) fn new(manager: Weak<ConversationManagerState>, bus: AgentBus) -> Self {
        Self { manager, bus }
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

        self.bus
            .record_status(&new_conversation.conversation_id, AgentStatus::PendingInit)
            .await;

        if headless {
            spawn_headless_drain(
                Arc::clone(&new_conversation.conversation),
                new_conversation.conversation_id,
                self.clone(),
            );
        }

        self.send_prompt(new_conversation.conversation_id, prompt)
            .await?;

        self.bus
            .record_status(&new_conversation.conversation_id, AgentStatus::Running)
            .await;

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
fn spawn_headless_drain(
    conversation: Arc<CodexConversation>,
    conversation_id: ConversationId,
    agent_control: AgentControl,
) {
    tokio::spawn(async move {
        loop {
            match conversation.next_event().await {
                Ok(event) => {
                    if matches!(event.msg, EventMsg::ShutdownComplete) {
                        break;
                    }
                }
                Err(err) => {
                    agent_control
                        .bus
                        .record_status(&conversation_id, AgentStatus::Errored(err.to_string()))
                        .await;
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn record_status_persists_to_bus() {
        let control = AgentControl::default();
        let conversation_id = ConversationId::new();

        control
            .bus
            .record_status(&conversation_id, AgentStatus::PendingInit)
            .await;

        let got = control.bus.status(conversation_id).await;
        assert_eq!(got, AgentStatus::PendingInit);
    }

    #[tokio::test]
    async fn on_event_updates_status_from_task_started() {
        let control = AgentControl::default();
        let conversation_id = ConversationId::new();

        control
            .bus
            .on_event(
                conversation_id,
                &EventMsg::TaskStarted(TaskStartedEvent {
                    model_context_window: None,
                }),
            )
            .await;

        let got = control.bus.status(conversation_id).await;
        assert_eq!(got, AgentStatus::Running);
    }

    #[tokio::test]
    async fn on_event_updates_status_from_task_complete() {
        let control = AgentControl::default();
        let conversation_id = ConversationId::new();

        control
            .bus
            .on_event(
                conversation_id,
                &EventMsg::TaskComplete(TaskCompleteEvent {
                    last_agent_message: Some("done".to_string()),
                }),
            )
            .await;

        let expected = AgentStatus::Completed(Some("done".to_string()));
        let got = control.bus.status(conversation_id).await;
        assert_eq!(got, expected);
    }

    #[tokio::test]
    async fn on_event_updates_status_from_error() {
        let control = AgentControl::default();
        let conversation_id = ConversationId::new();

        control
            .bus
            .on_event(
                conversation_id,
                &EventMsg::Error(ErrorEvent {
                    message: "boom".to_string(),
                    codex_error_info: None,
                }),
            )
            .await;

        let expected = AgentStatus::Errored("boom".to_string());
        let got = control.bus.status(conversation_id).await;
        assert_eq!(got, expected);
    }

    #[tokio::test]
    async fn on_event_updates_status_from_turn_aborted() {
        let control = AgentControl::default();
        let conversation_id = ConversationId::new();

        control
            .bus
            .on_event(
                conversation_id,
                &EventMsg::TurnAborted(TurnAbortedEvent {
                    reason: TurnAbortReason::Interrupted,
                }),
            )
            .await;

        let expected = AgentStatus::Errored("Interrupted".to_string());
        let got = control.bus.status(conversation_id).await;
        assert_eq!(got, expected);
    }

    #[tokio::test]
    async fn on_event_updates_status_from_shutdown_complete() {
        let control = AgentControl::default();
        let conversation_id = ConversationId::new();

        control
            .bus
            .on_event(conversation_id, &EventMsg::ShutdownComplete)
            .await;

        let got = control.bus.status(conversation_id).await;
        assert_eq!(got, AgentStatus::Shutdown);
    }
}
