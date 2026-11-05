//! Environment discovery must resolve runtime authentication for each HTTP request.

use super::*;
use codex_features::RuntimeAuthMode;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_runtime_agent_instance::AgentHostAuthorization;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use pretty_assertions::assert_eq;
use std::collections::HashSet;
use std::sync::Arc;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

struct ChangingCredentials(std::sync::RwLock<CodexAuth>);

impl codex_login::ExternalAuth for ChangingCredentials {
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
async fn discovery_signs_each_get_and_rejects_a_changed_credential_source() {
    for mode in [RuntimeAuthMode::DualHeader, RuntimeAuthMode::AssertionOnly] {
        let server = MockServer::start().await;
        Mock::given(path("/agent/register")).respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "runtime_agent_instance_id": "runtime-cloud", "agent_host_id": "host-a",
            "actor": {"kind": "platform", "user_id": "user-a", "organization_id": "org-a", "project_id": null},
        }))).mount(&server).await;
        Mock::given(path("/api/codex/environments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .expect(2)
            .mount(&server)
            .await;
        let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault);
        let auth = CodexAuth::from_api_key("key-a");
        let credentials = AuthManager::from_auth_for_testing(auth.clone());
        let source = Arc::new(ChangingCredentials(std::sync::RwLock::new(auth.clone())));
        credentials.set_external_auth(source.clone()).await.unwrap();
        let runtime = (
            Arc::new(RuntimeAuthManager::new(
                credentials,
                AgentHostAuthorization::InstallationId {
                    installation_id: uuid::Uuid::nil(),
                },
                RuntimeRegistrationMetadata {
                    harness: Some("codex".into()),
                    harness_version: None,
                    role: RuntimeRole::Orchestrator,
                },
                format!("{}/agent/register", server.uri()).parse().unwrap(),
            )),
            mode,
        );
        let (auth, factory) = ClientAuth::for_auth(
            &auth,
            Some(&runtime),
            factory,
            /*headers*/ &Default::default(),
        )
        .await
        .unwrap();
        let client = EnvironmentClient {
            http: RouteAwareClientPool::new_without_redirects_or_request_logging(
                factory,
                ClientRouteClass::Api,
            ),
            auth: Some(auth),
        };
        for _ in 0..2 {
            assert!(
                list_environments_with_origins(&client, &server.uri(), &HeaderMap::new(), &[])
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|request| request.url.path() == "/agent/register")
                .count(),
            1,
        );
        *source.0.write().unwrap() = CodexAuth::from_api_key("key-b");
        assert!(
            list_environments_with_origins(&client, &server.uri(), &HeaderMap::new(), &[])
                .await
                .is_err()
        );
        let requests = server.received_requests().await.unwrap();
        let proofs: HashSet<_> = requests
            .iter()
            .filter(|r| r.url.path() != "/agent/register")
            .map(|r| {
                let authorization = r.headers["authorization"].to_str().unwrap();
                match mode {
                    RuntimeAuthMode::DualHeader => {
                        assert_eq!(authorization, "Bearer key-a");
                        r.headers["x-openai-agent-assertion"].to_str().unwrap()
                    }
                    RuntimeAuthMode::AssertionOnly => {
                        assert!(!r.headers.contains_key("x-openai-agent-assertion"));
                        authorization.strip_prefix("AgentAssertion ").unwrap()
                    }
                }
            })
            .collect();
        assert_eq!(proofs.len(), 2);
        server.verify().await;
    }
}
