use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use http::HeaderMap;
use http::HeaderValue;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
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
async fn reconnect_reuses_registration_until_url_is_rejected() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let rendezvous_url = format!("ws://{}", listener.local_addr()?);
    let registry = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cloud/environment/environment-requested/register"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "environment_id": "environment-requested",
            "url": rendezvous_url,
            "security_profile": NOISE_RELAY_SECURITY_PROFILE,
            "executor_registration_id": "registration-1",
        })))
        .expect(2)
        .mount(&registry)
        .await;
    let config = RemoteEnvironmentConfig::new(
        registry.uri(),
        "environment-requested".to_string(),
        static_registry_auth_provider(),
    )?;
    let environment_task = tokio::spawn(run_remote_environment(
        config,
        ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?,
    ));

    let (first_socket, _peer_addr) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let mut first_websocket = accept_async(first_socket).await?;
    first_websocket.close(None).await?;

    // An ordinary disconnect retries the same URL without registering again.
    let (mut rejected_socket, _peer_addr) =
        timeout(Duration::from_secs(5), listener.accept()).await??;
    let mut request = [0u8; 4096];
    let _ = rejected_socket.read(&mut request).await?;
    rejected_socket
        .write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n")
        .await?;
    rejected_socket.shutdown().await?;

    // The 4xx response discards the old registration before this attempt.
    let (third_socket, _peer_addr) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let _third_websocket = accept_async(third_socket).await?;
    registry.verify().await;

    environment_task.abort();
    let _ = environment_task.await;
    Ok(())
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
