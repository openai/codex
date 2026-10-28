//! Exercise bootstrap and signing through a real HTTP server without changing process auth.

use super::*;
use crate::AgentHostAuthorization;
use crate::AuthenticatedActorAuthorization;
use crate::RuntimeAgentInstance;
use crate::RuntimeRegistrationMetadata;
use crate::RuntimeRole;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_http_client::DestinationPolicy;
use codex_http_client::NetworkPolicyController;
use codex_http_client::OutboundProxyPolicy;
use ed25519_dalek::Signature;
use ed25519_dalek::VerifyingKey;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use uuid::Uuid;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn pending() -> RuntimeRegistration {
    RuntimeRegistration::new(
        AuthenticatedActorAuthorization::PlatformApiKey {
            selection: Default::default(),
            credential: "actor-secret".into(),
        },
        AgentHostAuthorization::InstallationId {
            installation_id: Uuid::nil(),
        },
        RuntimeRegistrationMetadata {
            harness: Some("codex".into()),
            harness_version: Some("1.0.0".into()),
            role: RuntimeRole::OrchestratorAndExecutor,
        },
    )
    .unwrap()
}

fn client(server: &MockServer) -> RuntimeRegistrationClient {
    let registration_url = Url::parse(DEFAULT_REGISTRATION_URL).unwrap();
    let endpoint = Url::parse(&server.uri())
        .unwrap()
        .join(registration_url.path())
        .unwrap();
    RuntimeRegistrationClient::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        endpoint,
    )
    .unwrap()
}

fn registered() -> Value {
    json!({
        "runtime_agent_instance_id": "agent-instance-test",
        "agent_host_id": "host-test",
        "actor": {
            "kind": "platform",
            "user_id": "user-test",
            "organization_id": "org-test",
            "project_id": "proj-test"
        }
    })
}

#[tokio::test]
async fn registration_retries_keep_the_key_and_sign_fresh_assertions() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(registered()))
        .expect(2)
        .mount(&server)
        .await;
    let pending = pending();
    let client = client(&server);
    let info = client.register(&pending).await.unwrap();
    let retry_info = client.register(&pending).await.unwrap();
    assert_eq!(info, retry_info);
    let runtime = pending.finish(info);
    let retry = pending.finish(retry_info);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].body, requests[1].body);
    for request in &requests {
        assert!(!request.headers.contains_key("authorization"));
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
        assert_eq!(
            request.headers["idempotency-key"],
            pending.initialization_id.to_string()
        );
    }
    let request: Value = requests[0].body_json().unwrap();
    assert_eq!(request, serde_json::to_value(&pending.request).unwrap());
    let key = STANDARD
        .decode(
            request["public_key"]
                .as_str()
                .unwrap()
                .split_once(' ')
                .unwrap()
                .1,
        )
        .unwrap();
    let key = VerifyingKey::from_bytes(key[19..].try_into().unwrap()).unwrap();
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let first = verify_assertion(&runtime, &key);
    let second = verify_assertion(&retry, &key);
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert_ne!(first["id"], second["id"]);
    for payload in [first, second] {
        assert!(Uuid::parse_str(payload["id"].as_str().unwrap()).is_ok());
        assert!((before..=after).contains(&payload["iat"].as_u64().unwrap()));
        assert_eq!(
            payload,
            json!({
                "version": 1, "runtime_agent_instance": "agent-instance-test",
                "iat": payload["iat"], "id": payload["id"]
            })
        );
    }
}

fn verify_assertion(runtime: &RuntimeAgentInstance, key: &VerifyingKey) -> Value {
    let assertion = runtime.assertion().unwrap();
    let parts: Vec<_> = assertion.split('.').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "1");
    assert!(!assertion.contains('='));
    let protected: Value =
        serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(
        protected,
        json!({"alg": "EdDSA", "typ": "openai-agent-assertion+jws"})
    );
    let signature = Signature::from_slice(&URL_SAFE_NO_PAD.decode(parts[3]).unwrap()).unwrap();
    key.verify_strict(format!("{}.{}", parts[1], parts[2]).as_bytes(), &signature)
        .unwrap();
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[2]).unwrap()).unwrap()
}

#[test]
fn new_initializations_use_new_keys_and_retry_ids() {
    let first = pending();
    let second = pending();
    assert_ne!(first.initialization_id, second.initialization_id);
    assert_ne!(first.request.public_key, second.request.public_key);
}

#[tokio::test]
async fn rejected_registration_preserves_only_status_and_known_error_codes() {
    for (status, code, expected) in [
        (401, "token_expired", RegistrationErrorCode::TokenExpired),
        (
            403,
            "runtime_instance_revoked",
            RegistrationErrorCode::RuntimeInstanceRevoked,
        ),
        (500, "secret-from-server", RegistrationErrorCode::Unknown),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({
                "error": {"code": code, "message": "secret-from-server"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let error = client(&server).register(&pending()).await.unwrap_err();
        assert!(!format!("{error} {error:?}").contains("secret-from-server"));
        assert!(
            matches!(error, RegistrationError::Rejected { status: actual, code: Some(actual_code) }
            if actual.as_u16() == status && actual_code == expected)
        );
    }
}

#[tokio::test]
async fn registration_preserves_policy_revocation() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/agent/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(registered()))
        .expect(1)
        .mount(&server)
        .await;
    let controller = NetworkPolicyController::default();
    let policy = controller.policy();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    let client = RuntimeRegistrationClient::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(policy.clone().for_current_account()),
        format!("{}/agent/register", server.uri()).parse().unwrap(),
    )
    .unwrap();
    let pending = pending();
    client.register(&pending).await.unwrap();

    policy.invalidate();
    assert!(controller.publish(policy.revision(), DestinationPolicy::Unrestricted));
    assert!(matches!(
        client.register(&pending).await,
        Err(RegistrationError::Policy(NetworkPolicyDenied::Revoked))
    ));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn registration_rejects_incomplete_or_oversized_responses() {
    for body in [
        json!({"runtime_agent_instance_id": "old-response"}).to_string(),
        "x".repeat(MAX_BODY_BYTES + 1),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .expect(1)
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).register(&pending()).await,
            Err(RegistrationError::InvalidResponse)
        ));
    }
}

#[tokio::test]
async fn registration_never_forwards_credentials_through_a_redirect() {
    let server = MockServer::start().await;
    let destination = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(307).insert_header("Location", destination.uri()))
        .expect(1)
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server).register(&pending()).await,
        Err(RegistrationError::Rejected {
            status: StatusCode::TEMPORARY_REDIRECT,
            ..
        })
    ));
    assert!(destination.received_requests().await.unwrap().is_empty());
}

#[test]
fn registration_rejects_unsafe_endpoints() {
    for endpoint in [
        "http://auth.example.com/register",
        "https://user:secret@auth.example.com/register",
        "https://auth.example.com/register?token=secret",
    ] {
        assert!(matches!(
            RuntimeRegistrationClient::new(
                HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                endpoint.parse().unwrap(),
            ),
            Err(RegistrationError::InvalidEndpoint)
        ));
    }
}
