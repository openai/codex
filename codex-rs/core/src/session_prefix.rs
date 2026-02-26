use codex_protocol::protocol::AgentStatus;

/// Helpers for model-visible session state markers that are stored in user-role
/// messages but are not user intent.
use crate::contextual_user_message::SUBAGENT_NOTIFICATION_FRAGMENT;
#[cfg(test)]
pub(crate) const SUBAGENT_NOTIFICATION_OPEN_TAG: &str =
    crate::contextual_user_message::SUBAGENT_NOTIFICATION_OPEN_TAG;
pub(crate) const TURN_ABORTED_OPEN_TAG: &str =
    crate::contextual_user_message::TURN_ABORTED_OPEN_TAG;

pub(crate) fn format_subagent_notification_message(agent_id: &str, status: &AgentStatus) -> String {
    let payload_json = serde_json::json!({
        "agent_id": agent_id,
        "status": status,
    })
    .to_string();
    SUBAGENT_NOTIFICATION_FRAGMENT.wrap(payload_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_session_prefix_is_case_insensitive() {
        assert_eq!(
            SUBAGENT_NOTIFICATION_FRAGMENT
                .matches_text("<SUBAGENT_NOTIFICATION>{}</subagent_notification>"),
            true
        );
    }
}
