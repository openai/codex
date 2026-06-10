use std::sync::Arc;

use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use http::HeaderMap;
use http::HeaderValue;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

const HARNESS_KEY_AUTHORIZATION: &str = "authorization-that-must-not-leak";

#[derive(Debug)]
struct StaticRegistryAuthProvider;

impl AuthProvider for StaticRegistryAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let _ = headers.insert(
            http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer registry-token"),
        );
    }
}

fn static_registry_auth_provider() -> SharedAuthProvider {
    Arc::new(StaticRegistryAuthProvider)
}

#[tokio::test]
async fn register_noise_environment_posts_security_profile_and_public_key() {
    let server = MockServer::start().await;
    let executor_public_key = NoiseChannelIdentity::generate()
        .expect("identity")
        .public_key();
    Mock::given(method("POST"))
        .and(path("/cloud/environment/environment-requested/register"))
        .and(header("authorization", "Bearer registry-token"))
        .and(body_partial_json(serde_json::json!({
            "security_profile": NOISE_RELAY_SECURITY_PROFILE,
            "executor_public_key": executor_public_key.clone(),
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "environment_id": "environment-requested",
            "url": "wss://rendezvous.test/noise",
            "security_profile": NOISE_RELAY_SECURITY_PROFILE,
            "executor_registration_id": "registration-1",
        })))
        .mount(&server)
        .await;
    let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
        .expect("client");

    let response = client
        .register_noise_environment("environment-requested", &executor_public_key)
        .await
        .expect("register Noise environment");

    assert_eq!(response.environment_id, "environment-requested");
    assert_eq!(response.url, "wss://rendezvous.test/noise");
    assert_eq!(response.security_profile, NOISE_RELAY_SECURITY_PROFILE);
    assert_eq!(response.executor_registration_id, "registration-1");
}

#[tokio::test]
async fn validate_harness_key_requires_explicit_valid_response() {
    let server = MockServer::start().await;
    let harness_public_key = NoiseChannelIdentity::generate()
        .expect("identity")
        .public_key();
    Mock::given(method("POST"))
        .and(path("/cloud/environment/environment-requested/validate"))
        .and(header("authorization", "Bearer registry-token"))
        .and(body_partial_json(serde_json::json!({
            "executor_registration_id": "registration-1",
            "harness_public_key": harness_public_key.clone(),
            "harness_key_authorization": HARNESS_KEY_AUTHORIZATION,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": false,
        })))
        .mount(&server)
        .await;
    let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
        .expect("client");

    let error = RegistryHarnessKeyValidator {
        client,
        environment_id: "environment-requested".to_string(),
        executor_registration_id: "registration-1".to_string(),
    }
    .validate_harness_key(&harness_public_key, HARNESS_KEY_AUTHORIZATION)
    .await
    .expect_err("a false validation response must fail closed");

    assert!(matches!(
        error,
        ExecServerError::Protocol(message)
            if message == "environment registry rejected Noise relay harness key"
    ));
}

#[tokio::test]
async fn validate_harness_key_does_not_expose_error_body() {
    let server = MockServer::start().await;
    let harness_public_key = NoiseChannelIdentity::generate()
        .expect("identity")
        .public_key();
    Mock::given(method("POST"))
        .and(path("/cloud/environment/environment-requested/validate"))
        .respond_with(ResponseTemplate::new(500).set_body_string(HARNESS_KEY_AUTHORIZATION))
        .mount(&server)
        .await;
    let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
        .expect("client");

    let error = RegistryHarnessKeyValidator {
        client,
        environment_id: "environment-requested".to_string(),
        executor_registration_id: "registration-1".to_string(),
    }
    .validate_harness_key(&harness_public_key, HARNESS_KEY_AUTHORIZATION)
    .await
    .expect_err("validation HTTP error should fail closed");

    let display = error.to_string();
    assert!(!display.contains(HARNESS_KEY_AUTHORIZATION));
    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryHttp { message, .. }
            if message == "environment registry harness key validation failed"
    ));
}

#[test]
fn noise_environment_id_validation_rejects_path_injection() {
    validate_environment_id("ccarenv_b64_Y2Fhcy1zdGFnaW5nLWV4ZWN1dG9yLWVudmlyb25tZW50LTE")
        .expect("valid cloud environment id");

    let error = validate_environment_id("ccarenv_b64_valid/../../status")
        .expect_err("path delimiter must not reach an authenticated registry request");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryConfig(message) if message.contains("ASCII letters")
    ));
}

#[test]
fn executor_registration_id_validation_rejects_ambiguous_values() {
    for invalid in ["", " registration-1", "registration-1 "] {
        assert!(validate_executor_registration_id(invalid).is_err());
    }
    assert!(
        validate_executor_registration_id(&"x".repeat(MAX_EXECUTOR_REGISTRATION_ID_LEN)).is_ok()
    );
    assert!(
        validate_executor_registration_id(&"x".repeat(MAX_EXECUTOR_REGISTRATION_ID_LEN + 1))
            .is_err()
    );
}
