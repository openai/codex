use codex_protocol::protocol::AgentStatus;
use serde::Deserialize;

/// Helpers for identifying model-visible "session prefix" messages.
///
/// A session prefix is a user-role message that carries configuration or state needed by
/// follow-up turns (e.g. `<environment_context>`, `<turn_aborted>`). These items are persisted in
/// history so the model can see them, but they are not user intent and must not create user-turn
/// boundaries.
pub(crate) const ENVIRONMENT_CONTEXT_OPEN_TAG: &str = "<environment_context>";
pub(crate) const TURN_ABORTED_OPEN_TAG: &str = "<turn_aborted>";
pub(crate) const SUBAGENT_NOTIFICATION_OPEN_TAG: &str = "<subagent_notification>";
pub(crate) const SUBAGENT_NOTIFICATION_CLOSE_TAG: &str = "</subagent_notification>";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct SubagentNotification {
    pub(crate) agent_id: String,
    pub(crate) status: AgentStatus,
}

fn starts_with_ascii_case_insensitive(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

/// Returns true if `text` starts with a session prefix marker (case-insensitive).
pub(crate) fn is_session_prefix(text: &str) -> bool {
    let trimmed = text.trim_start();
    starts_with_ascii_case_insensitive(trimmed, ENVIRONMENT_CONTEXT_OPEN_TAG)
        || starts_with_ascii_case_insensitive(trimmed, TURN_ABORTED_OPEN_TAG)
        || starts_with_ascii_case_insensitive(trimmed, SUBAGENT_NOTIFICATION_OPEN_TAG)
}

pub(crate) fn format_subagent_notification_message(agent_id: &str, status: &AgentStatus) -> String {
    let payload_json = serde_json::json!({
        "agent_id": agent_id,
        "status": status,
    })
    .to_string();
    format!("{SUBAGENT_NOTIFICATION_OPEN_TAG}\n{payload_json}\n{SUBAGENT_NOTIFICATION_CLOSE_TAG}")
}

pub(crate) fn parse_subagent_notification(text: &str) -> Option<SubagentNotification> {
    let trimmed = text.trim();
    if !starts_with_ascii_case_insensitive(trimmed, SUBAGENT_NOTIFICATION_OPEN_TAG) {
        return None;
    }
    let end_index = trimmed.find(SUBAGENT_NOTIFICATION_CLOSE_TAG)?;
    let open_tag_len = SUBAGENT_NOTIFICATION_OPEN_TAG.len();
    if end_index <= open_tag_len {
        return None;
    }
    let payload = &trimmed[open_tag_len..end_index];
    serde_json::from_str::<SubagentNotification>(payload.trim()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_subagent_notification_round_trips_format() {
        let message = format_subagent_notification_message(
            "agent-1",
            &AgentStatus::Completed(Some("done".to_string())),
        );
        let parsed = parse_subagent_notification(&message);

        assert_eq!(
            parsed,
            Some(SubagentNotification {
                agent_id: "agent-1".to_string(),
                status: AgentStatus::Completed(Some("done".to_string())),
            })
        );
    }

    #[test]
    fn is_session_prefix_is_case_insensitive() {
        assert_eq!(
            is_session_prefix("<SUBAGENT_NOTIFICATION>{}</subagent_notification>"),
            true
        );
    }
}
