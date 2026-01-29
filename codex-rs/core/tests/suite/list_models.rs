use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::ThreadManager;
use codex_core::built_in_model_providers;
use codex_core::features::Feature;
use codex_core::models_manager::manager::RefreshStrategy;
use codex_core::models_manager::model_presets::all_model_presets;
use codex_protocol::openai_models::ModelPreset;
use core_test_support::load_default_config_for_test;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_api_key_models() -> Result<()> {
    let codex_home = tempdir()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.disable(Feature::RemoteModels);
    let manager = ThreadManager::with_models_provider(
        CodexAuth::from_api_key("sk-test"),
        built_in_model_providers()["openai"].clone(),
    );
    let models = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;

    let expected_models = expected_models(false);
    assert_eq!(expected_models, models);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_models_returns_chatgpt_models() -> Result<()> {
    let codex_home = tempdir()?;
    let mut config = load_default_config_for_test(&codex_home).await;
    config.features.disable(Feature::RemoteModels);
    let manager = ThreadManager::with_models_provider(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        built_in_model_providers()["openai"].clone(),
    );
    let models = manager
        .list_models(&config, RefreshStrategy::OnlineIfUncached)
        .await;

    let expected_models = expected_models(true);
    assert_eq!(expected_models, models);

    Ok(())
}

fn expected_models(chatgpt_mode: bool) -> Vec<ModelPreset> {
    let mut models = ModelPreset::filter_by_auth(all_model_presets().clone(), chatgpt_mode);
    for preset in &mut models {
        preset.is_default = false;
    }
    if let Some(default) = models.iter_mut().find(|preset| preset.show_in_picker) {
        default.is_default = true;
    } else if let Some(default) = models.first_mut() {
        default.is_default = true;
    }
    models
}
