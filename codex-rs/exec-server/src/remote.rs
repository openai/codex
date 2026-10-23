use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_http_client::CHATGPT_ACCOUNT_ID_HEADER;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
use codex_http_client::HttpResponse;
use codex_http_client::Request;
use codex_http_client::RouteAwareClientPool;
use futures::FutureExt;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use http::StatusCode;
use http::header::AUTHORIZATION;
use serde::Deserialize;
use tokio::time::sleep;
use tokio::time::timeout_at;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::debug;
use tracing::info;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use codex_websocket_client::WebSocketConnection;
use codex_websocket_client::WebSocketConnector;
use codex_websocket_client::WebSocketTlsMode;

use crate::EnvironmentRegistryConnectRequest;
use crate::EnvironmentRegistryConnectResponse;
use crate::EnvironmentRegistryHarnessKeyValidationRequest;
use crate::EnvironmentRegistryHarnessKeyValidationResponse;
use crate::EnvironmentRegistryRegistrationRequest;
use crate::EnvironmentRegistryRegistrationResponse;
use crate::ExecServerError;
use crate::ExecServerRuntimeOptions;
use crate::ExecServerTelemetry;
use crate::NoiseChannelIdentity;
use crate::NoiseChannelPublicKey;
use crate::NoiseRendezvousConnectBundle;
use crate::NoiseRendezvousConnectProvider;
use crate::client_api::DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT;
use crate::forward::Forwarder;
use crate::noise_relay::noise_relay_websocket_config;
use crate::noise_relay::stream_handler::NoiseStreamHandler;
use crate::relay::HarnessKeyValidator;
use crate::relay::run_multiplexed_environment;
use crate::server::ConnectionProcessor;
use crate::server::RequestDispatchMode;
use crate::trace_context::current_rendezvous_headers;
use crate::trace_context::current_trace_context_headers;

#[path = "remote/direct.rs"]
mod direct;

use direct::run_direct_environment;

const ERROR_BODY_PREVIEW_BYTES: usize = 4096;
const NOISE_RELAY_SECURITY_PROFILE: &str = "noise_hybrid_ik_v1";

mod auth;
#[path = "remote/connection_diagnostics.rs"]
mod connection_diagnostics;
mod reconnect_backoff;
mod registration_retry;

/// Wire transport used after registering a remote exec-server.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// The transport used to connect a remote exec-server environment.
pub enum RemoteEnvironmentTransport {
    #[default]
    Noise,
    Direct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegistryAuthMode {
    Legacy,
    ExecutorCredential,
}

#[derive(Clone)]
struct EnvironmentRegistryClient {
    base_url: String,
    auth_provider: SharedAuthProvider,
    auth_mode: RegistryAuthMode,
    http: RouteAwareClientPool,
    http_direct: RouteAwareClientPool,
    connect_timeout: Duration,
    telemetry: ExecServerTelemetry,
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
    #[cfg(test)]
    fn new(base_url: String, auth_provider: SharedAuthProvider) -> Result<Self, ExecServerError> {
        Self::new_with_telemetry(
            base_url,
            auth_provider,
            ExecServerTelemetry::default(),
            HttpClientFactory::new(codex_http_client::OutboundProxyPolicy::ReqwestDefault),
        )
    }

    fn new_with_telemetry(
        base_url: String,
        auth_provider: SharedAuthProvider,
        telemetry: ExecServerTelemetry,
        http_client_factory: HttpClientFactory,
    ) -> Result<Self, ExecServerError> {
        let base_url = normalize_base_url(base_url)?;
        Ok(Self {
            base_url,
            auth_provider,
            auth_mode: RegistryAuthMode::Legacy,
            http: RouteAwareClientPool::new_without_redirects_or_request_logging(
                http_client_factory.clone(),
                ClientRouteClass::Api,
            ),
            http_direct: RouteAwareClientPool::new_without_redirects_or_request_logging(
                http_client_factory,
                ClientRouteClass::Api,
            )
            .with_legacy_direct_proxy_and_custom_ca_fallback(),
            connect_timeout: DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT,
            telemetry,
        })
    }

    /// Register the executor public key and obtain the rendezvous allocation.
    /// The returned registration ID is included in each stream's Noise prologue.
    #[tracing::instrument(
        name = "codex.exec_server.remote.register",
        skip_all,
        fields(
            otel.kind = "client",
            otel.name = "codex.exec_server.remote.register",
            result = tracing::field::Empty,
        )
    )]
    async fn register_environment(
        &self,
        environment_id: &str,
        executor_public_key: &NoiseChannelPublicKey,
    ) -> Result<EnvironmentRegistryRegistrationResponse, ExecServerError> {
        let started_at = Instant::now();
        let response = self
            .register_environment_inner(environment_id, executor_public_key)
            .await;
        let result = if response.is_ok() { "success" } else { "error" };
        tracing::Span::current().record("result", result);
        self.telemetry
            .remote_registration_completed(result, started_at.elapsed());
        response
    }

    async fn register_environment_inner(
        &self,
        environment_id: &str,
        executor_public_key: &NoiseChannelPublicKey,
    ) -> Result<EnvironmentRegistryRegistrationResponse, ExecServerError> {
        let deadline = tokio::time::Instant::now() + self.connect_timeout;
        let url = endpoint_url(
            &self.base_url,
            &format!("/cloud/environment/{environment_id}/register"),
        );
        let body = EnvironmentRegistryRegistrationRequest {
            security_profile: NOISE_RELAY_SECURITY_PROFILE.to_string(),
            executor_public_key: executor_public_key.clone(),
        };
        let (response, executor_credential) = timeout_at(
            deadline,
            self.send_authenticated(Request::new(Method::POST, url).with_json(&body)),
        )
        .await
        .unwrap_or_else(|_| {
            Err(ExecServerError::EnvironmentRegistryRequest(
                codex_http_client::RouteAwareRequestError::Timeout,
            ))
        })?;
        let status = response.status();
        // Read diagnostics within the same attempt budget, preserving a known error status.
        let response: EnvironmentRegistryRegistrationResponse = timeout_at(deadline, async {
            if let Some(credential) = executor_credential
                .as_ref()
                .and_then(|value| value.to_str().ok())
            {
                parse_executor_registration_response(response, credential).await
            } else {
                self.parse_json_response(response).await
            }
        })
        .await
        .unwrap_or_else(|_| {
            Err(match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                    environment_registry_auth_error(status, "response body timed out")
                }
                status if !status.is_success() => {
                    environment_registry_http_error(status, "response body timed out")
                }
                _ => ExecServerError::EnvironmentRegistryRequest(
                    codex_http_client::RouteAwareRequestError::Timeout,
                ),
            })
        })?;
        if response.environment_id != environment_id {
            return Err(ExecServerError::Protocol(
                "environment registry returned a different environment id".to_string(),
            ));
        }
        if response.security_profile != NOISE_RELAY_SECURITY_PROFILE {
            return Err(ExecServerError::Protocol(format!(
                "environment registry returned unsupported security profile `{}`",
                response.security_profile
            )));
        }
        info!(
            noise_event = "registration",
            noise_outcome = "ok",
            security_profile = NOISE_RELAY_SECURITY_PROFILE,
            "Noise executor registration completed"
        );
        debug!(
            environment_id = response.environment_id,
            executor_registration_id = response.executor_registration_id,
            "Noise executor registration details"
        );
        Ok(response)
    }

    /// Authorize one Noise harness key and obtain the full rendezvous bundle.
    #[tracing::instrument(
        name = "codex.exec_server.remote.environment_registry.connect",
        skip_all,
        fields(
            otel.kind = "client",
            otel.name = "codex.exec_server.remote.environment_registry.connect",
            environment_id = %environment_id,
        )
    )]
    async fn connect_environment(
        &self,
        environment_id: &str,
        harness_public_key: NoiseChannelPublicKey,
    ) -> Result<NoiseRendezvousConnectBundle, ExecServerError> {
        let url = endpoint_url(
            &self.base_url,
            &format!("/cloud/environment/{environment_id}/connect"),
        );
        let body = EnvironmentRegistryConnectRequest { harness_public_key };
        let response: EnvironmentRegistryConnectResponse =
            timeout_at(tokio::time::Instant::now() + self.connect_timeout, async {
                let (response, _) = self
                    .send_authenticated(Request::new(Method::POST, url).with_json(&body))
                    .await?;
                self.parse_json_response(response).await
            })
            .await
            .unwrap_or_else(|_| {
                Err(ExecServerError::EnvironmentRegistryRequest(
                    codex_http_client::RouteAwareRequestError::Timeout,
                ))
            })?;
        if response.environment_id != environment_id {
            return Err(ExecServerError::Protocol(
                "environment registry returned a different environment id".to_string(),
            ));
        }
        if response.security_profile != NOISE_RELAY_SECURITY_PROFILE {
            return Err(ExecServerError::Protocol(format!(
                "environment registry returned unsupported security profile `{}`",
                response.security_profile
            )));
        }
        if response.url.trim().is_empty()
            || response.executor_registration_id.trim().is_empty()
            || response.harness_key_authorization.trim().is_empty()
        {
            return Err(ExecServerError::Protocol(
                "environment registry returned incomplete Noise connection data".to_string(),
            ));
        }
        Ok(NoiseRendezvousConnectBundle {
            websocket_url: response.url,
            environment_id: response.environment_id,
            executor_registration_id: response.executor_registration_id,
            executor_public_key: response.executor_public_key,
            harness_key_authorization: response.harness_key_authorization,
        })
    }

    async fn resolve_auth(&self) -> Result<(HeaderMap, Option<HeaderValue>), ExecServerError> {
        let mut headers = self
            .auth_provider
            .apply_auth_headers(HeaderMap::new())
            .await
            .map_err(|error| {
                ExecServerError::EnvironmentRegistryAuth(format!(
                    "failed to resolve environment registry authentication: {error}"
                ))
            })?;
        let executor_credential = if self.auth_mode == RegistryAuthMode::ExecutorCredential {
            // Scoped mode sends exactly one bearer and never accompanying account credentials.
            let token = (headers.len() == 1)
                .then(|| headers.get(AUTHORIZATION))
                .flatten()
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
                .filter(|token| {
                    !token.is_empty()
                        && token.len() <= 16 * 1024
                        && token.bytes().all(|byte| byte.is_ascii_graphic())
                })
                .ok_or_else(|| {
                    ExecServerError::EnvironmentRegistryAuth(
                        "invalid executor credential authorization".to_string(),
                    )
                })?;
            let mut credential = HeaderValue::from_str(token).map_err(|_| {
                ExecServerError::EnvironmentRegistryAuth(
                    "invalid executor credential authorization".to_string(),
                )
            })?;
            credential.set_sensitive(true);
            if let Some(authorization) = headers.get_mut(AUTHORIZATION) {
                authorization.set_sensitive(true);
            }
            direct::require_tls_or_loopback(&self.base_url, "https")?;
            Some(credential)
        } else {
            None
        };
        Ok((headers, executor_credential))
    }

    fn http_for_auth(&self) -> &RouteAwareClientPool {
        // A system proxy can otherwise receive a loopback HTTP credential in plaintext.
        if self.auth_mode == RegistryAuthMode::ExecutorCredential
            && url::Url::parse(&self.base_url).is_ok_and(|url| url.scheme() == "http")
        {
            &self.http_direct
        } else {
            &self.http
        }
    }

    async fn parse_json_response<R>(&self, response: HttpResponse) -> Result<R, ExecServerError>
    where
        R: for<'de> Deserialize<'de>,
    {
        if response.status().is_success() {
            let body = response
                .text()
                .await
                .map_err(ExecServerError::EnvironmentRegistryRequest)?;
            return serde_json::from_str(&body).map_err(ExecServerError::Json);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(environment_registry_auth_error(status, &body));
        }

        Err(environment_registry_http_error(status, &body))
    }
}

#[derive(Clone)]
struct RegistryHarnessKeyValidator {
    client: EnvironmentRegistryClient,
    environment_id: String,
    executor_registration_id: String,
}

impl HarnessKeyValidator for RegistryHarnessKeyValidator {
    /// Authorize the harness key recovered from the first IK message.
    /// Noise proves key possession; the registry decides whether that key may use
    /// this executor. The authorization token and public key are checked together.
    #[tracing::instrument(
        name = "codex.exec_server.remote.environment_registry.validate_harness_key",
        skip_all,
        fields(
            otel.kind = "client",
            otel.name = "codex.exec_server.remote.environment_registry.validate_harness_key",
            environment_id = %self.environment_id,
            executor_registration_id = %self.executor_registration_id,
        )
    )]
    async fn validate_harness_key(
        &self,
        harness_public_key: &NoiseChannelPublicKey,
        authorization: &str,
        stream_id: Option<uuid::Uuid>,
    ) -> Result<(), ExecServerError> {
        let environment_id = &self.environment_id;
        let url = endpoint_url(
            &self.client.base_url,
            &format!("/cloud/environment/{environment_id}/validate"),
        );
        let body = EnvironmentRegistryHarnessKeyValidationRequest {
            executor_registration_id: self.executor_registration_id.clone(),
            harness_public_key: harness_public_key.clone(),
            harness_key_authorization: authorization.to_string(),
        };
        let mut request = Request::new(Method::POST, url).with_json(&body);
        if let Some(stream_id) = stream_id {
            request.headers.insert(
                "x-codex-noise-stream-id",
                HeaderValue::from_str(&stream_id.to_string()).map_err(|error| {
                    ExecServerError::Protocol(format!("invalid Noise stream ID header: {error}"))
                })?,
            );
        }
        let (response, executor_credential) = self.client.send_authenticated(request).await?;
        let status = response.status();
        if !status.is_success() {
            // The request contains the short-lived authorization. Do not include
            // a response body that might echo it in logs or error chains.
            if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
                return Err(ExecServerError::EnvironmentRegistryAuth(format!(
                    "environment registry harness key validation authentication failed ({status})"
                )));
            }
            return Err(ExecServerError::EnvironmentRegistryHttp {
                status,
                code: None,
                message: "environment registry harness key validation failed".to_string(),
            });
        }
        let response = response
            .json::<EnvironmentRegistryHarnessKeyValidationResponse>()
            .await
            .map_err(|error| {
                if executor_credential.is_some() {
                    ExecServerError::Protocol(
                        "invalid environment registry harness key validation response".to_string(),
                    )
                } else {
                    ExecServerError::EnvironmentRegistryRequest(error)
                }
            })?;
        if !response.valid {
            return Err(ExecServerError::Protocol(
                "environment registry rejected Noise relay harness key".to_string(),
            ));
        }
        Ok(())
    }
}

/// Noise connection configuration for a Codex harness.
///
/// Configuration stays inert until the effective outbound HTTP policy is known.
/// Its connection provider then holds the authenticated registry client so every
/// reconnect receives fresh URL and harness-key authorization material.
#[derive(Clone)]
pub(crate) struct NoiseRendezvousEnvironmentConfig {
    base_url: String,
    environment_id: String,
    auth_provider: SharedAuthProvider,
}

impl std::fmt::Debug for NoiseRendezvousEnvironmentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseRendezvousEnvironmentConfig")
            .field("base_url", &"<redacted>")
            .field("environment_id", &self.environment_id)
            .field("auth_provider", &"<redacted>")
            .finish()
    }
}

impl NoiseRendezvousEnvironmentConfig {
    pub(crate) fn new(
        base_url: String,
        environment_id: String,
        bearer_token: String,
        chatgpt_account_id: Option<String>,
    ) -> Result<Self, ExecServerError> {
        let base_url = normalize_base_url(base_url)?;
        let environment_id = normalize_environment_id(environment_id)?;
        let auth_provider = static_bearer_auth_provider(bearer_token, chatgpt_account_id)?;
        Ok(Self {
            base_url,
            environment_id,
            auth_provider,
        })
    }

    pub(crate) fn into_connect_provider(
        self,
        http_client_factory: HttpClientFactory,
    ) -> Result<Arc<dyn NoiseRendezvousConnectProvider>, ExecServerError> {
        let client = EnvironmentRegistryClient::new_with_telemetry(
            self.base_url,
            self.auth_provider,
            ExecServerTelemetry::default(),
            http_client_factory,
        )?;
        Ok(Arc::new(EnvironmentRegistryNoiseConnectProvider {
            client,
            environment_id: self.environment_id,
        }))
    }
}

#[derive(Clone, Debug)]
struct EnvironmentRegistryNoiseConnectProvider {
    client: EnvironmentRegistryClient,
    environment_id: String,
}

impl NoiseRendezvousConnectProvider for EnvironmentRegistryNoiseConnectProvider {
    fn connect_bundle(
        &self,
        harness_public_key: NoiseChannelPublicKey,
    ) -> futures::future::BoxFuture<'_, Result<NoiseRendezvousConnectBundle, ExecServerError>> {
        async move {
            self.client
                .connect_environment(&self.environment_id, harness_public_key)
                .await
        }
        .boxed()
    }
}

#[derive(Clone)]
struct StaticBearerAuthProvider {
    authorization: HeaderValue,
    chatgpt_account_id: Option<HeaderValue>,
}

impl std::fmt::Debug for StaticBearerAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StaticBearerAuthProvider")
            .field("authorization", &"<redacted>")
            .field(
                "chatgpt_account_id",
                &self.chatgpt_account_id.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl AuthProvider for StaticBearerAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
        headers.insert(http::header::AUTHORIZATION, self.authorization.clone());
        if let Some(chatgpt_account_id) = &self.chatgpt_account_id {
            headers.insert(
                HeaderName::from_static(CHATGPT_ACCOUNT_ID_HEADER),
                chatgpt_account_id.clone(),
            );
        }
        Ok(())
    }
}

fn static_bearer_auth_provider(
    bearer_token: String,
    chatgpt_account_id: Option<String>,
) -> Result<SharedAuthProvider, ExecServerError> {
    let bearer_token = bearer_token.trim();
    if bearer_token.is_empty() {
        return Err(ExecServerError::EnvironmentRegistryConfig(
            "environment registry bearer token is required".to_string(),
        ));
    }
    let authorization =
        HeaderValue::try_from(format!("Bearer {bearer_token}")).map_err(|error| {
            ExecServerError::EnvironmentRegistryConfig(format!(
                "environment registry bearer token is not a valid HTTP header: {error}"
            ))
        })?;
    let chatgpt_account_id = chatgpt_account_id
        .as_deref()
        .map(str::trim)
        .filter(|account_id| !account_id.is_empty())
        .map(|account_id| {
            HeaderValue::try_from(account_id).map_err(|error| {
                ExecServerError::EnvironmentRegistryConfig(format!(
                    "ChatGPT account id is not a valid HTTP header: {error}"
                ))
            })
        })
        .transpose()?;
    Ok(Arc::new(StaticBearerAuthProvider {
        authorization,
        chatgpt_account_id,
    }))
}

/// Configuration for registering an exec-server for remote use.
///
/// Scoped mode uses one `Authorization: Bearer <credential>` header with
/// the Noise transport. It requires an HTTPS registry URL, except that HTTP is
/// permitted for loopback. Scoped executor credentials are not supported by
/// the Direct transport.
#[derive(Clone)]
pub struct RemoteEnvironmentConfig {
    pub base_url: String,
    pub environment_id: String,
    pub name: String,
    pub request_dispatch_mode: RequestDispatchMode,
    transport: RemoteEnvironmentTransport,
    auth_provider: SharedAuthProvider,
    auth_mode: RegistryAuthMode,
    telemetry: ExecServerTelemetry,
    http_client_factory: HttpClientFactory,
}

impl std::fmt::Debug for RemoteEnvironmentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteEnvironmentConfig")
            .field("base_url", &self.base_url)
            .field("environment_id", &self.environment_id)
            .field("name", &self.name)
            .field("request_dispatch_mode", &self.request_dispatch_mode)
            .field("transport", &self.transport)
            .field("auth_provider", &"<redacted>")
            .field("auth_mode", &self.auth_mode)
            .finish()
    }
}

impl RemoteEnvironmentConfig {
    /// Creates a remote environment configuration using the default Noise transport.
    pub fn new(
        base_url: String,
        environment_id: String,
        auth_provider: SharedAuthProvider,
        http_client_factory: HttpClientFactory,
    ) -> Result<Self, ExecServerError> {
        Self::new_with_transport(
            base_url,
            environment_id,
            RemoteEnvironmentTransport::Noise,
            auth_provider,
            http_client_factory,
        )
    }

    /// Creates a remote environment configuration using an explicit transport.
    pub fn new_with_transport(
        base_url: String,
        environment_id: String,
        transport: RemoteEnvironmentTransport,
        auth_provider: SharedAuthProvider,
        http_client_factory: HttpClientFactory,
    ) -> Result<Self, ExecServerError> {
        let environment_id = normalize_environment_id(environment_id)?;
        Ok(Self {
            base_url,
            environment_id,
            name: "codex-exec-server".to_string(),
            request_dispatch_mode: RequestDispatchMode::Inline,
            transport,
            auth_provider,
            auth_mode: RegistryAuthMode::Legacy,
            telemetry: ExecServerTelemetry::default(),
            http_client_factory,
        })
    }

    /// Requires one environment-bound bearer from the auth provider, without account fallback.
    pub fn with_executor_credential_auth(mut self) -> Result<Self, ExecServerError> {
        if self.transport == RemoteEnvironmentTransport::Direct {
            return Err(ExecServerError::EnvironmentRegistryConfig(
                "direct transport does not support scoped executor credentials".to_string(),
            ));
        }
        direct::require_tls_or_loopback(&self.base_url, "https")?;
        self.auth_mode = RegistryAuthMode::ExecutorCredential;
        Ok(self)
    }

    pub fn with_telemetry(mut self, telemetry: ExecServerTelemetry) -> Self {
        self.telemetry = telemetry;
        self
    }
}

/// Register an exec-server for remote use and serve requests over its configured transport.
///
/// In Noise mode, the executor identity is generated once per process and reused across
/// reconnects. The registration and rendezvous URL are also reused until
/// rendezvous rejects the URL, at which point the next attempt registers again.
/// The websocket carries cleartext routing metadata and encrypted payloads.
///
/// Direct mode reuses its registration across reconnects. A WebSocket handshake
/// conflict refreshes the registration; other permanent client errors stop the runner.
pub async fn run_remote_environment(
    config: RemoteEnvironmentConfig,
    runtime_paths: ExecServerRuntimeOptions,
) -> Result<(), ExecServerError> {
    run_remote_environment_until_shutdown(config, runtime_paths, std::future::pending()).await
}

/// Serve a remote environment until its owner requests graceful shutdown.
///
/// Active sessions and their processes are drained before this function returns.
pub async fn run_remote_environment_until_shutdown<F>(
    config: RemoteEnvironmentConfig,
    runtime_paths: ExecServerRuntimeOptions,
    shutdown: F,
) -> Result<(), ExecServerError>
where
    F: std::future::Future<Output = ()>,
{
    let processor = ConnectionProcessor::new_with_telemetry(
        runtime_paths,
        config.telemetry.clone(),
        config.http_client_factory.clone(),
        config.request_dispatch_mode,
    );

    let result = match config.transport {
        RemoteEnvironmentTransport::Noise => {
            run_remote_transport(config, shutdown, |config, client| {
                run_remote_environment_connections(config, client, processor.clone())
            })
            .await
        }
        RemoteEnvironmentTransport::Direct => {
            run_remote_transport(config, shutdown, |config, client| {
                run_direct_environment(config, client, processor.clone())
            })
            .await
        }
    };
    processor.shutdown().await;
    result
}

/// Register a remote environment backed by an independently owned WebSocket executor.
pub async fn run_remote_environment_forward_until_shutdown<F>(
    config: RemoteEnvironmentConfig,
    websocket_url: String,
    shutdown: F,
) -> Result<(), ExecServerError>
where
    F: std::future::Future<Output = ()>,
{
    // Forwarder implements the Noise stream bridge. Direct forwarding needs a
    // separate Direct-compatible bridge, so reject it rather than use the
    // Noise-specific path.
    // Remove this guard when a Direct-compatible forwarder is added.
    if config.transport == RemoteEnvironmentTransport::Direct {
        return Err(ExecServerError::EnvironmentRegistryConfig(
            "direct exec-server transport does not support forwarding".to_string(),
        ));
    }
    let forwarder = Forwarder::new(
        websocket_url,
        &config.http_client_factory,
        config.telemetry.clone(),
    )?;
    run_remote_transport(config, shutdown, |config, client| {
        run_remote_environment_connections(config, client, forwarder)
    })
    .await
}

async fn run_remote_transport<F, R, T>(
    config: RemoteEnvironmentConfig,
    shutdown: F,
    run_loop: R,
) -> Result<(), ExecServerError>
where
    F: std::future::Future<Output = ()>,
    R: FnOnce(RemoteEnvironmentConfig, EnvironmentRegistryClient) -> T,
    T: std::future::Future<Output = Result<(), ExecServerError>>,
{
    ensure_rustls_crypto_provider();
    let mut client = EnvironmentRegistryClient::new_with_telemetry(
        config.base_url.clone(),
        config.auth_provider.clone(),
        config.telemetry.clone(),
        config.http_client_factory.clone(),
    )?;
    client.auth_mode = config.auth_mode;
    let run = run_loop(config, client);
    tokio::pin!(run, shutdown);
    tokio::select! {
        result = &mut run => result,
        _ = &mut shutdown => Ok(()),
    }
}

async fn run_remote_environment_connections<H: NoiseStreamHandler>(
    config: RemoteEnvironmentConfig,
    client: EnvironmentRegistryClient,
    handler: H,
) -> Result<(), ExecServerError> {
    let identity = NoiseChannelIdentity::generate().map_err(|error| {
        ExecServerError::Protocol(format!("failed to generate Noise relay identity: {error}"))
    })?;
    let mut backoff = reconnect_backoff::ReconnectBackoff::new();
    let mut response = client
        .register_environment_with_retry(&config.environment_id, &identity.public_key())
        .await?;

    let mut connection_attempt = 0_u64;
    loop {
        connection_attempt += 1;
        let request_id = format!("req_{}", uuid::Uuid::new_v4().simple());
        match connect_rendezvous(
            &response.url,
            &request_id,
            &config.telemetry,
            &config.http_client_factory,
        )
        .await
        {
            Ok(websocket) => {
                let connected_at = tokio::time::Instant::now();
                let executor_registration_id = response.executor_registration_id.clone();
                info!(
                    noise_event = "rendezvous_connection",
                    noise_outcome = "ok",
                    request_id,
                    connection_attempt,
                    "Noise executor connected to rendezvous"
                );
                let disconnect_reason = run_multiplexed_environment(
                    websocket,
                    handler.clone(),
                    response.environment_id.clone(),
                    executor_registration_id.clone(),
                    identity.clone(),
                    RegistryHarnessKeyValidator {
                        client: client.clone(),
                        environment_id: config.environment_id.clone(),
                        executor_registration_id,
                    },
                )
                .await;
                backoff.connection_closed(connected_at.elapsed());
                info!(
                    noise_event = "rendezvous_connection",
                    noise_outcome = "disconnected",
                    noise_reason = disconnect_reason.as_str(),
                    request_id,
                    connection_attempt,
                    "Noise executor disconnected from rendezvous"
                );
                config
                    .telemetry
                    .remote_reconnect(disconnect_reason.as_str());
            }
            Err(error) => {
                let registration_rejected = matches!(
                    &error,
                    tokio_tungstenite::tungstenite::Error::Http(response)
                        if response.status().is_client_error()
                );
                let failure = connection_diagnostics::ConnectionFailure::from(&error);
                warn!(
                    noise_event = "rendezvous_connection",
                    noise_outcome = "error",
                    noise_reason = "websocket_error",
                    request_id,
                    connection_attempt,
                    error_kind = failure.error_kind,
                    io_error_kind = failure.io_error_kind,
                    http_status = failure.http_status,
                    rejection_reason = failure.rejection_reason,
                    "Noise executor failed to connect to rendezvous"
                );
                if registration_rejected {
                    config.telemetry.remote_reconnect("registration_rejected");
                    response = client
                        .register_environment_with_retry(
                            &config.environment_id,
                            &identity.public_key(),
                        )
                        .await?;
                } else {
                    config.telemetry.remote_reconnect("connect_failed");
                }
            }
        }

        sleep(backoff.next_delay(uuid::Uuid::new_v4().as_u64_pair().1)).await;
    }
}

#[tracing::instrument(
    name = "codex.exec_server.remote.rendezvous.connect",
    skip_all,
    fields(
        otel.kind = "client",
        otel.name = "codex.exec_server.remote.rendezvous.connect",
        request_id = request_id,
        result = tracing::field::Empty,
    )
)]
async fn connect_rendezvous(
    url: &str,
    request_id: &str,
    telemetry: &ExecServerTelemetry,
    http_client_factory: &HttpClientFactory,
) -> Result<WebSocketConnection, tokio_tungstenite::tungstenite::Error> {
    let started_at = Instant::now();
    let result = tokio::time::timeout(DEFAULT_REMOTE_EXEC_SERVER_CONNECT_TIMEOUT, async {
        let mut request = url.into_client_request()?;
        request.headers_mut().extend(current_rendezvous_headers());
        request
            .headers_mut()
            .insert("x-request-id", HeaderValue::from_str(request_id)?);
        let connector = WebSocketConnector::new_with_tls_mode(
            http_client_factory,
            WebSocketTlsMode::TungsteniteDefault,
        )
        .map_err(|error| tokio_tungstenite::tungstenite::Error::Io(std::io::Error::other(error)))?;
        connector
            .with_tcp_nodelay()
            .connect(request, noise_relay_websocket_config())
            .await
            .map(|(websocket, _)| websocket)
    })
    .await
    .unwrap_or_else(|_| {
        Err(tokio_tungstenite::tungstenite::Error::Io(
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "rendezvous websocket connection timed out",
            ),
        ))
    });
    let result_name = if result.is_ok() { "success" } else { "error" };
    tracing::Span::current().record("result", result_name);
    telemetry.remote_rendezvous_completed(result_name, started_at.elapsed());
    result
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
    detail: Option<serde_json::Value>,
}

impl RegistryErrorBody {
    fn into_error(self) -> Option<RegistryError> {
        // Keep the canonical error authoritative when both envelopes are present.
        self.error
            .or_else(|| serde_json::from_value(self.detail?).ok())
    }
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

async fn parse_executor_registration_response(
    response: HttpResponse,
    credential: &str,
) -> Result<EnvironmentRegistryRegistrationResponse, ExecServerError> {
    const SAFE_ERROR: &str = "invalid environment registry executor registration response";
    let status = response.status();
    if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
        return Err(environment_registry_auth_error(
            status,
            "response body omitted",
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|_| ExecServerError::Protocol(SAFE_ERROR.to_string()))?;
    if !status.is_success() {
        // Only these fixed codes affect retry behavior. Registry diagnostics can echo the credential.
        let code = serde_json::from_str::<RegistryErrorBody>(&body)
            .ok()
            .and_then(RegistryErrorBody::into_error)
            .and_then(|error| error.code)
            .filter(|code| {
                matches!(
                    code.as_str(),
                    "registration_conflict" | "authentication_service_unavailable"
                )
            });
        return Err(ExecServerError::EnvironmentRegistryHttp {
            status,
            code,
            message: "environment registry executor registration failed".to_string(),
        });
    }
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| ExecServerError::Protocol(SAFE_ERROR.to_string()))?;
    if body.contains(credential) || json_contains_executor_credential(&value, credential) {
        return Err(ExecServerError::Protocol(SAFE_ERROR.to_string()));
    }
    serde_json::from_value(value).map_err(|_| ExecServerError::Protocol(SAFE_ERROR.to_string()))
}

fn json_contains_executor_credential(value: &serde_json::Value, credential: &str) -> bool {
    match value {
        serde_json::Value::String(value) => value.contains(credential),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| json_contains_executor_credential(value, credential)),
        serde_json::Value::Object(fields) => fields.iter().any(|(key, value)| {
            key.contains(credential) || json_contains_executor_credential(value, credential)
        }),
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            false
        }
    }
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
        .and_then(RegistryErrorBody::into_error)
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
        .and_then(RegistryErrorBody::into_error)
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
    use codex_http_client::OutboundProxyPolicy;
    use http::HeaderMap;
    use http::HeaderValue;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use pretty_assertions::assert_eq;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tracing::Instrument;
    use tracing_subscriber::prelude::*;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_partial_json;
    use wiremock::matchers::header;
    use wiremock::matchers::header_regex;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use super::*;

    #[derive(Debug)]
    struct StaticRegistryAuthProvider;

    impl AuthProvider for StaticRegistryAuthProvider {
        fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
            Ok(())
        }

        fn apply_auth_headers(&self, mut headers: HeaderMap) -> codex_api::AuthHeadersFuture<'_> {
            Box::pin(async move {
                let _ = headers.insert(
                    http::header::AUTHORIZATION,
                    HeaderValue::from_static("Bearer registry-token"),
                );
                let _ = headers.insert(
                    "ChatGPT-Account-ID",
                    HeaderValue::from_static("workspace-123"),
                );
                Ok(headers)
            })
        }
    }

    fn static_registry_auth_provider() -> SharedAuthProvider {
        Arc::new(StaticRegistryAuthProvider)
    }

    struct ScopedExecutorAuth;

    impl AuthProvider for ScopedExecutorAuth {
        fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_static("Bearer synthetic-scoped-token"),
            );
            Ok(())
        }
    }

    #[tokio::test]
    async fn invalid_executor_credential_headers_are_never_sent() {
        struct InvalidExecutorAuth(HeaderMap);

        impl AuthProvider for InvalidExecutorAuth {
            fn add_auth_headers(
                &self,
                headers: &mut HeaderMap,
            ) -> Result<(), codex_api::AuthError> {
                headers.extend(self.0.clone());
                Ok(())
            }
        }

        let server = MockServer::start().await;
        let public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();
        for credentials in [
            vec![],
            vec![(AUTHORIZATION, HeaderValue::from_static("Bearer "))],
            vec![(
                AUTHORIZATION,
                HeaderValue::from_static("Basic synthetic-secret"),
            )],
            vec![(
                AUTHORIZATION,
                HeaderValue::from_bytes(b"Bearer synthetic-\xff-secret").expect("header value"),
            )],
            vec![
                (
                    AUTHORIZATION,
                    HeaderValue::from_static("Bearer synthetic-first-secret"),
                ),
                (
                    AUTHORIZATION,
                    HeaderValue::from_static("Bearer synthetic-second-secret"),
                ),
            ],
            vec![
                (
                    AUTHORIZATION,
                    HeaderValue::from_static("Bearer synthetic-secret"),
                ),
                (
                    HeaderName::from_static("chatgpt-account-id"),
                    HeaderValue::from_static("account-id"),
                ),
            ],
        ] {
            let mut headers = HeaderMap::new();
            for (name, value) in credentials {
                headers.append(name, value);
            }
            let mut client = EnvironmentRegistryClient::new(
                server.uri(),
                Arc::new(InvalidExecutorAuth(headers)),
            )
            .expect("registry client");
            client.auth_mode = RegistryAuthMode::ExecutorCredential;
            let registration = client
                .register_environment("environment-requested", &public_key)
                .await
                .expect_err("invalid credentials must be rejected");
            assert!(matches!(
                registration,
                ExecServerError::EnvironmentRegistryAuth(_)
            ));
            assert!(!format!("{registration:?} {registration}").contains("synthetic"));

            let validator = RegistryHarnessKeyValidator {
                client,
                environment_id: "environment-requested".to_string(),
                executor_registration_id: "registration".to_string(),
            };
            let validation = validator
                .validate_harness_key(
                    &public_key,
                    "harness-authorization",
                    /*stream_id*/ None,
                )
                .await
                .expect_err("invalid credentials must be rejected");
            assert!(matches!(
                validation,
                ExecServerError::EnvironmentRegistryAuth(_)
            ));
            assert!(!format!("{validation:?} {validation}").contains("synthetic"));
        }
        assert!(
            server
                .received_requests()
                .await
                .expect("requests")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn executor_credentials_require_https_or_loopback() {
        for (url, allowed) in [
            ("https://registry.example", true),
            ("http://localhost:8080", true),
            ("http://127.0.0.1:8080", true),
            ("http://[::1]:8080", true),
            ("http://registry.example", false),
            ("http://localhost.example", false),
            ("not a url", false),
        ] {
            let mut client =
                EnvironmentRegistryClient::new(url.to_string(), Arc::new(ScopedExecutorAuth))
                    .expect("registry client");
            client.auth_mode = RegistryAuthMode::ExecutorCredential;
            assert_eq!(client.resolve_auth().await.is_ok(), allowed, "{url}");
        }

        let legacy = EnvironmentRegistryClient::new(
            "http://registry.example".to_string(),
            static_registry_auth_provider(),
        )
        .expect("registry client");
        let (headers, credential) = legacy.resolve_auth().await.expect("legacy auth");
        assert_eq!(headers[AUTHORIZATION], "Bearer registry-token");
        assert_eq!(headers["ChatGPT-Account-ID"], "workspace-123");
        assert!(credential.is_none());
    }

    #[tokio::test]
    async fn insecure_registry_never_receives_executor_credentials() {
        let proxy = MockServer::start().await;
        let registry_url = "http://scoped-registry-proxy.invalid";
        for endpoint in ["register", "validate"] {
            let request_url =
                format!("{registry_url}/cloud/environment/environment-requested/{endpoint}");
            codex_http_client::cache_system_proxy_route_for_test(&request_url, proxy.uri());
        }
        let mut client = EnvironmentRegistryClient::new_with_telemetry(
            registry_url.to_string(),
            Arc::new(ScopedExecutorAuth),
            ExecServerTelemetry::default(),
            HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        )
        .expect("registry client");
        client.auth_mode = RegistryAuthMode::ExecutorCredential;
        let public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();
        let error = client
            .register_environment("environment-requested", &public_key)
            .await
            .expect_err("registration must reject a non-loopback HTTP registry");
        assert!(matches!(
            error,
            ExecServerError::EnvironmentRegistryConfig(_)
        ));

        let validator = RegistryHarnessKeyValidator {
            client,
            environment_id: "environment-requested".to_string(),
            executor_registration_id: "registration".to_string(),
        };
        let error = validator
            .validate_harness_key(
                &public_key,
                "harness-authorization",
                /*stream_id*/ None,
            )
            .await
            .expect_err("validation must reject a non-loopback HTTP registry");
        assert!(matches!(
            error,
            ExecServerError::EnvironmentRegistryConfig(_)
        ));
        assert!(
            proxy
                .received_requests()
                .await
                .expect("proxy requests")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn scoped_loopback_registry_bypasses_a_configured_proxy() {
        let registry = MockServer::start().await;
        let proxy = MockServer::start().await;
        let public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();
        for endpoint in ["register", "validate"] {
            let request_url = format!(
                "{}/cloud/environment/environment-requested/{endpoint}",
                registry.uri()
            );
            codex_http_client::cache_system_proxy_route_for_test(&request_url, proxy.uri());
        }
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/register"))
            .and(header(AUTHORIZATION, "Bearer synthetic-scoped-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "environment_id": "environment-requested",
                "url": "wss://rendezvous.test/",
                "security_profile": NOISE_RELAY_SECURITY_PROFILE,
                "executor_registration_id": "registration",
            })))
            .expect(1)
            .mount(&registry)
            .await;
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/validate"))
            .and(header(AUTHORIZATION, "Bearer synthetic-scoped-token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"valid": true})),
            )
            .expect(1)
            .mount(&registry)
            .await;
        let http_client_factory = HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy);
        assert!(matches!(
            http_client_factory.resolve_proxy_route(&format!(
                "{}/cloud/environment/environment-requested/register",
                registry.uri()
            )),
            codex_http_client::OutboundProxyRoute::Proxy { .. }
        ));
        let mut client = EnvironmentRegistryClient::new_with_telemetry(
            registry.uri(),
            Arc::new(ScopedExecutorAuth),
            ExecServerTelemetry::default(),
            http_client_factory,
        )
        .expect("registry client");
        client.auth_mode = RegistryAuthMode::ExecutorCredential;
        let (headers, credential) = client.resolve_auth().await.expect("scoped auth");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[AUTHORIZATION], "Bearer synthetic-scoped-token");
        assert!(headers[AUTHORIZATION].is_sensitive());
        let credential = credential.expect("raw executor credential");
        assert_eq!(credential, "synthetic-scoped-token");
        assert!(credential.is_sensitive());
        client
            .register_environment("environment-requested", &public_key)
            .await
            .expect("register directly");
        RegistryHarnessKeyValidator {
            client,
            environment_id: "environment-requested".to_string(),
            executor_registration_id: "registration".to_string(),
        }
        .validate_harness_key(
            &public_key,
            "harness-authorization",
            /*stream_id*/ None,
        )
        .await
        .expect("validate directly");
        registry.verify().await;
        assert!(
            proxy
                .received_requests()
                .await
                .expect("proxy requests")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn executor_credential_is_never_returned_in_registry_diagnostics() {
        const TOKEN: &str = "synthetic-scoped-registry-secret";
        struct ExecutorAuth;
        impl AuthProvider for ExecutorAuth {
            fn add_auth_headers(
                &self,
                headers: &mut HeaderMap,
            ) -> Result<(), codex_api::AuthError> {
                let mut credential =
                    HeaderValue::from_str(&format!("Bearer {TOKEN}")).expect("bearer");
                credential.set_sensitive(true);
                headers.insert(AUTHORIZATION, credential);
                Ok(())
            }
        }
        let encoded_echo = serde_json::json!({
            "environment_id": "environment-requested",
            "url": "wss://rendezvous.test/",
            "security_profile": NOISE_RELAY_SECURITY_PROFILE,
            "executor_registration_id": TOKEN,
        })
        .to_string()
        .replace("synthetic", "\\u0073ynthetic");
        for (status, body) in [
            (401, format!(r#"{{"error":{{"message":"{TOKEN}"}}}}"#)),
            (
                502,
                format!(
                    r#"{{"detail":{{"code":"authentication_service_unavailable","message":"{TOKEN}"}}}}"#
                ),
            ),
            (
                500,
                format!(r#"{{"error":{{"code":"{TOKEN}","message":"{TOKEN}"}}}}"#),
            ),
            (200, format!(r#"{{"environment_id":"{TOKEN}"}}"#)),
            (200, encoded_echo),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path("/cloud/environment/environment-requested/register"))
                .and(header(AUTHORIZATION, format!("Bearer {TOKEN}")))
                .respond_with(ResponseTemplate::new(status).set_body_string(body.clone()))
                .mount(&server)
                .await;
            Mock::given(method("POST"))
                .and(path("/cloud/environment/environment-requested/validate"))
                .and(header(AUTHORIZATION, format!("Bearer {TOKEN}")))
                .respond_with(
                    ResponseTemplate::new(status)
                        .set_body_string(format!(r#"{{"valid":"{TOKEN}"}}"#)),
                )
                .mount(&server)
                .await;
            let mut client = EnvironmentRegistryClient::new(server.uri(), Arc::new(ExecutorAuth))
                .expect("registry client");
            client.auth_mode = RegistryAuthMode::ExecutorCredential;
            let public_key = NoiseChannelIdentity::generate()
                .expect("identity")
                .public_key();
            let registration = client
                .register_environment("environment-requested", &public_key)
                .await
                .expect_err("registry echoes cannot enter diagnostics");
            assert!(!format!("{registration:?} {registration}").contains(TOKEN));
            if status == 502 {
                assert!(matches!(
                    &registration,
                    ExecServerError::EnvironmentRegistryHttp { code: Some(code), .. }
                        if code == "authentication_service_unavailable"
                ));
            }
            let validator = RegistryHarnessKeyValidator {
                client,
                environment_id: "environment-requested".to_string(),
                executor_registration_id: "registration".to_string(),
            };
            let validation = validator
                .validate_harness_key(
                    &public_key,
                    "harness-authorization",
                    /*stream_id*/ None,
                )
                .await
                .expect_err("invalid validation response");
            assert!(!format!("{validation:?} {validation}").contains(TOKEN));
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn register_environment_posts_with_auth_provider_headers() {
        let provider = SdkTracerProvider::builder().build();
        let tracer = provider.tracer("exec-server-test");
        let subscriber =
            tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
        let _guard = subscriber.set_default();
        tracing::callsite::rebuild_interest_cache();
        let server = MockServer::start().await;
        let executor_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/register"))
            .and(header("authorization", "Bearer registry-token"))
            .and(header("chatgpt-account-id", "workspace-123"))
            .and(header_regex(
                "traceparent",
                "^00-[0-9a-f]{32}-[0-9a-f]{16}-0[01]$",
            ))
            .and(body_partial_json(serde_json::json!({
                "security_profile": NOISE_RELAY_SECURITY_PROFILE,
                "executor_public_key": executor_public_key.clone(),
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "environment_id": "environment-requested",
                "url": "wss://rendezvous.test/cloud-agent/default/ws/environment/environment-requested?role=environment&sig=abc",
                "security_profile": NOISE_RELAY_SECURITY_PROFILE,
                "executor_registration_id": "registration-1",
            })))
            .mount(&server)
            .await;
        let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
            .expect("client");

        let response = client
            .register_environment("environment-requested", &executor_public_key)
            .instrument(tracing::info_span!("remote-operation"))
            .await
            .expect("register environment");

        assert_eq!(
            response,
            EnvironmentRegistryRegistrationResponse {
                environment_id: "environment-requested".to_string(),
                url: "wss://rendezvous.test/cloud-agent/default/ws/environment/environment-requested?role=environment&sig=abc".to_string(),
                security_profile: NOISE_RELAY_SECURITY_PROFILE.to_string(),
                executor_registration_id: "registration-1".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn noise_connect_provider_requests_and_validates_a_full_bundle() {
        let server = MockServer::start().await;
        let harness_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();
        let executor_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/connect"))
            .and(header("authorization", "Bearer registry-token"))
            .and(header("chatgpt-account-id", "workspace-123"))
            .and(body_partial_json(serde_json::json!({
                "harness_public_key": harness_public_key.clone(),
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "environment_id": "environment-requested",
                "url": "wss://rendezvous.test/cloud-agent/default/ws/environment/environment-requested?role=harness&sig=abc",
                "security_profile": NOISE_RELAY_SECURITY_PROFILE,
                "executor_registration_id": "registration-1",
                "executor_public_key": executor_public_key.clone(),
                "harness_key_authorization": "authorization-1",
            })))
            .mount(&server)
            .await;
        let config = NoiseRendezvousEnvironmentConfig::new(
            server.uri(),
            "environment-requested".to_string(),
            "registry-token".to_string(),
            Some("workspace-123".to_string()),
        )
        .expect("noise configuration");

        let bundle = config
            .into_connect_provider(HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ))
            .expect("Noise connect provider")
            .connect_bundle(harness_public_key)
            .await
            .expect("Noise connect bundle");

        assert_eq!(
            bundle.websocket_url,
            "wss://rendezvous.test/cloud-agent/default/ws/environment/environment-requested?role=harness&sig=abc"
        );
        assert_eq!(bundle.environment_id, "environment-requested");
        assert_eq!(bundle.executor_registration_id, "registration-1");
        assert_eq!(bundle.executor_public_key, executor_public_key);
        assert_eq!(bundle.harness_key_authorization, "authorization-1");
    }

    #[tokio::test]
    async fn connect_environment_times_out_when_registry_stalls() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/connect"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(1)))
            .mount(&server)
            .await;
        let mut client =
            EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
                .expect("client");
        client.connect_timeout = Duration::from_millis(50);
        let harness_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();

        let error = match client
            .connect_environment("environment-requested", harness_public_key)
            .await
        {
            Ok(_) => panic!("stalled connect response should time out"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ExecServerError::EnvironmentRegistryRequest(error) if error.is_timeout()
        ));
    }

    #[tokio::test]
    async fn connect_environment_times_out_when_registry_response_body_stalls() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("registry listener should bind");
        let registry_url = format!(
            "http://{}",
            listener
                .local_addr()
                .expect("registry listener should have an address")
        );
        tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("registry request should connect");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 256\r\n\r\n{",
                )
                .await
                .expect("registry response headers should write");
            sleep(Duration::from_secs(1)).await;
        });
        let mut client =
            EnvironmentRegistryClient::new(registry_url, static_registry_auth_provider())
                .expect("client");
        client.connect_timeout = Duration::from_millis(50);
        let harness_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();

        let error = match client
            .connect_environment("environment-requested", harness_public_key)
            .await
        {
            Ok(_) => panic!("stalled connect response body should time out"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ExecServerError::EnvironmentRegistryRequest(error) if error.is_timeout()
        ));
    }

    #[tokio::test]
    async fn connect_environment_retries_interrupted_registry_response_bodies() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("registry listener should bind");
        let registry_url = format!(
            "http://{}",
            listener
                .local_addr()
                .expect("registry listener should have an address")
        );
        tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("registry request should connect");
            let mut request = [0_u8; 4096];
            let bytes_read = stream
                .read(&mut request)
                .await
                .expect("registry request should arrive before the response");
            assert_ne!(bytes_read, 0, "registry request should not be empty");
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 256\r\n\r\n{",
                )
                .await
                .expect("registry response headers should write");
            stream
                .shutdown()
                .await
                .expect("registry connection should close");
        });
        let client = EnvironmentRegistryClient::new(registry_url, static_registry_auth_provider())
            .expect("client");
        let harness_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();

        let error = client
            .connect_environment("environment-requested", harness_public_key)
            .await
            .err()
            .expect("interrupted response body must fail");

        assert!(
            crate::client::is_retryable_registry_error(&error),
            "interrupted registry response body should be retryable: {error:?}"
        );
        assert!(matches!(
            error,
            ExecServerError::EnvironmentRegistryRequest(_)
        ));
    }

    #[tokio::test]
    async fn connect_environment_does_not_retry_malformed_successful_responses() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/connect"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{"))
            .mount(&server)
            .await;
        let client = EnvironmentRegistryClient::new(server.uri(), static_registry_auth_provider())
            .expect("client");
        let harness_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();

        let error = client
            .connect_environment("environment-requested", harness_public_key)
            .await
            .err()
            .expect("malformed response must fail");

        assert!(!crate::client::is_retryable_registry_error(&error));
        assert!(matches!(error, ExecServerError::Json(_)));
    }

    #[tokio::test]
    async fn register_environment_does_not_follow_redirects_with_auth_headers() {
        let server = MockServer::start().await;
        let executor_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();
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
            .register_environment("environment-requested", &executor_public_key)
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
    fn remote_environment_config_preserves_http_client_factory_policy() {
        let config = RemoteEnvironmentConfig::new(
            "https://registry.example".to_string(),
            "env-1".to_string(),
            static_registry_auth_provider(),
            HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        )
        .expect("config");

        assert_eq!(
            config.http_client_factory.outbound_proxy_policy(),
            OutboundProxyPolicy::RespectSystemProxy
        );
    }

    #[test]
    fn remote_environment_config_new_defaults_to_noise_transport() {
        let config = RemoteEnvironmentConfig::new(
            "https://registry.example".to_string(),
            "env-1".to_string(),
            static_registry_auth_provider(),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .expect("config");

        assert_eq!(config.transport, RemoteEnvironmentTransport::Noise);
    }

    #[test]
    fn remote_environment_config_new_with_transport_preserves_direct_transport() {
        let config = RemoteEnvironmentConfig::new_with_transport(
            "https://registry.example".to_string(),
            "env-1".to_string(),
            RemoteEnvironmentTransport::Direct,
            static_registry_auth_provider(),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .expect("config");

        assert_eq!(config.transport, RemoteEnvironmentTransport::Direct);
        assert!(matches!(
            config.with_executor_credential_auth(),
            Err(ExecServerError::EnvironmentRegistryConfig(_))
        ));
    }

    #[test]
    fn debug_output_redacts_auth_provider() {
        let config = RemoteEnvironmentConfig::new(
            "https://registry.example".to_string(),
            "env-1".to_string(),
            static_registry_auth_provider(),
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .expect("config");

        let debug = format!("{config:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("workspace-123"));
    }
}

#[cfg(test)]
#[path = "remote/noise_tests.rs"]
mod noise_tests;
