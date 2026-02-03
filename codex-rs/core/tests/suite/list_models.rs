use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::ThreadManager;
use codex_core::built_in_model_providers;
use codex_core::models_manager::manager::RefreshStrategy;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelUpgrade;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use core_test_support::load_default_config_for_test;
use indoc::indoc;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_api_key_models() -> Result<()> {
    let codex_home = tempdir()?;
    let config = load_default_config_for_test(&codex_home).await;
    let manager = ThreadManager::with_models_provider(
        CodexAuth::from_api_key("sk-test"),
        built_in_model_providers()["openai"].clone(),
    );
    let models = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;

    let expected_models = expected_models_for_api_key();
    assert_eq!(expected_models, models);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_chatgpt_models() -> Result<()> {
    let codex_home = tempdir()?;
    let config = load_default_config_for_test(&codex_home).await;
    let manager = ThreadManager::with_models_provider(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        built_in_model_providers()["openai"].clone(),
    );
    let models = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;

    let expected_models = expected_models_for_chatgpt();
    assert_eq!(expected_models, models);

    Ok(())
}

fn expected_models_for_api_key() -> Vec<ModelPreset> {
    vec![
        gpt_52_codex(),
        gpt_5_2(),
        gpt_5_1_codex_max(),
        gpt_5_1_codex(),
        gpt_5_1_codex_mini(),
        gpt_5_1(),
        gpt_5_codex(),
        gpt_5(),
        gpt_5_codex_mini(),
        bengalfox(),
        boomslang(),
    ]
}

fn expected_models_for_chatgpt() -> Vec<ModelPreset> {
    expected_models_for_api_key()
}

fn gpt_52_codex() -> ModelPreset {
    ModelPreset {
        id: "gpt-5.2-codex".to_string(),
        model: "gpt-5.2-codex".to_string(),
        display_name: "gpt-5.2-codex".to_string(),
        description: "Latest frontier agentic coding model.".to_string(),
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
        supports_personality: false,
        is_default: true,
        upgrade: None,
        show_in_picker: true,
        supported_in_api: true,
    }
}

fn gpt_5_1_codex_max() -> ModelPreset {
    ModelPreset {
        id: "gpt-5.1-codex-max".to_string(),
        model: "gpt-5.1-codex-max".to_string(),
        display_name: "gpt-5.1-codex-max".to_string(),
        description: "Codex-optimized flagship for deep and fast reasoning.".to_string(),
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
        supports_personality: false,
        is_default: false,
        upgrade: Some(gpt52_codex_upgrade(
            "gpt-5.1-codex-max",
            HashMap::from([
                (ReasoningEffort::Low, ReasoningEffort::Low),
                (ReasoningEffort::None, ReasoningEffort::Low),
                (ReasoningEffort::Medium, ReasoningEffort::Medium),
                (ReasoningEffort::High, ReasoningEffort::High),
                (ReasoningEffort::Minimal, ReasoningEffort::Low),
                (ReasoningEffort::XHigh, ReasoningEffort::XHigh),
            ]),
        )),
        show_in_picker: true,
        supported_in_api: true,
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
        supports_personality: false,
        is_default: false,
        upgrade: Some(gpt52_codex_upgrade(
            "gpt-5.1-codex-mini",
            HashMap::from([
                (ReasoningEffort::High, ReasoningEffort::High),
                (ReasoningEffort::XHigh, ReasoningEffort::High),
                (ReasoningEffort::Minimal, ReasoningEffort::Medium),
                (ReasoningEffort::None, ReasoningEffort::Medium),
                (ReasoningEffort::Low, ReasoningEffort::Medium),
                (ReasoningEffort::Medium, ReasoningEffort::Medium),
            ]),
        )),
        show_in_picker: true,
        supported_in_api: true,
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
                "Maximizes reasoning depth for complex or ambiguous problems",
            ),
            effort(
                ReasoningEffort::XHigh,
                "Extra high reasoning for complex problems",
            ),
        ],
        supports_personality: false,
        is_default: false,
        upgrade: Some(gpt52_codex_upgrade(
            "gpt-5.2",
            HashMap::from([
                (ReasoningEffort::High, ReasoningEffort::High),
                (ReasoningEffort::None, ReasoningEffort::Low),
                (ReasoningEffort::Minimal, ReasoningEffort::Low),
                (ReasoningEffort::Low, ReasoningEffort::Low),
                (ReasoningEffort::Medium, ReasoningEffort::Medium),
                (ReasoningEffort::XHigh, ReasoningEffort::XHigh),
            ]),
        )),
        show_in_picker: true,
        supported_in_api: true,
    }
}

fn bengalfox() -> ModelPreset {
    ModelPreset {
        id: "bengalfox".to_string(),
        model: "bengalfox".to_string(),
        display_name: "bengalfox".to_string(),
        description: "bengalfox".to_string(),
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
        supports_personality: true,
        is_default: false,
        upgrade: None,
        show_in_picker: false,
        supported_in_api: true,
    }
}

fn boomslang() -> ModelPreset {
    ModelPreset {
        id: "boomslang".to_string(),
        model: "boomslang".to_string(),
        display_name: "boomslang".to_string(),
        description: "boomslang".to_string(),
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
            effort(
                ReasoningEffort::XHigh,
                "Extra high reasoning depth for complex problems",
            ),
        ],
        supports_personality: false,
        is_default: false,
        upgrade: None,
        show_in_picker: false,
        supported_in_api: true,
    }
}

fn gpt_5_codex() -> ModelPreset {
    ModelPreset {
        id: "gpt-5-codex".to_string(),
        model: "gpt-5-codex".to_string(),
        display_name: "gpt-5-codex".to_string(),
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
        supports_personality: false,
        is_default: false,
        upgrade: Some(gpt52_codex_upgrade(
            "gpt-5-codex",
            HashMap::from([
                (ReasoningEffort::Minimal, ReasoningEffort::Low),
                (ReasoningEffort::High, ReasoningEffort::High),
                (ReasoningEffort::Medium, ReasoningEffort::Medium),
                (ReasoningEffort::XHigh, ReasoningEffort::High),
                (ReasoningEffort::None, ReasoningEffort::Low),
                (ReasoningEffort::Low, ReasoningEffort::Low),
            ]),
        )),
        show_in_picker: false,
        supported_in_api: true,
    }
}

fn gpt_5_codex_mini() -> ModelPreset {
    ModelPreset {
        id: "gpt-5-codex-mini".to_string(),
        model: "gpt-5-codex-mini".to_string(),
        display_name: "gpt-5-codex-mini".to_string(),
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
        supports_personality: false,
        is_default: false,
        upgrade: Some(gpt52_codex_upgrade(
            "gpt-5-codex-mini",
            HashMap::from([
                (ReasoningEffort::None, ReasoningEffort::Medium),
                (ReasoningEffort::XHigh, ReasoningEffort::High),
                (ReasoningEffort::High, ReasoningEffort::High),
                (ReasoningEffort::Low, ReasoningEffort::Medium),
                (ReasoningEffort::Medium, ReasoningEffort::Medium),
                (ReasoningEffort::Minimal, ReasoningEffort::Medium),
            ]),
        )),
        show_in_picker: false,
        supported_in_api: true,
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
        supports_personality: false,
        is_default: false,
        upgrade: Some(gpt52_codex_upgrade(
            "gpt-5.1-codex",
            HashMap::from([
                (ReasoningEffort::Minimal, ReasoningEffort::Low),
                (ReasoningEffort::Low, ReasoningEffort::Low),
                (ReasoningEffort::Medium, ReasoningEffort::Medium),
                (ReasoningEffort::None, ReasoningEffort::Low),
                (ReasoningEffort::High, ReasoningEffort::High),
                (ReasoningEffort::XHigh, ReasoningEffort::High),
            ]),
        )),
        show_in_picker: false,
        supported_in_api: true,
    }
}

fn gpt_5() -> ModelPreset {
    ModelPreset {
        id: "gpt-5".to_string(),
        model: "gpt-5".to_string(),
        display_name: "gpt-5".to_string(),
        description: "Broad world knowledge with strong general reasoning.".to_string(),
        default_reasoning_effort: ReasoningEffort::Medium,
        supported_reasoning_efforts: vec![
            effort(
                ReasoningEffort::Minimal,
                "Fastest responses with little reasoning",
            ),
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
        supports_personality: false,
        is_default: false,
        upgrade: Some(gpt52_codex_upgrade(
            "gpt-5",
            HashMap::from([
                (ReasoningEffort::XHigh, ReasoningEffort::High),
                (ReasoningEffort::Minimal, ReasoningEffort::Minimal),
                (ReasoningEffort::Low, ReasoningEffort::Low),
                (ReasoningEffort::None, ReasoningEffort::Minimal),
                (ReasoningEffort::High, ReasoningEffort::High),
                (ReasoningEffort::Medium, ReasoningEffort::Medium),
            ]),
        )),
        show_in_picker: false,
        supported_in_api: true,
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
        supports_personality: false,
        is_default: false,
        upgrade: Some(gpt52_codex_upgrade(
            "gpt-5.1",
            HashMap::from([
                (ReasoningEffort::None, ReasoningEffort::Low),
                (ReasoningEffort::Medium, ReasoningEffort::Medium),
                (ReasoningEffort::High, ReasoningEffort::High),
                (ReasoningEffort::XHigh, ReasoningEffort::High),
                (ReasoningEffort::Low, ReasoningEffort::Low),
                (ReasoningEffort::Minimal, ReasoningEffort::Low),
            ]),
        )),
        show_in_picker: false,
        supported_in_api: true,
    }
}

fn gpt52_codex_upgrade(
    migration_config_key: &str,
    reasoning_effort_mapping: HashMap<ReasoningEffort, ReasoningEffort>,
) -> ModelUpgrade {
    ModelUpgrade {
        id: "gpt-5.2-codex".to_string(),
        reasoning_effort_mapping: Some(reasoning_effort_mapping),
        migration_config_key: migration_config_key.to_string(),
        model_link: None,
        upgrade_copy: None,
        migration_markdown: Some(
            indoc! {r#"
                **Codex just got an upgrade. Introducing {model_to}.**

                Codex is now powered by {model_to}, our latest frontier agentic coding model. It is smarter and faster than its predecessors and capable of long-running project-scale work. Learn more about {model_to} at https://openai.com/index/introducing-gpt-5-2-codex

                You can continue using {model_from} if you prefer.
            "#}
            .to_string(),
        ),
    }
}

fn effort(reasoning_effort: ReasoningEffort, description: &str) -> ReasoningEffortPreset {
    ReasoningEffortPreset {
        effort: reasoning_effort,
        description: description.to_string(),
    }
}
