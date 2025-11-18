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
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Fastest responses with limited reasoning",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Balanced responses that adapt to the task",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximum reasoning depth for complex problems",
                },
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
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                },
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
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                },
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

/// Label metadata for featured pickers (e.g., codex-auto variants).
#[derive(Debug, Clone, Copy)]
struct FeaturedEffortLabel {
    model: &'static str,
    effort: ReasoningEffort,
    label: &'static str,
}

static FEATURED_EFFORT_LABELS: &[FeaturedEffortLabel] = &[
    FeaturedEffortLabel {
        model: "codex-auto",
        effort: ReasoningEffort::Low,
        label: "Fast",
    },
    FeaturedEffortLabel {
        model: "codex-auto",
        effort: ReasoningEffort::Medium,
        label: "Balanced",
    },
    FeaturedEffortLabel {
        model: "codex-auto",
        effort: ReasoningEffort::High,
        label: "Thorough",
    },
];

/// Returns a friendly label for the given model/effort combination when available.
pub fn effort_label_for_model(
    model: &str,
    effort: Option<ReasoningEffort>,
) -> Option<&'static str> {
    let effort = effort?;
    FEATURED_EFFORT_LABELS
        .iter()
        .find(|entry| entry.model == model && entry.effort == effort)
        .map(|entry| entry.label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_default_model_is_configured() {
        let default_models = PRESETS.iter().filter(|preset| preset.is_default).count();
        assert!(default_models == 1);
    }

    #[test]
    fn codex_auto_featured_options_define_labels() {
        assert_eq!(
            effort_label_for_model("codex-auto", Some(ReasoningEffort::Low)),
            Some("Fast")
        );
        assert_eq!(
            effort_label_for_model("codex-auto", Some(ReasoningEffort::Medium)),
            Some("Balanced")
        );
        assert_eq!(
            effort_label_for_model("codex-auto", Some(ReasoningEffort::High)),
            Some("Thorough")
        );
    }
}
