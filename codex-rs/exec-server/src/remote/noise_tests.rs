use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::cache_system_proxy_route_for_test;
use http::HeaderMap;
use http::HeaderValue;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

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

#[tokio::test(flavor = "current_thread")]
async fn registry_requests_do_not_log_sensitive_urls_or_response_headers() -> Result<()> {
    let log_buffer = Arc::new(Mutex::new(Vec::new()));
    let writer_buffer = Arc::clone(&log_buffer);
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(move || RegistryLogWriter(Arc::clone(&writer_buffer)))
            .with_filter(
                tracing_subscriber::filter::Targets::new()
                    .with_target("codex_http_client", tracing::Level::TRACE)
                    .with_target("codex_exec_server", tracing::Level::TRACE),
            ),
    );
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::debug!(target: "codex_exec_server", "registry log capture sentinel");

    let server = MockServer::start().await;
    let harness_public_key = NoiseChannelIdentity::generate()?.public_key();
    let executor_public_key = NoiseChannelIdentity::generate()?.public_key();
    for (operation, response, cookie_secret, location_secret) in [
        (
            "register",
            serde_json::json!({
                "environment_id": "environment-requested",
                "url": "wss://rendezvous.test/environment",
                "security_profile": NOISE_RELAY_SECURITY_PROFILE,
                "executor_registration_id": "registration-1",
            }),
            "register-cookie-secret",
            "register-location-secret",
        ),
        (
            "connect",
            serde_json::json!({
                "environment_id": "environment-requested",
                "url": "wss://rendezvous.test/harness",
                "security_profile": NOISE_RELAY_SECURITY_PROFILE,
                "executor_registration_id": "registration-1",
                "executor_public_key": executor_public_key.clone(),
                "harness_key_authorization": HARNESS_KEY_AUTHORIZATION,
            }),
            "connect-cookie-secret",
            "connect-location-secret",
        ),
        (
            "validate",
            serde_json::json!({ "valid": true }),
            "validate-cookie-secret",
            "validate-location-secret",
        ),
    ] {
        Mock::given(method("POST"))
            .and(path("/registry-path-secret"))
            .and(query_param(
                "registry_token",
                format!(
                    "registry-query-secret/cloud/environment/environment-requested/{operation}"
                ),
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("set-cookie", format!("session={cookie_secret}"))
                    .insert_header(
                        "location",
                        format!("https://registry.example/private?token={location_secret}"),
                    )
                    .set_body_json(response),
            )
            .expect(1)
            .mount(&server)
            .await;
    }

    let registry_url =
        server
            .uri()
            .replacen("http://", "http://registry-user:registry-password@", 1);
    let registry_url =
        format!("{registry_url}/registry-path-secret?registry_token=registry-query-secret");
    let client = EnvironmentRegistryClient::new(registry_url, static_registry_auth_provider())?;
    client
        .register_environment("environment-requested", &executor_public_key)
        .await?;
    client
        .connect_environment("environment-requested", harness_public_key.clone())
        .await?;
    RegistryHarnessKeyValidator {
        client,
        environment_id: "environment-requested".to_string(),
        executor_registration_id: "registration-1".to_string(),
    }
    .validate_harness_key(&harness_public_key, HARNESS_KEY_AUTHORIZATION)
    .await?;

    let logs = String::from_utf8(log_buffer.lock().expect("log buffer lock").clone())?;
    assert!(logs.contains("registry log capture sentinel"));
    for secret in [
        "registry-user",
        "registry-password",
        "registry-path-secret",
        "registry-query-secret",
        "registry-token",
        HARNESS_KEY_AUTHORIZATION,
        "register-cookie-secret",
        "register-location-secret",
        "connect-cookie-secret",
        "connect-location-secret",
        "validate-cookie-secret",
        "validate-location-secret",
    ] {
        assert!(!logs.contains(secret), "logs exposed {secret}:\n{logs}");
    }

    Ok(())
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
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
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
async fn noise_connect_provider_uses_supplied_system_proxy_policy() -> Result<()> {
    let proxy = MockServer::start().await;
    let registry_url = "http://registry-policy-proxy.test";
    let request_url = format!("{registry_url}/cloud/environment/environment-requested/connect");
    cache_system_proxy_route_for_test(&request_url, proxy.uri());

    let harness_public_key = NoiseChannelIdentity::generate()?.public_key();
    let executor_public_key = NoiseChannelIdentity::generate()?.public_key();
    Mock::given(method("POST"))
        .and(path("/cloud/environment/environment-requested/connect"))
        .and(header("authorization", "Bearer registry-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "environment_id": "environment-requested",
            "url": "wss://rendezvous.test/cloud-agent/default/ws/environment/environment-requested",
            "security_profile": NOISE_RELAY_SECURITY_PROFILE,
            "executor_registration_id": "registration-1",
            "executor_public_key": executor_public_key.clone(),
            "harness_key_authorization": HARNESS_KEY_AUTHORIZATION,
        })))
        .expect(1)
        .mount(&proxy)
        .await;

    let provider = NoiseRendezvousEnvironmentConfig::new(
        registry_url.to_string(),
        "environment-requested".to_string(),
        "registry-token".to_string(),
        /*chatgpt_account_id*/ None,
    )?
    .into_connect_provider(HttpClientFactory::new(
        OutboundProxyPolicy::RespectSystemProxy,
    ))?;
    let bundle = timeout(
        Duration::from_secs(5),
        provider.connect_bundle(harness_public_key),
    )
    .await??;
    let requests = proxy
        .received_requests()
        .await
        .expect("proxy request recording should be enabled");

    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.as_str(), request_url);
    assert_eq!(bundle.executor_public_key, executor_public_key);

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

struct RegistryLogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for RegistryLogWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .expect("log buffer lock")
            .extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
