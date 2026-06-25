use crate::event_mapping::is_contextual_user_message_content;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::InterAgentCommunication;

/// Returns whether a response item starts a user-directed turn in stored history.
///
/// Context-only user messages do not start turns. Agent messages and structured
/// assistant instructions do, since they represent input that expects a response.
pub fn is_user_turn_boundary(item: &ResponseItem) -> bool {
    if matches!(item, ResponseItem::AgentMessage { .. }) {
        return true;
    }
    let ResponseItem::Message { role, content, .. } = item else {
        return false;
    };

    (role == "user" && !is_contextual_user_message_content(content))
        || (role == "assistant" && InterAgentCommunication::is_message_content(content))
}

#[cfg(test)]
#[path = "rollout_history_tests.rs"]
mod tests;
