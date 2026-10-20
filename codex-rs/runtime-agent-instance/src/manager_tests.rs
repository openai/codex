//! Exercise shared bootstrap against HTTP and the existing external credential-provider boundary.

use super::*;
use crate::RuntimeRole;
use base64::Engine;
use codex_http_client::DestinationPolicy;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyController;
use codex_http_client::OutboundProxyPolicy;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthRouteConfig;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::Notify;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;

struct Credentials {
    current: RwLock<CodexAuth>,
    refreshed: CodexAuth,
}

impl ExternalAuth for Credentials {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.current.read().unwrap().clone()) })
    }

    fn refresh(&self, _: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async {
            *self.current.write().unwrap() = self.refreshed.clone();
            Ok(self.refreshed.clone())
        })
    }
}

async fn setup(
    server: &MockServer,
    initial: CodexAuth,
    refreshed: CodexAuth,
) -> (Arc<RuntimeAuthManager>, Arc<Credentials>, Arc<AuthManager>) {
    let credentials = AuthManager::from_auth_for_testing(initial.clone());
    let source = Arc::new(Credentials {
        current: RwLock::new(initial),
        refreshed,
    });
    credentials.set_external_auth(source.clone()).await.unwrap();
    let manager = RuntimeAuthManager::new(
        credentials.clone(),
        AgentHostAuthorization::InstallationId {
            installation_id: Uuid::nil(),
        },
        RuntimeRegistrationMetadata {
            harness: Some("codex".into()),
            harness_version: None,
            role: RuntimeRole::OrchestratorAndExecutor,
        },
        format!("{}/agent/register", server.uri()).parse().unwrap(),
    );
    (Arc::new(manager), source, credentials)
}

fn registered(request: &Request) -> ResponseTemplate {
    let body: Value = request.body_json().unwrap();
    let actor = if body["actor_auth"]["type"] == "oauth" {
        json!({"kind": "chatgpt", "user_id": "user-a", "account_id": "workspace-a"})
    } else {
        json!({"kind": "platform", "user_id": "user-a", "organization_id": "org-a", "project_id": "project-a"})
    };
    ResponseTemplate::new(200).set_body_json(json!({
        "runtime_agent_instance_id": request.headers["idempotency-key"].to_str().unwrap(),
        "agent_host_id": "host-a", "actor": actor,
    }))
}

fn oauth(session: Option<&str>, token: &str) -> CodexAuth {
    let claims = json!({
        "iss": "https://auth.openai.com/", "session_id": session, "jti": token,
        "https://api.openai.com/auth": {"user_id": "user-a", "chatgpt_account_id": "workspace-a"},
    });
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
    CodexAuth::from_external_chatgpt_tokens(
        &format!("header.{payload}.signature"),
        "workspace-a",
        /*chatgpt_plan_type*/ None,
    )
    .unwrap()
}

#[tokio::test]
async fn concurrent_requests_share_registration_and_replacement_preserves_in_flight_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(registered)
        .mount(&server)
        .await;
    let initial = CodexAuth::from_api_key("key-a");
    let (manager, source, _) = setup(&server, initial.clone(), initial).await;
    assert!(manager.auth().await.is_err());
    let (first, second) = tokio::join!(manager.auth(), manager.auth());
    let first = first.unwrap();
    let second = second.unwrap();
    assert!(Arc::ptr_eq(first.runtime(), second.runtime()));
    *source.current.write().unwrap() = CodexAuth::from_api_key("key-b");
    let replacement = manager.auth().await.unwrap();
    assert_ne!(first.runtime().id(), replacement.runtime().id());
    assert_eq!(first.auth().get_token().unwrap(), "key-a");
    assert!(!format!("{first:?}").contains("key-a"));
    assert_eq!(replacement.auth().get_token().unwrap(), "key-b");
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].body, requests[1].body);
    assert_eq!(
        requests[0].headers["idempotency-key"],
        requests[1].headers["idempotency-key"]
    );
    assert_ne!(
        requests[1].body_json::<Value>().unwrap()["public_key"],
        requests[2].body_json::<Value>().unwrap()["public_key"]
    );
}

#[tokio::test]
async fn policy_recovery_preserves_pending_registration() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .respond_with(registered)
        .mount(&server)
        .await;
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    let home = tempfile::tempdir().unwrap();
    let credentials = Arc::new(
        AuthManager::new(
            home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::Ephemeral,
            /*forced_chatgpt_workspace_id*/ None,
            /*chatgpt_base_url*/ None,
            AuthKeyringBackendKind::Direct,
            AuthRouteConfig::from_http_client_factory(
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
                    .with_network_policy(policy.clone()),
            ),
        )
        .await,
    );
    let auth = CodexAuth::from_api_key("key-a");
    credentials
        .set_external_auth(Arc::new(Credentials {
            current: RwLock::new(auth.clone()),
            refreshed: auth,
        }))
        .await
        .unwrap();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let manager = RuntimeAuthManager::new(
        credentials,
        AgentHostAuthorization::InstallationId {
            installation_id: Uuid::nil(),
        },
        RuntimeRegistrationMetadata {
            harness: Some("codex".into()),
            harness_version: None,
            role: RuntimeRole::OrchestratorAndExecutor,
        },
        format!("{}/agent/register", server.uri()).parse().unwrap(),
    );
    assert!(matches!(
        manager.auth().await,
        Err(RuntimeAuthError::Registration(
            RegistrationError::Rejected {
                status: StatusCode::SERVICE_UNAVAILABLE,
                ..
            }
        ))
    ));

    policy.invalidate();
    assert!(matches!(
        manager.auth().await,
        Err(RuntimeAuthError::Policy(NetworkPolicyDenied::Unavailable))
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);

    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let selected = manager.auth().await.unwrap();
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body, requests[1].body);
    assert_eq!(
        requests[0].headers["idempotency-key"],
        requests[1].headers["idempotency-key"]
    );
    assert_eq!(
        selected.runtime().id(),
        requests[1].headers["idempotency-key"].to_str().unwrap()
    );
}

#[tokio::test]
async fn bootstrap_refresh_reuses_known_sources_but_replaces_sessionless_initialization() {
    for session in [Some("login-a"), None] {
        let server = MockServer::start().await;
        let initial = oauth(session, "old");
        Mock::given(method("POST"))
            .and(body_partial_json(
                json!({"actor_auth": {"credential": initial.get_token().unwrap()}}),
            ))
            .respond_with(ResponseTemplate::new(401))
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .respond_with(registered)
            .mount(&server)
            .await;
        let (manager, source, _) =
            setup(&server, initial, oauth(Some("login-a"), "refreshed")).await;
        let first = manager.auth().await.unwrap();
        *source.current.write().unwrap() = oauth(Some("login-a"), "next-token");
        let next = manager.auth().await.unwrap();
        assert!(Arc::ptr_eq(first.runtime(), next.runtime()));
        assert_ne!(
            first.auth().get_token().unwrap(),
            next.auth().get_token().unwrap()
        );
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].headers["idempotency-key"] == requests[1].headers["idempotency-key"],
            session.is_some()
        );
        assert_eq!(
            requests[0].body_json::<Value>().unwrap()["public_key"]
                == requests[1].body_json::<Value>().unwrap()["public_key"],
            session.is_some()
        );
    }
}

#[tokio::test]
async fn recovery_is_bounded_and_revocation_cannot_trigger_reregistration() {
    for status in [401, 403] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(status))
            .mount(&server)
            .await;
        let (manager, _, _) = setup(
            &server,
            oauth(Some("login-a"), "old"),
            oauth(Some("login-a"), "refreshed"),
        )
        .await;
        assert!(
            matches!(manager.auth().await, Err(RuntimeAuthError::Registration(RegistrationError::Rejected { status: actual, .. })) if actual.as_u16() == status)
        );
        if status == 403 {
            assert!(manager.auth().await.is_err());
        }
        assert_eq!(
            server.received_requests().await.unwrap().len(),
            if status == 401 { 2 } else { 1 }
        );
    }
}

#[tokio::test]
async fn a_late_registration_cannot_publish_the_previous_actor() {
    let server = MockServer::start().await;
    let received = Arc::new(Notify::new());
    let signal = received.clone();
    Mock::given(method("POST"))
        .respond_with(move |request: &Request| {
            signal.notify_one();
            registered(request).set_delay(Duration::from_secs(1))
        })
        .mount(&server)
        .await;
    let initial = CodexAuth::from_api_key("key-a");
    let (manager, source, credentials) = setup(&server, initial.clone(), initial).await;
    let task_manager = manager.clone();
    let attempt = tokio::spawn(async move { task_manager.auth().await });
    received.notified().await;
    *source.current.write().unwrap() = CodexAuth::from_api_key("key-b");
    credentials.auth().await.unwrap();
    assert!(matches!(
        attempt.await.unwrap(),
        Err(RuntimeAuthError::Changed)
    ));
    let current = manager.auth().await.unwrap();
    assert_eq!(current.auth().get_token().unwrap(), "key-b");
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_ne!(
        requests[0].headers["idempotency-key"],
        requests[1].headers["idempotency-key"]
    );
}
