//! Exercise modern and legacy plugin requests with a shared, late-initialized runtime.

use super::*;
use base64::Engine;
use codex_features::RuntimeAuthMode;
use codex_http_client::DestinationPolicy;
use codex_http_client::NetworkPolicyController;
use codex_http_client::NetworkPolicyDenied;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use codex_login::AuthRouteConfig;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeAuthSlot;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use serde_json::json;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

#[tokio::test]
async fn plugin_runtime_auth_shares_registration_and_strips_cross_origin_auth() {
    for (mode, pooled) in [
        (RuntimeAuthMode::DualHeader, false),
        (RuntimeAuthMode::AssertionOnly, false),
        (RuntimeAuthMode::DualHeader, true),
        (RuntimeAuthMode::AssertionOnly, true),
    ] {
        let server = MockServer::start().await;
        let destination = MockServer::start().await;
        Mock::given(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "enabled": true, "plugins": [],
            })))
            .expect(1)
            .mount(&destination)
            .await;
        Mock::given(path("/agent/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": "runtime-plugins", "agent_host_id": "host-a",
                "actor": {"kind": "chatgpt", "user_id": "user-a", "account_id": "account-a"},
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/backend-api/ps/plugins/list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [], "pagination": {"next_page_token": null},
            })))
            .expect(2)
            .mount(&server)
            .await;
        Mock::given(path("/backend-api/plugins/featured"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/backend-api/ps/plugins/suggested/codex"))
            .respond_with(ResponseTemplate::new(307).insert_header("location", destination.uri()))
            .expect(1)
            .mount(&server)
            .await;
        let claims = json!({
            "iss": "https://auth.openai.com/", "session_id": "login-a",
            "https://api.openai.com/auth": {"user_id": "user-a", "chatgpt_account_id": "account-a"},
        });
        let token = format!(
            "header.{}.signature",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let auth = CodexAuth::from_external_chatgpt_tokens(
            &token,
            "account-a",
            /*chatgpt_plan_type*/ None,
        )
        .unwrap();
        let slot = RuntimeAuthSlot::default();
        let controller = NetworkPolicyController::default();
        let policy = controller.policy();
        let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(policy.clone());
        let home = tempfile::tempdir().unwrap();
        codex_login::auth::login_with_chatgpt_auth_tokens(
            home.path(),
            &token,
            "account-a",
            /*chatgpt_plan_type*/ None,
        )
        .unwrap();
        let credentials = Arc::new(
            AuthManager::new(
                home.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::Ephemeral,
                /*forced_chatgpt_workspace_id*/ None,
                /*chatgpt_base_url*/ None,
                AuthKeyringBackendKind::Direct,
                AuthRouteConfig::from_http_client_factory(factory.clone()),
            )
            .await,
        );
        assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
        let mut config = if pooled {
            crate::manager::PluginsConfigInput::new(
                Default::default(),
                Default::default(),
                "openai".into(),
                /*plugins_enabled*/ true,
                /*remote_plugin_enabled*/ true,
                "https://chatgpt.com/backend-api".into(),
                factory,
                /*product_sku*/ None,
            )
            .remote_plugin_service_config(Some((Arc::clone(&slot), mode)))
        } else {
            RemotePluginServiceConfig::new_with_selected_runtime_auth(
                "https://chatgpt.com/backend-api".into(),
                factory,
                /*product_sku*/ None,
                (Arc::clone(&slot), mode),
            )
        };
        config.chatgpt_base_url = format!("{}/backend-api", server.uri());
        // Enabled requests cannot silently fall back before the harness initializes the slot.
        assert!(
            fetch_recommended_plugins(&config, Some(&auth))
                .await
                .is_err()
        );
        assert!(server.received_requests().await.unwrap().is_empty());
        slot.set(Arc::new(RuntimeAuthManager::new(
            credentials,
            AgentHostAuthorization::InstallationId {
                installation_id: Uuid::nil(),
            },
            RuntimeRegistrationMetadata {
                harness: Some("codex".into()),
                harness_version: None,
                role: RuntimeRole::Orchestrator,
            },
            format!("{}/agent/register", server.uri()).parse().unwrap(),
        )))
        .unwrap();
        get_remote_plugin_list_page(
            &config,
            &auth,
            RemotePluginScope::Global,
            /*page_token*/ None,
            /*collection*/ None,
        )
        .await
        .unwrap();
        crate::remote_legacy::fetch_remote_featured_plugin_ids(
            &config,
            Some(&auth),
            /*product*/ None,
        )
        .await
        .unwrap();
        fetch_recommended_plugins(&config, Some(&auth))
            .await
            .unwrap();
        let redirected = destination.received_requests().await.unwrap();
        assert_eq!(redirected.len(), 1);
        assert!(!redirected[0].headers.contains_key("authorization"));
        assert!(
            !redirected[0]
                .headers
                .contains_key("x-openai-agent-assertion")
        );
        destination.verify().await;
        let url = format!("{}/backend-api/ps/plugins/list", server.uri());
        let pending = config
            .authenticated_request(Method::GET, &url, &auth)
            .await
            .unwrap();
        policy.invalidate();
        assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
        assert!(matches!(
            pending.send().await,
            Err(RouteAwareRequestError::Policy(NetworkPolicyDenied::Revoked))
        ));
        let response = config
            .authenticated_request(Method::GET, &url, &auth)
            .await
            .unwrap()
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let requests = server.received_requests().await.unwrap();
        let mut proof_ids = HashSet::new();
        for request in requests
            .iter()
            .filter(|r| r.url.path() != "/agent/register")
        {
            let authorization = request.headers["authorization"].to_str().unwrap();
            let proof = match mode {
                RuntimeAuthMode::DualHeader => {
                    assert_eq!(authorization, format!("Bearer {token}"));
                    request.headers["x-openai-agent-assertion"]
                        .to_str()
                        .unwrap()
                }
                RuntimeAuthMode::AssertionOnly => {
                    assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                    authorization.strip_prefix("AgentAssertion ").unwrap()
                }
            };
            assert_eq!(request.headers["chatgpt-account-id"], "account-a");
            let payload: JsonValue = serde_json::from_slice(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .decode(proof.split('.').nth(2).unwrap())
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(payload["runtime_agent_instance"], "runtime-plugins");
            assert!(proof_ids.insert(payload["id"].as_str().unwrap().to_string()));
        }
        assert_eq!(proof_ids.len(), 4);
        server.verify().await;
    }
}

#[tokio::test]
async fn custom_plugin_backend_keeps_legacy_auth_with_runtime_enabled() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        Mock::given(path("/backend-api/plugins/featured"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;
        let config = RemotePluginServiceConfig::new_with_runtime_auth(
            format!("{}/backend-api", server.uri()),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            /*product_sku*/ None,
            (RuntimeAuthSlot::default(), mode),
        );
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        crate::remote_legacy::fetch_remote_featured_plugin_ids(
            &config,
            Some(&auth),
            /*product*/ None,
        )
        .await
        .unwrap();
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(
            requests[0].headers["authorization"]
                .to_str()
                .unwrap()
                .starts_with("Bearer ")
        );
        assert!(!requests[0].headers.contains_key("x-openai-agent-assertion"));
        server.verify().await;
    }
}

#[tokio::test]
async fn unsupported_openai_plugin_backend_fails_before_registration() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        let auth = CodexAuth::from_api_key("key-a");
        let runtime = (
            Arc::new(std::sync::OnceLock::from(Arc::new(
                RuntimeAuthManager::new(
                    AuthManager::from_auth_for_testing(auth.clone()),
                    AgentHostAuthorization::InstallationId {
                        installation_id: Uuid::nil(),
                    },
                    RuntimeRegistrationMetadata {
                        harness: Some("codex".into()),
                        harness_version: None,
                        role: RuntimeRole::Orchestrator,
                    },
                    format!("{}/register", server.uri()).parse().unwrap(),
                ),
            ))),
            mode,
        );
        let config = RemotePluginServiceConfig::new_with_runtime_auth(
            "https://future.openai.com/backend-api".to_string(),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            /*product_sku*/ None,
            runtime,
        );
        assert!(
            config
                .authenticated_request(
                    Method::GET,
                    "https://future.openai.com/backend-api/plugins/featured",
                    &auth,
                )
                .await
                .is_err()
        );
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
