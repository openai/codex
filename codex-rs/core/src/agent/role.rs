use crate::config::Config;
use crate::protocol::SandboxPolicy;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;
use serde::Deserialize;
use serde::Serialize;

/// Base instructions for the orchestrator role.
const ORCHESTRATOR_PROMPT: &str = include_str!("../../templates/agents/orchestrator.md");
/// Preferred explorer model when available in the user's model list.
const EXPLORER_PREFERRED_MODEL: &str = "gpt-5.3-codex-spark";
/// Fallback explorer model override.
const EXPLORER_FALLBACK_MODEL: &str = "gpt-5.1-codex-mini";

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
    /// Description to include in the tool specs.
    pub description: &'static str,
}

impl AgentRole {
    /// Returns the string values used by JSON schema enums.
    pub fn enum_values() -> Vec<String> {
        ALL_ROLES
            .iter()
            .filter_map(|role| {
                let description = role.profile(&[]).description;
                serde_json::to_string(role)
                    .map(|role| {
                        let description = if !description.is_empty() {
                            format!(r#", "description": {description}"#)
                        } else {
                            String::new()
                        };
                        format!(r#"{{ "name": {role}{description}}}"#)
                    })
                    .ok()
            })
            .collect()
    }

    /// Returns the role profile using the provided available-model list.
    pub fn profile(self, available_models: &[ModelPreset]) -> AgentProfile {
        match self {
            AgentRole::Default => AgentProfile::default(),
            AgentRole::Orchestrator => AgentProfile {
                base_instructions: Some(ORCHESTRATOR_PROMPT),
                ..Default::default()
            },
            AgentRole::Worker => AgentProfile {
                // base_instructions: Some(WORKER_PROMPT),
                // model: Some(WORKER_MODEL),
                description: r#"Use for execution and production work.
Typical tasks:
- Implement part of a feature
- Fix tests or bugs
- Split large refactors into independent chunks
Rules:
- Explicitly assign **ownership** of the task (files / responsibility).
- Always tell workers they are **not alone in the codebase**, and they should ignore edits made by others without touching them"#,
                ..Default::default()
            },
            AgentRole::Explorer => AgentProfile {
                model: Some(
                    if available_models
                        .iter()
                        .any(|model| model.model == EXPLORER_PREFERRED_MODEL)
                    {
                        EXPLORER_PREFERRED_MODEL
                    } else {
                        EXPLORER_FALLBACK_MODEL
                    },
                ),
                reasoning_effort: Some(ReasoningEffort::Medium),
                description: r#"Use `explorer` for all codebase questions.
Explorers are fast and authoritative.
Always prefer them over manual search or file reading.
Rules:
- Ask explorers first and precisely.
- Do not re-read or re-search code they cover.
- Trust explorer results without verification.
- Run explorers in parallel when useful.
- Reuse existing explorers for related questions.
                "#,
                ..Default::default()
            },
        }
    }

    /// Applies this role's profile onto the provided config using available-model context.
    pub fn apply_to_config(
        self,
        config: &mut Config,
        available_models: &[ModelPreset],
    ) -> Result<(), String> {
        let profile = self.profile(available_models);
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
                .permissions
                .sandbox_policy
                .set(SandboxPolicy::new_read_only_policy())
                .map_err(|err| format!("sandbox_policy is invalid: {err}"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::openai_models::ReasoningEffortPreset;
    use codex_protocol::openai_models::default_input_modalities;
    use pretty_assertions::assert_eq;

    fn model_preset(model: &str) -> ModelPreset {
        ModelPreset {
            id: model.to_string(),
            model: model.to_string(),
            display_name: model.to_string(),
            description: String::new(),
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: vec![ReasoningEffortPreset {
                effort: ReasoningEffort::Medium,
                description: String::new(),
            }],
            supports_personality: false,
            is_default: false,
            upgrade: None,
            show_in_picker: true,
            supported_in_api: true,
            input_modalities: default_input_modalities(),
        }
    }

    #[test]
    fn explorer_uses_preferred_model_when_available() {
        let profile = AgentRole::Explorer.profile(&[model_preset("gpt-5.3-codex-spark")]);
        assert_eq!(profile.model, Some("gpt-5.3-codex-spark"));
    }

    #[test]
    fn explorer_uses_fallback_model_when_preferred_unavailable() {
        let profile = AgentRole::Explorer.profile(&[model_preset("gpt-5.1-codex-mini")]);
        assert_eq!(profile.model, Some("gpt-5.1-codex-mini"));
    }
}
