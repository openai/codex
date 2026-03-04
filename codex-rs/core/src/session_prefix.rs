use codex_protocol::protocol::AgentStatus;

/// Helpers for model-visible session state markers that are stored in user-role
/// messages but are not user intent.
use crate::contextual_user_message::ContextualUserFragment;
use crate::contextual_user_message::SUBAGENT_NOTIFICATION_FRAGMENT;

struct SubagentNotification<'a> {
    agent_id: &'a str,
    status: &'a AgentStatus,
}

impl ContextualUserFragment for SubagentNotification<'_> {
    fn definition(&self) -> crate::contextual_user_message::ContextualUserFragmentDefinition {
        SUBAGENT_NOTIFICATION_FRAGMENT
    }

    fn serialize_to_text(&self) -> String {
        let payload_json = serde_json::json!({
            "agent_id": self.agent_id,
            "status": self.status,
        })
        .to_string();
        SUBAGENT_NOTIFICATION_FRAGMENT.wrap_body(payload_json)
    }
}

pub(crate) fn format_subagent_notification_message(agent_id: &str, status: &AgentStatus) -> String {
    SubagentNotification { agent_id, status }.serialize_to_text()
}

pub(crate) fn format_subagent_context_line(agent_id: &str, agent_nickname: Option<&str>) -> String {
    match agent_nickname.filter(|nickname| !nickname.is_empty()) {
        Some(agent_nickname) => format!("- {agent_id}: {agent_nickname}"),
        None => format!("- {agent_id}"),
    }
}
