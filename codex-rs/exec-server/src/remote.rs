use std::time::Duration;

use codex_api::SharedAuthProvider;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::connect_async_with_config;
use tracing::debug;
use tracing::info;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::NoiseChannelIdentity;
use crate::NoiseChannelPublicKey;
use crate::noise_relay::environment::HarnessKeyValidator;
use crate::noise_relay::environment::run_noise_multiplexed_environment;
use crate::noise_relay::noise_relay_websocket_config;
use crate::server::ConnectionProcessor;

const ERROR_BODY_PREVIEW_BYTES: usize = 4096;
const ENVIRONMENT_REGISTRY_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REMOTE_ENVIRONMENT_ID_LEN: usize = 256;
const NOISE_RELAY_SECURITY_PROFILE: &str = "noise_hybrid_ik_v1";
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
                .build()?,
        })
    }

    /// Register the executor public key and obtain the rendezvous allocation.
    /// The returned registration ID is included in each stream's Noise prologue.
    async fn register_environment(
        &self,
        environment_id: &str,
        executor_public_key: &NoiseChannelPublicKey,
    ) -> Result<EnvironmentRegistryRegistrationResponse, ExecServerError> {
        let response = self
            .http
            .post(endpoint_url(
                &self.base_url,
                &format!("/cloud/environment/{environment_id}/register"),
            ))
            .headers(self.auth_provider.to_auth_headers())
            .timeout(ENVIRONMENT_REGISTRY_REQUEST_TIMEOUT)
            .json(&EnvironmentRegistryRegistrationRequest {
                security_profile: NOISE_RELAY_SECURITY_PROFILE,
                executor_public_key,
            })
            .send()
            .await?;
        self.parse_json_response(response).await
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

#[derive(Serialize)]
struct EnvironmentRegistryRegistrationRequest<'a> {
    security_profile: &'static str,
    executor_public_key: &'a NoiseChannelPublicKey,
}

#[derive(Deserialize)]
struct EnvironmentRegistryRegistrationResponse {
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
    /// Authorize the harness key recovered from the first IK message.
    /// Noise proves key possession; the registry decides whether that key may use
    /// this executor. The authorization token and public key are checked together.
    async fn validate_harness_key(
        &self,
        harness_public_key: &NoiseChannelPublicKey,
        authorization: &str,
    ) -> Result<(), ExecServerError> {
        let environment_id = &self.environment_id;
        let response = self
            .client
            .http
            .post(endpoint_url(
                &self.client.base_url,
                &format!("/cloud/environment/{environment_id}/validate"),
            ))
            .headers(self.client.auth_provider.to_auth_headers())
            .timeout(ENVIRONMENT_REGISTRY_REQUEST_TIMEOUT)
            .json(&EnvironmentRegistryHarnessKeyValidationRequest {
                executor_registration_id: &self.executor_registration_id,
                harness_public_key,
                harness_key_authorization: authorization,
            })
            .send()
            .await?;
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
            .await?;
        if !response.valid {
            return Err(ExecServerError::Protocol(
                "environment registry rejected Noise relay harness key".to_string(),
            ));
        }
        Ok(())
    }
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

/// Register an exec-server for remote use and serve requests over Noise.
///
/// The executor identity is generated once per process and reused across
/// reconnects. Each reconnect gets a fresh registration and rendezvous URL.
/// The websocket carries cleartext routing metadata and encrypted payloads.
pub async fn run_remote_environment(
    config: RemoteEnvironmentConfig,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), ExecServerError> {
    ensure_rustls_crypto_provider();
    let client =
        EnvironmentRegistryClient::new(config.base_url.clone(), config.auth_provider.clone())?;
    let processor = ConnectionProcessor::new(runtime_paths);
    validate_environment_id(&config.environment_id)?;
    let identity = NoiseChannelIdentity::generate().map_err(|error| {
        ExecServerError::Protocol(format!("failed to generate Noise relay identity: {error}"))
    })?;
    let mut backoff = Duration::from_secs(1);

    loop {
        let response = client
            .register_environment(&config.environment_id, &identity.public_key())
            .await?;
        if response.environment_id != config.environment_id {
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

        match timeout(
            REMOTE_RENDEZVOUS_CONNECT_TIMEOUT,
            connect_async_with_config(
                response.url.as_str(),
                Some(noise_relay_websocket_config()),
                /*disable_nagle*/ false,
            ),
        )
        .await
        {
            Ok(Ok((websocket, _))) => {
                backoff = Duration::from_secs(1);
                let executor_registration_id = response.executor_registration_id;
                info!(
                    noise_event = "rendezvous_connection",
                    noise_outcome = "ok",
                    "Noise executor connected to rendezvous"
                );
                run_noise_multiplexed_environment(
                    websocket,
                    processor.clone(),
                    response.environment_id,
                    executor_registration_id.clone(),
                    identity.clone(),
                    RegistryHarnessKeyValidator {
                        client: client.clone(),
                        environment_id: config.environment_id.clone(),
                        executor_registration_id,
                    },
                )
                .await;
            }
            Ok(Err(error)) => {
                warn!(
                    noise_event = "rendezvous_connection",
                    noise_outcome = "error",
                    noise_reason = "websocket_error",
                    "Noise executor failed to connect to rendezvous"
                );
                debug!(error = %error, "Noise executor rendezvous connection error");
            }
            Err(_) => warn!(
                noise_event = "rendezvous_connection",
                noise_outcome = "error",
                noise_reason = "timeout",
                "Noise executor rendezvous connection timed out"
            ),
        }

        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
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

fn validate_environment_id(environment_id: &str) -> Result<(), ExecServerError> {
    if environment_id.is_empty() {
        return Err(ExecServerError::EnvironmentRegistryConfig(
            "environment id is required for Noise registration".to_string(),
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
    Ok(())
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

#[cfg(test)]
#[path = "remote/noise_tests.rs"]
mod noise_tests;
