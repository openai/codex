use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use codex_login::AuthManager;
use codex_login::CodexAuth;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use sha2::Digest as _;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::connection::JsonRpcConnection;
use crate::server::ConnectionProcessor;

/// Environment variable fallback for the cloud environments base URL.
pub const CODEX_CLOUD_ENVIRONMENTS_BASE_URL_ENV_VAR: &str = "CODEX_CLOUD_ENVIRONMENTS_BASE_URL";

const PROTOCOL_VERSION: &str = "codex-exec-server-v1";
const ERROR_BODY_PREVIEW_BYTES: usize = 4096;

#[derive(Clone)]
struct CloudEnvironmentClient {
    base_url: String,
    http: reqwest::Client,
    auth_manager: Arc<AuthManager>,
}

impl std::fmt::Debug for CloudEnvironmentClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudEnvironmentClient")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl CloudEnvironmentClient {
    fn new(base_url: String, auth_manager: Arc<AuthManager>) -> Result<Self, ExecServerError> {
        let base_url = normalize_base_url(base_url)?;
        Ok(Self {
            base_url,
            http: reqwest::Client::new(),
            auth_manager,
        })
    }

    async fn register_executor(
        &self,
        request: &CloudEnvironmentRegisterExecutorRequest,
    ) -> Result<CloudEnvironmentExecutorRegistrationResponse, ExecServerError> {
        self.post_json("/api/cloud/executor", request).await
    }

    async fn post_json<T, R>(&self, path: &str, request: &T) -> Result<R, ExecServerError>
    where
        T: Serialize + Sync,
        R: for<'de> Deserialize<'de>,
    {
        for attempt in 0..=1 {
            let auth = cloud_environment_chatgpt_auth(&self.auth_manager).await?;
            let response = self
                .http
                .post(endpoint_url(&self.base_url, path))
                .bearer_auth(chatgpt_bearer_token(&auth)?)
                .header("chatgpt-account-id", chatgpt_account_id(&auth)?)
                .json(request)
                .send()
                .await?;

            if response.status().is_success() {
                return response.json::<R>().await.map_err(ExecServerError::from);
            }

            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
                && attempt == 0
                && recover_unauthorized(&self.auth_manager).await
            {
                continue;
            }

            return Err(cloud_http_error(status, &body));
        }

        unreachable!("cloud environments request loop is bounded to two attempts")
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
struct CloudEnvironmentRegisterExecutorRequest {
    idempotency_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    labels: BTreeMap<String, String>,
    metadata: Value,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
struct CloudEnvironmentExecutorRegistrationResponse {
    id: String,
    environment_id: String,
    url: String,
}

/// Configuration for registering an exec-server with cloud environments.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloudExecutorConfig {
    pub cloud_base_url: String,
    pub cloud_environment_id: Option<String>,
    pub cloud_name: String,
}

impl CloudExecutorConfig {
    pub fn new(cloud_base_url: String) -> Self {
        Self {
            cloud_base_url,
            cloud_environment_id: None,
            cloud_name: "codex-exec-server".to_string(),
        }
    }

    fn registration_request(
        &self,
        auth: &CodexAuth,
        registration_id: Uuid,
    ) -> Result<CloudEnvironmentRegisterExecutorRequest, ExecServerError> {
        Ok(CloudEnvironmentRegisterExecutorRequest {
            idempotency_id: self.default_idempotency_id(auth, registration_id)?,
            environment_id: self.cloud_environment_id.clone(),
            name: Some(self.cloud_name.clone()),
            labels: BTreeMap::new(),
            metadata: Value::Object(Default::default()),
        })
    }

    fn default_idempotency_id(
        &self,
        auth: &CodexAuth,
        registration_id: Uuid,
    ) -> Result<String, ExecServerError> {
        let mut hasher = sha2::Sha256::new();
        hasher.update(chatgpt_account_id(auth)?.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.cloud_environment_id.as_deref().unwrap_or("auto"));
        hasher.update(b"\0");
        hasher.update(self.cloud_name.as_bytes());
        hasher.update(b"\0");
        hasher.update(PROTOCOL_VERSION);
        hasher.update(b"\0");
        hasher.update(registration_id.as_bytes());
        let digest = hasher.finalize();
        Ok(format!("codex-exec-server-{digest:x}"))
    }
}

/// Register an exec-server with cloud environments and serve requests over the
/// returned rendezvous websocket.
pub async fn run_cloud_executor(
    config: CloudExecutorConfig,
    auth_manager: Arc<AuthManager>,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), ExecServerError> {
    let client = CloudEnvironmentClient::new(config.cloud_base_url.clone(), auth_manager.clone())?;
    let processor = ConnectionProcessor::new(runtime_paths);
    let registration_id = Uuid::new_v4();
    let mut backoff = Duration::from_secs(1);

    loop {
        let auth = cloud_environment_chatgpt_auth(&auth_manager).await?;
        let request = config.registration_request(&auth, registration_id)?;
        let response = client.register_executor(&request).await?;
        eprintln!(
            "codex exec-server cloud executor {} registered in environment {}",
            response.id, response.environment_id
        );

        match connect_async(response.url.as_str()).await {
            Ok((websocket, _)) => {
                backoff = Duration::from_secs(1);
                processor
                    .run_connection(JsonRpcConnection::from_websocket(
                        websocket,
                        "cloud exec-server websocket".to_string(),
                    ))
                    .await;
            }
            Err(err) => {
                warn!("failed to connect cloud exec-server websocket: {err}");
            }
        }

        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

async fn cloud_environment_chatgpt_auth(
    auth_manager: &AuthManager,
) -> Result<CodexAuth, ExecServerError> {
    let mut reloaded = false;
    let auth = loop {
        let Some(auth) = auth_manager.auth().await else {
            if reloaded {
                return Err(ExecServerError::CloudEnvironmentAuth(
                    "cloud environments require ChatGPT authentication".to_string(),
                ));
            }
            auth_manager.reload();
            reloaded = true;
            continue;
        };
        if !auth.is_chatgpt_auth() {
            return Err(ExecServerError::CloudEnvironmentAuth(
                "cloud environments require ChatGPT authentication; API key auth is not supported"
                    .to_string(),
            ));
        }
        if auth.get_account_id().is_none() && !reloaded {
            auth_manager.reload();
            reloaded = true;
            continue;
        }
        break auth;
    };

    let _ = chatgpt_bearer_token(&auth)?;
    let _ = chatgpt_account_id(&auth)?;
    Ok(auth)
}

fn chatgpt_bearer_token(auth: &CodexAuth) -> Result<String, ExecServerError> {
    auth.get_token()
        .map_err(|err| ExecServerError::CloudEnvironmentAuth(err.to_string()))
        .and_then(|token| {
            if token.is_empty() {
                Err(ExecServerError::CloudEnvironmentAuth(
                    "cloud environments require a non-empty ChatGPT bearer token".to_string(),
                ))
            } else {
                Ok(token)
            }
        })
}

fn chatgpt_account_id(auth: &CodexAuth) -> Result<String, ExecServerError> {
    auth.get_account_id().ok_or_else(|| {
        ExecServerError::CloudEnvironmentAuth(
            "cloud environments are waiting for a ChatGPT account id".to_string(),
        )
    })
}

async fn recover_unauthorized(auth_manager: &Arc<AuthManager>) -> bool {
    let mut recovery = auth_manager.unauthorized_recovery();
    if !recovery.has_next() {
        return false;
    }

    let mode = recovery.mode_name();
    let step = recovery.step_name();
    match recovery.next().await {
        Ok(step_result) => {
            info!(
                "cloud environment auth recovery succeeded: mode={mode}, step={step}, auth_state_changed={:?}",
                step_result.auth_state_changed()
            );
            true
        }
        Err(err) => {
            warn!("cloud environment auth recovery failed: mode={mode}, step={step}: {err}");
            false
        }
    }
}

#[derive(Deserialize)]
struct CloudErrorBody {
    error: Option<CloudError>,
}

#[derive(Deserialize)]
struct CloudError {
    code: Option<String>,
    message: Option<String>,
}

fn normalize_base_url(base_url: String) -> Result<String, ExecServerError> {
    let trimmed = base_url.trim().trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        return Err(ExecServerError::CloudEnvironmentConfig(
            "cloud environments base URL is required".to_string(),
        ));
    }
    Ok(trimmed)
}

fn endpoint_url(base_url: &str, path: &str) -> String {
    format!("{base_url}/{}", path.trim_start_matches('/'))
}

fn cloud_http_error(status: StatusCode, body: &str) -> ExecServerError {
    let parsed = serde_json::from_str::<CloudErrorBody>(body).ok();
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
    ExecServerError::CloudEnvironmentHttp {
        status,
        code,
        message,
    }
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
    use codex_login::CodexAuth;
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

    fn auth_manager() -> Arc<AuthManager> {
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing())
    }

    #[tokio::test]
    async fn register_executor_posts_with_chatgpt_auth_headers() {
        let server = MockServer::start().await;
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let registration_id = Uuid::from_u128(1);
        let request = CloudExecutorConfig::new(server.uri())
            .registration_request(&auth, registration_id)
            .expect("registration request");
        let expected_request = serde_json::to_value(&request).expect("serialize request");
        Mock::given(method("POST"))
            .and(path("/api/cloud/executor"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(expected_request))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "exec-1",
                "environment_id": "env-1",
                "url": "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc"
            })))
            .mount(&server)
            .await;
        let client = CloudEnvironmentClient::new(server.uri(), auth_manager()).expect("client");

        let response = client
            .register_executor(&request)
            .await
            .expect("register executor");

        assert_eq!(
            response,
            CloudEnvironmentExecutorRegistrationResponse {
                id: "exec-1".to_string(),
                environment_id: "env-1".to_string(),
                url: "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc".to_string(),
            }
        );
    }

    #[test]
    fn registration_idempotency_key_is_stable_within_process_and_unique_across_launches() {
        let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
        let config = CloudExecutorConfig::new("http://127.0.0.1:18084".to_string());
        let first_registration_id = Uuid::from_u128(1);
        let second_registration_id = Uuid::from_u128(2);

        let first = config
            .registration_request(&auth, first_registration_id)
            .expect("first registration");
        let repeated = config
            .registration_request(&auth, first_registration_id)
            .expect("repeated registration");
        let second = config
            .registration_request(&auth, second_registration_id)
            .expect("second registration");

        assert_eq!(first.idempotency_id, repeated.idempotency_id);
        assert_ne!(first.idempotency_id, second.idempotency_id);
    }
}
