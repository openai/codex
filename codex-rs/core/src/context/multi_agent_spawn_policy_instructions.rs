use super::ContextualUserFragment;
use crate::config::DEFAULT_MULTI_AGENT_V2_NO_SPAWN_HINT_TEXT;
use codex_protocol::config_types::MultiAgentMode;
use codex_protocol::protocol::MULTI_AGENT_SPAWN_POLICY_CLOSE_TAG;
use codex_protocol::protocol::MULTI_AGENT_SPAWN_POLICY_OPEN_TAG;

const PROACTIVE_MULTI_AGENT_SPAWN_POLICY_TEXT: &str = "Proactive multi-agent delegation is active. Any earlier instruction requiring an explicit user request before spawning sub-agents no longer applies. Use sub-agents when parallel work would materially improve speed or quality. This policy remains active until a later multi-agent spawn-policy developer message changes it.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MultiAgentSpawnPolicyInstructions {
    policy: MultiAgentMode,
}

impl MultiAgentSpawnPolicyInstructions {
    pub(crate) fn new(policy: MultiAgentMode) -> Self {
        Self { policy }
    }
}

impl ContextualUserFragment for MultiAgentSpawnPolicyInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            MULTI_AGENT_SPAWN_POLICY_OPEN_TAG,
            MULTI_AGENT_SPAWN_POLICY_CLOSE_TAG,
        )
    }

    fn body(&self) -> String {
        match self.policy {
            MultiAgentMode::ExplicitRequestOnly => {
                DEFAULT_MULTI_AGENT_V2_NO_SPAWN_HINT_TEXT.to_string()
            }
            MultiAgentMode::Proactive => PROACTIVE_MULTI_AGENT_SPAWN_POLICY_TEXT.to_string(),
        }
    }
}
