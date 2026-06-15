use codex_protocol::protocol::AgentStatus;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::truncate_text;

use super::ContextualUserFragment;

const ERROR_MAX_TOKENS: usize = 1_000;
const ERROR_NEXT_ACTION: &str = "This agent's turn failed. If you still need this agent, use `followup_task` to give it another task.";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SubagentNotification {
    pub(crate) agent_reference: String,
    pub(crate) status: AgentStatus,
    next_action: Option<&'static str>,
}

impl SubagentNotification {
    pub(crate) fn new(agent_reference: impl Into<String>, status: AgentStatus) -> Self {
        let next_action = matches!(status, AgentStatus::Errored(_)).then_some(ERROR_NEXT_ACTION);
        let status = match status {
            AgentStatus::Errored(error) => AgentStatus::Errored(truncate_text(
                &error,
                TruncationPolicy::Tokens(ERROR_MAX_TOKENS),
            )),
            status => status,
        };
        Self {
            agent_reference: agent_reference.into(),
            status,
            next_action,
        }
    }
}

impl ContextualUserFragment for SubagentNotification {
    fn role(&self) -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<subagent_notification>", "</subagent_notification>")
    }

    fn body(&self) -> String {
        let body = match self.next_action {
            Some(next_action) => serde_json::json!({
                "agent_path": &self.agent_reference,
                "status": &self.status,
                "next_action": next_action,
            }),
            None => serde_json::json!({
                "agent_path": &self.agent_reference,
                "status": &self.status,
            }),
        };
        format!("\n{}\n", body)
    }
}
