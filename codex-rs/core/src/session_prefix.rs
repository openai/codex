use codex_protocol::protocol::AgentStatus;

/// Helpers for model-visible subagent session state rendered in the developer
/// envelope.
use crate::model_visible_context::DEVELOPER_FRAGMENT_SPEC;
use crate::model_visible_context::DeveloperContextRole;
use crate::model_visible_context::ModelVisibleContextFragment;
use crate::model_visible_context::SUBAGENT_NOTIFICATION_CLOSE_TAG;
use crate::model_visible_context::SUBAGENT_NOTIFICATION_OPEN_TAG;
use crate::model_visible_context::SUBAGENTS_CLOSE_TAG;
use crate::model_visible_context::SUBAGENTS_OPEN_TAG;

pub(crate) struct SubagentRosterContext {
    subagents: String,
}

impl SubagentRosterContext {
    pub(crate) fn new(subagents: String) -> Option<Self> {
        if subagents.is_empty() {
            None
        } else {
            Some(Self { subagents })
        }
    }
}

impl ModelVisibleContextFragment for SubagentRosterContext {
    type Role = DeveloperContextRole;

    fn spec(&self) -> crate::model_visible_context::ModelVisibleContextFragmentSpec {
        DEVELOPER_FRAGMENT_SPEC
    }

    fn render_text(&self) -> String {
        let lines = self
            .subagents
            .lines()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{SUBAGENTS_OPEN_TAG}\n{lines}\n{SUBAGENTS_CLOSE_TAG}")
    }
}

struct SubagentNotification<'a> {
    agent_id: &'a str,
    status: &'a AgentStatus,
}

impl ModelVisibleContextFragment for SubagentNotification<'_> {
    type Role = DeveloperContextRole;

    fn spec(&self) -> crate::model_visible_context::ModelVisibleContextFragmentSpec {
        DEVELOPER_FRAGMENT_SPEC
    }

    fn render_text(&self) -> String {
        let payload_json = serde_json::json!({
            "agent_id": self.agent_id,
            "status": self.status,
        })
        .to_string();
        format!(
            "{SUBAGENT_NOTIFICATION_OPEN_TAG}\n{payload_json}\n{SUBAGENT_NOTIFICATION_CLOSE_TAG}"
        )
    }
}

pub(crate) fn format_subagent_notification_message(agent_id: &str, status: &AgentStatus) -> String {
    SubagentNotification { agent_id, status }.render_text()
}

pub(crate) fn format_subagent_context_line(agent_id: &str, agent_nickname: Option<&str>) -> String {
    match agent_nickname.filter(|nickname| !nickname.is_empty()) {
        Some(agent_nickname) => format!("- {agent_id}: {agent_nickname}"),
        None => format!("- {agent_id}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn serializes_subagent_roster_context() {
        let context =
            SubagentRosterContext::new("- agent-1: Atlas\n- agent-2: Juniper".to_string())
                .expect("context expected");

        assert_eq!(
            context.render_text(),
            "<subagents>\n  - agent-1: Atlas\n  - agent-2: Juniper\n</subagents>"
        );
    }

    #[test]
    fn skips_empty_subagent_roster_context() {
        assert!(SubagentRosterContext::new(String::new()).is_none());
    }
}
