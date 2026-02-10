use anyhow::Result;
use codex_core::CodexAuth;
use codex_core::ThreadManager;
use codex_core::built_in_model_providers;
use codex_core::models_manager::manager::RefreshStrategy;
use core_test_support::load_default_config_for_test;
use pretty_assertions::assert_eq;
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

    let slugs: Vec<String> = models.into_iter().map(|m| m.id).collect();
    assert_eq!(expected_slugs(), slugs);

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

    let slugs: Vec<String> = models.into_iter().map(|m| m.id).collect();
    assert_eq!(expected_slugs(), slugs);

    Ok(())
}

fn expected_slugs() -> Vec<String> {
    vec![
        "gpt-5.2-codex".into(),
        "gpt-5.1-codex-max".into(),
        "gpt-5.1-codex".into(),
        "gpt-5.2".into(),
        "gpt-5.1".into(),
        "gpt-5-codex".into(),
        "gpt-5".into(),
        "gpt-5.1-codex-mini".into(),
        "gpt-5-codex-mini".into(),
        "bengalfox".into(),
        "boomslang".into(),
    ]
}
