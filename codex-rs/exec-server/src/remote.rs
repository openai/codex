use std::env;
use std::time::Duration;

use codex_login::CodexAuth;
use codex_model_provider::auth_provider_from_auth;
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tracing::warn;

use codex_utils_rustls_provider::ensure_rustls_crypto_provider;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::relay::run_multiplexed_executor;
use crate::server::ConnectionProcessor;

pub const CODEX_EXEC_SERVER_REMOTE_AGENT_IDENTITY_JWT_ENV_VAR: &str =
    "CODEX_EXEC_SERVER_REMOTE_AGENT_IDENTITY_JWT";

const ERROR_BODY_PREVIEW_BYTES: usize = 4096;

#[derive(Clone)]
struct ExecutorRegistryClient {
    base_url: String,
    auth: ExecutorRegistryAuth,
    http: reqwest::Client,
}

#[derive(Clone)]
enum ExecutorRegistryAuth {
    AgentIdentity(CodexAuth),
    StaticHeaders(reqwest::header::HeaderMap),
}

impl std::fmt::Debug for ExecutorRegistryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorRegistryClient")
            .field("base_url", &self.base_url)
            .field("agent_identity_jwt", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl ExecutorRegistryClient {
    async fn new(base_url: String, agent_identity_jwt: String) -> Result<Self, ExecServerError> {
        let base_url = normalize_base_url(base_url)?;
        let auth = CodexAuth::from_agent_identity_jwt(&agent_identity_jwt, None)
            .await
            .map_err(|err| {
                ExecServerError::ExecutorRegistryAuth(format!(
                    "failed to load executor registry Agent Identity JWT: {err}"
                ))
            })?;
        Ok(Self {
            base_url,
            auth: ExecutorRegistryAuth::AgentIdentity(auth),
            http: reqwest::Client::new(),
        })
    }

    fn with_static_headers(
        base_url: String,
        headers: Vec<(String, String)>,
    ) -> Result<Self, ExecServerError> {
        let mut normalized_headers = reqwest::header::HeaderMap::new();
        for (name, value) in headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|err| {
                    ExecServerError::ExecutorRegistryConfig(format!(
                        "invalid executor registry test header name `{name}`: {err}"
                    ))
                })?;
            let header_value =
                reqwest::header::HeaderValue::from_bytes(value.as_bytes()).map_err(|err| {
                    ExecServerError::ExecutorRegistryConfig(format!(
                        "invalid executor registry test header value for `{name}`: {err}"
                    ))
                })?;
            normalized_headers.insert(header_name, header_value);
        }
        Ok(Self {
            base_url: normalize_base_url(base_url)?,
            auth: ExecutorRegistryAuth::StaticHeaders(normalized_headers),
            http: reqwest::Client::new(),
        })
    }

    async fn register_executor(
        &self,
        executor_id: &str,
    ) -> Result<ExecutorRegistryExecutorRegistrationResponse, ExecServerError> {
        let response = self
            .http
            .post(endpoint_url(
                &self.base_url,
                &format!("/cloud/executor/{executor_id}/register"),
            ))
            .headers(self.registry_auth_headers())
            .send()
            .await?;
        self.parse_json_response(response).await
    }

    fn registry_auth_headers(&self) -> reqwest::header::HeaderMap {
        match &self.auth {
            ExecutorRegistryAuth::AgentIdentity(auth) => {
                auth_provider_from_auth(auth).to_auth_headers()
            }
            ExecutorRegistryAuth::StaticHeaders(headers) => headers.clone(),
        }
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
            return Err(executor_registry_auth_error(status, &body));
        }

        Err(executor_registry_http_error(status, &body))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
struct ExecutorRegistryExecutorRegistrationResponse {
    executor_id: String,
    url: String,
}

/// Configuration for registering an exec-server for remote use.
#[derive(Clone, Eq, PartialEq)]
pub struct RemoteExecutorConfig {
    pub base_url: String,
    pub executor_id: String,
    pub name: String,
    agent_identity_jwt: String,
    test_registration_headers: Option<Vec<(String, String)>>,
}

impl std::fmt::Debug for RemoteExecutorConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteExecutorConfig")
            .field("base_url", &self.base_url)
            .field("executor_id", &self.executor_id)
            .field("name", &self.name)
            .field("agent_identity_jwt", &"<redacted>")
            .field(
                "test_registration_headers",
                &self
                    .test_registration_headers
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl RemoteExecutorConfig {
    pub fn new(base_url: String, executor_id: String) -> Result<Self, ExecServerError> {
        Self::with_agent_identity_jwt(
            base_url,
            executor_id,
            read_remote_agent_identity_jwt_from_env()?,
        )
    }

    pub fn with_agent_identity_jwt(
        base_url: String,
        executor_id: String,
        agent_identity_jwt: String,
    ) -> Result<Self, ExecServerError> {
        let executor_id = normalize_executor_id(executor_id)?;
        let agent_identity_jwt = normalize_agent_identity_jwt(agent_identity_jwt)?;
        Ok(Self {
            base_url,
            executor_id,
            name: "codex-exec-server".to_string(),
            agent_identity_jwt,
            test_registration_headers: None,
        })
    }

    #[doc(hidden)]
    pub fn with_registration_headers_for_tests(
        base_url: String,
        executor_id: String,
        headers: Vec<(String, String)>,
    ) -> Result<Self, ExecServerError> {
        let executor_id = normalize_executor_id(executor_id)?;
        Ok(Self {
            base_url,
            executor_id,
            name: "codex-exec-server".to_string(),
            agent_identity_jwt: String::new(),
            test_registration_headers: Some(headers),
        })
    }
}

/// Register an exec-server for remote use and serve requests over the returned
/// rendezvous websocket.
pub async fn run_remote_executor(
    config: RemoteExecutorConfig,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), ExecServerError> {
    ensure_rustls_crypto_provider();
    let client = if let Some(headers) = config.test_registration_headers.clone() {
        ExecutorRegistryClient::with_static_headers(config.base_url.clone(), headers)?
    } else {
        ExecutorRegistryClient::new(config.base_url.clone(), config.agent_identity_jwt.clone())
            .await?
    };
    let processor = ConnectionProcessor::new(runtime_paths);
    let mut backoff = Duration::from_secs(1);

    loop {
        let response = client.register_executor(&config.executor_id).await?;
        eprintln!(
            "codex exec-server remote executor registered with executor_id {}",
            response.executor_id
        );

        match connect_async(response.url.as_str()).await {
            Ok((websocket, _)) => {
                backoff = Duration::from_secs(1);
                run_multiplexed_executor(websocket, processor.clone()).await;
            }
            Err(err) => {
                warn!("failed to connect remote exec-server websocket: {err}");
            }
        }

        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

fn read_remote_agent_identity_jwt_from_env() -> Result<String, ExecServerError> {
    read_remote_agent_identity_jwt_from_env_with(|name| env::var(name))
}

fn read_remote_agent_identity_jwt_from_env_with<F>(get_var: F) -> Result<String, ExecServerError>
where
    F: FnOnce(&str) -> Result<String, env::VarError>,
{
    let agent_identity_jwt = get_var(CODEX_EXEC_SERVER_REMOTE_AGENT_IDENTITY_JWT_ENV_VAR)
        .map_err(|_| {
            ExecServerError::ExecutorRegistryAuth(format!(
                "executor registry Agent Identity JWT environment variable `{CODEX_EXEC_SERVER_REMOTE_AGENT_IDENTITY_JWT_ENV_VAR}` is not set"
            ))
        })?;
    normalize_agent_identity_jwt(agent_identity_jwt)
}

fn normalize_agent_identity_jwt(agent_identity_jwt: String) -> Result<String, ExecServerError> {
    let agent_identity_jwt = agent_identity_jwt.trim().to_string();
    if agent_identity_jwt.is_empty() {
        return Err(ExecServerError::ExecutorRegistryAuth(format!(
            "executor registry Agent Identity JWT environment variable `{CODEX_EXEC_SERVER_REMOTE_AGENT_IDENTITY_JWT_ENV_VAR}` is empty"
        )));
    }
    Ok(agent_identity_jwt)
}

fn normalize_executor_id(executor_id: String) -> Result<String, ExecServerError> {
    let executor_id = executor_id.trim().to_string();
    if executor_id.is_empty() {
        return Err(ExecServerError::ExecutorRegistryConfig(
            "executor id is required for remote exec-server registration".to_string(),
        ));
    }
    Ok(executor_id)
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
        return Err(ExecServerError::ExecutorRegistryConfig(
            "executor registry base URL is required".to_string(),
        ));
    }
    Ok(trimmed)
}

fn endpoint_url(base_url: &str, path: &str) -> String {
    format!("{base_url}/{}", path.trim_start_matches('/'))
}

fn executor_registry_auth_error(status: StatusCode, body: &str) -> ExecServerError {
    let message = registry_error_message(body).unwrap_or_else(|| "empty error body".to_string());
    ExecServerError::ExecutorRegistryAuth(format!(
        "executor registry authentication failed ({status}): {message}"
    ))
}

fn executor_registry_http_error(status: StatusCode, body: &str) -> ExecServerError {
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
    ExecServerError::ExecutorRegistryHttp {
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
    use pretty_assertions::assert_eq;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use super::*;

    #[tokio::test]
    async fn register_executor_posts_with_agent_assertion_headers() {
        let server = MockServer::start().await;
        let config = RemoteExecutorConfig::with_agent_identity_jwt(
            server.uri(),
            "exec-requested".to_string(),
            "agent-identity-jwt".to_string(),
        )
        .expect("config");
        Mock::given(method("POST"))
            .and(path("/cloud/executor/exec-requested/register"))
            .and(header("authorization", "AgentAssertion registry-assertion"))
            .and(header("chatgpt-account-id", "account-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "executor_id": "exec-1",
                "url": "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc"
            })))
            .mount(&server)
            .await;
        let client = ExecutorRegistryClient::with_static_headers(
            server.uri(),
            vec![
                (
                    "Authorization".to_string(),
                    "AgentAssertion registry-assertion".to_string(),
                ),
                ("ChatGPT-Account-ID".to_string(), "account-123".to_string()),
            ],
        )
        .expect("client");

        let response = client
            .register_executor(&config.executor_id)
            .await
            .expect("register executor");

        assert_eq!(
            response,
            ExecutorRegistryExecutorRegistrationResponse {
                executor_id: "exec-1".to_string(),
                url: "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc".to_string(),
            }
        );
    }

    #[test]
    fn debug_output_redacts_agent_identity_jwt() {
        let config = RemoteExecutorConfig::with_agent_identity_jwt(
            "https://registry.example".to_string(),
            "exec-1".to_string(),
            "secret-agent-identity-jwt".to_string(),
        )
        .expect("config");

        let debug = format!("{config:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret-agent-identity-jwt"));
    }
}
