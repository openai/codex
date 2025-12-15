use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::built_in_model_providers;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use core_test_support::load_default_config_for_test;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_api_key_models() -> Result<()> {
    let codex_home = tempdir()?;
    let config = load_default_config_for_test(&codex_home);
    let manager = ConversationManager::with_models_provider(
        CodexAuth::from_api_key("sk-test"),
        built_in_model_providers()["openai"].clone(),
    );
    let models = manager.list_models(&config).await;

    let expected_models = expected_models_for_api_key();
    assert_eq!(expected_models, models);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_chatgpt_models() -> Result<()> {
    let codex_home = tempdir()?;
    let config = load_default_config_for_test(&codex_home);
    let manager = ConversationManager::with_models_provider(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        built_in_model_providers()["openai"].clone(),
    );
    let models = manager.list_models(&config).await;

    let expected_models = expected_models_for_chatgpt();
    assert_eq!(expected_models, models);

    Ok(())
}

fn expected_models_for_api_key() -> Vec<ModelPreset> {
    vec![
        gpt_5_1_codex_max(),
        gpt_5_1_codex(),
        gpt_5_1_codex_mini(),
        gpt_5_2(),
        gpt_5_1(),
    ]
}

fn expected_models_for_chatgpt() -> Vec<ModelPreset> {
    vec![
        gpt_5_1_codex_max(),
        gpt_5_1_codex(),
        gpt_5_1_codex_mini(),
        gpt_5_2(),
        gpt_5_1(),
    ]
}

fn gpt_5_1_codex_max() -> ModelPreset {
    ModelPreset {
        id: "gpt-5.1-codex-max".to_string(),
        model: "gpt-5.1-codex-max".to_string(),
        display_name: "gpt-5.1-codex-max".to_string(),
        description: "Latest Codex-optimized flagship for deep and fast reasoning.".to_string(),
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: vec![
            effort(
                ReasoningEffort::Low,
                "Fast responses with lighter reasoning",
            ),
            effort(
                ReasoningEffort::Medium,
                "Balances speed and reasoning depth for everyday tasks",
            ),
            effort(
                ReasoningEffort::High,
                "Greater reasoning depth for complex problems",
            ),
            effort(
                ReasoningEffort::XHigh,
                "Extra high reasoning depth for complex problems",
            ),
        ],
        is_default: true,
        upgrade: None,
        show_in_picker: true,
    }
}

fn gpt_5_1_codex() -> ModelPreset {
    ModelPreset {
        id: "gpt-5.1-codex".to_string(),
        model: "gpt-5.1-codex".to_string(),
        display_name: "gpt-5.1-codex".to_string(),
        description: "Optimized for codex.".to_string(),
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: vec![
            effort(
                ReasoningEffort::Low,
                "Fastest responses with limited reasoning",
            ),
            effort(
                ReasoningEffort::Medium,
                "Dynamically adjusts reasoning based on the task",
            ),
            effort(
                ReasoningEffort::High,
                "Maximizes reasoning depth for complex or ambiguous problems",
            ),
        ],
        is_default: false,
        upgrade: Some(gpt_5_1_codex_max_upgrade()),
        show_in_picker: true,
    }
}

fn gpt_5_1_codex_mini() -> ModelPreset {
    ModelPreset {
        id: "gpt-5.1-codex-mini".to_string(),
        model: "gpt-5.1-codex-mini".to_string(),
        display_name: "gpt-5.1-codex-mini".to_string(),
        description: "Optimized for codex. Cheaper, faster, but less capable.".to_string(),
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: vec![
            effort(
                ReasoningEffort::Medium,
                "Dynamically adjusts reasoning based on the task",
            ),
            effort(
                ReasoningEffort::High,
                "Maximizes reasoning depth for complex or ambiguous problems",
            ),
        ],
        is_default: false,
        upgrade: Some(gpt_5_1_codex_max_upgrade()),
        show_in_picker: true,
    }
}

fn gpt_5_2() -> ModelPreset {
    ModelPreset {
        id: "gpt-5.2".to_string(),
        model: "gpt-5.2".to_string(),
        display_name: "gpt-5.2".to_string(),
        description:
            "Latest frontier model with improvements across knowledge, reasoning and coding"
                .to_string(),
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: vec![
            effort(
                ReasoningEffort::Low,
                "Balances speed with some reasoning; useful for straightforward queries and short explanations",
            ),
            effort(
                ReasoningEffort::Medium,
                "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
            ),
            effort(
                ReasoningEffort::High,
                "Greater reasoning depth for complex or ambiguous problems",
            ),
            effort(
                ReasoningEffort::XHigh,
                "Extra high reasoning for complex problems",
            ),
        ],
        is_default: false,
        upgrade: None,
        show_in_picker: true,
    }
}

fn gpt_5_1() -> ModelPreset {
    ModelPreset {
        id: "gpt-5.1".to_string(),
        model: "gpt-5.1".to_string(),
        display_name: "gpt-5.1".to_string(),
        description: "Broad world knowledge with strong general reasoning.".to_string(),
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: vec![
            effort(
                ReasoningEffort::Low,
                "Balances speed with some reasoning; useful for straightforward queries and short explanations",
            ),
            effort(
                ReasoningEffort::Medium,
                "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
            ),
            effort(
                ReasoningEffort::High,
                "Maximizes reasoning depth for complex or ambiguous problems",
            ),
        ],
        is_default: false,
        upgrade: Some(gpt_5_1_codex_max_upgrade()),
        show_in_picker: true,
    }
}

fn gpt_5_1_codex_max_upgrade() -> codex_protocol::openai_models::ModelUpgrade {
    codex_protocol::openai_models::ModelUpgrade {
        id: "gpt-5.1-codex-max".to_string(),
        reasoning_effort_mapping: None,
        migration_config_key: "hide_gpt-5.1-codex-max_migration_prompt".to_string(),
    }
}

fn effort(reasoning_effort: ReasoningEffort, description: &str) -> ReasoningEffortPreset {
    ReasoningEffortPreset {
        effort: reasoning_effort,
        description: description.to_string(),
    }
}
