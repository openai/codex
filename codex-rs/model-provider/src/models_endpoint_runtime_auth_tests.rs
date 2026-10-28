use super::*;
use base64::Engine;
use codex_features::RuntimeAuthMode;
use codex_http_client::OutboundProxyPolicy;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn provider_auth_rejects_selectors_that_do_not_match_the_registered_actor() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        for selector in ["chatgpt-account-id", "openai-organization"] {
            let server = MockServer::start().await;
            let (auth, actor, expected) = if selector == "chatgpt-account-id" {
                let claims = json!({"session_id": "session-a", "https://api.openai.com/auth": {"user_id": "user-a", "chatgpt_account_id": "workspace-a"}});
                let token = format!(
                    "e30.{}.signature",
                    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string())
                );
                (
                    CodexAuth::from_external_chatgpt_tokens(
                        &token,
                        "workspace-a",
                        /*chatgpt_plan_type*/ None,
                    )
                    .unwrap(),
                    json!({"kind":"chatgpt", "user_id":"user-a", "account_id":"workspace-a"}),
                    "workspace-a",
                )
            } else {
                (
                    CodexAuth::from_api_key("key-a"),
                    json!({"kind":"platform", "user_id":"user-a", "organization_id":"org-a", "project_id":"project-a"}),
                    "org-a",
                )
            };
            Mock::given(method("POST"))
                .and(path("/register"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "runtime_agent_instance_id": "runtime-a", "agent_host_id": "host-a", "actor": actor,
                })))
                .mount(&server)
                .await;
            let credentials = AuthManager::from_auth_for_testing(auth);
            let context = crate::ProviderAuthContext {
                runtime_auth: Some((runtime_manager(&server, Arc::clone(&credentials)), mode)),
                credentials,
            };
            let mut info = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
            info.env_http_headers = None;
            info.http_headers = Some(std::collections::HashMap::from([(
                selector.into(),
                expected.into(),
            )]));
            let provider = context.model_provider(info.clone());
            let auth = provider.api_auth().await.unwrap();
            assert!(
                auth.to_auth_headers()
                    .unwrap()
                    .contains_key("authorization")
            );

            info.http_headers = Some(std::collections::HashMap::from([(
                selector.into(),
                "different".into(),
            )]));
            let provider = context.model_provider(info);
            let error = provider
                .api_auth()
                .await
                .err()
                .expect("mismatched selector must be rejected");
            assert!(matches!(
                error.details(),
                codex_protocol::error::CodexErrorDetails::UnsupportedOperation(_)
            ));
            assert!(error.to_string().contains(selector));
            assert!(!server.received_requests().await.unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn model_requests_keep_the_selected_project_while_reusing_one_runtime() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": "runtime-a",
                "agent_host_id": "host-a",
                "actor": {"kind":"platform", "user_id":"user-a", "organization_id":"org-a", "project_id":"project-a"},
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models":[]})))
            .expect(4)
            .mount(&server)
            .await;
        let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("key-a"));
        let runtime = runtime_manager(&server, Arc::clone(&credentials));
        let context = crate::ProviderAuthContext {
            runtime_auth: Some((Arc::clone(&runtime), mode)),
            credentials: Arc::clone(&credentials),
        };
        let projects = [
            Some("project-a"),
            Some("project-b"),
            None,
            Some("project-a"),
        ];
        for project in projects {
            let mut info = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
            info.env_http_headers = None;
            let mut headers =
                std::collections::HashMap::from([("openai-organization".into(), "org-a".into())]);
            if let Some(project) = project {
                headers.insert("openai-project".into(), project.into());
            }
            info.http_headers = Some(headers);
            let auth = context
                .model_provider(info.clone())
                .api_auth()
                .await
                .unwrap();
            assert!(
                auth.to_auth_headers()
                    .unwrap()
                    .contains_key("authorization")
            );

            info.base_url = Some(server.uri());
            let endpoint = OpenAiModelsEndpoint::new(
                info,
                Some(Arc::clone(&credentials)),
                /*gateway_auth_manager*/ None,
            )
            .with_runtime_auth(Some((Arc::clone(&runtime), mode)));
            endpoint
                .list_models(
                    "test",
                    HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                )
                .await
                .unwrap();
        }
        let requests = server.received_requests().await.unwrap();
        let models: Vec<_> = requests
            .iter()
            .filter(|request| request.url.path() == "/models")
            .collect();
        assert_eq!(models.len(), projects.len());
        for (request, project) in models.into_iter().zip(projects) {
            assert_eq!(
                request
                    .headers
                    .get("openai-project")
                    .map(|value| value.to_str().unwrap()),
                project
            );
            assert_eq!(request.headers["openai-organization"], "org-a");
            match mode {
                RuntimeAuthMode::DualHeader => {
                    assert_eq!(request.headers["authorization"], "Bearer key-a");
                    assert!(request.headers.contains_key("x-openai-agent-assertion"));
                }
                RuntimeAuthMode::AssertionOnly => {
                    assert!(
                        request.headers["authorization"]
                            .to_str()
                            .unwrap()
                            .starts_with("AgentAssertion ")
                    );
                    assert!(!request.headers.contains_key("x-openai-agent-assertion"));
                }
            }
        }
        server.verify().await;
    }
}

#[tokio::test]
async fn unsupported_openai_destinations_do_not_fall_back_to_bearer_auth() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("key-a"));
        let runtime = runtime_manager(&server, Arc::clone(&credentials));
        let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
        for destination in [
            "https://unknown.api.openai.com/v1",
            "http://api.openai.com/v1",
            "https://api.openai.com:8443/v1",
        ] {
            let info = ModelProviderInfo::create_openai_provider(Some(destination.into()));
            let provider = crate::provider::create_model_provider_with_runtime(
                info.clone(),
                Some(Arc::clone(&credentials)),
                Some((Arc::clone(&runtime), mode)),
            );
            let error = provider
                .request_auth(/*scope*/ None)
                .await
                .err()
                .expect("unsupported inference origin");
            assert!(
                error
                    .to_string()
                    .contains("does not support this OpenAI endpoint")
            );
            let mut catalog_info = info;
            catalog_info.base_url = Some("https://api.openai.com/v1".into());
            catalog_info.model_catalog_url = Some(format!("{destination}/models").into());
            let catalog = OpenAiModelsEndpoint::new(
                catalog_info,
                Some(Arc::clone(&credentials)),
                /*gateway_auth_manager*/ None,
            )
            .with_runtime_auth(Some((Arc::clone(&runtime), mode)));
            let error = catalog
                .list_models("0.0.0", factory.clone())
                .await
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("does not support this OpenAI endpoint")
            );
        }
        // An explicitly configured other provider retains its existing bearer auth.
        let info = ModelProviderInfo::create_openai_provider(Some(server.uri()));
        let provider = crate::provider::create_model_provider_with_runtime(
            info,
            Some(credentials),
            Some((runtime, mode)),
        );
        let resolved = provider.request_auth(/*scope*/ None).await.unwrap();
        let headers = resolved
            .auth
            .apply_auth_headers(http::HeaderMap::new())
            .await
            .unwrap();
        assert_eq!(headers["authorization"], "Bearer key-a");
        assert!(!headers.contains_key("x-openai-agent-assertion"));
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}

#[tokio::test]
async fn catalog_timeout_keeps_registration_alive_for_the_next_request() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/register"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(6)).set_body_json(json!({
            "runtime_agent_instance_id": "runtime-a",
            "agent_host_id": "host-a",
            "actor": {"kind":"platform", "user_id":"user-a", "organization_id":"org-a", "project_id":null},
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models":[]})))
        .expect(1)
        .mount(&server)
        .await;
    let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("key-a"));
    let runtime = runtime_manager(&server, Arc::clone(&credentials));
    let mut provider = ModelProviderInfo::create_openai_provider(Some(server.uri()));
    provider.env_http_headers = None;
    let endpoint = OpenAiModelsEndpoint::new(
        provider,
        Some(credentials),
        /*gateway_auth_manager*/ None,
    )
    .with_runtime_auth(Some((runtime, RuntimeAuthMode::DualHeader)));
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
    let result = timeout(
        Duration::from_secs(10),
        endpoint.list_models("0.0.0", factory.clone()),
    )
    .await
    .expect("catalog timeout must precede the registration client's timeout");
    assert!(matches!(
        result.unwrap_err().details(),
        codex_protocol::error::CodexErrorDetails::RequestTimeout
    ));
    endpoint.list_models("0.0.0", factory).await.unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.url.path() == "/register")
            .count(),
        1
    );
    server.verify().await;
}

#[tokio::test]
async fn runtime_catalog_follows_redirects_without_forwarding_credentials_to_another_origin() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        for policy in [
            OutboundProxyPolicy::ReqwestDefault,
            OutboundProxyPolicy::RespectSystemProxy,
        ] {
            let server = MockServer::start().await;
            let target = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/register"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "runtime_agent_instance_id": "runtime-a",
                    "agent_host_id": "host-a",
                    "actor": {"kind":"platform", "user_id":"user-a", "organization_id":"org-a", "project_id":null},
                })))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/models"))
                .respond_with(ResponseTemplate::new(307).insert_header("location", "/same-origin"))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/same-origin"))
                .respond_with(
                    ResponseTemplate::new(302)
                        .insert_header("location", format!("{}/models", target.uri())),
                )
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/models"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
                .expect(1)
                .mount(&target)
                .await;
            let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("key-a"));
            let runtime = runtime_manager(&server, Arc::clone(&credentials));
            let mut info = ModelProviderInfo::create_openai_provider(Some(server.uri()));
            info.env_http_headers = None;
            let endpoint = OpenAiModelsEndpoint::new(
                info,
                Some(credentials),
                /*gateway_auth_manager*/ None,
            )
            .with_runtime_auth(Some((runtime, mode)));
            endpoint
                .list_models("test", HttpClientFactory::new(policy))
                .await
                .unwrap();
            let requests = server.received_requests().await.unwrap();
            let initial = requests
                .iter()
                .find(|request| request.url.path() == "/models")
                .unwrap();
            let same_origin = requests
                .iter()
                .find(|request| request.url.path() == "/same-origin")
                .unwrap();
            assert_eq!(
                same_origin.headers.get("authorization"),
                initial.headers.get("authorization")
            );
            assert_eq!(
                same_origin.headers.get("x-openai-agent-assertion"),
                initial.headers.get("x-openai-agent-assertion")
            );
            match mode {
                RuntimeAuthMode::DualHeader => {
                    assert_eq!(initial.headers["authorization"], "Bearer key-a");
                    assert!(initial.headers.contains_key("x-openai-agent-assertion"));
                }
                RuntimeAuthMode::AssertionOnly => {
                    assert!(
                        initial.headers["authorization"]
                            .to_str()
                            .unwrap()
                            .starts_with("AgentAssertion ")
                    );
                    assert!(!initial.headers.contains_key("x-openai-agent-assertion"));
                }
            }
            let redirected = target.received_requests().await.unwrap();
            assert_eq!(redirected.len(), 1);
            assert!(!redirected[0].headers.contains_key("authorization"));
            assert!(
                !redirected[0]
                    .headers
                    .contains_key("x-openai-agent-assertion")
            );
            server.verify().await;
            target.verify().await;
        }
    }
}

fn runtime_manager(server: &MockServer, credentials: Arc<AuthManager>) -> Arc<RuntimeAuthManager> {
    Arc::new(RuntimeAuthManager::new(
        credentials,
        AgentHostAuthorization::InstallationId {
            installation_id: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
        },
        RuntimeRegistrationMetadata {
            harness: None,
            harness_version: None,
            role: RuntimeRole::Orchestrator,
        },
        format!("{}/register", server.uri()).parse().unwrap(),
    ))
}
