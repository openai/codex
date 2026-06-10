use std::time::Duration;
use std::time::Instant;

use codex_api::SharedAuthProvider;
use codex_client::backoff;
use reqwest::StatusCode;
use reqwest::header::RETRY_AFTER;
use serde::Deserialize;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::client::uri_mode;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::relay::run_multiplexed_environment;
use crate::server::ConnectionProcessor;

const ERROR_BODY_PREVIEW_BYTES: usize = 4096;
const ENVIRONMENT_REGISTRY_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const RENDEZVOUS_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const INITIAL_RECONNECT_BACKOFF: Duration = Duration::from_secs(1);
const MAX_RECONNECT_BACKOFF: Duration = Duration::from_secs(30);
const STABLE_CONNECTION_DURATION: Duration = Duration::from_secs(30);

#[derive(Clone)]
struct EnvironmentRegistryClient {
    base_url: String,
    auth_provider: SharedAuthProvider,
    http: reqwest::Client,
}

impl std::fmt::Debug for EnvironmentRegistryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvironmentRegistryClient")
            .field("base_url", &self.base_url)
            .field("auth_provider", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl EnvironmentRegistryClient {
    fn new(base_url: String, auth_provider: SharedAuthProvider) -> Result<Self, ExecServerError> {
        let base_url = normalize_base_url(base_url)?;
        Ok(Self {
            base_url,
            auth_provider,
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(ENVIRONMENT_REGISTRY_REQUEST_TIMEOUT)
                .build()?,
        })
    }

    async fn register_environment(
        &self,
        environment_id: &str,
    ) -> Result<EnvironmentRegistryRegistrationResponse, EnvironmentRegistrationError> {
        let response = match self
            .http
            .post(endpoint_url(
                &self.base_url,
                &format!("/cloud/environment/{environment_id}/register"),
            ))
            .headers(self.auth_provider.to_auth_headers())
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) if error.is_builder() => {
                return Err(EnvironmentRegistrationError::Permanent {
                    source: error.into(),
                });
            }
            Err(error) => {
                return Err(EnvironmentRegistrationError::Retryable {
                    source: error.into(),
                    retry_after: None,
                });
            }
        };
        self.parse_registration_response(response).await
    }

    async fn parse_registration_response(
        &self,
        response: reqwest::Response,
    ) -> Result<EnvironmentRegistryRegistrationResponse, EnvironmentRegistrationError> {
        if response.status().is_success() {
            let body = response.bytes().await.map_err(|error| {
                EnvironmentRegistrationError::Retryable {
                    source: error.into(),
                    retry_after: None,
                }
            })?;
            let response = serde_json::from_slice(&body).map_err(|error| {
                EnvironmentRegistrationError::Permanent {
                    source: error.into(),
                }
            })?;
            return validate_registration_response(response)
                .map_err(|source| EnvironmentRegistrationError::Permanent { source });
        }

        let status = response.status();
        let retry_after = response
            .headers()
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs);
        let body =
            response
                .bytes()
                .await
                .map_err(|error| EnvironmentRegistrationError::Retryable {
                    source: error.into(),
                    retry_after,
                })?;
        let body = String::from_utf8_lossy(&body);
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(EnvironmentRegistrationError::Permanent {
                source: environment_registry_auth_error(status, body.as_ref()),
            });
        }

        let source = environment_registry_http_error(status, body.as_ref());
        let ExecServerError::EnvironmentRegistryHttp { code, .. } = &source else {
            unreachable!("environment registry HTTP errors use the HTTP error variant");
        };
        if is_retryable_registration_status(status, code.as_deref()) {
            Err(EnvironmentRegistrationError::Retryable {
                source,
                retry_after,
            })
        } else {
            Err(EnvironmentRegistrationError::Permanent { source })
        }
    }
}

#[derive(Debug)]
enum EnvironmentRegistrationError {
    Retryable {
        source: ExecServerError,
        retry_after: Option<Duration>,
    },
    Permanent {
        source: ExecServerError,
    },
}

#[derive(Default)]
struct ReconnectBackoff {
    attempt: u64,
}

impl ReconnectBackoff {
    fn reset(&mut self) {
        self.attempt = 0;
    }

    fn next_delay(&mut self, retry_after: Option<Duration>) -> Duration {
        self.attempt = self.attempt.saturating_add(1);
        let jittered_backoff = backoff(INITIAL_RECONNECT_BACKOFF, self.attempt);
        retry_after
            .map_or(jittered_backoff, |retry_after| {
                retry_after.max(jittered_backoff)
            })
            .min(MAX_RECONNECT_BACKOFF)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
struct EnvironmentRegistryRegistrationResponse {
    environment_id: String,
    url: String,
}

fn validate_registration_response(
    response: EnvironmentRegistryRegistrationResponse,
) -> Result<EnvironmentRegistryRegistrationResponse, ExecServerError> {
    if response.environment_id.trim().is_empty() {
        return Err(ExecServerError::Protocol(
            "environment registry returned an empty environment id".to_string(),
        ));
    }
    let valid_websocket_url = response
        .url
        .as_str()
        .into_client_request()
        .and_then(|request| uri_mode(request.uri()).map(|_| ()))
        .is_ok();
    if !valid_websocket_url {
        return Err(ExecServerError::Protocol(
            "environment registry returned an invalid rendezvous websocket URL".to_string(),
        ));
    }
    Ok(response)
}

/// Configuration for registering an exec-server for remote use.
#[derive(Clone)]
pub struct RemoteEnvironmentConfig {
    pub base_url: String,
    pub environment_id: String,
    pub name: String,
    auth_provider: SharedAuthProvider,
}

impl std::fmt::Debug for RemoteEnvironmentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteEnvironmentConfig")
            .field("base_url", &self.base_url)
            .field("environment_id", &self.environment_id)
            .field("name", &self.name)
            .field("auth_provider", &"<redacted>")
            .finish()
    }
}

impl RemoteEnvironmentConfig {
    pub fn new(
        base_url: String,
        environment_id: String,
        auth_provider: SharedAuthProvider,
    ) -> Result<Self, ExecServerError> {
        let environment_id = normalize_environment_id(environment_id)?;
        Ok(Self {
            base_url,
            environment_id,
            name: "codex-exec-server".to_string(),
            auth_provider,
        })
    }
}

/// Register an exec-server for remote use and serve requests over the returned
/// rendezvous websocket.
pub async fn run_remote_environment(
    config: RemoteEnvironmentConfig,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), ExecServerError> {
    ensure_rustls_crypto_provider();
    let client =
        EnvironmentRegistryClient::new(config.base_url.clone(), config.auth_provider.clone())?;
    let processor = ConnectionProcessor::new(runtime_paths);
    let mut reconnect_backoff = ReconnectBackoff::default();

    loop {
        let retry_after = match client.register_environment(&config.environment_id).await {
            Ok(response) => {
                eprintln!(
                    "codex exec-server remote environment registered with environment_id {}",
                    response.environment_id
                );

                match timeout(
                    RENDEZVOUS_CONNECT_TIMEOUT,
                    connect_async(response.url.as_str()),
                )
                .await
                {
                    Ok(Ok((websocket, _))) => {
                        let connected_at = Instant::now();
                        run_multiplexed_environment(websocket, processor.clone()).await;
                        if connected_at.elapsed() >= STABLE_CONNECTION_DURATION {
                            reconnect_backoff.reset();
                        }
                    }
                    Ok(Err(error)) => {
                        warn!("failed to connect remote exec-server websocket: {error}");
                    }
                    Err(_) => warn!(
                        "timed out connecting remote exec-server websocket after {:?}",
                        RENDEZVOUS_CONNECT_TIMEOUT
                    ),
                }
                None
            }
            Err(EnvironmentRegistrationError::Retryable {
                source,
                retry_after,
            }) => {
                warn!(error = %source, "failed to register remote exec-server environment");
                retry_after
            }
            Err(EnvironmentRegistrationError::Permanent { source }) => {
                return Err(source);
            }
        };

        sleep(reconnect_backoff.next_delay(retry_after)).await;
    }
}

fn is_retryable_registration_status(status: StatusCode, code: Option<&str>) -> bool {
    status.is_server_error()
        || matches!(
            status,
            StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS
        )
        || (status == StatusCode::CONFLICT && code == Some("route_unavailable"))
}

fn normalize_environment_id(environment_id: String) -> Result<String, ExecServerError> {
    let environment_id = environment_id.trim().to_string();
    if environment_id.is_empty() {
        return Err(ExecServerError::EnvironmentRegistryConfig(
            "environment id is required for remote exec-server registration".to_string(),
        ));
    }
    Ok(environment_id)
}

#[derive(Deserialize)]
struct RegistryErrorBody {
    error: Option<RegistryError>,
}

#[derive(Deserialize)]
struct RegistryError {
    code: Option<String>,
    message: Option<String>,
}

fn normalize_base_url(base_url: String) -> Result<String, ExecServerError> {
    let trimmed = base_url.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return Err(ExecServerError::EnvironmentRegistryConfig(
            "environment registry base URL is required".to_string(),
        ));
    }
    Ok(trimmed)
}

fn endpoint_url(base_url: &str, path: &str) -> String {
    format!("{base_url}/{}", path.trim_start_matches('/'))
}

fn environment_registry_auth_error(status: StatusCode, body: &str) -> ExecServerError {
    let message = registry_error_message(body).unwrap_or_else(|| "empty error body".to_string());
    ExecServerError::EnvironmentRegistryAuth(format!(
        "environment registry authentication failed ({status}): {message}"
    ))
}

fn environment_registry_http_error(status: StatusCode, body: &str) -> ExecServerError {
    let parsed = serde_json::from_str::<RegistryErrorBody>(body).ok();
    let (code, message) = parsed
        .and_then(|body| body.error)
        .map(|error| {
            (
                error.code,
                error.message.unwrap_or_else(|| {
                    preview_error_body(body).unwrap_or_else(|| "empty error body".to_string())
                }),
            )
        })
        .unwrap_or_else(|| {
            (
                None,
                preview_error_body(body)
                    .unwrap_or_else(|| "empty or malformed error body".to_string()),
            )
        });
    ExecServerError::EnvironmentRegistryHttp {
        status,
        code,
        message,
    }
}

fn registry_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<RegistryErrorBody>(body)
        .ok()
        .and_then(|body| body.error)
        .and_then(|error| error.message)
        .or_else(|| preview_error_body(body))
}

fn preview_error_body(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(ERROR_BODY_PREVIEW_BYTES).collect())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_api::AuthProvider;
    use http::HeaderMap;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
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
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "environment_id": "env-1",
                "url": "wss://rendezvous.test/cloud-agent/default/ws/environment/env-1?role=environment&sig=abc"
            })))
            .mount(&server)
            .await;
        let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
            .expect("client");

        let response = client
            .register_environment(&config.environment_id)
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
    async fn register_environment_does_not_follow_redirects_with_auth_headers() {
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
            .register_environment("environment-requested")
            .await
            .expect_err("redirect response should not be followed");

        match error {
            EnvironmentRegistrationError::Permanent {
                source:
                    ExecServerError::EnvironmentRegistryHttp {
                        status: StatusCode::FOUND,
                        ..
                    },
            } => {}
            other => panic!("expected permanent redirect error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_environment_classifies_server_errors_as_retryable() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/register"))
            .respond_with(
                ResponseTemplate::new(StatusCode::SERVICE_UNAVAILABLE)
                    .insert_header("retry-after", "7"),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
            .expect("client");

        let error = client
            .register_environment("environment-requested")
            .await
            .expect_err("server error should be retryable");

        match error {
            EnvironmentRegistrationError::Retryable {
                source:
                    ExecServerError::EnvironmentRegistryHttp {
                        status: StatusCode::SERVICE_UNAVAILABLE,
                        ..
                    },
                retry_after: Some(retry_after),
            } => assert_eq!(retry_after, Duration::from_secs(7)),
            other => panic!("expected retryable server error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_environment_treats_malformed_success_as_permanent() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/register"))
            .respond_with(ResponseTemplate::new(StatusCode::OK).set_body_string("not json"))
            .expect(1)
            .mount(&server)
            .await;
        let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
            .expect("client");

        let error = client
            .register_environment("environment-requested")
            .await
            .expect_err("malformed success should fail permanently");

        match error {
            EnvironmentRegistrationError::Permanent {
                source: ExecServerError::Json(_),
            } => {}
            other => panic!("expected permanent decode error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn register_environment_retries_truncated_success_body() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("listener should accept");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 512\r\nconnection: close\r\n\r\n{\"environment_id\":\"env-1\"",
                )
                .await
                .expect("partial response should write");
        });
        let client = EnvironmentRegistryClient::new(
            format!("http://{address}"),
            static_registry_auth_provider(),
        )
        .expect("client");

        let error = client
            .register_environment("environment-requested")
            .await
            .expect_err("truncated success body should be retryable");

        match error {
            EnvironmentRegistrationError::Retryable {
                source: ExecServerError::EnvironmentRegistryRequest(_),
                retry_after: None,
            } => {}
            other => panic!("expected retryable body error, got {other:?}"),
        }
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn register_environment_retries_truncated_error_body() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("listener should have address");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("listener should accept");
            stream
                .write_all(
                    b"HTTP/1.1 409 Conflict\r\ncontent-type: application/json\r\ncontent-length: 512\r\nretry-after: 7\r\nconnection: close\r\n\r\n{\"error\":{\"code\":\"route_unavailable\"",
                )
                .await
                .expect("partial response should write");
        });
        let client = EnvironmentRegistryClient::new(
            format!("http://{address}"),
            static_registry_auth_provider(),
        )
        .expect("client");

        let error = client
            .register_environment("environment-requested")
            .await
            .expect_err("truncated error body should be retryable");

        match error {
            EnvironmentRegistrationError::Retryable {
                source: ExecServerError::EnvironmentRegistryRequest(_),
                retry_after: Some(retry_after),
            } => assert_eq!(retry_after, Duration::from_secs(7)),
            other => panic!("expected retryable body error, got {other:?}"),
        }
        server.await.expect("server task should finish");
    }

    #[tokio::test]
    async fn register_environment_rejects_url_that_websocket_client_cannot_parse() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/register"))
            .respond_with(
                ResponseTemplate::new(StatusCode::OK).set_body_json(serde_json::json!({
                    "environment_id": "env-1",
                    "url": "ws://rendezvous.test/environment with space",
                })),
            )
            .expect(1)
            .mount(&server)
            .await;
        let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
            .expect("client");

        let error = client
            .register_environment("environment-requested")
            .await
            .expect_err("invalid websocket URL should fail permanently");

        match error {
            EnvironmentRegistrationError::Permanent {
                source: ExecServerError::Protocol(message),
            } => assert_eq!(
                message,
                "environment registry returned an invalid rendezvous websocket URL"
            ),
            other => panic!("expected permanent URL error, got {other:?}"),
        }
    }

    #[test]
    fn reconnect_backoff_owns_one_advancing_sequence() {
        let mut reconnect_backoff = ReconnectBackoff::default();

        let first_delay = reconnect_backoff.next_delay(/*retry_after*/ None);
        assert!(first_delay >= Duration::from_millis(900));
        assert!(first_delay <= Duration::from_millis(1_100));
        let second_delay = reconnect_backoff.next_delay(/*retry_after*/ None);
        assert!(second_delay >= Duration::from_millis(1_800));
        assert!(second_delay <= Duration::from_millis(2_200));

        reconnect_backoff.reset();
        assert_eq!(
            reconnect_backoff.next_delay(Some(Duration::from_secs(10))),
            Duration::from_secs(10)
        );
        assert_eq!(
            reconnect_backoff.next_delay(Some(Duration::from_secs(60))),
            MAX_RECONNECT_BACKOFF
        );
        reconnect_backoff.reset();
        let reset_delay = reconnect_backoff.next_delay(/*retry_after*/ None);
        assert!(reset_delay >= Duration::from_millis(900));
        assert!(reset_delay <= Duration::from_millis(1_100));
    }

    #[test]
    fn registration_retryability_distinguishes_transient_and_permanent_statuses() {
        for status in [
            StatusCode::REQUEST_TIMEOUT,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
            StatusCode::SERVICE_UNAVAILABLE,
        ] {
            assert!(is_retryable_registration_status(status, /*code*/ None));
        }
        assert!(is_retryable_registration_status(
            StatusCode::CONFLICT,
            Some("route_unavailable")
        ));

        for status in [StatusCode::BAD_REQUEST, StatusCode::NOT_FOUND] {
            assert!(!is_retryable_registration_status(
                status, /*code*/ None
            ));
        }
        assert!(!is_retryable_registration_status(
            StatusCode::CONFLICT,
            Some("permanent_conflict")
        ));
        assert!(!is_retryable_registration_status(
            StatusCode::UNAUTHORIZED,
            /*code*/ None
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
    }
}
