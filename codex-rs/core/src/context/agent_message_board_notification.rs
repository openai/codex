//! Attributed discussion notices with bounded previews.

use super::ContextualUserFragment;
use codex_message_board::PostPreview;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

pub(crate) struct AgentMessageBoardNotification(pub(crate) PostPreview);

impl ContextualUserFragment for AgentMessageBoardNotification {
    fn role(&self) -> &'static str {
        "assistant"
    }
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("agent_message_board.notification".into())
    }
    fn markers(&self) -> (&'static str, &'static str) {
        ("", "")
    }
    fn type_markers() -> (&'static str, &'static str) {
        // Keep recognizing user-role notices in rollouts written before this format.
        (
            "<agent_message_board_notification>",
            "</agent_message_board_notification>",
        )
    }
    fn body(&self) -> ContentItem {
        let post = &self.0;
        let suffix = if post.truncated {
            "\n[Use read_post for the rest.]"
        } else {
            ""
        };
        input_text(format!(
            "Message Type: CHANNEL_POST\nSender: {}\nChannel: {}\nMessage ID: {}\nThread ID: {}\nPayload:\n{}{suffix}",
            post.metadata.author,
            post.metadata.channel_name,
            post.metadata.message_id,
            post.metadata.thread_id,
            post.text_preview,
        ))
    }
}
