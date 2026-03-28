use codex_core::CodexAuth;
use codex_core::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::openai_models::default_input_modalities;
use core_test_support::load_default_config_for_test;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_model_info_without_tool_output_override() {
    let codex_home = TempDir::new().expect("create temp dir");
    let config = load_default_config_for_test(&codex_home).await;
    let auth_manager = codex_core::test_support::auth_manager_from_auth(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let manager = ModelsManager::new(
        config.codex_home.clone(),
        auth_manager,
        /*model_catalog*/ None,
        HashMap::new(),
        CollaborationModesConfig::default(),
    );

    let model_info = manager.get_model_info("gpt-5.1", &config).await;

    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::bytes(/*limit*/ 10_000)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_model_info_with_tool_output_override() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.tool_output_token_limit = Some(123);
    let auth_manager = codex_core::test_support::auth_manager_from_auth(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let manager = ModelsManager::new(
        config.codex_home.clone(),
        auth_manager,
        /*model_catalog*/ None,
        HashMap::new(),
        CollaborationModesConfig::default(),
    );

    let model_info = manager.get_model_info("gpt-5.1-codex", &config).await;

    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::tokens(/*limit*/ 123)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_model_alias_applies_request_model_and_context_overrides() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.custom_models.insert(
        "gpt-5.4 1m".to_string(),
        codex_core::config::CustomModelConfig {
            model: "gpt-5.4".to_string(),
            model_context_window: Some(1_000_000),
            model_auto_compact_token_limit: Some(900_000),
        },
    );

    let auth_manager = codex_core::test_support::auth_manager_from_auth(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let manager = ModelsManager::new(
        config.codex_home.clone(),
        auth_manager,
        Some(codex_protocol::openai_models::ModelsResponse {
            models: vec![codex_protocol::openai_models::ModelInfo {
                slug: "gpt-5.4".to_string(),
                request_model: None,
                display_name: "GPT-5.4".to_string(),
                description: Some("desc".to_string()),
                default_reasoning_level: None,
                supported_reasoning_levels: Vec::new(),
                shell_type: codex_protocol::openai_models::ConfigShellToolType::ShellCommand,
                visibility: codex_protocol::openai_models::ModelVisibility::List,
                supported_in_api: true,
                priority: 1,
                availability_nux: None,
                upgrade: None,
                base_instructions: "base".to_string(),
                model_messages: None,
                supports_reasoning_summaries: false,
                default_reasoning_summary: codex_protocol::config_types::ReasoningSummary::Auto,
                support_verbosity: false,
                default_verbosity: None,
                supports_search_tool: false,
                apply_patch_tool_type: None,
                truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
                supports_parallel_tool_calls: false,
                supports_image_detail_original: false,
                context_window: Some(272_000),
                auto_compact_token_limit: None,
                effective_context_window_percent: 95,
                experimental_supported_tools: Vec::new(),
                input_modalities: default_input_modalities(),
                web_search_tool_type: WebSearchToolType::Text,
                used_fallback_model_metadata: false,
            }],
        }),
        config.custom_models.clone(),
        CollaborationModesConfig::default(),
    );

    let model_info = manager.get_model_info("gpt-5.4 1m", &config).await;

    assert_eq!(model_info.slug, "gpt-5.4 1m");
    assert_eq!(model_info.request_model.as_deref(), Some("gpt-5.4"));
    assert_eq!(model_info.context_window, Some(1_000_000));
    assert_eq!(model_info.auto_compact_token_limit, Some(900_000));
}
