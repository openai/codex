use crate::config::Config;
use crate::protocol::SandboxPolicy;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;

/// Base instructions for the orchestrator role.
const ORCHESTRATOR_PROMPT: &str = include_str!("../../templates/agents/orchestrator.md");
/// Default model override used.
// TODO(jif) update when we have something smarter.
const EXPLORER_MODEL: &str = "gpt-5.2-codex";

/// Enumerated list of all supported agent roles.
const ALL_ROLES: [AgentRole; 3] = [
    AgentRole::Default,
    AgentRole::Explorer,
    AgentRole::Worker,
    // TODO(jif) add when we have stable prompts + models
    // AgentRole::Orchestrator,
];

/// Hard-coded agent role selection used when spawning sub-agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Inherit the parent agent's configuration unchanged.
    Default,
    /// Coordination-only agent that delegates to workers.
    Orchestrator,
    /// Task-executing agent with a fixed model override.
    Worker,
    /// Task-executing agent with a fixed model override.
    Explorer,
}

/// Immutable profile data that drives per-agent configuration overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AgentProfile {
    /// Optional base instructions override.
    pub base_instructions: Option<&'static str>,
    /// Optional model override.
    pub model: Option<&'static str>,
    /// Optional reasoning effort override.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Whether to force a read-only sandbox policy.
    pub read_only: bool,
}

impl AgentRole {
    /// Returns the string values used by JSON schema enums.
    pub fn enum_values() -> Vec<String> {
        ALL_ROLES
            .iter()
            .filter_map(|role| serde_json::to_string(role).ok())
            .collect()
    }

    /// Returns the hard-coded profile for this role.
    pub fn profile(self) -> AgentProfile {
        match self {
            AgentRole::Default => AgentProfile::default(),
            AgentRole::Orchestrator => AgentProfile {
                base_instructions: Some(ORCHESTRATOR_PROMPT),
                ..Default::default()
            },
            AgentRole::Worker => AgentProfile {
                // base_instructions: Some(WORKER_PROMPT),
                // model: Some(WORKER_MODEL),
                ..Default::default()
            },
            AgentRole::Explorer => AgentProfile {
                model: Some(EXPLORER_MODEL),
                reasoning_effort: Some(ReasoningEffort::Low),
                ..Default::default()
            },
        }
    }

    /// Applies this role's profile onto the provided config.
    pub fn apply_to_config(self, config: &mut Config) -> Result<(), String> {
        let profile = self.profile();
        if let Some(base_instructions) = profile.base_instructions {
            config.base_instructions = Some(base_instructions.to_string());
        }
        if let Some(model) = profile.model {
            config.model = Some(model.to_string());
        }
        if let Some(reasoning_effort) = profile.reasoning_effort {
            config.model_reasoning_effort = Some(reasoning_effort)
        }
        if profile.read_only {
            config
                .sandbox_policy
                .set(SandboxPolicy::new_read_only_policy())
                .map_err(|err| format!("sandbox_policy is invalid: {err}"))?;
        }
        Ok(())
    }
}
