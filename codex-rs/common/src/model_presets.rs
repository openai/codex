use codex_app_server_protocol::AuthMode;
use codex_core::protocol_config_types::ReasoningEffort;

/// A simple preset pairing a model slug with a reasoning effort.
#[derive(Debug, Clone, Copy)]
pub struct ModelPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Display label shown in UIs.
    pub label: &'static str,
    /// Short human description shown next to the label in UIs.
    pub description: &'static str,
    /// Model slug (e.g., "gpt-5").
    pub model: &'static str,
    /// Reasoning effort to apply for this preset.
    pub effort: Option<ReasoningEffort>,
}

const PRESETS: &[ModelPreset] = &[
    ModelPreset {
        id: "gpt-5-codex-low",
        label: "gpt-5-codex low",
        description: "Fastest responses with limited reasoning",
        model: "gpt-5-codex",
        effort: Some(ReasoningEffort::Low),
    },
    ModelPreset {
        id: "gpt-5-codex-medium",
        label: "gpt-5-codex medium",
        description: "Dynamically adjusts reasoning based on the task",
        model: "gpt-5-codex",
        effort: Some(ReasoningEffort::Medium),
    },
    ModelPreset {
        id: "gpt-5-codex-high",
        label: "gpt-5-codex high",
        description: "Maximizes reasoning depth for complex or ambiguous problems",
        model: "gpt-5-codex",
        effort: Some(ReasoningEffort::High),
    },
    ModelPreset {
        id: "gpt-5-minimal",
        label: "gpt-5 minimal",
        description: "Fastest responses with little reasoning",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Minimal),
    },
    ModelPreset {
        id: "gpt-5-low",
        label: "gpt-5 low",
        description: "Balances speed with some reasoning; useful for straightforward queries and short explanations",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Low),
    },
    ModelPreset {
        id: "gpt-5-medium",
        label: "gpt-5 medium",
        description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Medium),
    },
    ModelPreset {
        id: "gpt-5-high",
        label: "gpt-5 high",
        description: "Maximizes reasoning depth for complex or ambiguous problems",
        model: "gpt-5",
        effort: Some(ReasoningEffort::High),
    },
];

pub fn builtin_model_presets(_auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    PRESETS.to_vec()
}
