use std::time::Instant;

use codex_api::AuthError;
use codex_api::AuthProvider;
use codex_http_client::NetworkPolicy;
use codex_http_client::NetworkPolicyDenied;
use codex_http_client::Request;
use codex_websocket_client::WebSocketConnection;
use codex_websocket_client::WebSocketConnector;
use http::Method;
use http::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::info;
use tracing::warn;

use super::EnvironmentRegistryClient;
use super::RemoteEnvironmentConfig;
use crate::ExecServerError;
use crate::client::is_retryable_recovery_error;
use crate::client::registry_recovery_retry_delay;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::client_transport::authenticate_websocket_request;
use crate::client_transport::connect_websocket_request;
use crate::connection::JsonRpcConnection;
use crate::server::ConnectionProcessor;
use crate::telemetry::ConnectionTransport;
use crate::trace_context::current_trace_context_headers;

const DIRECT_TRANSPORT: &str = "direct_jsonrpc_v1";

#[derive(Debug, Serialize)]
struct DirectRegistrationRequest {
    transport: &'static str,
}

#[derive(Debug, Deserialize)]
struct DirectRegistrationResponse {
    environment_id: String,
    transport: String,
    registration_id: String,
    url: String,
}

impl EnvironmentRegistryClient {
    async fn register_direct_environment_when_available(
        &self,
        environment_id: &str,
        policy: &NetworkPolicy,
    ) -> Result<DirectRegistrationResponse, ExecServerError> {
        let destination = super::endpoint_url(
            &self.base_url,
            &format!("/cloud/environment/{environment_id}/direct/register"),
        );
        let mut retry_attempt = 0;
        loop {
            match self.register_direct_environment(environment_id).await {
                Err(error) if is_policy_recoverable(&error, policy, &destination) => {}
                result => return result,
            }
            sleep(registry_recovery_retry_delay(environment_id, retry_attempt)).await;
            retry_attempt = retry_attempt.saturating_add(1);
        }
    }

    #[tracing::instrument(
        name = "codex.exec_server.remote.register",
        skip_all,
        fields(
            otel.kind = "client",
            otel.name = "codex.exec_server.remote.register",
            result = tracing::field::Empty,
        )
    )]
    async fn register_direct_environment(
        &self,
        environment_id: &str,
    ) -> Result<DirectRegistrationResponse, ExecServerError> {
        let started_at = Instant::now();
        let response = async {
            let url = super::endpoint_url(
                &self.base_url,
                &format!("/cloud/environment/{environment_id}/direct/register"),
            );
            let request = Request::new(Method::POST, url)
                .with_json(&DirectRegistrationRequest {
                    transport: DIRECT_TRANSPORT,
                })
                .into_prepared()
                .map_err(ExecServerError::EnvironmentRegistryConfig)?;
            let request = self
                .auth_provider
                .apply_auth(request)
                .await
                .map_err(direct_auth_error)?;
            let prepared = request
                .prepare_body_for_send()
                .map_err(ExecServerError::EnvironmentRegistryConfig)?;
            let response = self
                .http
                .request(request.method, request.url)
                .headers(prepared.headers)
                .headers(current_trace_context_headers())
                .body(prepared.body.unwrap_or_default())
                .timeout(self.connect_timeout)
                .send()
                .await?;
            let response: DirectRegistrationResponse = self.parse_json_response(response).await?;
            if response.environment_id != environment_id {
                return Err(ExecServerError::Protocol(
                    "environment registry returned a different environment id".to_string(),
                ));
            }
            if response.transport != DIRECT_TRANSPORT {
                return Err(ExecServerError::Protocol(format!(
                    "environment registry returned unsupported direct transport `{}`",
                    response.transport
                )));
            }
            if response.registration_id.trim().is_empty() || response.url.trim().is_empty() {
                return Err(ExecServerError::Protocol(
                    "environment registry returned incomplete direct connection data".to_string(),
                ));
            }
            require_tls_or_loopback(&response.url, "wss")?;
            Ok(response)
        }
        .await;
        let result = if response.is_ok() { "success" } else { "error" };
        tracing::Span::current().record("result", result);
        self.telemetry
            .remote_registration_completed(result, started_at.elapsed());
        response
    }
}

pub(super) async fn run_direct_environment(
    config: RemoteEnvironmentConfig,
    client: EnvironmentRegistryClient,
    processor: ConnectionProcessor,
) -> Result<(), ExecServerError> {
    require_tls_or_loopback(&config.base_url, "https")?;
    let mut retry_attempt = 0;
    let mut registration = client
        .register_direct_environment_when_available(
            &config.environment_id,
            config.http_client_factory.network_policy(),
        )
        .await?;

    loop {
        match connect_direct(
            &registration.url,
            config.auth_provider.as_ref(),
            &config.http_client_factory,
        )
        .await
        {
            Ok(websocket) => {
                retry_attempt = 0;
                info!(
                    environment_id = registration.environment_id,
                    registration_id = registration.registration_id,
                    "direct exec-server connected"
                );
                processor
                    .run_connection(
                        JsonRpcConnection::from_websocket(
                            websocket,
                            format!(
                                "direct exec-server websocket {}",
                                websocket_diagnostic_url(&registration.url)
                            ),
                        ),
                        ConnectionTransport::WebSocket,
                    )
                    .await;
                config.telemetry.remote_reconnect("disconnected");
            }
            Err(error)
                if (is_retryable_recovery_error(&error)
                    || is_policy_recoverable(
                        &error,
                        config.http_client_factory.network_policy(),
                        &registration.url,
                    ))
                    && !matches!(
                        &error,
                        ExecServerError::WebSocketConnect {
                            source: tokio_tungstenite::tungstenite::Error::Http(response),
                            ..
                        } if response.status().is_client_error()
                            && !matches!(
                                response.status(),
                                StatusCode::REQUEST_TIMEOUT
                                    | StatusCode::CONFLICT
                                    | StatusCode::TOO_MANY_REQUESTS
                            )
                    ) =>
            {
                // A handshake conflict rejects this registration; other transient failures
                // reconnect using the existing URL.
                if matches!(
                    &error,
                    ExecServerError::WebSocketConnect {
                        source: tokio_tungstenite::tungstenite::Error::Http(response),
                        ..
                    } if response.status() == StatusCode::CONFLICT
                ) {
                    registration = client
                        .register_direct_environment_when_available(
                            &config.environment_id,
                            config.http_client_factory.network_policy(),
                        )
                        .await?;
                }
                warn!("direct exec-server connection failed; retrying");
                config.telemetry.remote_reconnect("connect_failed");
            }
            Err(error) => return Err(error),
        }

        sleep(registry_recovery_retry_delay(
            &config.environment_id,
            retry_attempt,
        ))
        .await;
        retry_attempt = retry_attempt.saturating_add(1);
    }
}

fn is_policy_recoverable(
    error: &ExecServerError,
    policy: &NetworkPolicy,
    destination: &str,
) -> bool {
    match error.application_network_policy_denial() {
        Some(NetworkPolicyDenied::Unavailable) => true,
        Some(NetworkPolicyDenied::Revoked) => url::Url::parse(destination).is_ok_and(|url| {
            matches!(
                policy.acquire(&url),
                Ok(_) | Err(NetworkPolicyDenied::Unavailable)
            )
        }),
        Some(NetworkPolicyDenied::Destination | NetworkPolicyDenied::UnsupportedTransport)
        | None => false,
    }
}

async fn connect_direct(
    url: &str,
    auth_provider: &dyn AuthProvider,
    http_client_factory: &codex_http_client::HttpClientFactory,
) -> Result<WebSocketConnection, ExecServerError> {
    let mut request =
        url.into_client_request()
            .map_err(|source| ExecServerError::WebSocketConnect {
                url: websocket_diagnostic_url(url),
                source,
            })?;
    request
        .headers_mut()
        .extend(current_trace_context_headers());
    authenticate_websocket_request(&mut request, auth_provider)
        .await
        .map_err(direct_auth_error)?;
    let authenticated_url = request.uri().to_string();
    require_tls_or_loopback(&authenticated_url, "wss")?;
    let connector = WebSocketConnector::new(http_client_factory)
        .map_err(|error| ExecServerError::WebSocketConfiguration(error.to_string()))?
        .with_tcp_nodelay();
    connect_websocket_request(
        request,
        websocket_diagnostic_url(&authenticated_url),
        connector,
        DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
        /*use_loopback_direct*/ false,
    )
    .await
}

pub(super) fn require_tls_or_loopback(
    url: &str,
    secure_scheme: &str,
) -> Result<(), ExecServerError> {
    let parsed = url::Url::parse(url).map_err(|error| {
        ExecServerError::EnvironmentRegistryConfig(format!("invalid remote endpoint URL: {error}"))
    })?;
    if parsed.scheme() == secure_scheme {
        return Ok(());
    }

    let loopback = match parsed.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(host)) => host.is_loopback(),
        Some(url::Host::Ipv6(host)) => host.is_loopback(),
        None => false,
    };
    if loopback
        && secure_scheme
            .strip_suffix('s')
            .is_some_and(|scheme| parsed.scheme() == scheme)
    {
        return Ok(());
    }

    Err(ExecServerError::EnvironmentRegistryConfig(format!(
        "remote transport requires {secure_scheme} for non-loopback endpoints"
    )))
}

fn websocket_diagnostic_url(url: &str) -> String {
    url.split(['?', '#']).next().unwrap_or(url).to_string()
}

fn direct_auth_error(error: AuthError) -> ExecServerError {
    let message = format!("failed to resolve environment registry authentication: {error}");
    match error {
        AuthError::Build(_) => ExecServerError::EnvironmentRegistryAuth(message),
        AuthError::Transient(_) => ExecServerError::Disconnected(message),
    }
}

#[cfg(test)]
#[path = "direct_tests.rs"]
mod tests;
