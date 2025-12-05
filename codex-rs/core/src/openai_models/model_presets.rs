use codex_app_server_protocol::AuthMode;
use codex_protocol::openai_models::ModelName;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelUpgrade;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use once_cell::sync::Lazy;

pub const HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG: &str = "hide_gpt5_1_migration_prompt";
pub const HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG: &str =
    "hide_gpt-5.1-codex-max_migration_prompt";

fn make_preset(
    name: ModelName,
    description: &str,
    default_reasoning_effort: ReasoningEffort,
    supported_reasoning_efforts: Vec<ReasoningEffortPreset>,
    is_default: bool,
    upgrade: Option<ModelUpgrade>,
    show_in_picker: bool,
) -> ModelPreset {
    let slug = name.as_str().to_string();
    ModelPreset {
        id: slug.clone(),
        model: slug.clone(),
        display_name: slug,
        description: description.to_string(),
        default_reasoning_effort,
        supported_reasoning_efforts,
        is_default,
        upgrade,
        show_in_picker,
    }
}

static PRESETS: Lazy<Vec<ModelPreset>> = Lazy::new(|| {
    vec![
        make_preset(
            ModelName::Gpt51CodexMax,
            "Latest Codex-optimized flagship for deep and fast reasoning.",
            ReasoningEffort::Medium,
            vec![
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Fast responses with lighter reasoning".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Balances speed and reasoning depth for everyday tasks".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex problems".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::XHigh,
                    description: "Extra high reasoning depth for complex problems".to_string(),
                },
            ],
            true,
            None,
            true,
        ),
        make_preset(
            ModelName::Gpt51Codex,
            "Optimized for codex.",
            ReasoningEffort::Medium,
            vec![
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Fastest responses with limited reasoning".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems"
                        .to_string(),
                },
            ],
            false,
            Some(ModelUpgrade {
                id: ModelName::Gpt51CodexMax.as_str().to_string(),
                reasoning_effort_mapping: None,
                migration_config_key: HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG.to_string(),
            }),
            true,
        ),
        make_preset(
            ModelName::Gpt51CodexMini,
            "Optimized for codex. Cheaper, faster, but less capable.",
            ReasoningEffort::Medium,
            vec![
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems"
                        .to_string(),
                },
            ],
            false,
            Some(ModelUpgrade {
                id: ModelName::Gpt51CodexMax.as_str().to_string(),
                reasoning_effort_mapping: None,
                migration_config_key: HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG.to_string(),
            }),
            true,
        ),
        make_preset(
            ModelName::Gpt51,
            "Broad world knowledge with strong general reasoning.",
            ReasoningEffort::Medium,
            vec![
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Balances speed with some reasoning; useful for straightforward queries and short explanations".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems".to_string(),
                },
            ],
            false,
            Some(ModelUpgrade {
                id: ModelName::Gpt51CodexMax.as_str().to_string(),
                reasoning_effort_mapping: None,
                migration_config_key: HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG.to_string(),
            }),
            true,
        ),
        // Deprecated models.
        make_preset(
            ModelName::Gpt5Codex,
            "Optimized for codex.",
            ReasoningEffort::Medium,
            vec![
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Fastest responses with limited reasoning".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems".to_string(),
                },
            ],
            false,
            Some(ModelUpgrade {
                id: ModelName::Gpt51CodexMax.as_str().to_string(),
                reasoning_effort_mapping: None,
                migration_config_key: HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG.to_string(),
            }),
            false,
        ),
        make_preset(
            ModelName::Gpt5CodexMini,
            "Optimized for codex. Cheaper, faster, but less capable.",
            ReasoningEffort::Medium,
            vec![
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems".to_string(),
                },
            ],
            false,
            Some(ModelUpgrade {
                id: ModelName::Gpt51CodexMini.as_str().to_string(),
                reasoning_effort_mapping: None,
                migration_config_key: HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG.to_string(),
            }),
            false,
        ),
        make_preset(
            ModelName::Gpt5,
            "Broad world knowledge with strong general reasoning.",
            ReasoningEffort::Medium,
            vec![
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Minimal,
                    description: "Fastest responses with little reasoning".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Balances speed with some reasoning; useful for straightforward queries and short explanations".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems".to_string(),
                },
            ],
            false,
            Some(ModelUpgrade {
                id: ModelName::Gpt51CodexMax.as_str().to_string(),
                reasoning_effort_mapping: None,
                migration_config_key: HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG.to_string(),
            }),
            false,
        ),
    ]
});

pub(crate) fn builtin_model_presets(auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    PRESETS
        .iter()
        .filter(|preset| match auth_mode {
            Some(AuthMode::ApiKey) => preset.show_in_picker && preset.id != "gpt-5.1-codex-max",
            _ => preset.show_in_picker,
        })
        .cloned()
        .collect()
}

// todo(aibrahim): remove this once we migrate tests
pub fn all_model_presets() -> &'static Vec<ModelPreset> {
    &PRESETS
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_app_server_protocol::AuthMode;

    #[test]
    fn only_one_default_model_is_configured() {
        let default_models = PRESETS.iter().filter(|preset| preset.is_default).count();
        assert!(default_models == 1);
    }

    #[test]
    fn gpt_5_1_codex_max_hidden_for_api_key_auth() {
        let presets = builtin_model_presets(Some(AuthMode::ApiKey));
        assert!(
            presets
                .iter()
                .all(|preset| preset.id != "gpt-5.1-codex-max")
        );
    }
}
