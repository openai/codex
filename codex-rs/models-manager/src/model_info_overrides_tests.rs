use codex_login::AuthManager;
use codex_login::CodexAuth;

use crate::ModelsManagerConfig;
use crate::collaboration_mode_presets::CollaborationModesConfig;
use crate::manager::ModelsManager;
use codex_protocol::openai_models::TruncationPolicyConfig;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_model_info_without_tool_output_override() {
    let codex_home = TempDir::new().expect("create temp dir");
    let config = ModelsManagerConfig::default();
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let manager = ModelsManager::new(
        codex_home.path().to_path_buf(),
        auth_manager,
        /*model_catalog*/ None,
        CollaborationModesConfig::default(),
    );

    let model_info = manager.get_model_info("gpt-5.2", &config).await;

    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::bytes(/*limit*/ 10_000)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_model_info_with_tool_output_override() {
    let codex_home = TempDir::new().expect("create temp dir");
    let config = ModelsManagerConfig {
        tool_output_token_limit: Some(123),
        ..Default::default()
    };
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
    let manager = ModelsManager::new(
        codex_home.path().to_path_buf(),
        auth_manager,
        /*model_catalog*/ None,
        CollaborationModesConfig::default(),
    );

    let model_info = manager.get_model_info("gpt-5.4", &config).await;

    assert_eq!(
        model_info.truncation_policy,
        TruncationPolicyConfig::tokens(/*limit*/ 123)
    );
}
