//! Tests gateway-manager sharing, credential refresh, and model catalog cache isolation.
//! Discovery and inference must preserve primary auth and reject requests when gateway auth fails.

use super::*;
use crate::AgentIdentitySessionFallback;
use crate::ProviderAuthScope;
use crate::create_model_provider;
use crate::test_support::seed_gateway_auth;
use codex_api::Compression;
use codex_api::ResponsesClient;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::ReqwestTransport;
use codex_login::AuthHeaders;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::auth::AgentIdentityAuthPolicy;
use codex_login::default_client::ClientRedirectPolicy;
use codex_login::default_client::create_client_for_route;
use codex_model_provider_info::GatewayOAuthDelivery;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::protocol::SessionSource;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn gateway_credentials_accompany_primary_auth_in_models_and_responses() {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/responses"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(
                    "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r\"}}\n\n",
                ),
        )
        .expect(2)
        .mount(&server)
        .await;
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    for delivery in [
        GatewayOAuthDelivery::Header {
            name: "x-gateway-auth".into(),
            scheme: "Bearer".into(),
        },
        GatewayOAuthDelivery::Cookie {
            name: "gateway_session".into(),
        },
    ] {
        let config = GatewayOAuthConfig {
            authorization_url: format!("{}/authorize", server.uri()),
            token_url: format!("{}/token", server.uri()),
            client_id: "test".into(),
            resource: None,
            scopes: vec![],
            redirect_port: None,
            delivery: delivery.clone(),
        };
        let info = ModelProviderInfo {
            gateway_oauth: Some(config.clone()),
            ..ModelProviderInfo::create_openai_provider(Some(server.uri()))
        };
        let mut primary_headers = HeaderMap::new();
        primary_headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer primary-token"),
        );
        primary_headers.insert("chatgpt-account-id", HeaderValue::from_static("account"));
        let primary = AuthManager::from_auth_for_testing_with_home(
            CodexAuth::Headers(AuthHeaders::new(primary_headers)),
            home.path().to_path_buf(),
        );
        let manager = seed_gateway_auth(
            &info,
            &primary,
            json!({"access_token": "gateway-token", "refresh_token": "refresh-secret", "expires_at": i64::MAX}),
        );
        let provider = create_model_provider(info.clone(), Some(primary));
        assert!(Arc::ptr_eq(
            &provider.gateway_auth_manager().unwrap().unwrap(),
            &manager,
        ));
        let models = provider.models_manager_without_cache(/*config_model_catalog*/ None);
        models.set_api_key_model_discovery_enabled(/*enabled*/ true);
        models
            .raw_model_catalog(RefreshStrategy::Online, factory.clone())
            .await;
        let auth = provider.api_auth().await.unwrap();
        let scoped = provider
            .api_auth_for_scope(ProviderAuthScope {
                agent_identity_policy: AgentIdentityAuthPolicy::JwtOnly,
                session_source: SessionSource::Cli,
                agent_identity_session_fallback: AgentIdentitySessionFallback::default(),
            })
            .await
            .unwrap();
        let mut expected = HeaderMap::from_iter([
            (
                http::header::AUTHORIZATION,
                HeaderValue::from_static("Bearer primary-token"),
            ),
            (
                HeaderName::from_static("chatgpt-account-id"),
                HeaderValue::from_static("account"),
            ),
        ]);
        let (name, value) = match delivery {
            GatewayOAuthDelivery::Header { .. } => ("x-gateway-auth", "Bearer gateway-token"),
            GatewayOAuthDelivery::Cookie { .. } => ("cookie", "gateway_session=gateway-token"),
        };
        expected.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
        // WebSocket handshakes use the synchronous header interface.
        assert_eq!(auth.to_auth_headers(), expected);
        assert_eq!(auth.resolve_auth_headers().await.unwrap(), expected);
        assert_eq!(scoped.auth.to_auth_headers(), expected);
        assert!(auth.to_auth_headers()[name].is_sensitive());
        let api = info.to_api_provider(/*auth_mode*/ None).unwrap();
        let transport = ReqwestTransport::from_http_client(
            create_client_for_route(
                &factory,
                &api.url_for_path("responses"),
                ClientRouteClass::Api,
                ClientRedirectPolicy::Default,
            )
            .unwrap(),
        );
        let client = ResponsesClient::new(transport, api, auth);
        let _stream = client
            .stream(
                json!({"model": "test", "input": []}),
                HeaderMap::new(),
                Compression::None,
                /*turn_state*/ None,
            )
            .await
            .unwrap();
    }
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 4);
    for request in requests {
        assert_eq!(request.headers["authorization"], "Bearer primary-token");
        assert!(
            request
                .headers
                .get("x-gateway-auth")
                .is_some_and(|value| value == "Bearer gateway-token")
                || request
                    .headers
                    .get("cookie")
                    .is_some_and(|value| value == "gateway_session=gateway-token")
        );
    }
}

#[tokio::test]
async fn gateway_refresh_preserves_primary_auth_and_hides_issuer_errors() {
    for succeeds in [true, false] {
        let server = MockServer::start().await;
        let home = tempfile::tempdir().unwrap();
        let primary = AuthManager::from_auth_for_testing_with_home(
            CodexAuth::from_api_key("primary-token"),
            home.path().to_path_buf(),
        );
        let info = ModelProviderInfo {
            gateway_oauth: Some(GatewayOAuthConfig {
                authorization_url: format!("{}/authorize", server.uri()),
                token_url: format!("{}/token?credential=query-secret", server.uri()),
                client_id: "client".into(),
                resource: None,
                scopes: vec![],
                redirect_port: None,
                delivery: GatewayOAuthDelivery::Header {
                    name: "x-gateway-auth".into(),
                    scheme: "Bearer".into(),
                },
            }),
            ..ModelProviderInfo::create_openai_provider(Some(server.uri()))
        };
        let body = if succeeds {
            json!({"access_token": "rotated-token", "expires_in": 3600})
        } else {
            json!({"error": "server_error", "error_description": format!("query-secret {}", "sensitive ".repeat(4000))})
        };
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(if succeeds { 200 } else { 500 }).set_body_json(body),
            )
            .expect(1)
            .mount(&server)
            .await;
        let _gateway = seed_gateway_auth(
            &info,
            &primary,
            json!({"access_token": "expired", "refresh_token": "refresh-secret", "expires_at": 0}),
        );
        let provider = create_model_provider(info, Some(primary.clone()));
        let result = provider.api_auth().await;
        if succeeds {
            assert_eq!(
                result.unwrap().to_auth_headers(),
                HeaderMap::from_iter([
                    (
                        http::header::AUTHORIZATION,
                        HeaderValue::from_static("Bearer primary-token")
                    ),
                    (
                        HeaderName::from_static("x-gateway-auth"),
                        HeaderValue::from_static("Bearer rotated-token")
                    ),
                ])
            );
        } else {
            let error = match result {
                Ok(_) => panic!("issuer failure should fail auth"),
                Err(error) => error,
            };
            assert_eq!(
                error.to_string(),
                "Gateway OAuth authentication failed; check the gateway configuration and credential store."
            );
        }
        assert_eq!(
            primary.auth_cached(),
            Some(CodexAuth::from_api_key("primary-token"))
        );
        let requests = server.received_requests().await.unwrap();
        assert!(!requests[0].headers.contains_key("authorization"));
        assert!(!requests[0].headers.contains_key("x-gateway-auth"));
        assert_eq!(
            std::str::from_utf8(&requests[0].body)
                .unwrap()
                .split('&')
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from([
                "grant_type=refresh_token",
                "refresh_token=refresh-secret",
                "client_id=client"
            ])
        );
    }
}

#[tokio::test]
async fn models_cache_is_reused_only_for_matching_gateway_configuration() {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    let primary = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::from_api_key("primary-token"),
        home.path().to_path_buf(),
    );
    let mut info = ModelProviderInfo {
        http_headers: Some(std::collections::HashMap::from([(
            codex_login::default_client::RESIDENCY_HEADER_NAME.to_string(),
            "us".into(),
        )])),
        gateway_oauth: Some(GatewayOAuthConfig {
            authorization_url: format!("{}/authorize", server.uri()),
            token_url: format!("{}/token", server.uri()),
            client_id: "client".into(),
            resource: None,
            scopes: vec![],
            redirect_port: None,
            delivery: GatewayOAuthDelivery::Header {
                name: "x-gateway-auth".into(),
                scheme: "Bearer".into(),
            },
        }),
        ..ModelProviderInfo::create_openai_provider(Some(server.uri()))
    };
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    let mut model = codex_models_manager::bundled_models_response()
        .unwrap()
        .models
        .remove(0);
    for resource in ["first", "second"] {
        info.gateway_oauth.as_mut().unwrap().resource =
            Some(format!("{}/{resource}", server.uri()));
        model.slug = format!("catalog-{resource}");
        let _gateway = seed_gateway_auth(
            &info,
            &primary,
            json!({"access_token": resource, "refresh_token": "refresh-secret", "expires_at": i64::MAX}),
        );
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(wiremock::matchers::header(
                "x-gateway-auth",
                format!("Bearer {resource}"),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [&model]})))
            .expect(1)
            .mount(&server)
            .await;

        // Recreate the provider and models manager to exercise the shared disk cache.
        // The first call for each resource fetches; the second must reuse its catalog.
        for _ in 0..2 {
            let provider = create_model_provider(info.clone(), Some(primary.clone()));
            let models = provider.models_manager(
                home.path().to_path_buf(),
                /*config_model_catalog*/ None,
            );
            models.set_api_key_model_discovery_enabled(/*enabled*/ true);
            let catalog = models
                .raw_model_catalog(RefreshStrategy::OnlineIfUncached, factory.clone())
                .await;
            assert_eq!(catalog.models, vec![model.clone()]);
        }
    }
}

#[tokio::test]
async fn models_observe_gateway_rotation_from_the_same_provider_instance() {
    models_observe_gateway_rotation(ProviderInstance::Same).await;
}

#[tokio::test]
async fn models_observe_gateway_rotation_from_a_separate_provider_instance() {
    models_observe_gateway_rotation(ProviderInstance::Separate).await;
}

enum ProviderInstance {
    Same,
    Separate,
}

async fn models_observe_gateway_rotation(instance: ProviderInstance) {
    let server = MockServer::start().await;
    let home = tempfile::tempdir().unwrap();
    let primary = AuthManager::from_auth_for_testing_with_home(
        CodexAuth::from_api_key("primary-token"),
        home.path().to_path_buf(),
    );
    let info = ModelProviderInfo {
        gateway_oauth: Some(GatewayOAuthConfig {
            authorization_url: format!("{}/authorize", server.uri()),
            token_url: format!("{}/token", server.uri()),
            client_id: "client".into(),
            resource: None,
            scopes: vec![],
            redirect_port: None,
            delivery: GatewayOAuthDelivery::Header {
                name: "x-gateway-auth".into(),
                scheme: "Bearer".into(),
            },
        }),
        ..ModelProviderInfo::create_openai_provider(Some(server.uri()))
    };
    let gateway = seed_gateway_auth(
        &info,
        &primary,
        json!({"access_token": "first", "refresh_token": "refresh-secret", "expires_at": i64::MAX}),
    );
    let provider = create_model_provider(info.clone(), Some(primary.clone()));
    let models = provider.models_manager_without_cache(/*config_model_catalog*/ None);
    models.set_api_key_model_discovery_enabled(/*enabled*/ true);
    let requests_provider = match instance {
        ProviderInstance::Same => provider,
        ProviderInstance::Separate => create_model_provider(info, Some(primary)),
    };
    let mut first = codex_models_manager::bundled_models_response()
        .unwrap()
        .models
        .remove(0);
    first.slug = "catalog-first".into();
    let mut second = first.clone();
    second.slug = "catalog-second".into();
    for (token, model) in [("first", &first), ("second", &second)] {
        Mock::given(method("GET"))
            .and(path("/models"))
            .and(wiremock::matchers::header(
                "x-gateway-auth",
                format!("Bearer {token}"),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [model]})))
            .expect(1)
            .mount(&server)
            .await;
    }
    assert_eq!(
        requests_provider
            .api_auth()
            .await
            .unwrap()
            .to_auth_headers(),
        HeaderMap::from_iter([
            (
                http::header::AUTHORIZATION,
                HeaderValue::from_static("Bearer primary-token")
            ),
            (
                HeaderName::from_static("x-gateway-auth"),
                HeaderValue::from_static("Bearer first")
            ),
        ])
    );
    models
        .raw_model_catalog(
            RefreshStrategy::Online,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await;
    assert_eq!(models.get_remote_models().await, vec![first]);
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token": "second", "expires_in": 3600})),
        )
        .expect(1)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    gateway.refresh_access_token("first").await.unwrap();
    assert_eq!(
        requests_provider
            .api_auth()
            .await
            .unwrap()
            .to_auth_headers(),
        HeaderMap::from_iter([
            (
                http::header::AUTHORIZATION,
                HeaderValue::from_static("Bearer primary-token")
            ),
            (
                HeaderName::from_static("x-gateway-auth"),
                HeaderValue::from_static("Bearer second")
            ),
        ])
    );
    models
        .raw_model_catalog(
            RefreshStrategy::Online,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await;
    assert_eq!(models.get_remote_models().await, vec![second]);
}

#[tokio::test]
async fn gateway_setup_errors_are_reported_when_authentication_is_requested() {
    use codex_models_manager::manager::ModelsEndpointClient;

    let server = MockServer::start().await;
    let mut info = ModelProviderInfo {
        gateway_oauth: Some(GatewayOAuthConfig {
            authorization_url: format!("{}/authorize", server.uri()),
            token_url: format!("{}/token", server.uri()),
            client_id: "client".into(),
            resource: None,
            scopes: vec![],
            redirect_port: None,
            delivery: GatewayOAuthDelivery::Header {
                name: "x-gateway-auth".into(),
                scheme: "Bearer".into(),
            },
        }),
        ..ModelProviderInfo::create_openai_provider(Some(server.uri()))
    };
    let provider = create_model_provider(info.clone(), /*auth_manager*/ None);
    assert_eq!(
        provider.gateway_auth_manager().unwrap_err().to_string(),
        "gateway_oauth requires auth runtime configuration"
    );
    let error = provider.api_auth().await.err().unwrap();
    assert_eq!(
        error.to_string(),
        "gateway_oauth requires auth runtime configuration"
    );

    let primary = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("primary"));
    let endpoint = crate::models_endpoint::OpenAiModelsEndpoint::new(
        info.clone(),
        Some(primary),
        Some(Err("failed to create provider OAuth HTTP client".into())),
    );
    let error = endpoint
        .list_models(
            "test",
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "failed to create provider OAuth HTTP client"
    );

    info.gateway_oauth.as_mut().unwrap().client_id.clear();
    let primary = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("primary"));
    let error = create_model_provider(info, Some(primary))
        .api_auth()
        .await
        .err()
        .unwrap();
    assert!(matches!(
        error.details(),
        codex_protocol::error::CodexErrorDetails::InvalidRequest(_)
    ));
    assert!(server.received_requests().await.unwrap().is_empty());
}
