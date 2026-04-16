use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use codex_client::CodexHttpClient;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::default_client::create_client;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use sha2::Digest as _;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tracing::info;
use tracing::warn;

use crate::ExecServerError;
use crate::ExecServerRuntimePaths;
use crate::connection::JsonRpcConnection;
use crate::server::ConnectionProcessor;

pub const CODEX_CLOUD_ENVIRONMENT_ID_ENV_VAR: &str = "CODEX_CLOUD_ENVIRONMENT_ID";
pub const CODEX_CLOUD_ENVIRONMENTS_BASE_URL_ENV_VAR: &str = "CODEX_CLOUD_ENVIRONMENTS_BASE_URL";

const PROTOCOL_VERSION: &str = "codex-exec-server-v1";
const ERROR_BODY_PREVIEW_BYTES: usize = 4096;

#[derive(Clone)]
pub struct CloudEnvironmentClient {
    base_url: String,
    http: CodexHttpClient,
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
    pub fn new(base_url: String, auth_manager: Arc<AuthManager>) -> Result<Self, ExecServerError> {
        let base_url = normalize_base_url(base_url)?;
        Ok(Self {
            base_url,
            http: create_client(),
            auth_manager,
        })
    }

    #[cfg(test)]
    fn endpoint_url(&self, path: &str) -> String {
        endpoint_url(&self.base_url, path)
    }

    pub async fn connect_environment(
        &self,
        environment_id: &str,
    ) -> Result<CloudEnvironmentConnectResponse, ExecServerError> {
        let path = format!("/api/cloud/environment/{environment_id}");
        self.post_json(&path, &EmptyRequest {}).await
    }

    pub async fn register_executor(
        &self,
        request: &CloudEnvironmentRegisterExecutorRequest,
    ) -> Result<CloudEnvironmentExecutorRegistrationResponse, ExecServerError> {
        self.post_json("/api/cloud/executor", request).await
    }

    pub async fn reconnect_executor(
        &self,
        executor_id: &str,
    ) -> Result<CloudEnvironmentExecutorRegistrationResponse, ExecServerError> {
        let path = format!("/api/cloud/executor/{executor_id}");
        self.post_json(&path, &EmptyRequest {}).await
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
pub struct CloudEnvironmentRegisterExecutorRequest {
    pub idempotency_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
pub struct CloudEnvironmentConnectResponse {
    pub environment_id: String,
    pub executor_id: String,
    pub url: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
pub struct CloudEnvironmentExecutorRegistrationResponse {
    pub id: String,
    pub environment_id: String,
    pub url: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloudExecutorConfig {
    pub cloud_base_url: String,
    pub cloud_environment_id: Option<String>,
    pub cloud_executor_id: Option<String>,
    pub cloud_idempotency_id: Option<String>,
    pub cloud_name: String,
    pub cloud_labels: BTreeMap<String, String>,
    pub cloud_metadata: Value,
}

impl CloudExecutorConfig {
    pub fn new(cloud_base_url: String) -> Self {
        Self {
            cloud_base_url,
            cloud_environment_id: None,
            cloud_executor_id: None,
            cloud_idempotency_id: None,
            cloud_name: default_executor_name(),
            cloud_labels: BTreeMap::new(),
            cloud_metadata: Value::Object(Default::default()),
        }
    }

    fn registration_request(
        &self,
        auth: &CodexAuth,
    ) -> Result<CloudEnvironmentRegisterExecutorRequest, ExecServerError> {
        let idempotency_id = match &self.cloud_idempotency_id {
            Some(idempotency_id) => idempotency_id.clone(),
            None => self.default_idempotency_id(auth)?,
        };

        Ok(CloudEnvironmentRegisterExecutorRequest {
            idempotency_id,
            environment_id: self.cloud_environment_id.clone(),
            name: Some(self.cloud_name.clone()),
            labels: self.cloud_labels.clone(),
            metadata: self.cloud_metadata.clone(),
        })
    }

    fn default_idempotency_id(&self, auth: &CodexAuth) -> Result<String, ExecServerError> {
        let mut hasher = sha2::Sha256::new();
        let account_id = chatgpt_account_id(auth)?;
        hasher.update(account_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(self.cloud_environment_id.as_deref().unwrap_or("auto"));
        hasher.update(b"\0");
        hasher.update(self.cloud_name.as_bytes());
        hasher.update(b"\0");
        hasher.update(serde_json::to_string(&self.cloud_labels).unwrap_or_default());
        hasher.update(b"\0");
        hasher.update(canonical_json(&self.cloud_metadata));
        hasher.update(b"\0");
        hasher.update(PROTOCOL_VERSION);
        let digest = hasher.finalize();
        Ok(format!("codex-exec-server-{digest:x}"))
    }
}

pub async fn run_cloud_executor(
    config: CloudExecutorConfig,
    auth_manager: Arc<AuthManager>,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), ExecServerError> {
    let client = CloudEnvironmentClient::new(config.cloud_base_url.clone(), auth_manager.clone())?;
    let processor = ConnectionProcessor::new(runtime_paths);
    let mut executor_id = config.cloud_executor_id.clone();
    let mut backoff = Duration::from_secs(1);

    loop {
        let signed_url = if let Some(existing_executor_id) = executor_id.as_deref() {
            let response = client.reconnect_executor(existing_executor_id).await?;
            executor_id = Some(response.id.clone());
            eprintln!(
                "codex exec-server cloud executor {} connected to environment {}",
                response.id, response.environment_id
            );
            response.url
        } else {
            let auth = cloud_environment_chatgpt_auth(&auth_manager).await?;
            let request = config.registration_request(&auth)?;
            let response = client.register_executor(&request).await?;
            executor_id = Some(response.id.clone());
            eprintln!(
                "codex exec-server cloud executor {} registered in environment {}",
                response.id, response.environment_id
            );
            response.url
        };

        ensure_rustls_crypto_provider();
        match connect_async(signed_url.as_str()).await {
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

#[derive(Serialize)]
struct EmptyRequest {}

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

fn default_executor_name() -> String {
    gethostname::gethostname()
        .to_str()
        .filter(|hostname| !hostname.is_empty())
        .unwrap_or("codex-exec-server")
        .to_string()
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let sorted = map
                .iter()
                .map(|(key, value)| (key, sorted_json_value(value)))
                .collect::<BTreeMap<_, _>>();
            serde_json::to_string(&sorted).unwrap_or_default()
        }
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn sorted_json_value(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(sorted_json_value).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| (key.clone(), sorted_json_value(value)))
                .collect(),
        ),
        value => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use codex_config::types::AuthCredentialsStoreMode;
    use codex_login::CodexAuth;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use tempfile::TempDir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use super::*;

    const TEST_ACCESS_TOKEN: &str = "test-access-token";
    const TEST_REFRESHED_ACCESS_TOKEN: &str = "test-refreshed-access-token";
    const TEST_ACCOUNT_ID: &str = "acct-1";

    fn auth_manager() -> Arc<AuthManager> {
        AuthManager::from_auth_for_testing(CodexAuth::create_dummy_chatgpt_auth_for_testing())
    }

    fn auth_manager_with_stored_chatgpt_auth() -> (TempDir, Arc<AuthManager>) {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        write_auth_json(codex_home.path(), TEST_ACCESS_TOKEN, TEST_ACCOUNT_ID);
        let auth_manager = AuthManager::shared(
            codex_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
        );
        (codex_home, auth_manager)
    }

    fn write_auth_json(codex_home: &std::path::Path, access_token: &str, account_id: &str) {
        let auth_json = json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": fake_jwt(account_id),
                "access_token": access_token,
                "refresh_token": "test-refresh-token",
                "account_id": account_id,
            },
            "last_refresh": "2999-01-01T00:00:00Z",
        });
        std::fs::write(
            codex_home.join("auth.json"),
            serde_json::to_string_pretty(&auth_json).expect("serialize auth json"),
        )
        .expect("write auth json");
    }

    fn fake_jwt(account_id: &str) -> String {
        let header = json!({
            "alg": "none",
            "typ": "JWT",
        });
        let payload = json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id,
                "chatgpt_user_id": "user-12345",
            },
        });
        let b64 = |value: &serde_json::Value| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(serde_json::to_vec(value).expect("serialize jwt part"))
        };
        let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"sig");
        format!("{}.{}.{}", b64(&header), b64(&payload), signature)
    }

    #[test]
    fn normalizes_base_url_and_builds_endpoints() {
        let client = CloudEnvironmentClient::new(
            "https://cloud.example.test/root/".to_string(),
            auth_manager(),
        )
        .expect("client");

        assert_eq!(
            client.endpoint_url("/api/cloud/environment/env-1"),
            "https://cloud.example.test/root/api/cloud/environment/env-1"
        );
    }

    #[test]
    fn cloud_response_serde_matches_service_shape() {
        let connect: CloudEnvironmentConnectResponse = serde_json::from_value(json!({
            "environment_id": "env-1",
            "executor_id": "exec-1",
            "url": "wss://rendezvous.test/executor/exec-1?role=harness&sig=abc"
        }))
        .expect("connect response");
        let registration: CloudEnvironmentExecutorRegistrationResponse =
            serde_json::from_value(json!({
                "id": "exec-1",
                "environment_id": "env-1",
                "url": "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc"
            }))
            .expect("registration response");

        assert_eq!(
            connect,
            CloudEnvironmentConnectResponse {
                environment_id: "env-1".to_string(),
                executor_id: "exec-1".to_string(),
                url: "wss://rendezvous.test/executor/exec-1?role=harness&sig=abc".to_string(),
            }
        );
        assert_eq!(
            registration,
            CloudEnvironmentExecutorRegistrationResponse {
                id: "exec-1".to_string(),
                environment_id: "env-1".to_string(),
                url: "wss://rendezvous.test/executor/exec-1?role=executor&sig=abc".to_string(),
            }
        );
    }

    #[test]
    fn cloud_error_body_is_preserved() {
        let err = cloud_http_error(
            StatusCode::CONFLICT,
            r#"{"error":{"code":"no_online_executor","message":"no executor is online"}}"#,
        );

        assert_eq!(
            err.to_string(),
            "cloud environments request failed (409 Conflict, no_online_executor): no executor is online"
        );
    }

    #[tokio::test]
    async fn connect_environment_posts_with_chatgpt_auth_headers() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/cloud/environment/env-1"))
            .and(header(
                "authorization",
                format!("Bearer {TEST_ACCESS_TOKEN}"),
            ))
            .and(header("chatgpt-account-id", TEST_ACCOUNT_ID))
            .and(body_json(json!({})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "environment_id": "env-1",
                "executor_id": "exec-1",
                "url": "ws://127.0.0.1:1234"
            })))
            .mount(&server)
            .await;
        let (_codex_home, auth_manager) = auth_manager_with_stored_chatgpt_auth();
        let client = CloudEnvironmentClient::new(server.uri(), auth_manager).expect("client");

        let response = client
            .connect_environment("env-1")
            .await
            .expect("connect environment");

        assert_eq!(
            response,
            CloudEnvironmentConnectResponse {
                environment_id: "env-1".to_string(),
                executor_id: "exec-1".to_string(),
                url: "ws://127.0.0.1:1234".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn retries_once_after_unauthorized_recovery() {
        let (codex_home, auth_manager) = auth_manager_with_stored_chatgpt_auth();
        write_auth_json(
            codex_home.path(),
            TEST_REFRESHED_ACCESS_TOKEN,
            TEST_ACCOUNT_ID,
        );
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/cloud/environment/env-1"))
            .and(header(
                "authorization",
                format!("Bearer {TEST_ACCESS_TOKEN}"),
            ))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "error": {
                    "code": "unauthorized",
                    "message": "expired token"
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/cloud/environment/env-1"))
            .and(header(
                "authorization",
                format!("Bearer {TEST_REFRESHED_ACCESS_TOKEN}"),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "environment_id": "env-1",
                "executor_id": "exec-1",
                "url": "ws://127.0.0.1:1234"
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = CloudEnvironmentClient::new(server.uri(), auth_manager).expect("client");

        client
            .connect_environment("env-1")
            .await
            .expect("connect environment");
    }
}
