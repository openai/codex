use std::time::Duration;

use codex_api::SharedAuthProvider;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::connect_async_with_config;
use tracing::info;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::NoiseChannelIdentity;
use crate::NoiseChannelPublicKey;
use crate::noise_relay::HarnessKeyValidator;
use crate::noise_relay::noise_relay_websocket_config;
use crate::noise_relay::run_noise_multiplexed_environment;
use crate::relay::run_multiplexed_environment;
use crate::server::ConnectionProcessor;

const ERROR_BODY_PREVIEW_BYTES: usize = 4096;
const ENVIRONMENT_REGISTRY_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REMOTE_ENVIRONMENT_ID_LEN: usize = 256;
const REMOTE_RENDEZVOUS_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

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

    /// Register using the original body-less registry contract.
    ///
    /// This remains the default so a new exec-server can still connect through
    /// registries and harnesses that have not rolled out Noise relay support.
    async fn register_legacy_environment(
        &self,
        environment_id: &str,
    ) -> Result<EnvironmentRegistryRegistrationResponse, ExecServerError> {
        let response = self
            .http
            .post(endpoint_url(
                &self.base_url,
                &format!("/cloud/environment/{environment_id}/register"),
            ))
            .headers(self.auth_provider.to_auth_headers())
            .send()
            .await?;
        self.parse_json_response(response).await
    }

    /// Register the exec-server's static Noise identity with a Noise-aware registry.
    ///
    /// Supplying this request body is the protocol-level opt in. Registries can
    /// therefore distinguish Noise registrations from the legacy body-less
    /// contract without guessing based on binary version or rollout state.
    async fn register_noise_environment(
        &self,
        environment_id: &str,
        executor_public_key: &NoiseChannelPublicKey,
    ) -> Result<EnvironmentRegistryNoiseRegistrationResponse, ExecServerError> {
        let response = self
            .http
            .post(endpoint_url(
                &self.base_url,
                &format!("/cloud/environment/{environment_id}/register"),
            ))
            .headers(self.auth_provider.to_auth_headers())
            .json(&EnvironmentRegistryRegistrationRequest {
                security_profile: NOISE_RELAY_SECURITY_PROFILE,
                executor_public_key,
            })
            .send()
            .await?;
        self.parse_json_response(response).await
    }

    async fn validate_harness_key(
        &self,
        environment_id: &str,
        executor_registration_id: &str,
        harness_public_key: &NoiseChannelPublicKey,
        harness_key_authorization: &str,
    ) -> Result<(), ExecServerError> {
        let response = self
            .http
            .post(endpoint_url(
                &self.base_url,
                &format!("/cloud/environment/{environment_id}/validate"),
            ))
            .headers(self.auth_provider.to_auth_headers())
            .json(&EnvironmentRegistryHarnessKeyValidationRequest {
                executor_registration_id,
                harness_public_key,
                harness_key_authorization,
            })
            .send()
            .await?;
        let response = self
            .parse_json_response::<EnvironmentRegistryHarnessKeyValidationResponse>(response)
            .await?;
        if !response.valid {
            return Err(ExecServerError::Protocol(
                "environment registry rejected Noise relay harness key".to_string(),
            ));
        }
        Ok(())
    }

    async fn parse_json_response<R>(
        &self,
        response: reqwest::Response,
    ) -> Result<R, ExecServerError>
    where
        R: for<'de> Deserialize<'de>,
    {
        if response.status().is_success() {
            return response.json::<R>().await.map_err(ExecServerError::from);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(environment_registry_auth_error(status, &body));
        }

        Err(environment_registry_http_error(status, &body))
    }
}

const NOISE_RELAY_SECURITY_PROFILE: &str = "noise_hybrid_ik_v1";

#[derive(Serialize)]
struct EnvironmentRegistryRegistrationRequest<'a> {
    security_profile: &'static str,
    executor_public_key: &'a NoiseChannelPublicKey,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
struct EnvironmentRegistryRegistrationResponse {
    environment_id: String,
    url: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
struct EnvironmentRegistryNoiseRegistrationResponse {
    environment_id: String,
    url: String,
    security_profile: String,
    executor_registration_id: String,
}

#[derive(Serialize)]
struct EnvironmentRegistryHarnessKeyValidationRequest<'a> {
    executor_registration_id: &'a str,
    harness_public_key: &'a NoiseChannelPublicKey,
    harness_key_authorization: &'a str,
}

#[derive(Deserialize)]
struct EnvironmentRegistryHarnessKeyValidationResponse {
    valid: bool,
}

#[derive(Clone)]
struct RegistryHarnessKeyValidator {
    client: EnvironmentRegistryClient,
    environment_id: String,
    executor_registration_id: String,
}

impl HarnessKeyValidator for RegistryHarnessKeyValidator {
    async fn validate_harness_key(
        &self,
        harness_public_key: &NoiseChannelPublicKey,
        authorization: &str,
    ) -> Result<(), ExecServerError> {
        self.client
            .validate_harness_key(
                &self.environment_id,
                &self.executor_registration_id,
                harness_public_key,
                authorization,
            )
            .await
    }
}

/// Protocol used for an exec-server's registered remote relay.
///
/// Legacy is intentionally the default during rollout. Noise must be selected
/// explicitly so mixed-version deployments keep the original registry and relay
/// contract until both endpoints are ready for Noise.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RemoteRelayProtocol {
    #[default]
    Legacy,
    Noise,
}

enum RemoteRelayProtocolState {
    Legacy,
    Noise(NoiseChannelIdentity),
}

enum RegisteredRemoteEnvironment {
    Legacy(EnvironmentRegistryRegistrationResponse),
    Noise {
        response: EnvironmentRegistryNoiseRegistrationResponse,
        identity: NoiseChannelIdentity,
    },
}

impl RegisteredRemoteEnvironment {
    fn environment_id(&self) -> &str {
        match self {
            Self::Legacy(response) => &response.environment_id,
            Self::Noise { response, .. } => &response.environment_id,
        }
    }

    fn websocket_url(&self) -> &str {
        match self {
            Self::Legacy(response) => &response.url,
            Self::Noise { response, .. } => &response.url,
        }
    }
}

/// Configuration for registering an exec-server for remote use.
#[derive(Clone)]
pub struct RemoteEnvironmentConfig {
    pub base_url: String,
    pub environment_id: String,
    pub name: String,
    pub relay_protocol: RemoteRelayProtocol,
    auth_provider: SharedAuthProvider,
}

impl std::fmt::Debug for RemoteEnvironmentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteEnvironmentConfig")
            .field("base_url", &self.base_url)
            .field("environment_id", &self.environment_id)
            .field("name", &self.name)
            .field("relay_protocol", &self.relay_protocol)
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
            relay_protocol: RemoteRelayProtocol::Legacy,
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
    let protocol_state = match config.relay_protocol {
        RemoteRelayProtocol::Legacy => RemoteRelayProtocolState::Legacy,
        RemoteRelayProtocol::Noise => {
            RemoteRelayProtocolState::Noise(NoiseChannelIdentity::generate()?)
        }
    };
    let mut backoff = Duration::from_secs(1);

    loop {
        let registration = match &protocol_state {
            RemoteRelayProtocolState::Legacy => RegisteredRemoteEnvironment::Legacy(
                client
                    .register_legacy_environment(&config.environment_id)
                    .await?,
            ),
            RemoteRelayProtocolState::Noise(identity) => {
                let response = client
                    .register_noise_environment(&config.environment_id, &identity.public_key())
                    .await?;
                if response.security_profile != NOISE_RELAY_SECURITY_PROFILE {
                    return Err(ExecServerError::Protocol(format!(
                        "environment registry returned unsupported security profile `{}`",
                        response.security_profile
                    )));
                }
                RegisteredRemoteEnvironment::Noise {
                    response,
                    identity: identity.clone(),
                }
            }
        };
        if registration.environment_id() != config.environment_id {
            return Err(ExecServerError::Protocol(
                "environment registry returned a different environment id".to_string(),
            ));
        }
        let environment_id = registration.environment_id();
        info!(
            "codex exec-server remote environment registered with environment_id {environment_id}"
        );
        let websocket_config = match &registration {
            RegisteredRemoteEnvironment::Legacy(_) => None,
            RegisteredRemoteEnvironment::Noise { .. } => Some(noise_relay_websocket_config()),
        };

        match timeout(
            REMOTE_RENDEZVOUS_CONNECT_TIMEOUT,
            connect_async_with_config(
                registration.websocket_url(),
                websocket_config,
                /*disable_nagle*/ false,
            ),
        )
        .await
        {
            Ok(Ok((websocket, _))) => {
                backoff = Duration::from_secs(1);
                match registration {
                    RegisteredRemoteEnvironment::Legacy(_) => {
                        run_multiplexed_environment(websocket, processor.clone()).await;
                    }
                    RegisteredRemoteEnvironment::Noise { response, identity } => {
                        run_noise_multiplexed_environment(
                            websocket,
                            processor.clone(),
                            response.environment_id,
                            response.executor_registration_id.clone(),
                            identity,
                            RegistryHarnessKeyValidator {
                                client: client.clone(),
                                environment_id: config.environment_id.clone(),
                                executor_registration_id: response.executor_registration_id,
                            },
                        )
                        .await;
                    }
                }
            }
            Ok(Err(err)) => {
                warn!("failed to connect remote exec-server websocket: {err}");
            }
            Err(_) => warn!("timed out connecting remote exec-server websocket"),
        }

        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

fn normalize_environment_id(environment_id: String) -> Result<String, ExecServerError> {
    if environment_id.is_empty() {
        return Err(ExecServerError::EnvironmentRegistryConfig(
            "environment id is required for remote exec-server registration".to_string(),
        ));
    }
    if environment_id.trim() != environment_id {
        return Err(ExecServerError::EnvironmentRegistryConfig(
            "environment id must not contain surrounding whitespace".to_string(),
        ));
    }
    if environment_id.len() > MAX_REMOTE_ENVIRONMENT_ID_LEN {
        return Err(ExecServerError::EnvironmentRegistryConfig(format!(
            "environment id cannot be longer than {MAX_REMOTE_ENVIRONMENT_ID_LEN} characters"
        )));
    }
    // The ID is interpolated into authenticated registry request paths below.
    // Keep it to one URL path component so a caller cannot use a delimiter to
    // redirect the exec-server's registration credential to another endpoint.
    if !environment_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(ExecServerError::EnvironmentRegistryConfig(
            "environment id must contain only ASCII letters, numbers, '-' or '_'".to_string(),
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
#[path = "remote_tests.rs"]
mod tests;
