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
    pub fn label(&self) -> String {
        self.label
            .map(ToString::to_string)
            .unwrap_or_else(|| self.effort.to_string())
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
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Works faster",
                    label: Some("Fast"),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Balances speed with intelligence",
                    label: Some("Balanced"),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Works longer for harder tasks",
                    label: Some("Thorough"),
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
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task",
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                    label: None,
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
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                    label: None,
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
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                    label: None,
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
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task",
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                    label: None,
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
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                    label: None,
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
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Balances speed with some reasoning; useful for straightforward queries and short explanations",
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
                    label: None,
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                    label: None,
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

pub fn builtin_model_presets(auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    PRESETS
        .iter()
        .filter(|preset| preset.upgrade.is_none())
        .filter(|preset| match auth_mode {
            // `codex-auto` is only available when using ChatGPT-style auth.
            Some(AuthMode::ApiKey) => preset.id != "codex-auto",
            _ => true,
        })
        .cloned()
        .collect()
}

pub fn all_model_presets() -> &'static Vec<ModelPreset> {
    &PRESETS
}

impl ModelPreset {
    pub fn reasoning_effort_label(&self, effort: ReasoningEffort) -> String {
        self.supported_reasoning_efforts
            .iter()
            .find(|option| option.effort == effort)
            .map(ReasoningEffortPreset::label)
            .unwrap_or_else(|| effort.to_string())
    }
}

/// Return the display label for a reasoning effort on a given model, falling back to the effort
/// name when no label is provided.
pub fn reasoning_effort_label_for_model(model: &str, effort: ReasoningEffort) -> String {
    all_model_presets()
        .iter()
        .find(|preset| preset.model == model)
        .map(|preset| preset.reasoning_effort_label(effort))
        .unwrap_or_else(|| effort.to_string())
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
    fn codex_auto_is_included_for_non_api_auth() {
        let presets_no_auth = builtin_model_presets(None);
        assert!(
            presets_no_auth
                .iter()
                .any(|preset| preset.id == "codex-auto")
        );

        let presets_chatgpt = builtin_model_presets(Some(AuthMode::ChatGPT));
        assert!(
            presets_chatgpt
                .iter()
                .any(|preset| preset.id == "codex-auto")
        );
    }

    #[test]
    fn codex_auto_is_excluded_for_api_key_auth() {
        let presets_api_key = builtin_model_presets(Some(AuthMode::ApiKey));
        assert!(
            !presets_api_key
                .iter()
                .any(|preset| preset.id == "codex-auto")
        );
    }
}
