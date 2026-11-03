//! Recognizes legacy board notices in saved user-role messages. New notices use agent mail.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;

pub(crate) struct AgentMessageBoardNotification;

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
        input_text(String::new())
    }
}
