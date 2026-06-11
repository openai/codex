//! Noise registration and encrypted relay runtime.
//!
//! The registry returns the executor registration and rendezvous URL, and later
//! authorizes the harness key authenticated by Noise. The websocket only carries
//! routing metadata and ciphertext. Noise failures do not fall back to plaintext.

use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::connect_async_with_config;
use tracing::info;
use tracing::warn;

use super::EnvironmentRegistryClient;
use super::RemoteEnvironmentConfig;
use super::endpoint_url;
use crate::ExecServerError;
use crate::NoiseChannelIdentity;
use crate::NoiseChannelPublicKey;
use crate::noise_relay::environment::HarnessKeyValidator;
use crate::noise_relay::environment::run_noise_multiplexed_environment;
use crate::noise_relay::noise_relay_websocket_config;
use crate::server::ConnectionProcessor;

const ENVIRONMENT_REGISTRY_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_REMOTE_ENVIRONMENT_ID_LEN: usize = 256;
const NOISE_RELAY_SECURITY_PROFILE: &str = "noise_hybrid_ik_v1";
const REMOTE_RENDEZVOUS_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

impl EnvironmentRegistryClient {
    /// Register the executor public key and obtain the rendezvous allocation.
    /// The returned registration ID is included in each stream's Noise prologue.
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
            .timeout(ENVIRONMENT_REGISTRY_REQUEST_TIMEOUT)
            .json(&EnvironmentRegistryRegistrationRequest {
                security_profile: NOISE_RELAY_SECURITY_PROFILE,
                executor_public_key,
            })
            .send()
            .await?;
        self.parse_json_response(response).await
    }
}

#[derive(Serialize)]
struct EnvironmentRegistryRegistrationRequest<'a> {
    security_profile: &'static str,
    executor_public_key: &'a NoiseChannelPublicKey,
}

#[derive(Deserialize)]
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

/// Run the Noise registration and encrypted relay loop.
/// The executor identity is generated once per process and reused across
/// reconnects. Each reconnect gets a fresh registration and rendezvous URL.
pub(super) async fn run_remote_environment(
    config: &RemoteEnvironmentConfig,
    client: &EnvironmentRegistryClient,
    processor: ConnectionProcessor,
) -> Result<(), ExecServerError> {
    validate_environment_id(&config.environment_id)?;
    let identity = NoiseChannelIdentity::generate().map_err(|error| {
        ExecServerError::Protocol(format!("failed to generate Noise relay identity: {error}"))
    })?;
    let mut backoff = Duration::from_secs(1);

    loop {
        let response = client
            .register_noise_environment(&config.environment_id, &identity.public_key())
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
        let environment_id = &response.environment_id;
        info!(
            "codex exec-server Noise environment registered with environment_id {environment_id}"
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
            Ok(Err(err)) => warn!("failed to connect Noise remote exec-server websocket: {err}"),
            Err(_) => warn!("timed out connecting Noise remote exec-server websocket"),
        }

        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
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

#[cfg(test)]
#[path = "noise_tests.rs"]
mod tests;
