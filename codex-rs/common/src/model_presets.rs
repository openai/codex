use std::collections::HashMap;

use codex_app_server_protocol::AuthMode;
use codex_core::protocol_config_types::ReasoningEffort;
use once_cell::sync::Lazy;

/// A reasoning effort option that can be surfaced for a model.
#[derive(Debug, Clone, Copy)]
pub struct ReasoningEffortPreset {
    /// Effort level that the model supports.
    pub effort: ReasoningEffort,
    /// Short human description shown next to the effort in UIs.
    pub description: &'static str,
    /// Optional friendly label shown in featured pickers.
    pub label: Option<&'static str>,
}

impl ReasoningEffortPreset {
    pub const fn new(
        effort: ReasoningEffort,
        description: &'static str,
        label: Option<&'static str>,
    ) -> Self {
        Self {
            effort,
            description,
            label,
        }
    }

    pub const fn with_label(
        effort: ReasoningEffort,
        description: &'static str,
        label: &'static str,
    ) -> Self {
        Self {
            effort,
            description,
            label: Some(label),
        }
    }

    pub fn label(&self) -> &'static str {
        self.label
            .unwrap_or_else(|| default_reasoning_effort_label(self.effort))
    }
}

const fn default_reasoning_effort_label(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::None => "None",
        ReasoningEffort::Minimal => "Minimal",
        ReasoningEffort::Low => "Low",
        ReasoningEffort::Medium => "Medium",
        ReasoningEffort::High => "High",
    }
}

#[derive(Debug, Clone)]
pub struct ModelUpgrade {
    pub id: &'static str,
    pub reasoning_effort_mapping: Option<HashMap<ReasoningEffort, ReasoningEffort>>,
}

/// Metadata describing a Codex-supported model.
#[derive(Debug, Clone)]
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
    /// recommended upgrade model
    pub upgrade: Option<ModelUpgrade>,
}

static PRESETS: Lazy<Vec<ModelPreset>> = Lazy::new(|| {
    vec![
        ModelPreset {
            id: "codex-auto",
            model: "codex-auto",
            display_name: "codex-auto",
            description: "Automatically chooses the best Codex model configuration for your task.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset::with_label(ReasoningEffort::Low, "Works faster", "Fast"),
                ReasoningEffortPreset::with_label(
                    ReasoningEffort::Medium,
                    "Balances speed with intelligence",
                    "Balanced",
                ),
                ReasoningEffortPreset::with_label(
                    ReasoningEffort::High,
                    "Works longer for harder tasks",
                    "Thorough",
                ),
            ],
            is_default: true,
            upgrade: None,
        },
        ModelPreset {
            id: "gpt-5.1-codex",
            model: "gpt-5.1-codex",
            display_name: "gpt-5.1-codex",
            description: "Optimized for codex.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset::new(
                    ReasoningEffort::Low,
                    "Fastest responses with limited reasoning",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::Medium,
                    "Dynamically adjusts reasoning based on the task",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::High,
                    "Maximizes reasoning depth for complex or ambiguous problems",
                    None,
                ),
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "gpt-5.1-codex-mini",
            model: "gpt-5.1-codex-mini",
            display_name: "gpt-5.1-codex-mini",
            description: "Optimized for codex. Cheaper, faster, but less capable.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset::new(
                    ReasoningEffort::Medium,
                    "Dynamically adjusts reasoning based on the task",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::High,
                    "Maximizes reasoning depth for complex or ambiguous problems",
                    None,
                ),
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "gpt-5.1",
            model: "gpt-5.1",
            display_name: "gpt-5.1",
            description: "Broad world knowledge with strong general reasoning.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset::new(
                    ReasoningEffort::Low,
                    "Balances speed with some reasoning; useful for straightforward queries and short explanations",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::Medium,
                    "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::High,
                    "Maximizes reasoning depth for complex or ambiguous problems",
                    None,
                ),
            ],
            is_default: false,
            upgrade: None,
        },
        // Deprecated models.
        ModelPreset {
            id: "gpt-5-codex",
            model: "gpt-5-codex",
            display_name: "gpt-5-codex",
            description: "Optimized for codex.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset::new(
                    ReasoningEffort::Low,
                    "Fastest responses with limited reasoning",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::Medium,
                    "Dynamically adjusts reasoning based on the task",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::High,
                    "Maximizes reasoning depth for complex or ambiguous problems",
                    None,
                ),
            ],
            is_default: false,
            upgrade: Some(ModelUpgrade {
                id: "gpt-5.1-codex",
                reasoning_effort_mapping: None,
            }),
        },
        ModelPreset {
            id: "gpt-5-codex-mini",
            model: "gpt-5-codex-mini",
            display_name: "gpt-5-codex-mini",
            description: "Optimized for codex. Cheaper, faster, but less capable.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset::new(
                    ReasoningEffort::Medium,
                    "Dynamically adjusts reasoning based on the task",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::High,
                    "Maximizes reasoning depth for complex or ambiguous problems",
                    None,
                ),
            ],
            is_default: false,
            upgrade: Some(ModelUpgrade {
                id: "gpt-5.1-codex-mini",
                reasoning_effort_mapping: None,
            }),
        },
        ModelPreset {
            id: "gpt-5",
            model: "gpt-5",
            display_name: "gpt-5",
            description: "Broad world knowledge with strong general reasoning.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset::new(
                    ReasoningEffort::Minimal,
                    "Fastest responses with little reasoning",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::Low,
                    "Balances speed with some reasoning; useful for straightforward queries and short explanations",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::Medium,
                    "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
                    None,
                ),
                ReasoningEffortPreset::new(
                    ReasoningEffort::High,
                    "Maximizes reasoning depth for complex or ambiguous problems",
                    None,
                ),
            ],
            is_default: false,
            upgrade: Some(ModelUpgrade {
                id: "gpt-5.1",
                reasoning_effort_mapping: Some(HashMap::from([(
                    ReasoningEffort::Minimal,
                    ReasoningEffort::Low,
                )])),
            }),
        },
    ]
});

pub fn builtin_model_presets(_auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    // leave auth mode for later use
    PRESETS
        .iter()
        .filter(|preset| preset.upgrade.is_none())
        .cloned()
        .collect()
}

pub fn all_model_presets() -> &'static Vec<ModelPreset> {
    &PRESETS
}

impl ModelPreset {
    pub fn reasoning_effort_label(&self, effort: ReasoningEffort) -> &'static str {
        self.supported_reasoning_efforts
            .iter()
            .find(|option| option.effort == effort)
            .map(ReasoningEffortPreset::label)
            .unwrap_or_else(|| default_reasoning_effort_label(effort))
    }
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
