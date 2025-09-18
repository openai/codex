use codex_core::config::GPT_5_CODEX_MEDIUM_MODEL;
use codex_core::protocol_config_types::ReasoningEffort;
use codex_protocol::mcp_protocol::AuthMode;

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
        description: "",
        model: "gpt-5-codex",
        effort: Some(ReasoningEffort::Low),
    },
    ModelPreset {
        id: "gpt-5-codex-medium",
        label: "gpt-5-codex medium",
        description: "",
        model: "gpt-5-codex",
        effort: None,
    },
    ModelPreset {
        id: "gpt-5-codex-high",
        label: "gpt-5-codex high",
        description: "",
        model: "gpt-5-codex",
        effort: Some(ReasoningEffort::High),
    },
    ModelPreset {
        id: "gpt-5-minimal",
        label: "gpt-5 minimal",
        description: "— fastest responses with limited reasoning; ideal for coding, instructions, or lightweight tasks",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Minimal),
    },
    ModelPreset {
        id: "gpt-5-low",
        label: "gpt-5 low",
        description: "— balances speed with some reasoning; useful for straightforward queries and short explanations",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Low),
    },
    ModelPreset {
        id: "gpt-5-medium",
        label: "gpt-5 medium",
        description: "— default setting; provides a solid balance of reasoning depth and latency for general-purpose tasks",
        model: "gpt-5",
        effort: Some(ReasoningEffort::Medium),
    },
    ModelPreset {
        id: "gpt-5-high",
        label: "gpt-5 high",
        description: "— maximizes reasoning depth for complex or ambiguous problems",
        model: "gpt-5",
        effort: Some(ReasoningEffort::High),
    },
];

pub fn builtin_model_presets(auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    match auth_mode {
        Some(AuthMode::ApiKey) => PRESETS
            .iter()
            .copied()
            .filter(|p| p.model != GPT_5_CODEX_MEDIUM_MODEL)
            .collect(),
        _ => PRESETS.to_vec(),
    }
}
