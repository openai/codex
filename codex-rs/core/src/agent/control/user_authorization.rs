use super::AgentControl;
use crate::codex_thread::GuardianAuthorizationVersion;
use crate::codex_thread::GuardianRootMessage;
use crate::codex_thread::GuardianRootSnapshot;
use crate::compact::is_summary_message;
use crate::context::GuardianReviewEvidence;
use crate::event_mapping::parse_turn_item;
use crate::guardian::guardian_truncate_text;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::items::AgentMessageContent;
use codex_protocol::items::TurnItem;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::MultiAgentVersion;

const MAX_ROOT_MESSAGES: usize = 8;
const MAX_ROOT_MESSAGE_TOKENS: usize = 900;

impl AgentControl {
    /// Returns bounded root conversation and authorization state for a MultiAgent V2 worker.
    pub(crate) async fn root_user_authorization(
        &self,
        thread_id: ThreadId,
    ) -> Option<GuardianRootSnapshot> {
        let root_thread_id = self.state.agent_id_for_path(&AgentPath::root())?;
        if root_thread_id == thread_id {
            return None;
        }
        let manager = self.upgrade().ok()?;
        let root_thread = manager.get_thread(root_thread_id).await.ok()?;
        if root_thread.multi_agent_version() != Some(MultiAgentVersion::V2) {
            return None;
        }

        let root_history = root_thread.session.clone_history().await;
        let root_evidence = root_thread
            .session
            .services
            .thread_extension_data
            .get::<GuardianReviewEvidence>();
        let mut user_message_count = 0usize;
        let mut messages = root_history
            .raw_items()
            .filter_map(|item| match (parse_turn_item(item), item) {
                (Some(TurnItem::UserMessage(message)), _) => {
                    let message = message.message();
                    (!is_summary_message(&message)
                        && !message.trim_start().starts_with("<user_action>"))
                    .then(|| {
                        user_message_count = user_message_count.saturating_add(1);
                        GuardianRootMessage::User(
                            guardian_truncate_text(&message, MAX_ROOT_MESSAGE_TOKENS).0,
                        )
                    })
                }
                (Some(TurnItem::AgentMessage(message)), _)
                    if matches!(message.phase, None | Some(MessagePhase::FinalAnswer)) =>
                {
                    let text = message
                        .content
                        .iter()
                        .map(|content| match content {
                            AgentMessageContent::Text { text } => text.as_str(),
                        })
                        .collect::<String>();
                    Some(GuardianRootMessage::Assistant(
                        guardian_truncate_text(&text, MAX_ROOT_MESSAGE_TOKENS).0,
                    ))
                }
                (_, ResponseItem::FunctionCall { call_id, .. }) => root_evidence
                    .as_ref()
                    .and_then(|evidence| evidence.user_input_for_call(call_id))
                    .map(GuardianRootMessage::UserInput),
                _ => None,
            })
            .collect::<Vec<_>>();
        messages.drain(..messages.len().saturating_sub(MAX_ROOT_MESSAGES));
        let history = root_history.conversation_history_snapshot();
        let authorization_version = root_evidence.as_ref().map_or_else(
            || GuardianAuthorizationVersion::from_history(history.as_ref()),
            |evidence| evidence.authorization_version(history.as_ref()),
        );
        Some(GuardianRootSnapshot {
            authorization_version: GuardianAuthorizationVersion {
                user_message_count,
                ..authorization_version
            },
            messages,
        })
    }
}
