use std::time::Duration;

use codex_api::SharedAuthProvider;
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::relay::run_multiplexed_environment;
use crate::server::ConnectionProcessor;

mod noise;

const ERROR_BODY_PREVIEW_BYTES: usize = 4096;

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

    /// Register using the original body-less registry contract.
    ///
    /// This method intentionally preserves the legacy request shape and timeout
    /// behavior. Noise support is opt-in and must not silently alter existing
    /// remote exec-server registrations.
    async fn register_environment(
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

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
struct EnvironmentRegistryRegistrationResponse {
    environment_id: String,
    url: String,
}

/// Protocol used for an exec-server's registered remote relay.
///
/// Legacy is intentionally the default during rollout. Noise must be selected
/// explicitly so mixed-version deployments keep the original registry and relay
/// contract until both endpoints are ready for Noise.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RemoteRelayProtocol {
    /// Original cleartext JSON-RPC relay protocol.
    #[default]
    Legacy,
    /// Authenticated, end-to-end encrypted Noise relay protocol.
    Noise,
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
    if config.relay_protocol == RemoteRelayProtocol::Noise {
        return noise::run_remote_environment(&config, &client, processor).await;
    }

    let mut backoff = Duration::from_secs(1);

    loop {
        let response = client.register_environment(&config.environment_id).await?;
        eprintln!(
            "codex exec-server remote environment registered with environment_id {}",
            response.environment_id
        );

        match connect_async(response.url.as_str()).await {
            Ok((websocket, _)) => {
                backoff = Duration::from_secs(1);
                run_multiplexed_environment(websocket, processor.clone()).await;
            }
            Err(err) => {
                warn!("failed to connect remote exec-server websocket: {err}");
            }
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
