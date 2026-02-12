use codex_core::CodexAuth;
use codex_core::features::Feature;
use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::openai_models::TruncationPolicyConfig;
use core_test_support::load_default_config_for_test;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_model_info_without_tool_output_override() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);
    let auth_manager = codex_core::test_support::auth_manager_from_auth(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let manager = ModelsManager::new(config.codex_home.clone(), auth_manager);

    let model_info = manager.get_model_info("gpt-5.1", &config).await;

    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::bytes(10_000)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_model_info_with_tool_output_override() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);
    config.tool_output_token_limit = Some(123);
    let auth_manager = codex_core::test_support::auth_manager_from_auth(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let manager = ModelsManager::new(config.codex_home.clone(), auth_manager);

    let model_info = manager.get_model_info("gpt-5.1-codex", &config).await;

    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::tokens(123)
    );
}
