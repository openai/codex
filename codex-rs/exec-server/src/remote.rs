use std::collections::BTreeMap;
use std::env;
use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use sha2::Digest as _;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tracing::warn;
use uuid::Uuid;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::connection::JsonRpcConnection;
use crate::server::ConnectionProcessor;

pub const CODEX_EXEC_SERVER_REMOTE_BEARER_TOKEN_ENV_VAR: &str =
    "CODEX_EXEC_SERVER_REMOTE_BEARER_TOKEN";

const PROTOCOL_VERSION: &str = "codex-exec-server-v1";
const ERROR_BODY_PREVIEW_BYTES: usize = 4096;

#[derive(Clone)]
struct ExecutorRegistryClient {
    base_url: String,
    bearer_token: String,
    http: reqwest::Client,
}

impl std::fmt::Debug for ExecutorRegistryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorRegistryClient")
            .field("base_url", &self.base_url)
            .field("bearer_token", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl ExecutorRegistryClient {
    fn new(base_url: String, bearer_token: String) -> Result<Self, ExecServerError> {
        let base_url = normalize_base_url(base_url)?;
        Ok(Self {
            base_url,
            bearer_token,
            http: reqwest::Client::new(),
        })
    }

    async fn register_executor(
        &self,
        request: &ExecutorRegistryRegisterExecutorRequest,
    ) -> Result<ExecutorRegistryExecutorRegistrationResponse, ExecServerError> {
        self.post_json(
            &format!("/cloud/executor/{}/register", request.executor_id),
            request,
        )
        .await
    }

    async fn post_json<T, R>(&self, path: &str, request: &T) -> Result<R, ExecServerError>
    where
        T: Serialize + Sync,
        R: for<'de> Deserialize<'de>,
    {
        let response = self
            .http
            .post(endpoint_url(&self.base_url, path))
            .bearer_auth(&self.bearer_token)
            .json(request)
            .send()
            .await?;

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

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct ExecutorRegistryRegisterExecutorRequest {
    idempotency_id: String,
    executor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    labels: BTreeMap<String, String>,
    metadata: Value,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
struct ExecutorRegistryExecutorRegistrationResponse {
    id: String,
    executor_id: String,
    url: String,
}

/// Configuration for registering an exec-server for remote use.
#[derive(Clone, Eq, PartialEq)]
pub struct RemoteExecutorConfig {
    pub base_url: String,
    pub executor_id: String,
    pub name: String,
    bearer_token: String,
}

impl std::fmt::Debug for RemoteExecutorConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteExecutorConfig")
            .field("base_url", &self.base_url)
            .field("executor_id", &self.executor_id)
            .field("name", &self.name)
            .field("bearer_token", &"<redacted>")
            .finish()
    }
}

impl RemoteExecutorConfig {
    pub fn new(base_url: String, executor_id: String) -> Result<Self, ExecServerError> {
        Self::with_bearer_token(base_url, executor_id, read_remote_bearer_token_from_env()?)
    }

    fn with_bearer_token(
        base_url: String,
        executor_id: String,
        bearer_token: String,
    ) -> Result<Self, ExecServerError> {
        let executor_id = normalize_executor_id(executor_id)?;
        let bearer_token = normalize_bearer_token(bearer_token)?;
        Ok(Self {
            base_url,
            executor_id,
            name: "codex-exec-server".to_string(),
            bearer_token,
        })
    }

    fn registration_request(
        &self,
        registration_id: Uuid,
    ) -> ExecutorRegistryRegisterExecutorRequest {
        ExecutorRegistryRegisterExecutorRequest {
            idempotency_id: self.default_idempotency_id(registration_id),
            executor_id: self.executor_id.clone(),
            name: Some(self.name.clone()),
            labels: BTreeMap::new(),
            metadata: Value::Object(Default::default()),
        }
    }

    fn default_idempotency_id(&self, registration_id: Uuid) -> String {
        let mut hasher = sha2::Sha256::new();
        hasher.update(self.executor_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.name.as_bytes());
        hasher.update(b"\0");
        hasher.update(PROTOCOL_VERSION);
        hasher.update(b"\0");
        hasher.update(registration_id.as_bytes());
        let digest = hasher.finalize();
        format!("codex-exec-server-{digest:x}")
    }
}

/// Register an exec-server for remote use and serve requests over the returned
/// rendezvous websocket.
pub async fn run_remote_executor(
    config: RemoteExecutorConfig,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), ExecServerError> {
    let client = ExecutorRegistryClient::new(config.base_url.clone(), config.bearer_token.clone())?;
    let processor = ConnectionProcessor::new(runtime_paths);
    let registration_id = Uuid::new_v4();
    let mut backoff = Duration::from_secs(1);

    loop {
        let request = config.registration_request(registration_id);
        let response = client.register_executor(&request).await?;
        eprintln!(
            "codex exec-server remote executor {} registered with executor_id {}",
            response.id, response.executor_id
        );

        match connect_async(response.url.as_str()).await {
            Ok((websocket, _)) => {
                backoff = Duration::from_secs(1);
                processor
                    .run_connection(JsonRpcConnection::from_websocket(
                        websocket,
                        "remote exec-server websocket".to_string(),
                    ))
                    .await;
            }
            Err(err) => {
                warn!("failed to connect remote exec-server websocket: {err}");
            }
        }

        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

fn read_remote_bearer_token_from_env() -> Result<String, ExecServerError> {
    read_remote_bearer_token_from_env_with(|name| env::var(name))
}

fn read_remote_bearer_token_from_env_with<F>(get_var: F) -> Result<String, ExecServerError>
where
    F: FnOnce(&str) -> Result<String, env::VarError>,
{
    let bearer_token = get_var(CODEX_EXEC_SERVER_REMOTE_BEARER_TOKEN_ENV_VAR).map_err(|_| {
        ExecServerError::ExecutorRegistryAuth(format!(
            "executor registry bearer token environment variable `{CODEX_EXEC_SERVER_REMOTE_BEARER_TOKEN_ENV_VAR}` is not set"
        ))
    })?;
    normalize_bearer_token(bearer_token)
}

fn normalize_bearer_token(bearer_token: String) -> Result<String, ExecServerError> {
    let bearer_token = bearer_token.trim().to_string();
    if bearer_token.is_empty() {
        return Err(ExecServerError::ExecutorRegistryAuth(format!(
            "executor registry bearer token environment variable `{CODEX_EXEC_SERVER_REMOTE_BEARER_TOKEN_ENV_VAR}` is empty"
        )));
    }
    Ok(bearer_token)
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
    use serde_json::json;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use super::*;

    #[tokio::test]
    async fn register_executor_posts_with_bearer_token_header() {
        let server = MockServer::start().await;
        let registration_id = Uuid::from_u128(1);
        let config = RemoteExecutorConfig::with_bearer_token(
            server.uri(),
            "exec-requested".to_string(),
            "registry-token".to_string(),
        )
        .expect("config");
        let request = config.registration_request(registration_id);
        let expected_request = serde_json::to_value(&request).expect("serialize request");
        Mock::given(method("POST"))
            .and(path("/cloud/executor/exec-requested/register"))
            .and(header("authorization", "Bearer registry-token"))
            .and(body_json(expected_request))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "registration-1",
                "executor_id": "exec-1",
                "url": "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc"
            })))
            .mount(&server)
            .await;
        let client = ExecutorRegistryClient::new(server.uri(), "registry-token".to_string())
            .expect("client");

        let response = client
            .register_executor(&request)
            .await
            .expect("register executor");

        assert_eq!(
            response,
            ExecutorRegistryExecutorRegistrationResponse {
                id: "registration-1".to_string(),
                executor_id: "exec-1".to_string(),
                url: "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc".to_string(),
            }
        );
    }

    #[test]
    fn debug_output_redacts_bearer_token() {
        let config = RemoteExecutorConfig::with_bearer_token(
            "https://registry.example".to_string(),
            "exec-1".to_string(),
            "secret-token".to_string(),
        )
        .expect("config");

        let debug = format!("{config:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret-token"));
    }
}
