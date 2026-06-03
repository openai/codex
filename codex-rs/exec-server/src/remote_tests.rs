use std::sync::Arc;

use codex_api::AuthProvider;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use reqwest::StatusCode;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_bytes;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;

#[derive(Debug)]
struct StaticRegistryAuthProvider;

impl AuthProvider for StaticRegistryAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let _ = headers.insert(
            http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer registry-token"),
        );
        let _ = headers.insert(
            "ChatGPT-Account-ID",
            HeaderValue::from_static("workspace-123"),
        );
    }
}

fn static_registry_auth_provider() -> SharedAuthProvider {
    Arc::new(StaticRegistryAuthProvider)
}

#[tokio::test]
async fn register_environment_posts_with_auth_provider_headers() {
    let server = MockServer::start().await;
    let config = RemoteEnvironmentConfig::new(
        server.uri(),
        "environment-requested".to_string(),
        static_registry_auth_provider(),
    )
    .expect("config");
    Mock::given(method("POST"))
        .and(path("/cloud/environment/environment-requested/register"))
        .and(header("authorization", "Bearer registry-token"))
        .and(header("chatgpt-account-id", "workspace-123"))
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "environment_id": "env-1",
            "url": "wss://rendezvous.test/cloud-agent/default/ws/environment/env-1?role=environment&sig=abc",
        })))
        .mount(&server)
        .await;
    let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
        .expect("client");
    let response = client
        .register_legacy_environment(&config.environment_id)
        .await
        .expect("register environment");

    assert_eq!(
        response,
            EnvironmentRegistryRegistrationResponse {
                environment_id: "env-1".to_string(),
                url: "wss://rendezvous.test/cloud-agent/default/ws/environment/env-1?role=environment&sig=abc".to_string(),
            }
        );
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

    assert_eq!(
        response,
        EnvironmentRegistryNoiseRegistrationResponse {
            environment_id: "environment-requested".to_string(),
            url: "wss://rendezvous.test/noise".to_string(),
            security_profile: NOISE_RELAY_SECURITY_PROFILE.to_string(),
            executor_registration_id: "registration-1".to_string(),
        }
    );
}

#[tokio::test]
async fn validate_harness_key_requires_explicit_valid_response() {
    let server = MockServer::start().await;
    let harness_public_key = NoiseChannelIdentity::generate()
        .expect("identity")
        .public_key();
    Mock::given(method("POST"))
        .and(path(
            "/cloud/environment/environment-requested/validate",
        ))
        .and(header("authorization", "Bearer registry-token"))
        .and(body_partial_json(serde_json::json!({
            "executor_registration_id": "registration-1",
            "harness_public_key": harness_public_key.clone(),
            "harness_key_authorization": "authorization-1",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": false,
        })))
        .mount(&server)
        .await;
    let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
        .expect("client");

    let error = client
        .validate_harness_key(
            "environment-requested",
            "registration-1",
            &harness_public_key,
            "authorization-1",
        )
        .await
        .expect_err("a false validation response must fail closed");

    assert!(matches!(
        error,
        ExecServerError::Protocol(message)
            if message == "environment registry rejected Noise relay harness key"
    ));
}

#[tokio::test]
async fn register_legacy_environment_does_not_follow_redirects_with_auth_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cloud/environment/environment-requested/register"))
        .and(header("authorization", "Bearer registry-token"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/redirect-target", server.uri())),
        )
        .mount(&server)
        .await;
    Mock::given(path("/redirect-target"))
        .and(header("authorization", "Bearer registry-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
        .expect("client");
    let error = client
        .register_legacy_environment("environment-requested")
        .await
        .expect_err("redirect response should not be followed");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryHttp {
            status: StatusCode::FOUND,
            ..
        }
    ));
}

#[test]
fn debug_output_redacts_auth_provider() {
    let config = RemoteEnvironmentConfig::new(
        "https://registry.example".to_string(),
        "env-1".to_string(),
        static_registry_auth_provider(),
    )
    .expect("config");

    let debug = format!("{config:?}");

    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("workspace-123"));
    assert!(debug.contains("Legacy"));
}

#[test]
fn remote_environment_config_accepts_cloud_environment_id() {
    let environment_id = "ccarenv_b64_Y2Fhcy1zdGFnaW5nLWV4ZWN1dG9yLWVudmlyb25tZW50LTE".to_string();

    let config = RemoteEnvironmentConfig::new(
        "https://registry.example".to_string(),
        environment_id.clone(),
        static_registry_auth_provider(),
    )
    .expect("config");

    assert_eq!(config.environment_id, environment_id);
    assert_eq!(config.relay_protocol, RemoteRelayProtocol::Legacy);
}

#[test]
fn remote_environment_config_rejects_registry_path_injection() {
    let error = RemoteEnvironmentConfig::new(
        "https://registry.example".to_string(),
        "ccarenv_b64_valid/../../status".to_string(),
        static_registry_auth_provider(),
    )
    .expect_err("path delimiter must not reach an authenticated registry request");

    assert!(matches!(
        error,
        ExecServerError::EnvironmentRegistryConfig(message) if message.contains("ASCII letters")
    ));
}
