use crate::auth::AuthMode;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelUpgrade;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::default_input_modalities;
use indoc::indoc;
use once_cell::sync::Lazy;
use std::collections::HashMap;

pub const HIDE_GPT5_1_MIGRATION_PROMPT_CONFIG: &str = "hide_gpt5_1_migration_prompt";
pub const HIDE_GPT_5_1_CODEX_MAX_MIGRATION_PROMPT_CONFIG: &str =
    "hide_gpt-5.1-codex-max_migration_prompt";

static BUILTIN_PRESETS: Lazy<Vec<ModelPreset>> = Lazy::new(builtin_model_presets_from_models_json);

fn builtin_model_presets_from_models_json() -> Vec<ModelPreset> {
    let mut presets = serde_json::from_str::<ModelsResponse>(include_str!("../../models.json"))
        .expect("bundled models.json must parse")
        .models
        .into_iter()
        .map(ModelPreset::from)
        .collect::<Vec<_>>();
    for preset in &mut presets {
        override_builtin_preset_properties(preset);
    }
    presets.extend(internal_model_presets());
    presets
}

fn override_builtin_preset_properties(preset: &mut ModelPreset) {
    match preset.model.as_str() {
        "gpt-5.2-codex" => {
            preset.is_default = true;
            preset.supports_personality = true;
        }
        "gpt-5.1-codex-max" => {
            preset.upgrade = Some(gpt_52_codex_upgrade(
                "gpt-5.1-codex-max",
                HashMap::from([
                    (ReasoningEffort::Low, ReasoningEffort::Low),
                    (ReasoningEffort::None, ReasoningEffort::Low),
                    (ReasoningEffort::Medium, ReasoningEffort::Medium),
                    (ReasoningEffort::High, ReasoningEffort::High),
                    (ReasoningEffort::Minimal, ReasoningEffort::Low),
                    (ReasoningEffort::XHigh, ReasoningEffort::XHigh),
                ]),
            ));
        }
        "gpt-5.1-codex-mini" => {
            preset.upgrade = Some(gpt_52_codex_upgrade(
                "gpt-5.1-codex-mini",
                HashMap::from([
                    (ReasoningEffort::High, ReasoningEffort::High),
                    (ReasoningEffort::XHigh, ReasoningEffort::High),
                    (ReasoningEffort::Minimal, ReasoningEffort::Medium),
                    (ReasoningEffort::None, ReasoningEffort::Medium),
                    (ReasoningEffort::Low, ReasoningEffort::Medium),
                    (ReasoningEffort::Medium, ReasoningEffort::Medium),
                ]),
            ));
        }
        "gpt-5.2" => {
            preset.upgrade = Some(gpt_52_codex_upgrade(
                "gpt-5.2",
                HashMap::from([
                    (ReasoningEffort::High, ReasoningEffort::High),
                    (ReasoningEffort::None, ReasoningEffort::Low),
                    (ReasoningEffort::Minimal, ReasoningEffort::Low),
                    (ReasoningEffort::Low, ReasoningEffort::Low),
                    (ReasoningEffort::Medium, ReasoningEffort::Medium),
                    (ReasoningEffort::XHigh, ReasoningEffort::XHigh),
                ]),
            ));
        }
        "gpt-5.1-codex" => {
            preset.upgrade = Some(gpt_52_codex_upgrade(
                "gpt-5.1-codex",
                HashMap::from([
                    (ReasoningEffort::Minimal, ReasoningEffort::Low),
                    (ReasoningEffort::Low, ReasoningEffort::Low),
                    (ReasoningEffort::Medium, ReasoningEffort::Medium),
                    (ReasoningEffort::None, ReasoningEffort::Low),
                    (ReasoningEffort::High, ReasoningEffort::High),
                    (ReasoningEffort::XHigh, ReasoningEffort::High),
                ]),
            ));
        }
        "gpt-5-codex" => {
            preset.upgrade = Some(gpt_52_codex_upgrade(
                "gpt-5-codex",
                HashMap::from([
                    (ReasoningEffort::Minimal, ReasoningEffort::Low),
                    (ReasoningEffort::High, ReasoningEffort::High),
                    (ReasoningEffort::Medium, ReasoningEffort::Medium),
                    (ReasoningEffort::XHigh, ReasoningEffort::High),
                    (ReasoningEffort::None, ReasoningEffort::Low),
                    (ReasoningEffort::Low, ReasoningEffort::Low),
                ]),
            ));
        }
        "gpt-5" => {
            preset.upgrade = Some(gpt_52_codex_upgrade(
                "gpt-5",
                HashMap::from([
                    (ReasoningEffort::XHigh, ReasoningEffort::High),
                    (ReasoningEffort::Minimal, ReasoningEffort::Minimal),
                    (ReasoningEffort::Low, ReasoningEffort::Low),
                    (ReasoningEffort::None, ReasoningEffort::Minimal),
                    (ReasoningEffort::High, ReasoningEffort::High),
                    (ReasoningEffort::Medium, ReasoningEffort::Medium),
                ]),
            ));
        }
        "gpt-5-codex-mini" => {
            preset.upgrade = Some(gpt_52_codex_upgrade(
                "gpt-5-codex-mini",
                HashMap::from([
                    (ReasoningEffort::Minimal, ReasoningEffort::Medium),
                    (ReasoningEffort::XHigh, ReasoningEffort::High),
                    (ReasoningEffort::High, ReasoningEffort::High),
                    (ReasoningEffort::Low, ReasoningEffort::Medium),
                    (ReasoningEffort::Medium, ReasoningEffort::Medium),
                    (ReasoningEffort::None, ReasoningEffort::Medium),
                ]),
            ));
        }
        _ => {}
    }
}

fn internal_model_presets() -> Vec<ModelPreset> {
    vec![
        ModelPreset {
            id: "bengalfox".to_string(),
            model: "bengalfox".to_string(),
            display_name: "bengalfox".to_string(),
            description: "bengalfox".to_string(),
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: vec![
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
                    description: "Greater reasoning depth for complex problems".to_string(),
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::XHigh,
                    description: "Extra high reasoning depth for complex problems".to_string(),
                },
            ],
            supports_personality: true,
            is_default: false,
            upgrade: None,
            show_in_picker: false,
            supported_in_api: true,
            input_modalities: default_input_modalities(),
        },
        ModelPreset {
            id: "boomslang".to_string(),
            model: "boomslang".to_string(),
            display_name: "boomslang".to_string(),
            description: "boomslang".to_string(),
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: vec![
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
                ReasoningEffortPreset {
                    effort: ReasoningEffort::XHigh,
                    description: "Extra high reasoning depth for complex problems".to_string(),
                },
            ],
            supports_personality: false,
            is_default: false,
            upgrade: None,
            show_in_picker: false,
            supported_in_api: true,
            input_modalities: default_input_modalities(),
        },
    ]
}

fn gpt_52_codex_upgrade(
    migration_config_key: &str,
    reasoning_effort_mapping: HashMap<ReasoningEffort, ReasoningEffort>,
) -> ModelUpgrade {
    ModelUpgrade {
        id: "gpt-5.2-codex".to_string(),
        reasoning_effort_mapping: Some(reasoning_effort_mapping),
        migration_config_key: migration_config_key.to_string(),
        model_link: Some("https://openai.com/index/introducing-gpt-5-2-codex".to_string()),
        upgrade_copy: Some(
            "Codex is now powered by gpt-5.2-codex, our latest frontier agentic coding model. It is smarter and faster than its predecessors and capable of long-running project-scale work."
                .to_string(),
        ),
        migration_markdown: Some(
            indoc! {r#"
                **Codex just got an upgrade. Introducing {model_to}.**

                Codex is now powered by gpt-5.2-codex, our latest frontier agentic coding model. It is smarter and faster than its predecessors and capable of long-running project-scale work. Learn more about {model_to} at https://openai.com/index/introducing-gpt-5-2-codex

                You can continue using {model_from} if you prefer.
            "#}
            .to_string(),
        ),
    }
}

pub(super) fn builtin_model_presets(_auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    BUILTIN_PRESETS.iter().cloned().collect()
}

#[cfg(any(test, feature = "test-support"))]
pub fn all_model_presets() -> &'static Vec<ModelPreset> {
    &BUILTIN_PRESETS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_default_model_is_configured() {
        let default_models = builtin_model_presets(None)
            .iter()
            .filter(|preset| preset.is_default)
            .count();
        assert_eq!(default_models, 1);
    }
}
