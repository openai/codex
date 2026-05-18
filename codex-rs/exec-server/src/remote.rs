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
use crate::relay::run_multiplexed_executor;
use crate::server::ConnectionProcessor;

const ERROR_BODY_PREVIEW_BYTES: usize = 4096;

#[derive(Clone)]
struct ExecutorRegistryClient {
    base_url: String,
    auth_provider: SharedAuthProvider,
    http: reqwest::Client,
}

impl std::fmt::Debug for ExecutorRegistryClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutorRegistryClient")
            .field("base_url", &self.base_url)
            .field("auth_provider", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl ExecutorRegistryClient {
    fn new(base_url: String, auth_provider: SharedAuthProvider) -> Result<Self, ExecServerError> {
        let base_url = normalize_base_url(base_url)?;
        Ok(Self {
            base_url,
            auth_provider,
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
#[derive(Clone)]
pub struct RemoteExecutorConfig {
    pub base_url: String,
    pub executor_id: String,
    pub name: String,
    auth_provider: SharedAuthProvider,
}

impl std::fmt::Debug for RemoteExecutorConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteExecutorConfig")
            .field("base_url", &self.base_url)
            .field("executor_id", &self.executor_id)
            .field("name", &self.name)
            .field("auth_provider", &"<redacted>")
            .finish()
    }
}

impl RemoteExecutorConfig {
    pub fn new(
        base_url: String,
        executor_id: String,
        auth_provider: SharedAuthProvider,
    ) -> Result<Self, ExecServerError> {
        let executor_id = normalize_executor_id(executor_id)?;
        Ok(Self {
            base_url,
            executor_id,
            name: "codex-exec-server".to_string(),
            auth_provider,
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
    let client =
        ExecutorRegistryClient::new(config.base_url.clone(), config.auth_provider.clone())?;
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
    use std::sync::Arc;

    use codex_api::AuthProvider;
    use http::HeaderMap;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;
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
    async fn register_executor_posts_with_auth_provider_headers() {
        let server = MockServer::start().await;
        let config = RemoteExecutorConfig::new(
            server.uri(),
            "exec-requested".to_string(),
            static_registry_auth_provider(),
        )
        .expect("config");
        Mock::given(method("POST"))
            .and(path("/cloud/executor/exec-requested/register"))
            .and(header("authorization", "Bearer registry-token"))
            .and(header("chatgpt-account-id", "workspace-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "executor_id": "exec-1",
                "url": "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc"
            })))
            .mount(&server)
            .await;
        let client = ExecutorRegistryClient::new(server.uri(), static_registry_auth_provider())
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
    fn debug_output_redacts_auth_provider() {
        let config = RemoteExecutorConfig::new(
            "https://registry.example".to_string(),
            "exec-1".to_string(),
            static_registry_auth_provider(),
        )
        .expect("config");

        let debug = format!("{config:?}");

        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("workspace-123"));
    }
}
