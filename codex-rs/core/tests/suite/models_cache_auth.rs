//! Catalogs must follow auth and provider changes without leaking default billing tiers.

use std::sync::Arc;

use anyhow::Result;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::CodexAuth;
use codex_login::login_with_api_key;
use codex_models_manager::bundled_models_response;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use core_test_support::responses;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_and_provider_switches_do_not_reuse_chatgpt_catalog() -> Result<()> {
    let server = wiremock::MockServer::start().await;
    let home = Arc::new(TempDir::new()?);
    let mut model = bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-5.5")
        .unwrap();
    model.visibility = ModelVisibility::List;
    model.default_service_tier = Some("priority".into());
    let models_mock = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![model.clone()],
        },
    )
    .await;
    let chatgpt = test_codex()
        .with_home(home.clone())
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.5")
        .build_with_auto_env(&server)
        .await?;
    assert!(home.path().join("models_cache.json").exists());
    assert_eq!(
        chatgpt
            .thread_manager
            .get_models_manager()
            .get_remote_models()
            .await,
        vec![model.clone()]
    );
    login_with_api_key(
        home.path(),
        "api-key",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    assert!(chatgpt.thread_manager.auth_manager().reload().await);
    let manager = chatgpt.thread_manager.get_models_manager();
    let bundled = bundled_models_response()?.models;
    assert_eq!(manager.get_remote_models().await, bundled);
    assert_eq!(
        manager
            .raw_model_catalog(
                RefreshStrategy::Offline,
                codex_core::test_support::default_http_client_factory()
            )
            .await
            .models,
        bundled
    );
    assert_eq!(models_mock.requests().len(), 1);
    drop(chatgpt);
    server.reset().await;

    let mut api_model = model.clone();
    api_model.default_service_tier = None;
    let api_models_mock = responses::mount_models_once(
        &server,
        ModelsResponse {
            models: vec![api_model],
        },
    )
    .await;
    let api = test_codex()
        .with_home(home.clone())
        .with_auth(CodexAuth::from_api_key("api-key"))
        .with_config(|config| {
            config
                .features
                .enable(codex_features::Feature::ApiKeyModelDiscovery)
                .expect("enable API-key model discovery");
        })
        .with_model("gpt-5.5")
        .build_with_auto_env(&server)
        .await?;
    let ordinary = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("ordinary"),
            ev_completed("ordinary"),
        ]),
    )
    .await;
    api.submit_turn("hello").await?;
    assert_eq!(
        ordinary.single_request().body_json().get("service_tier"),
        None
    );

    let explicit = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("explicit"),
            ev_completed("explicit"),
        ]),
    )
    .await;
    api.submit_turn_with_service_tier("hello again", Some("priority"))
        .await?;
    assert_eq!(
        explicit.single_request().body_json()["service_tier"],
        "priority"
    );
    assert_eq!(api_models_mock.requests().len(), 1);
    drop(api);

    let other_server = wiremock::MockServer::start().await;
    model.default_service_tier = None;
    model.display_name = "Second provider model".into();
    let other_models = responses::mount_models_once(
        &other_server,
        ModelsResponse {
            models: vec![model.clone()],
        },
    )
    .await;
    let other = test_codex()
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.5")
        .with_config(|config| {
            config.model_provider_id = "second".into();
            config.model_provider.name = "Second".into();
        })
        .build_with_auto_env(&other_server)
        .await?;
    assert_eq!(
        other
            .thread_manager
            .get_models_manager()
            .get_remote_models()
            .await,
        vec![model]
    );
    assert_eq!(other_models.requests().len(), 1);
    Ok(())
}
