use super::*;
use crate::ProviderAuthContext;
use base64::Engine;
use codex_features::RuntimeAuthMode;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::CodexAuth;
use codex_login::auth::BedrockApiKeyAuth;
use codex_login::auth::RefreshTokenFailedReason;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_protocol::error::CodexErrorDetails;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::num::NonZeroU64;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

async fn context(
    server: &MockServer,
    credentials: Arc<AuthManager>,
    mode: RuntimeAuthMode,
) -> ProviderAuthContext {
    Mock::given(method("POST"))
        .respond_with(|request: &wiremock::Request| ResponseTemplate::new(200).set_body_json(json!({
            "runtime_agent_instance_id": request.headers["idempotency-key"].to_str().unwrap(),
            "agent_host_id": "host-a",
            "actor": {"kind": "platform", "user_id": "user-a", "organization_id": "org-a", "project_id": null},
        })))
        .mount(server).await;
    let runtime = RuntimeAuthManager::new(
        Arc::clone(&credentials),
        AgentHostAuthorization::InstallationId {
            installation_id: "00000000-0000-0000-0000-000000000000".parse().unwrap(),
        },
        RuntimeRegistrationMetadata {
            harness: None,
            harness_version: None,
            role: RuntimeRole::Orchestrator,
        },
        format!("{}/agent/register", server.uri()).parse().unwrap(),
    );
    ProviderAuthContext {
        credentials,
        runtime_auth: Some((Arc::new(runtime), mode)),
    }
}

#[tokio::test]
async fn explicit_keys_and_commands_use_their_own_shared_runtime() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        let home = tempfile::tempdir().unwrap();
        let credentials = AuthManager::from_auth_for_testing_with_home(
            CodexAuth::from_api_key("ambient-key"),
            home.path().to_path_buf(),
        );
        let context = context(&server, credentials, mode).await;
        let literal = ModelProviderInfo {
            experimental_bearer_token: Some("literal-key".into()),
            requires_openai_auth: false,
            ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
        };
        #[cfg(unix)]
        let (command, args) = ("sh", vec!["-c".into(), "printf command-key".into()]);
        #[cfg(windows)]
        let (command, args) = (
            "cmd.exe",
            vec![
                "/D".into(),
                "/Q".into(),
                "/C".into(),
                "echo command-key".into(),
            ],
        );
        let commanded = ModelProviderInfo {
            auth: Some(ModelProviderAuthInfo {
                command: command.into(),
                args,
                timeout_ms: NonZeroU64::new(5000).unwrap(),
                refresh_interval_ms: 300_000,
                cwd: home.path().to_path_buf().try_into().unwrap(),
            }),
            requires_openai_auth: false,
            ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
        };
        for (info, key) in [(literal.clone(), "literal-key"), (commanded, "command-key")] {
            let mut runtime_id = None;
            for _ in 0..2 {
                let provider = context.model_provider(info.clone());
                let selected = provider.request_auth(/*scope*/ None).await.unwrap();
                let runtime = selected.runtime.as_ref().unwrap();
                assert_eq!(runtime.auth().api_key(), Some(key));
                match &runtime_id {
                    Some(id) => assert_eq!(runtime.runtime().id(), id),
                    None => runtime_id = Some(runtime.runtime().id().to_string()),
                }
                let headers = selected
                    .auth
                    .apply_auth_headers(Default::default())
                    .await
                    .unwrap();
                match mode {
                    RuntimeAuthMode::DualHeader => {
                        assert_eq!(headers["authorization"], format!("Bearer {key}"));
                        assert!(headers.contains_key("x-openai-agent-assertion"));
                    }
                    RuntimeAuthMode::AssertionOnly => {
                        assert!(
                            headers["authorization"]
                                .to_str()
                                .unwrap()
                                .starts_with("AgentAssertion ")
                        );
                        assert!(!headers.contains_key("x-openai-agent-assertion"));
                    }
                }
            }
        }
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        for (request, key) in requests.iter().zip(["literal-key", "command-key"]) {
            assert_eq!(
                request.body_json::<serde_json::Value>().unwrap()["actor_auth"]["credential"],
                key
            );
        }
        let custom = context.model_provider(ModelProviderInfo {
            base_url: Some("https://provider.example/v1".into()),
            ..literal
        });
        let auth = custom.request_auth(/*scope*/ None).await.unwrap();
        assert!(auth.runtime.is_none());
        let headers = auth
            .auth
            .apply_auth_headers(Default::default())
            .await
            .unwrap();
        assert_eq!(headers["authorization"], "Bearer literal-key");
        assert!(!headers.contains_key("x-openai-agent-assertion"));
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }
}

#[tokio::test]
async fn explicit_provider_key_keeps_the_requested_project_scope() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        let credentials =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("ambient-key"));
        let context = context(&server, credentials, mode).await;
        server.reset().await;
        Mock::given(method("POST"))
            .and(wiremock::matchers::body_partial_json(json!({
                "actor_auth": {
                    "type": "platform_api_key",
                    "credential": "selected-key",
                    "project_id": "proj-selected",
                },
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": "rai-selected",
                "agent_host_id": "host-a",
                "actor": {
                    "kind": "platform",
                    "user_id": "user-a",
                    "organization_id": "org-a",
                    "project_id": "proj-selected",
                },
            })))
            .mount(&server)
            .await;
        let provider = context.model_provider(ModelProviderInfo {
            experimental_bearer_token: Some("selected-key".into()),
            http_headers: Some(std::collections::HashMap::from([(
                OPENAI_PROJECT_HEADER.into(),
                "proj-selected".into(),
            )])),
            requires_openai_auth: false,
            ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
        });
        let selected = provider.request_auth(/*scope*/ None).await.unwrap();
        let runtime = selected.runtime.as_ref().unwrap();
        assert_eq!(runtime.auth().api_key(), Some("selected-key"));
        assert!(matches!(
            runtime.runtime().actor().info(),
            ActorInfo::Platform { project_id: Some(project), .. } if project == "proj-selected"
        ));
    }
}

#[tokio::test]
async fn bedrock_auth_stays_independent_when_openai_runtime_auth_is_enabled() {
    let server = MockServer::start().await;
    let credentials =
        AuthManager::from_auth_for_testing(CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "bedrock-key".into(),
            region: "us-east-1".into(),
        }));
    let context = context(&server, credentials, RuntimeAuthMode::DualHeader).await;
    let bedrock = context.model_provider(ModelProviderInfo::create_amazon_bedrock_provider(
        /*aws*/ None,
    ));
    let headers = bedrock
        .api_auth()
        .await
        .unwrap()
        .apply_auth_headers(Default::default())
        .await
        .unwrap();
    assert_eq!(headers["authorization"], "Bearer bedrock-key");
    assert!(!headers.contains_key("x-openai-agent-assertion"));
    assert!(server.received_requests().await.unwrap().is_empty());

    let openai = context.model_provider(ModelProviderInfo {
        experimental_bearer_token: Some("openai-key".into()),
        ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
    });
    let headers = openai
        .api_auth()
        .await
        .unwrap()
        .apply_auth_headers(Default::default())
        .await
        .unwrap();
    assert_eq!(headers["authorization"], "Bearer openai-key");
    assert!(headers.contains_key("x-openai-agent-assertion"));
    let headers = bedrock
        .api_auth()
        .await
        .unwrap()
        .apply_auth_headers(Default::default())
        .await
        .unwrap();
    assert_eq!(headers["authorization"], "Bearer bedrock-key");
    assert!(!headers.contains_key("x-openai-agent-assertion"));
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].body_json::<serde_json::Value>().unwrap()["actor_auth"]["credential"],
        "openai-key"
    );
}

#[tokio::test]
async fn runtime_denial_recovery_is_scoped_to_openai() {
    let server = MockServer::start().await;
    let credentials = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("actor-key"));
    let context = context(&server, credentials, RuntimeAuthMode::AssertionOnly).await;
    for (url, has_runtime) in [
        ("https://api.openai.com/v1", true),
        ("https://provider.example/v1", false),
    ] {
        let provider = context.model_provider(ModelProviderInfo::create_openai_provider(Some(
            url.to_string(),
        )));
        for code in ["invalid_agent_instance_assertion", "token_expired"] {
            let error = codex_api::TransportError::Http {
                status: http::StatusCode::UNAUTHORIZED,
                retry_after: None,
                url: Some(format!("{url}/responses")),
                headers: None,
                body: Some(json!({"error": {"code": code}}).to_string()),
            };
            assert_eq!(
                provider.is_recoverable_auth_error(&error),
                !has_runtime || code == "token_expired"
            );
        }
    }
}

#[tokio::test]
async fn permanent_runtime_refresh_failure_preserves_sign_in_error() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        // Exercise proactive refresh and registration's existing 401 recovery.
        for last_refresh in ["2000-01-01T00:00:00Z", "2099-01-01T00:00:00Z"] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/oauth/token"))
                .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                    "error": {"code": "refresh_token_invalidated"},
                })))
                .expect(1)
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/register"))
                .respond_with(ResponseTemplate::new(401))
                .mount(&server)
                .await;
            let home = tempfile::tempdir().expect("create auth home");
            let claims = json!({
                "https://api.openai.com/auth": {
                    "chatgpt_user_id": "user-a",
                    "chatgpt_account_id": "workspace-a",
                },
            });
            let token = format!(
                "e30.{}.signature",
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string())
            );
            let stored = serde_json::from_value(json!({
                "auth_mode": "chatgpt",
                "tokens": {
                    "id_token": token, "access_token": "initial-token",
                    "refresh_token": "revoked-refresh-token", "account_id": "workspace-a",
                    "oauth_provenance": {
                        "issuer": server.uri(), "client_id": "runtime-refresh-test",
                    },
                },
                "last_refresh": last_refresh,
            }))
            .expect("construct stored OAuth credentials");
            codex_login::save_auth(
                home.path(),
                &stored,
                AuthCredentialsStoreMode::File,
                AuthKeyringBackendKind::Direct,
            )
            .expect("save OAuth credentials");
            let credentials = Arc::new(
                AuthManager::new(
                    home.path().to_path_buf(),
                    /*enable_codex_api_key_env*/ false,
                    AuthCredentialsStoreMode::File,
                    /*forced_chatgpt_workspace_id*/ None,
                    /*chatgpt_base_url*/ None,
                    AuthKeyringBackendKind::Direct,
                    codex_login::test_support::transport_default_auth_route_config(),
                )
                .await,
            );
            let original = credentials.auth_cached().expect("load OAuth credentials");
            let runtime = Arc::new(RuntimeAuthManager::new(
                Arc::clone(&credentials),
                AgentHostAuthorization::InstallationId {
                    installation_id: "00000000-0000-0000-0000-000000000000"
                        .parse()
                        .expect("parse installation ID"),
                },
                RuntimeRegistrationMetadata {
                    harness: None,
                    harness_version: None,
                    role: RuntimeRole::Orchestrator,
                },
                format!("{}/register", server.uri())
                    .parse()
                    .expect("parse registration URL"),
            ));
            let provider = ProviderAuthContext {
                credentials: Arc::clone(&credentials),
                runtime_auth: Some((Arc::clone(&runtime), mode)),
            }
            .model_provider(ModelProviderInfo::create_openai_provider(
                /*base_url*/ None,
            ));

            // A later request must preserve the cached rejection, not retry it as a stream error.
            for _ in 0..2 {
                let error = provider
                    .request_auth(/*scope*/ None)
                    .await
                    .err()
                    .expect("revoked refresh must reject request preparation");
                let failure = credentials
                    .refresh_failure_for_auth(&original)
                    .expect("permanent refresh rejection is cached");
                assert_eq!(failure.reason, RefreshTokenFailedReason::Revoked);
                let CodexErrorDetails::RefreshTokenFailed(actual) = error.details() else {
                    panic!("expected sign-in error for {mode:?}/{last_refresh}, got {error:?}");
                };
                assert_eq!(actual, &failure);
                assert_eq!(error.retry_delay(/*retry_count*/ 1), None);
            }

            server.verify().await;
        }
    }
}
