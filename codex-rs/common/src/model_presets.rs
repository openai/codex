use codex_app_server_protocol::AuthMode;
use codex_core::protocol_config_types::ReasoningEffort;

/// A reasoning effort option that can be surfaced for a model.
#[derive(Debug, Clone, Copy)]
pub struct ReasoningEffortPreset {
    /// Effort level that the model supports.
    pub effort: ReasoningEffort,
    /// Short human description shown next to the effort in UIs.
    pub description: &'static str,
}

/// Metadata describing a Codex-supported model.
#[derive(Debug, Clone, Copy)]
pub struct ModelPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Model slug (e.g., "gpt-5").
    pub model: &'static str,
    /// Display name shown in UIs.
    pub display_name: &'static str,
    /// Short human description shown in UIs.
    pub description: &'static str,
    /// Reasoning effort applied when none is explicitly chosen.
    pub default_reasoning_effort: ReasoningEffort,
    /// Supported reasoning effort options.
    pub supported_reasoning_efforts: &'static [ReasoningEffortPreset],
    /// Whether this is the default model for new users.
    pub is_default: bool,
}

const PRESETS: &[ModelPreset] = &[
    ModelPreset {
        id: "gpt-5-codex",
        model: "gpt-5-codex",
        display_name: "gpt-5-codex",
        description: "Optimized for coding tasks with many tools.",
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: &[
            ReasoningEffortPreset {
                effort: ReasoningEffort::Low,
                description: "Fastest responses with limited reasoning",
            },
            ReasoningEffortPreset {
                effort: ReasoningEffort::Medium,
                description: "Dynamically adjusts reasoning based on the task",
            },
            ReasoningEffortPreset {
                effort: ReasoningEffort::High,
                description: "Maximizes reasoning depth for complex or ambiguous problems",
            },
        ],
        is_default: true,
    },
    ModelPreset {
        id: "gpt-5",
        model: "gpt-5",
        display_name: "gpt-5",
        description: "Broad world knowledge with strong general reasoning.",
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: &[
            ReasoningEffortPreset {
                effort: ReasoningEffort::Minimal,
                description: "Fastest responses with little reasoning",
            },
            ReasoningEffortPreset {
                effort: ReasoningEffort::Low,
                description: "Balances speed with some reasoning; useful for straightforward queries and short explanations",
            },
            ReasoningEffortPreset {
                effort: ReasoningEffort::Medium,
                description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
            },
            ReasoningEffortPreset {
                effort: ReasoningEffort::High,
                description: "Maximizes reasoning depth for complex or ambiguous problems",
            },
        ],
        is_default: false,
    },
];

pub fn builtin_model_presets(_auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    PRESETS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_default_model_is_configured() {
        let default_models = PRESETS.iter().filter(|preset| preset.is_default).count();
        assert!(default_models == 1);
    }
}
