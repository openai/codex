use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use codex_api::AuthProvider;
use codex_http_client::DestinationPolicy;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyController;
use codex_http_client::OutboundProxyPolicy;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::Request as HandshakeRequest;
use tokio_tungstenite::tungstenite::handshake::server::Response as HandshakeResponse;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::*;
use crate::ExecServerRuntimePaths;
use crate::RemoteEnvironmentTransport;

#[derive(Debug)]
struct StaticAuthProvider;

impl AuthProvider for StaticAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        headers.insert(
            http::header::AUTHORIZATION,
            HeaderValue::from_static("AWS4-HMAC-SHA256 test-signature"),
        );
    }

    fn apply_auth(
        &self,
        mut request: codex_http_client::Request,
    ) -> codex_api::AuthProviderFuture<'_> {
        Box::pin(async move {
            if request.method == http::Method::GET {
                assert_eq!(request.headers.len(), 1);
                assert!(request.headers.contains_key(http::header::HOST));
                assert!(request.url.starts_with("http"));
            }
            self.add_auth_headers(&mut request.headers);
            Ok(request)
        })
    }
}

#[derive(Debug)]
struct QueryAuthProvider;

impl AuthProvider for QueryAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    fn apply_auth(
        &self,
        mut request: codex_http_client::Request,
    ) -> codex_api::AuthProviderFuture<'_> {
        Box::pin(async move {
            let mut url = url::Url::parse(&request.url)
                .map_err(|error| codex_api::AuthError::Build(error.to_string()))?;
            url.query_pairs_mut().append_pair("auth", "signed-query");
            request.url = url.into();
            Ok(request)
        })
    }
}

#[tokio::test]
async fn direct_websocket_signs_only_host_and_preserves_handshake_headers() -> Result<()> {
    let mut request = "wss://executor.example.com/connect".into_client_request()?;
    request
        .headers_mut()
        .insert("traceparent", HeaderValue::from_static("trace-context"));

    authenticate_websocket_request(&mut request, &StaticAuthProvider).await?;

    assert!(request.headers().contains_key("sec-websocket-key"));
    assert_eq!(request.headers()["traceparent"], "trace-context");
    assert_eq!(
        request.headers()[http::header::AUTHORIZATION],
        "AWS4-HMAC-SHA256 test-signature"
    );
    Ok(())
}

#[tokio::test]
async fn direct_websocket_uses_authenticated_url_query_parameters() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("ws://{}/connect?existing=value", listener.local_addr()?);
    let acceptor = tokio::spawn(async move {
        let (socket, _) = listener.accept().await?;
        let callback = |request: &HandshakeRequest, response: HandshakeResponse| {
            assert_eq!(request.uri().path(), "/connect");
            assert_eq!(
                request.uri().query(),
                Some("existing=value&auth=signed-query")
            );
            Ok(response)
        };
        accept_hdr_async(socket, callback).await.map(|_| ())
    });

    let connection = connect_direct(
        &url,
        &QueryAuthProvider,
        &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    )
    .await?;
    drop(connection);
    acceptor.await??;
    Ok(())
}

#[test]
fn direct_endpoints_require_tls_except_on_loopback() {
    for (url, scheme, allowed) in [
        ("https://registry.example.com", "https", true),
        ("wss://executor.example.com/connect", "wss", true),
        ("http://127.0.0.1:8080", "https", true),
        ("ws://localhost:8080/connect", "wss", true),
        ("http://registry.example.com", "https", false),
        ("ws://executor.example.com/connect", "wss", false),
    ] {
        assert_eq!(
            require_tls_or_loopback(url, scheme).is_ok(),
            allowed,
            "{url}"
        );
    }
}

#[test]
fn direct_authentication_retries_only_transient_credential_failures() {
    for (error, retryable) in [
        (AuthError::Transient("temporary".to_string()), true),
        (AuthError::Build("invalid".to_string()), false),
    ] {
        assert_eq!(
            is_retryable_recovery_error(&direct_auth_error(error)),
            retryable
        );
    }
}

#[tokio::test]
async fn direct_registration_validates_connection_data() -> Result<()> {
    for (field, invalid_value) in [
        ("environment_id", "different-environment"),
        ("transport", "noise_hybrid_ik_v1"),
        ("registration_id", ""),
        ("url", ""),
        ("url", "ws://executor.example.com/connect"),
    ] {
        let registry = MockServer::start().await;
        let mut response = serde_json::json!({
            "environment_id": "environment-requested",
            "transport": DIRECT_TRANSPORT,
            "registration_id": "registration-1",
            "url": "wss://executor.example.com/connect",
        });
        response[field] = serde_json::json!(invalid_value);
        Mock::given(method("POST"))
            .and(path(
                "/cloud/environment/environment-requested/direct/register",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .expect(1)
            .mount(&registry)
            .await;
        let client = EnvironmentRegistryClient::new(registry.uri(), Arc::new(StaticAuthProvider))?;
        let error = client
            .register_direct_environment("environment-requested")
            .await
            .expect_err("invalid registration must be rejected");
        if invalid_value.starts_with("ws://") {
            assert!(matches!(
                error,
                ExecServerError::EnvironmentRegistryConfig(_)
            ));
        } else {
            assert!(matches!(error, ExecServerError::Protocol(_)));
        }
        registry.verify().await;
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn direct_registration_uses_proxy_policy_without_logging_secrets() -> Result<()> {
    use tracing_subscriber::Layer;
    use tracing_subscriber::layer::SubscriberExt;

    let mut log_file = tempfile::tempfile()?;
    let writer = log_file.try_clone()?;
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(move || writer.try_clone().expect("clone log file"))
            .with_filter(
                tracing_subscriber::filter::Targets::new()
                    .with_target("codex_http_client", tracing::Level::TRACE)
                    .with_target("codex_exec_server", tracing::Level::TRACE),
            ),
    );
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::debug!(target: "codex_exec_server", "direct registry log capture sentinel");

    let proxy = MockServer::start().await;
    let registry_url = "http://direct-registry-proxy.invalid/registry-path-secret";
    let request_url =
        format!("{registry_url}/cloud/environment/environment-requested/direct/register");
    codex_http_client::cache_system_proxy_route_for_test(&request_url, proxy.uri());
    Mock::given(method("POST"))
        .and(path(
            "/registry-path-secret/cloud/environment/environment-requested/direct/register",
        ))
        .and(header("authorization", "AWS4-HMAC-SHA256 test-signature"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("set-cookie", "session=registry-cookie-secret")
                .insert_header(
                    "location",
                    "https://registry.example/?token=registry-location-secret",
                )
                .set_body_json(serde_json::json!({
                    "environment_id": "environment-requested",
                    "transport": DIRECT_TRANSPORT,
                    "registration_id": "registration-1",
                    "url": "wss://executor.example/connect?token=websocket-query-secret",
                })),
        )
        .expect(1)
        .mount(&proxy)
        .await;
    let client = EnvironmentRegistryClient::new_with_telemetry(
        registry_url.to_string(),
        Arc::new(StaticAuthProvider),
        crate::ExecServerTelemetry::default(),
        HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
    )?;
    let response = client
        .register_direct_environment("environment-requested")
        .await?;
    assert_eq!(
        response.url,
        "wss://executor.example/connect?token=websocket-query-secret"
    );
    let requests = proxy
        .received_requests()
        .await
        .expect("record proxy requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.as_str(), request_url);
    assert_eq!(requests[0].body, br#"{"transport":"direct_jsonrpc_v1"}"#);
    proxy.verify().await;

    std::io::Seek::rewind(&mut log_file)?;
    let mut logs = String::new();
    std::io::Read::read_to_string(&mut log_file, &mut logs)?;
    assert!(logs.contains("direct registry log capture sentinel"));
    for secret in [
        "test-signature",
        "registry-path-secret",
        "registry-cookie-secret",
        "registry-location-secret",
        "websocket-query-secret",
    ] {
        assert!(!logs.contains(secret), "registry logs exposed {secret}");
    }
    Ok(())
}

#[tokio::test]
async fn direct_registration_handles_policy_outages_and_permanent_failures() -> Result<()> {
    for (successful_registrations, policy_unavailable) in [(0, false), (1, false), (0, true)] {
        let controller = NetworkPolicyController::default();
        let policy = controller.policy();
        controller.publish(policy.revision(), DestinationPolicy::Unrestricted);
        if policy_unavailable && successful_registrations == 0 {
            controller.unavailable(policy.revision());
        }
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let websocket_url = format!("ws://{}/connect", listener.local_addr()?);
        let registry = MockServer::start().await;
        let registration_count = AtomicUsize::new(0);
        Mock::given(method("POST"))
            .and(path(
                "/cloud/environment/environment-requested/direct/register",
            ))
            .respond_with(move |_: &wiremock::Request| {
                let attempt = registration_count.fetch_add(1, Ordering::Relaxed);
                if policy_unavailable || attempt < successful_registrations {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "environment_id": "environment-requested",
                        "transport": DIRECT_TRANSPORT,
                        "registration_id": "registration-1",
                        "url": websocket_url,
                    }))
                } else {
                    ResponseTemplate::new(503)
                }
            })
            .expect((successful_registrations + 1) as u64)
            .mount(&registry)
            .await;
        let config = RemoteEnvironmentConfig::new_with_transport(
            registry.uri(),
            "environment-requested".to_string(),
            RemoteEnvironmentTransport::Direct,
            Arc::new(StaticAuthProvider),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
                .with_network_policy(policy.clone().for_current_account()),
        )?;
        let runtime_paths = ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?;
        let task = tokio::spawn(crate::run_remote_environment(config, runtime_paths));
        if successful_registrations == 1 {
            let (mut socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
            let mut request = [0; 4096];
            let _ = socket.read(&mut request).await?;
            socket
                .write_all(b"HTTP/1.1 409 Conflict\r\nContent-Length: 0\r\n\r\n")
                .await?;
            socket.shutdown().await?;
        }
        if policy_unavailable {
            timeout(Duration::from_secs(1), listener.accept())
                .await
                .expect_err("unavailable policy opened a connection");
            controller.publish(policy.revision(), DestinationPolicy::Unrestricted);
            let (socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
            let _websocket = tokio_tungstenite::accept_async(socket).await?;
            registry.verify().await;
            task.abort();
            let _ = task.await;
            continue;
        }
        let error = timeout(Duration::from_secs(5), task)
            .await??
            .expect_err("registration failure should stop the runner");
        assert!(matches!(
            error,
            ExecServerError::EnvironmentRegistryHttp {
                status: StatusCode::SERVICE_UNAVAILABLE,
                ..
            }
        ));
        registry.verify().await;
    }
    Ok(())
}

#[tokio::test]
async fn direct_websocket_reuses_registration_and_stops_on_permanent_errors() -> Result<()> {
    for (status, should_retry) in [
        (Some(StatusCode::BAD_REQUEST), false),
        (Some(StatusCode::UNAUTHORIZED), false),
        (Some(StatusCode::FORBIDDEN), false),
        (Some(StatusCode::NOT_FOUND), false),
        (Some(StatusCode::METHOD_NOT_ALLOWED), false),
        (Some(StatusCode::GONE), false),
        (Some(StatusCode::REQUEST_TIMEOUT), true),
        (Some(StatusCode::CONFLICT), true),
        (Some(StatusCode::TOO_MANY_REQUESTS), true),
        (Some(StatusCode::INTERNAL_SERVER_ERROR), true),
        (Some(StatusCode::SERVICE_UNAVAILABLE), true),
        (None, true), // Close the TCP socket without sending an HTTP response.
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let websocket_url = format!("ws://{}", listener.local_addr()?);
        let registry = MockServer::start().await;
        let expected_calls = if status == Some(StatusCode::CONFLICT) {
            2
        } else {
            1
        };
        let registration_count = AtomicUsize::new(0);
        Mock::given(method("POST"))
            .and(path(
                "/cloud/environment/environment-requested/direct/register",
            ))
            .and(header("authorization", "AWS4-HMAC-SHA256 test-signature"))
            .respond_with(move |_: &wiremock::Request| {
                let registration = registration_count.fetch_add(1, Ordering::Relaxed) + 1;
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "environment_id": "environment-requested",
                    "transport": DIRECT_TRANSPORT,
                    "registration_id": format!("registration-{registration}"),
                    "url": format!("{websocket_url}/registration-{registration}"),
                }))
            })
            .expect(expected_calls)
            .mount(&registry)
            .await;
        let config = RemoteEnvironmentConfig::new_with_transport(
            registry.uri(),
            "environment-requested".to_string(),
            RemoteEnvironmentTransport::Direct,
            Arc::new(StaticAuthProvider),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )?;
        let runtime_paths = ExecServerRuntimePaths::new(
            std::env::current_exe()?,
            /*codex_linux_sandbox_exe*/ None,
        )?;
        let task = tokio::spawn(crate::run_remote_environment(config, runtime_paths));

        let (mut socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
        let mut request = [0; 4096];
        let _ = socket.read(&mut request).await?;
        if let Some(status) = status {
            let response = format!(
                "HTTP/1.1 {} {}\r\nContent-Length: 0\r\n\r\n",
                status.as_u16(),
                status.canonical_reason().unwrap_or_default()
            );
            socket.write_all(response.as_bytes()).await?;
        }
        socket.shutdown().await?;
        drop(socket);

        if should_retry {
            let expected_path = format!("/registration-{expected_calls}");
            let check_registration = |request: &HandshakeRequest, response: HandshakeResponse| {
                assert_eq!(request.uri().path(), expected_path);
                Ok(response)
            };
            let (socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
            let websocket = accept_hdr_async(socket, &check_registration).await?;
            let retry_delay =
                registry_recovery_retry_delay("environment-requested", /*attempt*/ 0);
            let retry_started = Instant::now();
            drop(websocket);

            let (socket, _) = timeout(Duration::from_secs(5), listener.accept()).await??;
            assert!(retry_started.elapsed() >= retry_delay);
            let _websocket = accept_hdr_async(socket, &check_registration).await?;
            registry.verify().await;
            task.abort();
            let _ = task.await;
        } else {
            let error = timeout(Duration::from_secs(5), task)
                .await??
                .expect_err("permanent WebSocket client error should be terminal");
            assert!(matches!(
                error,
                ExecServerError::WebSocketConnect {
                    source: tokio_tungstenite::tungstenite::Error::Http(response),
                    ..
                } if Some(response.status()) == status
            ));
        }
    }
    Ok(())
}
