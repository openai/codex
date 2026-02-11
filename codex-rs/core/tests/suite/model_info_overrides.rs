use codex_core::CodexAuth;
use codex_core::features::Feature;
use codex_core::models_manager::manager::ModelsManager;
use codex_protocol::openai_models::ModelInfoPatch;
use codex_protocol::openai_models::ModelVisibility;
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

// Existing remote model path:
// fetch model metadata for a known slug, then apply per-slug patch values from config.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_info_patch_overrides_remote_model_fields() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);
    let auth_manager = codex_core::AuthManager::from_auth_for_testing(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let manager = ModelsManager::new(config.codex_home.clone(), auth_manager);

    let mut baseline_config = config.clone();
    baseline_config.model_info_overrides.clear();
    let baseline = manager.get_model_info("gpt-5.1", &baseline_config).await;

    config.model_info_overrides.insert(
        "gpt-5.1".to_string(),
        ModelInfoPatch {
            display_name: Some("gpt-5.1-dev".to_string()),
            context_window: Some(123_456),
            visibility: Some(ModelVisibility::Hide),
            supported_in_api: Some(false),
            ..Default::default()
        },
    );
    let model_info = manager.get_model_info("gpt-5.1", &config).await;

    let mut expected = baseline;
    expected.display_name = "gpt-5.1-dev".to_string();
    expected.context_window = Some(123_456);
    expected.visibility = ModelVisibility::Hide;
    expected.supported_in_api = false;

    assert_eq!(model_info, expected);
}

// Unknown model path:
// when slug is not known remotely, manager falls back to synthetic metadata and still
// applies the patch for that requested slug.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_info_patch_can_define_new_model_from_fallback() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);
    let auth_manager = codex_core::AuthManager::from_auth_for_testing(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let manager = ModelsManager::new(config.codex_home.clone(), auth_manager);

    let mut baseline_config = config.clone();
    baseline_config.model_info_overrides.clear();
    let baseline = manager.get_model_info("gpt-fake", &baseline_config).await;

    config.model_info_overrides.insert(
        "gpt-fake".to_string(),
        ModelInfoPatch {
            display_name: Some("gpt-fake-dev".to_string()),
            context_window: Some(400_000),
            supports_parallel_tool_calls: Some(true),
            base_instructions: Some("Custom model instructions".to_string()),
            ..Default::default()
        },
    );
    let model_info = manager.get_model_info("gpt-fake", &config).await;

    let mut expected = baseline;
    expected.slug = "gpt-fake".to_string();
    expected.display_name = "gpt-fake-dev".to_string();
    expected.context_window = Some(400_000);
    expected.supports_parallel_tool_calls = true;
    expected.base_instructions = "Custom model instructions".to_string();

    assert_eq!(model_info, expected);
}

// Offline helper parity path:
// construct_model_info_offline should apply model_info_overrides before global
// top-level overrides, matching get_model_info precedence.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_helper_applies_model_info_patch() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);
    config.model_context_window = Some(111_111);
    config.model_info_overrides.insert(
        "gpt-fake-offline".to_string(),
        ModelInfoPatch {
            display_name: Some("gpt-fake-offline-dev".to_string()),
            context_window: Some(222_222),
            ..Default::default()
        },
    );

    let model_info =
        codex_core::test_support::construct_model_info_offline("gpt-fake-offline", &config);
    assert_eq!(model_info.display_name, "gpt-fake-offline-dev".to_string());
    assert_eq!(model_info.context_window, Some(111_111));
}

// Prefix-resolution path (requested slug differs from resolved slug):
// request a custom slug that resolves to known remote base model via longest-prefix match,
// then ensure the requested-slug patch is still applied.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn model_info_patch_applies_when_requested_slug_differs_from_resolved_slug() {
    let codex_home = TempDir::new().expect("create temp dir");
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.enable(Feature::RemoteModels);
    let auth_manager = codex_core::AuthManager::from_auth_for_testing(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let manager = ModelsManager::new(config.codex_home.clone(), auth_manager);

    let requested_slug = "gpt-5.1-eval";
    let mut baseline_config = config.clone();
    baseline_config.model_info_overrides.clear();
    let baseline = manager
        .get_model_info(requested_slug, &baseline_config)
        .await;

    config.model_info_overrides.insert(
        requested_slug.to_string(),
        ModelInfoPatch {
            display_name: Some("gpt-5.1-eval-dev".to_string()),
            context_window: Some(456_789),
            ..Default::default()
        },
    );
    let model_info = manager.get_model_info(requested_slug, &config).await;

    let mut expected = baseline;
    expected.slug = requested_slug.to_string();
    expected.display_name = "gpt-5.1-eval-dev".to_string();
    expected.context_window = Some(456_789);

    assert_eq!(model_info, expected);
}
