//! Fixed-size discussion notices. Post text is fetched through bounded tools.

use super::ContextualUserFragment;
use codex_protocol::models::ContentItemKind;
use uuid::Uuid;

pub(crate) struct AgentMessageBoardNotification {
    pub(crate) message_id: Uuid,
    pub(crate) thread_id: Uuid,
}

impl ContextualUserFragment for AgentMessageBoardNotification {
    fn role(&self) -> &'static str {
        "user"
    }
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("agent_message_board.notification".into())
    }
    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }
    fn type_markers() -> (&'static str, &'static str) {
        (
            "<agent_message_board_notification>",
            "</agent_message_board_notification>",
        )
    }
    fn body(&self) -> String {
        format!(
            "\nNew post {} in discussion {}. Use collaboration.read_post or collaboration.read_thread to read it.\n",
            self.message_id, self.thread_id
        )
    }
}
