use super::*;

use base64::Engine;
use codex_config::types::AuthKeyringBackendKind;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::HttpRedirectPolicy;
use codex_exec_server::HttpRequestParams;
use codex_features::RuntimeAuthMode;
use codex_http_client::DestinationPolicy;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyController;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthManager;
use codex_login::AuthRouteConfig;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

struct RotatingAuth(std::sync::RwLock<CodexAuth>);

impl codex_login::ExternalAuth for RotatingAuth {
    fn resolve(&self) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.read().unwrap().clone()) })
    }

    fn refresh(
        &self,
        _: codex_login::ExternalAuthRefreshContext,
    ) -> codex_login::ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

#[tokio::test]
async fn deferred_connection_keeps_runtime_source_and_captured_account_policy() -> anyhow::Result<()>
{
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        Mock::given(path("/agent/register"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "runtime_agent_instance_id": "runtime-mcp", "agent_host_id": "host-a",
                "actor": {"kind": "chatgpt", "user_id": "user-a", "account_id": "workspace-a"},
            })))
            .mount(&server)
            .await;
        Mock::given(path("/mcp"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let home = tempfile::tempdir()?;
        let oauth = |session, token| {
            let claims = json!({
                "iss": "https://auth.openai.com/", "session_id": session, "jti": token,
                "https://api.openai.com/auth": {"user_id": "user-a", "chatgpt_account_id": "workspace-a"}
            });
            let payload =
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
            CodexAuth::from_external_chatgpt_tokens(
                &format!("header.{payload}.signature"),
                "workspace-a",
                /*chatgpt_plan_type*/ None,
            )
            .unwrap()
        };
        let initial = oauth("session-a", "initial");
        let refreshed = oauth("session-a", "refreshed");
        let source = Arc::new(RotatingAuth(std::sync::RwLock::new(initial.clone())));
        let controller = NetworkPolicyController::default();
        let policy = controller.policy();
        let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(policy.clone());
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
        credentials.set_external_auth(source.clone()).await?;
        assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
        let runtime = (
            Arc::new(std::sync::OnceLock::from(Arc::new(
                RuntimeAuthManager::new(
                    Arc::clone(&credentials),
                    AgentHostAuthorization::InstallationId {
                        installation_id: uuid::Uuid::nil(),
                    },
                    RuntimeRegistrationMetadata {
                        harness: Some("codex".into()),
                        harness_version: None,
                        role: RuntimeRole::Orchestrator,
                    },
                    format!("{}/agent/register", server.uri()).parse()?,
                ),
            ))),
            mode,
        );
        let context = McpRuntimeContext::new(
            Arc::new(EnvironmentManager::without_environments(factory)),
            home.path().to_path_buf(),
        );
        let deferred = McpAuthProvider::runtime(runtime.clone(), initial.clone());
        assert!(server.received_requests().await.unwrap().is_empty());
        let error = deferred
            .resolve(context.clone(), "https://future.openai.com/mcp")
            .await
            .err()
            .expect("unsupported OpenAI destination must fail before registration");
        assert!(
            error
                .downcast_ref::<codex_protocol::error::CodexErr>()
                .is_some_and(|error| matches!(
                    error.details(),
                    codex_protocol::error::CodexErrorDetails::UnsupportedOperation(_)
                ))
        );
        assert!(server.received_requests().await.unwrap().is_empty());
        let (provider, selected_context) = deferred.resolve(context.clone(), &server.uri()).await?;
        let first = provider
            .resolve(&Default::default())
            .await?
            .apply_auth_headers(Default::default())
            .await?;
        *source.0.write().unwrap() = refreshed.clone();
        let second = provider
            .resolve(&Default::default())
            .await?
            .apply_auth_headers(Default::default())
            .await?;
        let assertion_header = match mode {
            RuntimeAuthMode::DualHeader => "x-openai-agent-assertion",
            RuntimeAuthMode::AssertionOnly => "authorization",
        };
        assert_ne!(first[assertion_header], second[assertion_header]);
        if mode == RuntimeAuthMode::DualHeader {
            assert_eq!(
                first["authorization"],
                format!("Bearer {}", initial.get_token()?)
            );
            assert_eq!(
                second["authorization"],
                format!("Bearer {}", refreshed.get_token()?)
            );
        }
        let request = || HttpRequestParams {
            method: "GET".into(),
            url: format!("{}/mcp", server.uri()),
            headers: Vec::new(),
            body: None,
            timeout_ms: Some(1000),
            redirect_policy: HttpRedirectPolicy::Stop,
            request_id: "mcp-policy-test".into(),
            stream_response: false,
        };
        let client = selected_context.local_http_client();
        assert_eq!(client.http_request(request()).await?.status, 200);
        policy.invalidate();
        assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
        assert!(client.http_request(request()).await.is_err());
        // A new connection captures the new policy and reuses the same registered runtime.
        let current = McpAuthProvider::runtime(runtime, refreshed);
        let (_, current_context) = current.resolve(context, &server.uri()).await?;
        assert_eq!(
            current_context
                .local_http_client()
                .http_request(request())
                .await?
                .status,
            200
        );
        let requests = server.received_requests().await.unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.url.path() == "/agent/register")
                .count(),
            1
        );
        *source.0.write().unwrap() = oauth("another-session", "replacement");
        assert!(provider.resolve(&Default::default()).await.is_err());
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.url.path() == "/mcp")
                .count(),
            2
        );
    }
    Ok(())
}
