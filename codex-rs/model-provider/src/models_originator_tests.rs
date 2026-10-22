//! End-to-end catalog partitioning through provider headers, manager state, and file storage.

use super::*;
use codex_http_client::OutboundProxyPolicy;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::OpenAiModelsManager;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::openai_models::ModelsResponse;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Runtime discovery updates reach existing and newly created scopes, including cached catalogs.
#[tokio::test]
async fn originator_managers_share_api_key_discovery_policy() {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    let mut provider = ModelProviderInfo::create_openai_provider(Some(server.uri()));
    provider.model_catalog_url = Some(format!("{}/models", server.uri()).into());
    provider.http_headers = Some(std::collections::HashMap::from([(
        codex_model_provider_info::RESIDENCY_HEADER_NAME.to_string(),
        "us".into(),
    )]));
    let mut model = model_info_from_slug("gpt-5.5");
    model.visibility = codex_protocol::openai_models::ModelVisibility::List;
    model.used_fallback_model_metadata = false;
    let catalog = ModelsResponse {
        models: vec![model],
    };
    for originator in ["codex_work_cca", "chatgpt_cca"] {
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("originator", originator))
            .respond_with(ResponseTemplate::new(200).set_body_json(&catalog))
            .expect(1)
            .mount(&server)
            .await;
    }
    let auth = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("test-key"));
    let endpoint = Arc::new(OpenAiModelsEndpoint::new(
        provider,
        Some(Arc::clone(&auth)),
        /*gateway_auth_manager*/ None,
    ));
    let manager = OpenAiModelsManager::new(home.path().to_path_buf(), endpoint, Some(auth));
    let work = manager.for_originator("codex_work_cca").unwrap();
    let bundled = codex_models_manager::bundled_models_response().unwrap();
    let client = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    assert_eq!(
        work.raw_model_catalog(RefreshStrategy::Online, client.clone())
            .await,
        bundled
    );
    assert!(server.received_requests().await.unwrap().is_empty());

    manager.set_api_key_model_discovery_enabled(/*enabled*/ true);
    let chat = manager.for_originator("chatgpt_cca").unwrap();
    for child in [&work, &chat] {
        assert_eq!(
            child
                .raw_model_catalog(RefreshStrategy::OnlineIfUncached, client.clone())
                .await,
            catalog
        );
    }

    manager.set_api_key_model_discovery_enabled(/*enabled*/ false);
    for child in [&work, &chat] {
        assert_eq!(
            child
                .raw_model_catalog(RefreshStrategy::Online, client.clone())
                .await,
            bundled
        );
        assert_eq!(child.try_get_remote_models().unwrap(), bundled.models);
    }

    manager.set_api_key_model_discovery_enabled(/*enabled*/ true);
    for child in [&work, &chat] {
        assert_eq!(
            child
                .raw_model_catalog(RefreshStrategy::OnlineIfUncached, client.clone())
                .await,
            catalog
        );
    }
    server.verify().await;
}

/// Work, Chat, and legacy callers retain their own catalogs across concurrent access and restarts.
#[tokio::test]
async fn originator_catalogs_are_isolated_and_reused_from_disk() {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    let mut provider = ModelProviderInfo::create_openai_provider(Some(server.uri()));
    // The managed-residency test toggles the process-wide US policy. Keep this
    // provider's cache identity stable whether that policy is enabled or disabled.
    provider.http_headers = Some(std::collections::HashMap::from([
        ("Originator".to_string(), "provider-default".into()),
        (
            codex_model_provider_info::RESIDENCY_HEADER_NAME.to_string(),
            "us".into(),
        ),
    ]));
    let mut catalogs = Vec::new();
    for originator in [
        "provider-default",
        "codex_work_cca",
        "codex_work_web",
        "codex_work_desktop",
        "codex_work_mobile",
        "chatgpt_cca",
    ] {
        let mut model = model_info_from_slug("gpt-5.5");
        model.visibility = codex_protocol::openai_models::ModelVisibility::List;
        model.used_fallback_model_metadata = false;
        model.description = Some(originator.to_string());
        let catalog = ModelsResponse {
            models: vec![model],
        };
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("originator", originator))
            .respond_with(ResponseTemplate::new(200).set_body_json(&catalog))
            .expect(1)
            .mount(&server)
            .await;
        catalogs.push(catalog);
    }
    let client = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    for _ in 0..2 {
        let auth =
            AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing());
        let endpoint = Arc::new(OpenAiModelsEndpoint::new(
            provider.clone(),
            Some(Arc::clone(&auth)),
            /*gateway_auth_manager*/ None,
        ));
        let manager = OpenAiModelsManager::new(home.path().to_path_buf(), endpoint, Some(auth));
        let work = manager.for_originator("codex_work_cca").unwrap();
        let chat = manager.for_originator("chatgpt_cca").unwrap();
        assert!(Arc::ptr_eq(
            &work,
            &manager.for_originator("codex_work_cca").unwrap()
        ));
        assert!(Arc::ptr_eq(
            &chat,
            &manager.for_originator("chatgpt_cca").unwrap()
        ));
        let (default_catalog, work_catalog, chat_catalog) = tokio::join!(
            manager.raw_model_catalog(RefreshStrategy::OnlineIfUncached, client.clone()),
            work.raw_model_catalog(RefreshStrategy::OnlineIfUncached, client.clone()),
            chat.raw_model_catalog(RefreshStrategy::OnlineIfUncached, client.clone()),
        );
        let mut actual = vec![default_catalog, work_catalog];
        for originator in ["codex_work_web", "codex_work_desktop", "codex_work_mobile"] {
            actual.push(
                manager
                    .for_originator(originator)
                    .unwrap()
                    .raw_model_catalog(RefreshStrategy::OnlineIfUncached, client.clone())
                    .await,
            );
        }
        actual.push(chat_catalog);
        assert_eq!(actual, catalogs);
        for originator in [
            "",
            "unknown",
            "codex_cli_rs",
            "codex_vscode",
            "chatgpt_cca_local_dev",
        ] {
            assert!(manager.for_originator(originator).is_none());
        }
    }
    assert!(home.path().join("models_cache.json").exists());
    assert!(
        home.path()
            .join("models_cache.codex_work_cca.json")
            .exists()
    );
    assert!(home.path().join("models_cache.chatgpt_cca.json").exists());
}
