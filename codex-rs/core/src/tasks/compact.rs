use std::sync::Arc;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::codex::TurnContext;
use crate::codex::compact;
use crate::codex_delegate::SubAgentRunParams;
use crate::codex_delegate::run_codex_conversation_one_shot;
use crate::protocol::EventMsg;
use crate::protocol::SubAgentSource;
use crate::state::TaskKind;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::user_input::UserInput;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone, Copy, Default)]
pub(crate) struct CompactTask;

#[async_trait]
impl SessionTask for CompactTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Compact
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        // Persist a TurnContext entry in the parent rollout so manual compact
        // still appears as a separate API turn in rollout, matching prior behavior.
        crate::codex::compact::persist_turn_context_rollout(
            session.clone_session().as_ref(),
            ctx.as_ref(),
        )
        .await;

        // Build initial forked history from parent so the sub-agent sees the
        // same context without mutating the parent transcript.
        let parent_history: Vec<ResponseItem> =
            session.clone_session().clone_history().await.get_history();
        let forked: Vec<RolloutItem> = parent_history
            .into_iter()
            .map(RolloutItem::ResponseItem)
            .collect();

        // Start sub-agent one-shot conversation for summarization.
        let config = ctx.client.config().as_ref().clone();
        let io = run_codex_conversation_one_shot(
            SubAgentRunParams {
                config,
                auth_manager: session.auth_manager(),
                initial_history: Some(codex_protocol::protocol::InitialHistory::Forked(forked)),
                sub_source: SubAgentSource::Compact,
                parent_session: session.clone_session(),
                parent_ctx: ctx.clone(),
                cancel_token: cancellation_token.clone(),
            },
            input,
        )
        .await;

        // Drain events and capture last_agent_message from TaskComplete.
        let mut summary_text: Option<String> = None;
        if let Ok(io) = io {
            while let Ok(event) = io.next_event().await {
                match event.msg {
                    EventMsg::TaskComplete(done) => {
                        summary_text = done.last_agent_message;
                        break;
                    }
                    EventMsg::TurnAborted(_) => break,
                    _ => {}
                }
            }
        }

        // Apply compaction into the parent session if we captured a summary.
        if let Some(summary_text) = summary_text {
            let parent_sess = session.clone_session();
            compact::apply_compaction(&parent_sess, &ctx, &summary_text).await;
            // Inform users that compaction finished.
            session
                .clone_session()
                .send_event(
                    ctx.as_ref(),
                    EventMsg::AgentMessage(crate::protocol::AgentMessageEvent {
                        message: "Compact task completed".to_string(),
                    }),
                )
                .await;
        }
        None
    }
}
