//! Unit tests for the models_endpoint module.

use std::num::NonZeroU64;
use std::sync::Mutex;

use super::*;
use codex_http_client::OutboundProxyPolicy;
use codex_login::default_client::RESIDENCY_HEADER_NAME;
use codex_login::default_client::ResidencyRequirement;
use codex_login::default_client::create_client;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_models_manager::manager::ModelsManager;
use codex_models_manager::manager::OpenAiModelsManager;
use codex_models_manager::manager::RefreshStrategy;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

#[derive(Debug)]
struct RecordingTransportBuilder {
    observed_request: Arc<Mutex<Option<(OutboundProxyPolicy, String)>>>,
}

impl ModelsTransportBuilder for RecordingTransportBuilder {
    fn build(
        &self,
        http_client_factory: HttpClientFactory,
        request_url: String,
    ) -> ModelsTransportFuture<'_> {
        let observed_request = Arc::clone(&self.observed_request);
        Box::pin(async move {
            *observed_request
                .lock()
                .expect("observed request lock should not be poisoned") =
                Some((http_client_factory.outbound_proxy_policy(), request_url));
            Ok(ReqwestTransport::from_http_client(create_client()))
        })
    }
}

#[derive(Debug)]
struct CaptureModelsUrl(Mutex<Option<String>>);

impl ModelsTransportBuilder for CaptureModelsUrl {
    fn build(
        &self,
        _http_client_factory: HttpClientFactory,
        request_url: String,
    ) -> ModelsTransportFuture<'_> {
        *self.0.lock().unwrap() = Some(request_url);
        Box::pin(async { Err(std::io::Error::other("transport intentionally unavailable")) })
    }
}

#[tokio::test]
async fn api_key_discovery_respects_provider_routing() {
    let client_version = codex_models_manager::client_version_to_whole();
    for (api_key, name, base_url, models_url, inference_url) in [
        (
            "test-api-key",
            "OpenAI",
            None,
            Some("https://chatgpt.com/backend-api/codex/models"),
            "https://api.openai.com/v1",
        ),
        (
            "test-api-key",
            "OpenAI",
            Some("https://example.com/codex"),
            None,
            "https://example.com/codex",
        ),
        (
            "test-api-key",
            "Azure",
            Some("https://example.openai.azure.com/openai/v1"),
            None,
            "https://example.openai.azure.com/openai/v1",
        ),
        (
            "sk-proj-fedramp-synthetic-test-key",
            "OpenAI",
            None,
            Some("https://gov.chatgpt.com/backend-api/codex/models"),
            "https://gov.api.openai.com/v1",
        ),
        (
            "sk-svcacct-fedramp-synthetic-test-key",
            "OpenAI",
            None,
            Some("https://gov.chatgpt.com/backend-api/codex/models"),
            "https://gov.api.openai.com/v1",
        ),
        (
            "sk-proj-fedramp-synthetic-test-key",
            "OpenAI",
            Some("https://example.com/codex"),
            None,
            "https://example.com/codex",
        ),
        (
            "sk-svcacct-fedramp-synthetic-test-key",
            "Azure",
            None,
            None,
            "https://api.openai.com/v1",
        ),
    ] {
        let capture = Arc::new(CaptureModelsUrl(Mutex::new(/*t*/ None)));
        let auth = AuthManager::from_auth_for_testing(CodexAuth::from_api_key(api_key));
        let endpoint = Arc::new(OpenAiModelsEndpoint {
            provider_info: ModelProviderInfo {
                name: name.to_string(),
                ..ModelProviderInfo::create_openai_provider(base_url.map(str::to_string))
            },
            auth_manager: Some(auth.clone()),
            gateway_auth_manager: None,
            runtime_auth: None,
            transport_builder: capture.clone(),
        });
        let provider = crate::provider::create_model_provider(
            endpoint.provider_info.clone(),
            Some(auth.clone()),
        );
        let manager = OpenAiModelsManager::new_without_cache(endpoint, Some(auth));
        manager.set_api_key_model_discovery_enabled(/*enabled*/ true);
        manager
            .raw_model_catalog(
                RefreshStrategy::Online,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await;
        assert_eq!(
            *capture.0.lock().unwrap(),
            models_url.map(|url| format!("{url}?client_version={client_version}"))
        );
        assert_eq!(
            provider.api_provider().await.unwrap().base_url,
            inference_url
        );
    }
}

#[tokio::test]
async fn provider_api_key_without_endpoint_overrides_uses_codex_backend() {
    for (api_key, auth, models_base_url, inference_url) in [
        (
            "provider-key",
            None,
            CHATGPT_CODEX_BASE_URL,
            "https://api.openai.com/v1",
        ),
        (
            "provider-key",
            Some(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            CHATGPT_CODEX_BASE_URL,
            CHATGPT_CODEX_BASE_URL,
        ),
        (
            "provider-key",
            Some(CodexAuth::from_api_key("sk-proj-fedramp-ambient-key")),
            CHATGPT_CODEX_BASE_URL,
            "https://api.openai.com/v1",
        ),
        (
            "sk-proj-fedramp-provider-key",
            None,
            "https://gov.chatgpt.com/backend-api/codex",
            "https://gov.api.openai.com/v1",
        ),
        (
            "sk-svcacct-fedramp-provider-key",
            Some(CodexAuth::from_api_key("legacy-ambient-key")),
            "https://gov.chatgpt.com/backend-api/codex",
            "https://gov.api.openai.com/v1",
        ),
        (
            "sk-proj-fedramp-provider-key",
            Some(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            "https://gov.chatgpt.com/backend-api/codex",
            "https://gov.api.openai.com/v1",
        ),
    ] {
        let capture = Arc::new(CaptureModelsUrl(Mutex::new(/*t*/ None)));
        let auth_manager = auth.map(AuthManager::from_auth_for_testing);
        let endpoint = Arc::new(OpenAiModelsEndpoint {
            provider_info: ModelProviderInfo {
                experimental_bearer_token: Some(api_key.into()),
                ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
            },
            auth_manager: auth_manager.clone(),
            gateway_auth_manager: None,
            runtime_auth: None,
            transport_builder: capture.clone(),
        });
        let provider = crate::provider::create_model_provider(
            endpoint.provider_info.clone(),
            auth_manager.clone(),
        );
        let manager = OpenAiModelsManager::new_without_cache(endpoint, auth_manager);
        manager.set_api_key_model_discovery_enabled(/*enabled*/ true);
        manager
            .raw_model_catalog(
                RefreshStrategy::Online,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await;
        assert_eq!(
            *capture.0.lock().unwrap(),
            Some(format!(
                "{models_base_url}/models?client_version={}",
                codex_models_manager::client_version_to_whole(),
            ))
        );
        assert_eq!(
            provider.api_provider().await.unwrap().base_url,
            inference_url
        );
    }
}

fn provider_info_with_command_auth() -> ModelProviderInfo {
    ModelProviderInfo {
        auth: Some(ModelProviderAuthInfo {
            command: "print-token".to_string(),
            args: Vec::new(),
            timeout_ms: NonZeroU64::new(5_000).expect("timeout should be non-zero"),
            refresh_interval_ms: 300_000,
            cwd: std::env::current_dir()
                .expect("current dir should be available")
                .try_into()
                .expect("current dir should be absolute"),
        }),
        requires_openai_auth: false,
        ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
    }
}

#[test]
fn command_auth_provider_reports_command_auth_without_cached_auth() {
    let endpoint = OpenAiModelsEndpoint::new(
        provider_info_with_command_auth(),
        /*auth_manager*/ None,
        /*gateway_auth_manager*/ None,
    );

    assert!(endpoint.has_command_auth());
    let auth = CodexAuth::from_api_key("sk-proj-fedramp-command-key");
    assert_eq!(
        resolve_api_provider(&endpoint.provider_info, Some(&auth))
            .unwrap()
            .base_url,
        "https://api.openai.com/v1"
    );
}

#[test]
fn provider_without_command_auth_reports_no_command_auth() {
    let endpoint = OpenAiModelsEndpoint::new(
        ModelProviderInfo::create_openai_provider(/*base_url*/ None),
        /*auth_manager*/ None,
        /*gateway_auth_manager*/ None,
    );

    assert!(!endpoint.has_command_auth());
}

#[tokio::test]
async fn model_request_uses_request_time_proxy_policy_and_exact_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(query_param("client_version", "0.0.0"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(ModelsResponse { models: Vec::new() }),
        )
        .expect(1)
        .mount(&server)
        .await;

    let observed_request = Arc::new(Mutex::new(None));
    let endpoint = OpenAiModelsEndpoint {
        provider_info: ModelProviderInfo::create_openai_provider(Some(server.uri())),
        auth_manager: None,
        gateway_auth_manager: None,
        runtime_auth: None,
        transport_builder: Arc::new(RecordingTransportBuilder {
            observed_request: Arc::clone(&observed_request),
        }),
    };

    endpoint
        .list_models(
            "0.0.0",
            HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        )
        .await
        .expect("models request should succeed");

    assert_eq!(
        *observed_request
            .lock()
            .expect("observed request lock should not be poisoned"),
        Some((
            OutboundProxyPolicy::RespectSystemProxy,
            format!("{}/models?client_version=0.0.0", server.uri()),
        ))
    );
}

#[tokio::test]
async fn model_discovery_enforces_managed_residency_over_provider_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header(RESIDENCY_HEADER_NAME, "us"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(ModelsResponse { models: Vec::new() }),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut provider_info = ModelProviderInfo::create_openai_provider(Some(server.uri()));
    provider_info.http_headers = Some(std::collections::HashMap::from([(
        RESIDENCY_HEADER_NAME.to_string(),
        "eu".into(),
    )]));
    let endpoint = OpenAiModelsEndpoint {
        provider_info,
        auth_manager: None,
        gateway_auth_manager: None,
        runtime_auth: None,
        transport_builder: Arc::new(RecordingTransportBuilder {
            observed_request: Arc::new(Mutex::new(None)),
        }),
    };

    set_default_client_residency_requirement(Some(ResidencyRequirement::Us));
    endpoint
        .list_models(
            "0.0.0",
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await
        .expect("managed residency model discovery should succeed");
    set_default_client_residency_requirement(/*enforce_residency*/ None);

    assert_eq!(
        endpoint
            .provider_info
            .http_headers
            .as_ref()
            .and_then(|headers| headers.get(RESIDENCY_HEADER_NAME)),
        Some(&"eu".into())
    );
}

#[derive(Debug)]
struct RotatingAuth(std::sync::atomic::AtomicUsize);

impl codex_login::ExternalAuth for RotatingAuth {
    fn resolve(&self) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async move {
            let generation = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(CodexAuth::from_api_key(&format!("token-{generation}")))
        })
    }

    fn refresh(
        &self,
        _context: codex_login::ExternalAuthRefreshContext,
    ) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

#[tokio::test]
async fn command_auth_refresh_fetches_a_catalog_for_the_current_credentials() {
    use codex_models_manager::manager::ModelsManager;
    use codex_models_manager::manager::OpenAiModelsManager;
    use codex_models_manager::manager::RefreshStrategy;

    let server = MockServer::start().await;
    let auth = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("initial"));
    auth.set_external_auth(Arc::new(RotatingAuth(std::sync::atomic::AtomicUsize::new(
        0,
    ))))
    .await
    .unwrap();
    let model = model_info_from_slug("command-auth-model");
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ModelsResponse {
            models: vec![model.clone()],
        }))
        .expect(/*r*/ 3)
        .mount(&server)
        .await;
    let mut provider = provider_info_with_command_auth();
    provider.base_url = Some(server.uri());
    // Keep this test independent of the residency override exercised in parallel.
    provider.http_headers = Some(std::collections::HashMap::from([(
        RESIDENCY_HEADER_NAME.to_string(),
        "us".into(),
    )]));
    let manager = OpenAiModelsManager::new_without_cache(
        Arc::new(OpenAiModelsEndpoint::new(
            provider,
            Some(auth.clone()),
            /*gateway_auth_manager*/ None,
        )),
        Some(auth.clone()),
    );
    let catalog = manager
        .raw_model_catalog(
            RefreshStrategy::OnlineIfUncached,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await;
    assert_eq!(
        catalog
            .models
            .iter()
            .find(|candidate| candidate.slug == model.slug),
        Some(&model)
    );
    // The cached identity still matches here; resolving command auth must
    // detect the next token before deciding whether a refresh is needed.
    manager
        .refresh_after_auth_change(HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault))
        .await;
    assert_eq!(manager.get_remote_models().await, catalog.models);
    auth.auth().await;
    let bundled = codex_models_manager::bundled_models_response().unwrap();
    assert_eq!(manager.get_remote_models().await, bundled.models);
    assert_eq!(manager.try_get_remote_models().unwrap(), bundled.models);
    assert_eq!(
        manager
            .get_model_info(&model.slug, &Default::default())
            .await,
        codex_models_manager::CatalogModel {
            model: model_info_from_slug(&model.slug).into(),
            origin: codex_models_manager::ModelOrigin::Synthetic,
        }
    );
    assert_eq!(
        manager
            .raw_model_catalog(
                RefreshStrategy::Offline,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await,
        bundled
    );
    assert_eq!(
        manager
            .raw_model_catalog(
                RefreshStrategy::OnlineIfUncached,
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await,
        catalog
    );
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.headers["authorization"].to_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Bearer token-2", "Bearer token-5", "Bearer token-9"]
    );
}

#[tokio::test]
async fn explicit_catalog_reuses_auth_headers_and_query_parameters() {
    for auth in [
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        CodexAuth::from_api_key("test-key"),
        CodexAuth::from_api_key("sk-proj-fedramp-synthetic-test-key"),
        CodexAuth::from_api_key("sk-svcacct-fedramp-synthetic-test-key"),
    ] {
        let server = MockServer::start().await;
        let mut model = model_info_from_slug("provider-model");
        model.visibility = ModelVisibility::List;
        model.supported_in_api = true;

        let expected = ModelsResponse {
            models: vec![model],
        };
        Mock::given(method("GET"))
            .and(path("/codex/models"))
            .and(header(
                "authorization",
                format!("Bearer {}", auth.get_token().unwrap()),
            ))
            .and(header("x-provider-header", "preserved"))
            .and(query_param("deployment", "one"))
            .and(query_param("api-version", "2026-09"))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(&expected))
            .expect(1)
            .mount(&server)
            .await;
        let auth_manager = AuthManager::from_auth_for_testing(auth);
        // The parallel residency test must not change this catalog's cache identity.
        let provider = ModelProviderInfo {
            model_catalog_url: Some(format!("{}/codex/models?deployment=one", server.uri()).into()),
            query_params: Some(std::collections::HashMap::from([(
                "api-version".to_string(),
                "2026-09".into(),
            )])),
            http_headers: Some(std::collections::HashMap::from([
                ("x-provider-header".to_string(), "preserved".into()),
                (
                    codex_login::default_client::RESIDENCY_HEADER_NAME.to_string(),
                    "us".into(),
                ),
            ])),
            ..ModelProviderInfo::create_openai_provider(Some(format!("{}/v1", server.uri())))
        };
        let home = tempfile::tempdir().unwrap();
        let endpoint = Arc::new(OpenAiModelsEndpoint::new(
            provider,
            Some(auth_manager.clone()),
            /*gateway_auth_manager*/ None,
        ));
        let manager = OpenAiModelsManager::new(
            home.path().to_path_buf(),
            endpoint,
            Some(auth_manager.clone()),
        )
        .with_provider_catalog();
        // SIWC does not require the API-key rollout flag.
        manager.set_api_key_model_discovery_enabled(
            auth_manager.auth_mode() == Some(codex_protocol::auth::AuthMode::ApiKey),
        );
        for _ in 0..2 {
            assert_eq!(
                manager
                    .raw_model_catalog(
                        RefreshStrategy::OnlineIfUncached,
                        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
                    )
                    .await,
                expected
            );
        }
    }
}

#[tokio::test]
async fn explicit_catalog_requires_unique_nonempty_slugs() {
    for slugs in [vec![" \t"], vec!["private-model", "private-model"]] {
        let server = MockServer::start().await;
        let catalog = ModelsResponse {
            models: slugs.into_iter().map(model_info_from_slug).collect(),
        };
        Mock::given(method("GET"))
            .and(path("/codex/models"))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(catalog))
            .expect(1)
            .mount(&server)
            .await;
        let endpoint = OpenAiModelsEndpoint::new(
            ModelProviderInfo {
                model_catalog_url: Some(format!("{}/codex/models", server.uri()).into()),
                ..ModelProviderInfo::default()
            },
            /*auth_manager*/ None,
            /*gateway_auth_manager*/ None,
        );
        let error = endpoint
            .list_models(
                "1.2.3",
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            )
            .await
            .expect_err("ambiguous model identity must be rejected");
        assert_eq!(
            error.to_string(),
            "model catalog must contain unique, non-empty model slugs"
        );
    }
}

#[tokio::test]
async fn explicit_catalog_rejects_redirects_without_forwarding_provider_credentials() {
    let server = MockServer::start().await;
    let destination = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/codex/models"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 302)
                .insert_header("location", format!("{}/codex/models", destination.uri()))
                .set_body_string("catalog-secret"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let endpoint = OpenAiModelsEndpoint::new(
        ModelProviderInfo {
            base_url: Some(server.uri()),
            model_catalog_url: Some(
                format!("{}/codex/models?token=catalog-secret", server.uri()).into(),
            ),
            experimental_bearer_token: Some("provider-key".into()),
            ..ModelProviderInfo::default()
        },
        /*auth_manager*/ None,
        /*gateway_auth_manager*/ None,
    );
    let error = endpoint
        .list_models(
            "1.2.3",
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await
        .expect_err("redirect should be rejected");
    assert!(!error.to_string().contains("catalog-secret"));
    assert!(!format!("{error:?}").contains("catalog-secret"));
    assert_eq!(destination.received_requests().await.unwrap().len(), 0);
}
