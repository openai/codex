use std::sync::Arc;
use std::time::Duration;

use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use futures::FutureExt;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::time::sleep;
use tokio_tungstenite::connect_async_with_config;
use tracing::debug;
use tracing::info;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::EnvironmentRegistryConnectRequest;
use crate::EnvironmentRegistryConnectResponse;
use crate::EnvironmentRegistryHarnessKeyValidationRequest;
use crate::EnvironmentRegistryHarnessKeyValidationResponse;
use crate::EnvironmentRegistryRegistrationRequest;
use crate::EnvironmentRegistryRegistrationResponse;
use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::NoiseChannelIdentity;
use crate::NoiseChannelPublicKey;
use crate::NoiseRendezvousConnectBundle;
use crate::NoiseRendezvousConnectProvider;
use crate::noise_relay::noise_relay_websocket_config;
use crate::relay::HarnessKeyValidator;
use crate::relay::run_multiplexed_environment;
use crate::server::ConnectionProcessor;

const ERROR_BODY_PREVIEW_BYTES: usize = 4096;
const NOISE_RELAY_SECURITY_PROFILE: &str = "noise_hybrid_ik_v1";

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
            .json(&EnvironmentRegistryRegistrationRequest {
                security_profile: NOISE_RELAY_SECURITY_PROFILE.to_string(),
                executor_public_key: executor_public_key.clone(),
            })
            .send()
            .await?;
        let response: EnvironmentRegistryRegistrationResponse =
            self.parse_json_response(response).await?;
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
    async fn connect_environment(
        &self,
        environment_id: &str,
        harness_public_key: NoiseChannelPublicKey,
    ) -> Result<NoiseRendezvousConnectBundle, ExecServerError> {
        let response = self
            .http
            .post(endpoint_url(
                &self.base_url,
                &format!("/cloud/environment/{environment_id}/connect"),
            ))
            .headers(self.auth_provider.to_auth_headers())
            .json(&EnvironmentRegistryConnectRequest { harness_public_key })
            .send()
            .await?;
        let response: EnvironmentRegistryConnectResponse =
            self.parse_json_response(response).await?;
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
            .json(&EnvironmentRegistryHarnessKeyValidationRequest {
                executor_registration_id: self.executor_registration_id.clone(),
                harness_public_key: harness_public_key.clone(),
                harness_key_authorization: authorization.to_string(),
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

/// Noise connection configuration for a Codex harness.
///
/// The provider holds the authenticated registry client so every reconnect
/// receives fresh URL and harness-key authorization material.
#[derive(Clone)]
pub(crate) struct NoiseRendezvousEnvironmentConfig {
    provider: Arc<dyn NoiseRendezvousConnectProvider>,
}

impl std::fmt::Debug for NoiseRendezvousEnvironmentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseRendezvousEnvironmentConfig")
            .field("provider", &"<redacted>")
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
        let environment_id = normalize_environment_id(environment_id)?;
        let auth_provider = static_bearer_auth_provider(bearer_token, chatgpt_account_id)?;
        let client = EnvironmentRegistryClient::new(base_url, auth_provider)?;
        Ok(Self {
            provider: Arc::new(EnvironmentRegistryNoiseConnectProvider {
                client,
                environment_id,
            }),
        })
    }

    pub(crate) fn connect_provider(&self) -> Arc<dyn NoiseRendezvousConnectProvider> {
        Arc::clone(&self.provider)
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
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        headers.insert(http::header::AUTHORIZATION, self.authorization.clone());
        if let Some(chatgpt_account_id) = &self.chatgpt_account_id {
            headers.insert(
                HeaderName::from_static("chatgpt-account-id"),
                chatgpt_account_id.clone(),
            );
        }
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
/// reconnects. The registration and rendezvous URL are also reused until
/// rendezvous rejects the URL, at which point the next attempt registers again.
/// The websocket carries cleartext routing metadata and encrypted payloads.
pub async fn run_remote_environment(
    config: RemoteEnvironmentConfig,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), ExecServerError> {
    ensure_rustls_crypto_provider();
    let client =
        EnvironmentRegistryClient::new(config.base_url.clone(), config.auth_provider.clone())?;
    let processor = ConnectionProcessor::new(runtime_paths);
    let identity = NoiseChannelIdentity::generate().map_err(|error| {
        ExecServerError::Protocol(format!("failed to generate Noise relay identity: {error}"))
    })?;
    let mut backoff = Duration::from_secs(1);
    let mut response = client
        .register_environment(&config.environment_id, &identity.public_key())
        .await?;

    loop {
        match connect_async_with_config(
            response.url.as_str(),
            Some(noise_relay_websocket_config()),
            /*disable_nagle*/ false,
        )
        .await
        {
            Ok((websocket, _)) => {
                backoff = Duration::from_secs(1);
                let executor_registration_id = response.executor_registration_id.clone();
                info!(
                    noise_event = "rendezvous_connection",
                    noise_outcome = "ok",
                    "Noise executor connected to rendezvous"
                );
                run_multiplexed_environment(
                    websocket,
                    processor.clone(),
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
            }
            Err(error) => {
                let registration_rejected = matches!(
                    &error,
                    tokio_tungstenite::tungstenite::Error::Http(response)
                        if response.status().is_client_error()
                );
                warn!(
                    noise_event = "rendezvous_connection",
                    noise_outcome = "error",
                    noise_reason = "websocket_error",
                    "Noise executor failed to connect to rendezvous"
                );
                debug!(error = %error, "Noise executor rendezvous connection error");
                if registration_rejected {
                    response = client
                        .register_environment(&config.environment_id, &identity.public_key())
                        .await?;
                }
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_api::AuthProvider;
    use http::HeaderMap;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_partial_json;
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
        let executor_public_key = NoiseChannelIdentity::generate()
            .expect("identity")
            .public_key();
        Mock::given(method("POST"))
            .and(path("/cloud/environment/environment-requested/register"))
            .and(header("authorization", "Bearer registry-token"))
            .and(header("chatgpt-account-id", "workspace-123"))
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
            .connect_provider()
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
