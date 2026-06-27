use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
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
async fn reconnect_reregisters_after_disconnect_but_reuses_preconnect_failures() -> Result<()> {
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
            "transport_policy": {
                "version": ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
                "assignment_epoch": "experiment-1",
                "outbound_tcp_nodelay": false,
                "rendezvous_accepted_tcp_nodelay": false,
            },
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

    // Closing an established socket forces a fresh registration before the
    // next connection attempt. A pre-establishment server failure then reuses
    // that new registration.
    let (mut rejected_socket, _peer_addr) =
        timeout(Duration::from_secs(5), listener.accept()).await??;
    let mut request = [0u8; 4096];
    let _ = rejected_socket.read(&mut request).await?;
    rejected_socket
        .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n")
        .await?;
    rejected_socket.shutdown().await?;

    // The 5xx response retries without a third registration.
    let (third_socket, _peer_addr) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let _third_websocket = accept_async(third_socket).await?;
    registry.verify().await;

    environment_task.abort();
    let _ = environment_task.await;
    Ok(())
}

#[tokio::test]
async fn active_disconnect_retries_registration_refresh_when_registry_is_unavailable() -> Result<()>
{
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let rendezvous_url = format!("ws://{}", listener.local_addr()?);
    let registry = MockServer::start().await;
    let registration_count = Arc::new(AtomicUsize::new(0));
    let response_count = Arc::clone(&registration_count);
    Mock::given(method("POST"))
        .and(path("/cloud/environment/environment-requested/register"))
        .respond_with(move |_request: &wiremock::Request| {
            let call = response_count.fetch_add(1, Ordering::SeqCst);
            if call == 1 {
                return ResponseTemplate::new(503);
            }
            let assignment_epoch = if call == 0 { "experiment-1" } else { "off" };
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "environment_id": "environment-requested",
                "url": rendezvous_url,
                "security_profile": NOISE_RELAY_SECURITY_PROFILE,
                "executor_registration_id": "registration-1",
                "transport_policy": {
                    "version": ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
                    "assignment_epoch": assignment_epoch,
                    "outbound_tcp_nodelay": false,
                    "rendezvous_accepted_tcp_nodelay": false,
                },
            }))
        })
        .expect(3)
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

    let (first_socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let mut first_websocket = accept_async(first_socket).await?;
    first_websocket.close(None).await?;

    let (second_socket, _) = timeout(Duration::from_secs(8), listener.accept()).await??;
    let _second_websocket = accept_async(second_socket).await?;
    assert_eq!(registration_count.load(Ordering::SeqCst), 3);
    registry.verify().await;

    environment_task.abort();
    let _ = environment_task.await;
    Ok(())
}

#[tokio::test]
async fn inactive_disconnect_refreshes_registration_for_enrollment() -> Result<()> {
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
            "transport_policy": {
                "version": ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
                "assignment_epoch": "off",
                "outbound_tcp_nodelay": true,
                "rendezvous_accepted_tcp_nodelay": true,
            },
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

    let (first_socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let mut first_websocket = accept_async(first_socket).await?;
    first_websocket.close(None).await?;
    let (second_socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let _second_websocket = accept_async(second_socket).await?;
    registry.verify().await;

    environment_task.abort();
    let _ = environment_task.await;
    Ok(())
}

#[tokio::test]
async fn legacy_disconnect_reuses_registration() -> Result<()> {
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
        .expect(1)
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

    let (first_socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let mut first_websocket = accept_async(first_socket).await?;
    first_websocket.close(None).await?;
    let (second_socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
    let _second_websocket = accept_async(second_socket).await?;
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
