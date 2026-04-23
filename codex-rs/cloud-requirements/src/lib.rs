//! Cloud-hosted config requirements for Codex.
//!
//! This crate fetches `requirements.toml` data from the backend as an alternative to loading it
//! from the local filesystem. It only applies to Business (aka Enterprise CBP) or Enterprise ChatGPT
//! customers.
//!
//! Fetching fails closed for eligible ChatGPT Business and Enterprise accounts. When cloud
//! requirements cannot be loaded for those accounts, Codex fails configuration loading rather than
//! continuing without them.

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use codex_backend_client::Client as BackendClient;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::config_loader::CloudRequirementsLoadError;
use codex_core::config_loader::CloudRequirementsLoadErrorCode;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigRequirementsToml;
use codex_core::util::backoff;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::RefreshTokenError;
use codex_protocol::account::PlanType;
use hmac::Hmac;
use hmac::Mac;
use serde::Deserialize;
use serde::Serialize;
use sha2::Sha256;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;
use tokio::fs;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::timeout;

const CLOUD_REQUIREMENTS_TIMEOUT: Duration = Duration::from_secs(15);
const CLOUD_REQUIREMENTS_MAX_ATTEMPTS: usize = 5;
const CLOUD_REQUIREMENTS_CACHE_FILENAME: &str = "cloud-requirements-cache.json";
const CLOUD_REQUIREMENTS_CACHE_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const CLOUD_REQUIREMENTS_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const CLOUD_REQUIREMENTS_FETCH_ATTEMPT_METRIC: &str = "codex.cloud_requirements.fetch_attempt";
const CLOUD_REQUIREMENTS_FETCH_FINAL_METRIC: &str = "codex.cloud_requirements.fetch_final";
const CLOUD_REQUIREMENTS_LOAD_METRIC: &str = "codex.cloud_requirements.load";
const CLOUD_REQUIREMENTS_LOAD_FAILED_MESSAGE: &str =
    "Failed to load cloud requirements (workspace-managed policies).";
const CLOUD_REQUIREMENTS_PARSE_FAILED_MESSAGE: &str = concat!(
    "Cloud requirements (workspace-managed policies) are invalid and could not be parsed. ",
    "Please contact your workspace admin."
);
const CLOUD_REQUIREMENTS_AUTH_RECOVERY_FAILED_MESSAGE: &str = concat!(
    "Your authentication session could not be refreshed automatically. ",
    "Please log out and sign in again."
);
const CLOUD_REQUIREMENTS_CACHE_WRITE_HMAC_KEY: &[u8] =
    b"codex-cloud-requirements-cache-v3-064f8542-75b4-494c-a294-97d3ce597271";
const CLOUD_REQUIREMENTS_CACHE_READ_HMAC_KEYS: &[&[u8]] =
    &[CLOUD_REQUIREMENTS_CACHE_WRITE_HMAC_KEY];

type HmacSha256 = Hmac<Sha256>;

fn refresher_task_slot() -> &'static Mutex<Option<JoinHandle<()>>> {
    static REFRESHER_TASK: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();
    REFRESHER_TASK.get_or_init(|| Mutex::new(None))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetryableFailureKind {
    BackendClientInit,
    Request { status_code: Option<u16> },
}

impl RetryableFailureKind {
    fn status_code(self) -> Option<u16> {
        match self {
            Self::BackendClientInit => None,
            Self::Request { status_code } => status_code,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FetchAttemptError {
    Retryable(RetryableFailureKind),
    Unauthorized {
        status_code: Option<u16>,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
enum CacheLoadStatus {
    #[error("Skipping cloud requirements cache read because auth identity is incomplete.")]
    AuthIdentityIncomplete,
    #[error("Cloud requirements cache file not found.")]
    CacheFileNotFound,
    #[error("Failed to read cloud requirements cache: {0}.")]
    CacheReadFailed(String),
    #[error("Failed to parse cloud requirements cache: {0}.")]
    CacheParseFailed(String),
    #[error("Cloud requirements cache failed signature verification.")]
    CacheSignatureInvalid,
    #[error("Ignoring cloud requirements cache because cached identity is incomplete.")]
    CacheIdentityIncomplete,
    #[error("Ignoring cloud requirements cache for different auth identity.")]
    CacheIdentityMismatch,
    #[error("Cloud requirements cache expired.")]
    CacheExpired,
}

#[derive(Debug, Error)]
enum CloudRequirementsError {
    #[error("failed to write cloud requirements cache")]
    CacheWrite,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CloudRequirementsCacheFile {
    signed_payload: CloudRequirementsCacheSignedPayload,
    signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CloudRequirementsCacheSignedPayload {
    cached_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    chatgpt_user_id: Option<String>,
    account_id: Option<String>,
    contents: Option<String>,
}

impl CloudRequirementsCacheSignedPayload {
    fn requirements(&self) -> Option<ConfigRequirementsToml> {
        self.contents
            .as_deref()
            .and_then(|contents| parse_cloud_requirements(contents).ok().flatten())
    }
}
fn sign_cache_payload(payload_bytes: &[u8]) -> Option<String> {
    let mut mac = HmacSha256::new_from_slice(CLOUD_REQUIREMENTS_CACHE_WRITE_HMAC_KEY).ok()?;
    mac.update(payload_bytes);
    let signature = mac.finalize().into_bytes();
    Some(BASE64_STANDARD.encode(signature))
}

fn verify_cache_signature_with_key(
    payload_bytes: &[u8],
    signature_bytes: &[u8],
    key: &[u8],
) -> bool {
    let mut mac = match HmacSha256::new_from_slice(key) {
        Ok(mac) => mac,
        Err(_) => return false,
    };
    mac.update(payload_bytes);
    mac.verify_slice(signature_bytes).is_ok()
}

fn verify_cache_signature(payload_bytes: &[u8], signature: &str) -> bool {
    let signature_bytes = match BASE64_STANDARD.decode(signature) {
        Ok(signature_bytes) => signature_bytes,
        Err(_) => return false,
    };

    CLOUD_REQUIREMENTS_CACHE_READ_HMAC_KEYS
        .iter()
        .any(|key| verify_cache_signature_with_key(payload_bytes, &signature_bytes, key))
}

fn auth_identity(auth: &CodexAuth) -> (Option<String>, Option<String>) {
    let token_data = auth.get_token_data().ok();
    let chatgpt_user_id = token_data
        .as_ref()
        .and_then(|token_data| token_data.id_token.chatgpt_user_id.as_deref())
        .map(str::to_owned);
    let account_id = auth.get_account_id();
    (chatgpt_user_id, account_id)
}

fn cache_payload_bytes(payload: &CloudRequirementsCacheSignedPayload) -> Option<Vec<u8>> {
    serde_json::to_vec(&payload).ok()
}

#[async_trait]
trait RequirementsFetcher: Send + Sync {
    /// Returns `Ok(None)` when there are no cloud requirements for the account.
    ///
    /// Returning `Err` indicates cloud requirements could not be fetched.
    async fn fetch_requirements(
        &self,
        auth: &CodexAuth,
    ) -> Result<Option<String>, FetchAttemptError>;
}

struct BackendRequirementsFetcher {
    base_url: String,
}

impl BackendRequirementsFetcher {
    fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

#[async_trait]
impl RequirementsFetcher for BackendRequirementsFetcher {
    async fn fetch_requirements(
        &self,
        auth: &CodexAuth,
    ) -> Result<Option<String>, FetchAttemptError> {
        let client = BackendClient::from_auth(self.base_url.clone(), auth)
            .inspect_err(|err| {
                tracing::warn!(
                    error = %err,
                    "Failed to construct backend client for cloud requirements"
                );
            })
            .map_err(|_| FetchAttemptError::Retryable(RetryableFailureKind::BackendClientInit))?;

        let response = client
            .get_config_requirements_file()
            .await
            .inspect_err(|err| tracing::warn!(error = %err, "Failed to fetch cloud requirements"))
            .map_err(|err| {
                let status_code = err.status().map(|status| status.as_u16());
                if err.is_unauthorized() {
                    FetchAttemptError::Unauthorized {
                        status_code,
                        message: err.to_string(),
                    }
                } else {
                    FetchAttemptError::Retryable(RetryableFailureKind::Request { status_code })
                }
            })?;

        let Some(contents) = response.contents else {
            tracing::info!(
                "Cloud requirements response missing contents; treating as no requirements"
            );
            return Ok(None);
        };

        Ok(Some(contents))
    }
}

#[derive(Clone)]
struct CloudRequirementsService {
    auth_manager: Arc<AuthManager>,
    fetcher: Arc<dyn RequirementsFetcher>,
    cache_path: PathBuf,
    timeout: Duration,
}

impl CloudRequirementsService {
    fn new(
        auth_manager: Arc<AuthManager>,
        fetcher: Arc<dyn RequirementsFetcher>,
        codex_home: PathBuf,
        timeout: Duration,
    ) -> Self {
        Self {
            auth_manager,
            fetcher,
            cache_path: codex_home.join(CLOUD_REQUIREMENTS_CACHE_FILENAME),
            timeout,
        }
    }

    async fn fetch_with_timeout(
        &self,
    ) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsLoadError> {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_requirements.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let fetch_result = timeout(self.timeout, self.fetch())
            .await
            .inspect_err(|_| {
                let message = format!(
                    "Timed out waiting for cloud requirements after {}s",
                    self.timeout.as_secs()
                );
                tracing::error!("{message}");
                emit_load_metric("startup", "error");
            })
            .map_err(|_| {
                CloudRequirementsLoadError::new(
                    CloudRequirementsLoadErrorCode::Timeout,
                    /*status_code*/ None,
                    format!(
                        "timed out waiting for cloud requirements after {}s",
                        self.timeout.as_secs()
                    ),
                )
            })?;

        let result = match fetch_result {
            Ok(result) => result,
            Err(err) => {
                emit_load_metric("startup", "error");
                return Err(err);
            }
        };

        match result.as_ref() {
            Some(requirements) => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    requirements = ?requirements,
                    "Cloud requirements load completed"
                );
                emit_load_metric("startup", "success");
            }
            None => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "Cloud requirements load completed (none)"
                );
                emit_load_metric("startup", "success");
            }
        }

        Ok(result)
    }

    async fn fetch(&self) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsLoadError> {
        let Some(auth) = self.auth_manager.auth().await else {
            return Ok(None);
        };
        let Some(plan_type) = auth.account_plan_type() else {
            return Ok(None);
        };
        if !auth.is_chatgpt_auth()
            || !(plan_type.is_business_like() || matches!(plan_type, PlanType::Enterprise))
        {
            return Ok(None);
        }
        let (chatgpt_user_id, account_id) = auth_identity(&auth);

        match self
            .load_cache(chatgpt_user_id.as_deref(), account_id.as_deref())
            .await
        {
            Ok(signed_payload) => {
                tracing::info!(
                    path = %self.cache_path.display(),
                    "Using cached cloud requirements"
                );
                return Ok(signed_payload.requirements());
            }
            Err(cache_load_status) => {
                self.log_cache_load_status(&cache_load_status);
            }
        }

        self.fetch_with_retries(auth, "startup").await
    }

    async fn fetch_with_retries(
        &self,
        mut auth: CodexAuth,
        trigger: &'static str,
    ) -> Result<Option<ConfigRequirementsToml>, CloudRequirementsLoadError> {
        let mut attempt = 1;
        let mut last_status_code: Option<u16> = None;
        let mut auth_recovery = self.auth_manager.unauthorized_recovery();

        while attempt <= CLOUD_REQUIREMENTS_MAX_ATTEMPTS {
            let contents = match self.fetcher.fetch_requirements(&auth).await {
                Ok(contents) => {
                    emit_fetch_attempt_metric(
                        trigger, attempt, "success", /*status_code*/ None,
                    );
                    contents
                }
                Err(FetchAttemptError::Retryable(status)) => {
                    let status_code = status.status_code();
                    last_status_code = status_code;
                    emit_fetch_attempt_metric(trigger, attempt, "error", status_code);
                    if attempt < CLOUD_REQUIREMENTS_MAX_ATTEMPTS {
                        tracing::warn!(
                            status = ?status,
                            attempt,
                            max_attempts = CLOUD_REQUIREMENTS_MAX_ATTEMPTS,
                            "Failed to fetch cloud requirements; retrying"
                        );
                        sleep(backoff(attempt as u64)).await;
                    }
                    attempt += 1;
                    continue;
                }
                Err(FetchAttemptError::Unauthorized {
                    status_code,
                    message,
                }) => {
                    last_status_code = status_code;
                    emit_fetch_attempt_metric(trigger, attempt, "unauthorized", status_code);
                    if auth_recovery.has_next() {
                        tracing::warn!(
                            attempt,
                            max_attempts = CLOUD_REQUIREMENTS_MAX_ATTEMPTS,
                            "Cloud requirements request was unauthorized; attempting auth recovery"
                        );
                        match auth_recovery.next().await {
                            Ok(_) => {
                                let Some(refreshed_auth) = self.auth_manager.auth().await else {
                                    tracing::error!(
                                        "Auth recovery succeeded but no auth is available for cloud requirements"
                                    );
                                    emit_fetch_final_metric(
                                        trigger,
                                        "error",
                                        "auth_recovery_missing_auth",
                                        attempt,
                                        status_code,
                                    );
                                    return Err(CloudRequirementsLoadError::new(
                                        CloudRequirementsLoadErrorCode::Auth,
                                        status_code,
                                        CLOUD_REQUIREMENTS_AUTH_RECOVERY_FAILED_MESSAGE,
                                    ));
                                };
                                auth = refreshed_auth;
                                continue;
                            }
                            Err(RefreshTokenError::Permanent(failed)) => {
                                tracing::warn!(
                                    error = %failed,
                                    "Failed to recover from unauthorized cloud requirements request"
                                );
                                emit_fetch_final_metric(
                                    trigger,
                                    "error",
                                    "auth_recovery_unrecoverable",
                                    attempt,
                                    status_code,
                                );
                                return Err(CloudRequirementsLoadError::new(
                                    CloudRequirementsLoadErrorCode::Auth,
                                    status_code,
                                    failed.message,
                                ));
                            }
                            Err(RefreshTokenError::Transient(recovery_err)) => {
                                if attempt < CLOUD_REQUIREMENTS_MAX_ATTEMPTS {
                                    tracing::warn!(
                                        error = %recovery_err,
                                        attempt,
                                        max_attempts = CLOUD_REQUIREMENTS_MAX_ATTEMPTS,
                                        "Failed to recover from unauthorized cloud requirements request; retrying"
                                    );
                                    sleep(backoff(attempt as u64)).await;
                                }
                                attempt += 1;
                                continue;
                            }
                        }
                    }

                    tracing::warn!(
                        error = %message,
                        "Cloud requirements request was unauthorized and no auth recovery is available"
                    );
                    emit_fetch_final_metric(
                        trigger,
                        "error",
                        "auth_recovery_unavailable",
                        attempt,
                        status_code,
                    );
                    return Err(CloudRequirementsLoadError::new(
                        CloudRequirementsLoadErrorCode::Auth,
                        status_code,
                        CLOUD_REQUIREMENTS_AUTH_RECOVERY_FAILED_MESSAGE,
                    ));
                }
            };

            let requirements = match contents.as_deref() {
                Some(contents) => match parse_cloud_requirements(contents) {
                    Ok(requirements) => requirements,
                    Err(err) => {
                        tracing::error!(error = %err, "Failed to parse cloud requirements");
                        emit_fetch_final_metric(
                            trigger,
                            "error",
                            "parse_error",
                            attempt,
                            last_status_code,
                        );
                        return Err(CloudRequirementsLoadError::new(
                            CloudRequirementsLoadErrorCode::Parse,
                            /*status_code*/ None,
                            format_cloud_requirements_parse_failed_message(contents, &err),
                        ));
                    }
                },
                None => None,
            };

            let (chatgpt_user_id, account_id) = auth_identity(&auth);
            if let Err(err) = self.save_cache(chatgpt_user_id, account_id, contents).await {
                tracing::warn!(error = %err, "Failed to write cloud requirements cache");
            }

            emit_fetch_final_metric(
                trigger, "success", "none", attempt, /*status_code*/ None,
            );
            return Ok(requirements);
        }

        emit_fetch_final_metric(
            trigger,
            "error",
            "request_retry_exhausted",
            CLOUD_REQUIREMENTS_MAX_ATTEMPTS,
            last_status_code,
        );
        tracing::error!(
            path = %self.cache_path.display(),
            "{CLOUD_REQUIREMENTS_LOAD_FAILED_MESSAGE}"
        );
        Err(CloudRequirementsLoadError::new(
            CloudRequirementsLoadErrorCode::RequestFailed,
            last_status_code,
            CLOUD_REQUIREMENTS_LOAD_FAILED_MESSAGE,
        ))
    }

    async fn refresh_cache_in_background(&self) {
        loop {
            sleep(CLOUD_REQUIREMENTS_CACHE_REFRESH_INTERVAL).await;
            match timeout(self.timeout, self.refresh_cache()).await {
                Ok(true) => {}
                Ok(false) => break,
                Err(_) => {
                    tracing::error!(
                        "Timed out refreshing cloud requirements cache from remote; keeping existing cache"
                    );
                    emit_load_metric("refresh", "error");
                }
            }
        }
    }

    async fn refresh_cache(&self) -> bool {
        let Some(auth) = self.auth_manager.auth().await else {
            return false;
        };
        let Some(plan_type) = auth.account_plan_type() else {
            return false;
        };
        if !auth.is_chatgpt_auth()
            || !(plan_type.is_business_like() || matches!(plan_type, PlanType::Enterprise))
        {
            return false;
        }

        match self.fetch_with_retries(auth, "refresh").await {
            Ok(_) => emit_load_metric("refresh", "success"),
            Err(err) => {
                tracing::error!(
                    path = %self.cache_path.display(),
                    error = %err,
                    "Failed to refresh cloud requirements cache from remote"
                );
                emit_load_metric("refresh", "error");
            }
        }
        true
    }

    async fn load_cache(
        &self,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
    ) -> Result<CloudRequirementsCacheSignedPayload, CacheLoadStatus> {
        let (Some(chatgpt_user_id), Some(account_id)) = (chatgpt_user_id, account_id) else {
            return Err(CacheLoadStatus::AuthIdentityIncomplete);
        };

        let bytes = match fs::read(&self.cache_path).await {
            Ok(bytes) => bytes,
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    return Err(CacheLoadStatus::CacheReadFailed(err.to_string()));
                }
                return Err(CacheLoadStatus::CacheFileNotFound);
            }
        };

        let cache_file: CloudRequirementsCacheFile = match serde_json::from_slice(&bytes) {
            Ok(cache_file) => cache_file,
            Err(err) => {
                return Err(CacheLoadStatus::CacheParseFailed(err.to_string()));
            }
        };
        let payload_bytes = match cache_payload_bytes(&cache_file.signed_payload) {
            Some(payload_bytes) => payload_bytes,
            None => {
                return Err(CacheLoadStatus::CacheParseFailed(
                    "failed to serialize cache payload".to_string(),
                ));
            }
        };
        if !verify_cache_signature(&payload_bytes, &cache_file.signature) {
            return Err(CacheLoadStatus::CacheSignatureInvalid);
        }

        let (Some(cached_chatgpt_user_id), Some(cached_account_id)) = (
            cache_file.signed_payload.chatgpt_user_id.as_deref(),
            cache_file.signed_payload.account_id.as_deref(),
        ) else {
            return Err(CacheLoadStatus::CacheIdentityIncomplete);
        };

        if cached_chatgpt_user_id != chatgpt_user_id || cached_account_id != account_id {
            return Err(CacheLoadStatus::CacheIdentityMismatch);
        }

        if cache_file.signed_payload.expires_at <= Utc::now() {
            return Err(CacheLoadStatus::CacheExpired);
        }

        Ok(cache_file.signed_payload)
    }

    fn log_cache_load_status(&self, status: &CacheLoadStatus) {
        if matches!(status, CacheLoadStatus::CacheFileNotFound) {
            return;
        }

        let warn = matches!(
            status,
            CacheLoadStatus::CacheReadFailed(_)
                | CacheLoadStatus::CacheParseFailed(_)
                | CacheLoadStatus::CacheSignatureInvalid
        );

        if warn {
            tracing::warn!(path = %self.cache_path.display(), "{status}");
        } else {
            tracing::info!(path = %self.cache_path.display(), "{status}");
        }
    }

    async fn save_cache(
        &self,
        chatgpt_user_id: Option<String>,
        account_id: Option<String>,
        contents: Option<String>,
    ) -> Result<(), CloudRequirementsError> {
        let now = Utc::now();
        let expires_at = now
            .checked_add_signed(
                ChronoDuration::from_std(CLOUD_REQUIREMENTS_CACHE_TTL)
                    .map_err(|_| CloudRequirementsError::CacheWrite)?,
            )
            .ok_or(CloudRequirementsError::CacheWrite)?;
        let signed_payload = CloudRequirementsCacheSignedPayload {
            cached_at: now,
            expires_at,
            chatgpt_user_id,
            account_id,
            contents,
        };
        let payload_bytes =
            cache_payload_bytes(&signed_payload).ok_or(CloudRequirementsError::CacheWrite)?;
        let serialized = serde_json::to_vec_pretty(&CloudRequirementsCacheFile {
            signature: sign_cache_payload(&payload_bytes)
                .ok_or(CloudRequirementsError::CacheWrite)?,
            signed_payload,
        })
        .map_err(|_| CloudRequirementsError::CacheWrite)?;

        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|_| CloudRequirementsError::CacheWrite)?;
        }

        fs::write(&self.cache_path, serialized)
            .await
            .map_err(|_| CloudRequirementsError::CacheWrite)?;
        Ok(())
    }
}

pub fn cloud_requirements_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
) -> CloudRequirementsLoader {
    let service = CloudRequirementsService::new(
        auth_manager,
        Arc::new(BackendRequirementsFetcher::new(chatgpt_base_url)),
        codex_home,
        CLOUD_REQUIREMENTS_TIMEOUT,
    );
    let refresh_service = service.clone();
    let task = tokio::spawn(async move { service.fetch_with_timeout().await });
    let refresh_task =
        tokio::spawn(async move { refresh_service.refresh_cache_in_background().await });
    let mut refresher_guard = refresher_task_slot().lock().unwrap_or_else(|err| {
        tracing::warn!("cloud requirements refresher task slot was poisoned");
        err.into_inner()
    });
    if let Some(existing_task) = refresher_guard.replace(refresh_task) {
        existing_task.abort();
    }
    CloudRequirementsLoader::new(async move {
        task.await.map_err(|err| {
            tracing::error!(error = %err, "Cloud requirements task failed");
            CloudRequirementsLoadError::new(
                CloudRequirementsLoadErrorCode::Internal,
                /*status_code*/ None,
                format!("cloud requirements load failed: {err}"),
            )
        })?
    })
}

pub fn cloud_requirements_loader_for_storage(
    codex_home: PathBuf,
    enable_codex_api_key_env: bool,
    credentials_store_mode: AuthCredentialsStoreMode,
    chatgpt_base_url: String,
) -> CloudRequirementsLoader {
    let auth_manager = AuthManager::shared(
        codex_home.clone(),
        enable_codex_api_key_env,
        credentials_store_mode,
        Some(chatgpt_base_url.clone()),
    );
    cloud_requirements_loader(auth_manager, chatgpt_base_url, codex_home)
}

fn parse_cloud_requirements(
    contents: &str,
) -> Result<Option<ConfigRequirementsToml>, toml::de::Error> {
    if contents.trim().is_empty() {
        return Ok(None);
    }

    let requirements: ConfigRequirementsToml = toml::from_str(contents)?;
    if requirements.is_empty() {
        Ok(None)
    } else {
        Ok(Some(requirements))
    }
}

fn format_cloud_requirements_parse_failed_message(
    _contents: &str,
    err: &toml::de::Error,
) -> String {
    format!("{CLOUD_REQUIREMENTS_PARSE_FAILED_MESSAGE}\n\nDetails:\n{err}")
}

fn emit_fetch_attempt_metric(
    trigger: &str,
    attempt: usize,
    outcome: &str,
    status_code: Option<u16>,
) {
    let attempt_tag = attempt.to_string();
    let status_code_tag = status_code_tag(status_code);
    emit_metric(
        CLOUD_REQUIREMENTS_FETCH_ATTEMPT_METRIC,
        vec![
            ("trigger", trigger.to_string()),
            ("attempt", attempt_tag),
            ("outcome", outcome.to_string()),
            ("status_code", status_code_tag),
        ],
    );
}

fn emit_fetch_final_metric(
    trigger: &str,
    outcome: &str,
    reason: &str,
    attempt_count: usize,
    status_code: Option<u16>,
) {
    let attempt_count_tag = attempt_count.to_string();
    let status_code_tag = status_code_tag(status_code);
    emit_metric(
        CLOUD_REQUIREMENTS_FETCH_FINAL_METRIC,
        vec![
            ("trigger", trigger.to_string()),
            ("outcome", outcome.to_string()),
            ("reason", reason.to_string()),
            ("attempt_count", attempt_count_tag),
            ("status_code", status_code_tag),
        ],
    );
}

fn emit_load_metric(trigger: &str, outcome: &str) {
    emit_metric(
        CLOUD_REQUIREMENTS_LOAD_METRIC,
        vec![
            ("trigger", trigger.to_string()),
            ("outcome", outcome.to_string()),
        ],
    );
}

fn status_code_tag(status_code: Option<u16>) -> String {
    status_code
        .map(|status_code| status_code.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn emit_metric(metric_name: &str, tags: Vec<(&str, String)>) {
    if let Some(metrics) = codex_otel::global() {
        let tag_refs = tags
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();
        let _ = metrics.counter(metric_name, /*inc*/ 1, &tag_refs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use codex_config::types::AuthCredentialsStoreMode;
    use codex_protocol::protocol::AskForApproval;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::collections::VecDeque;
    use std::future::pending;
    use std::path::Path;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::TempDir;
    use tempfile::tempdir;

    fn write_auth_json(codex_home: &Path, value: serde_json::Value) -> std::io::Result<()> {
        std::fs::write(codex_home.join("auth.json"), serde_json::to_string(&value)?)?;
        Ok(())
    }

    fn auth_manager_with_api_key() -> Arc<AuthManager> {
        let tmp = tempdir().expect("tempdir");
        let auth_json = json!({
            "OPENAI_API_KEY": "sk-test-key",
            "tokens": null,
            "last_refresh": null,
        });
        write_auth_json(tmp.path(), auth_json).expect("write auth");
        Arc::new(AuthManager::new(
            tmp.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        ))
    }

    fn auth_manager_with_plan_and_identity(
        plan_type: &str,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
    ) -> Arc<AuthManager> {
        let tmp = tempdir().expect("tempdir");
        write_auth_json(
            tmp.path(),
            chatgpt_auth_json(
                plan_type,
                chatgpt_user_id,
                account_id,
                "test-access-token",
                "test-refresh-token",
            ),
        )
        .expect("write auth");
        Arc::new(AuthManager::new(
            tmp.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        ))
    }

    fn chatgpt_auth_json(
        plan_type: &str,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
        access_token: &str,
        refresh_token: &str,
    ) -> serde_json::Value {
        chatgpt_auth_json_with_last_refresh(
            plan_type,
            chatgpt_user_id,
            account_id,
            access_token,
            refresh_token,
            "2025-01-01T00:00:00Z",
        )
    }

    fn chatgpt_auth_json_with_last_refresh(
        plan_type: &str,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
        access_token: &str,
        refresh_token: &str,
        last_refresh: &str,
    ) -> serde_json::Value {
        chatgpt_auth_json_with_mode(
            plan_type,
            chatgpt_user_id,
            account_id,
            access_token,
            refresh_token,
            last_refresh,
            /*auth_mode*/ None,
        )
    }

    fn chatgpt_auth_json_with_mode(
        plan_type: &str,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
        access_token: &str,
        refresh_token: &str,
        last_refresh: &str,
        auth_mode: Option<&str>,
    ) -> serde_json::Value {
        let header = json!({ "alg": "none", "typ": "JWT" });
        let auth_payload = json!({
            "chatgpt_plan_type": plan_type,
            "chatgpt_user_id": chatgpt_user_id,
            "user_id": chatgpt_user_id,
        });
        let payload = json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": auth_payload,
        });
        let header_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&header).expect("header"));
        let payload_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("payload"));
        let signature_b64 = URL_SAFE_NO_PAD.encode(b"sig");
        let fake_jwt = format!("{header_b64}.{payload_b64}.{signature_b64}");

        let mut auth_json = json!({
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": fake_jwt,
                "access_token": access_token,
                "refresh_token": refresh_token,
                "account_id": account_id,
            },
            "last_refresh": last_refresh,
        });
        if let Some(auth_mode) = auth_mode {
            auth_json["auth_mode"] = serde_json::Value::String(auth_mode.to_string());
        }
        auth_json
    }

    struct ManagedAuthContext {
        _home: TempDir,
        manager: Arc<AuthManager>,
    }

    fn managed_auth_context(
        plan_type: &str,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
        access_token: &str,
        refresh_token: &str,
    ) -> ManagedAuthContext {
        let home = tempdir().expect("tempdir");
        write_auth_json(
            home.path(),
            chatgpt_auth_json(
                plan_type,
                chatgpt_user_id,
                account_id,
                access_token,
                refresh_token,
            ),
        )
        .expect("write auth");
        ManagedAuthContext {
            manager: Arc::new(AuthManager::new(
                home.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                /*chatgpt_base_url*/ None,
            )),
            _home: home,
        }
    }

    fn auth_manager_with_plan(plan_type: &str) -> Arc<AuthManager> {
        auth_manager_with_plan_and_identity(plan_type, Some("user-12345"), Some("account-12345"))
    }

    fn parse_for_fetch(contents: Option<&str>) -> Option<ConfigRequirementsToml> {
        contents.and_then(|contents| parse_cloud_requirements(contents).ok().flatten())
    }

    fn request_error() -> FetchAttemptError {
        FetchAttemptError::Retryable(RetryableFailureKind::Request { status_code: None })
    }

    struct StaticFetcher {
        contents: Option<String>,
    }

    #[async_trait::async_trait]
    impl RequirementsFetcher for StaticFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, FetchAttemptError> {
            Ok(self.contents.clone())
        }
    }

    struct PendingFetcher;

    #[async_trait::async_trait]
    impl RequirementsFetcher for PendingFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, FetchAttemptError> {
            pending::<()>().await;
            Ok(None)
        }
    }

    struct SequenceFetcher {
        responses: tokio::sync::Mutex<VecDeque<Result<Option<String>, FetchAttemptError>>>,
        request_count: AtomicUsize,
    }

    impl SequenceFetcher {
        fn new(responses: Vec<Result<Option<String>, FetchAttemptError>>) -> Self {
            Self {
                responses: tokio::sync::Mutex::new(VecDeque::from(responses)),
                request_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl RequirementsFetcher for SequenceFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, FetchAttemptError> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            let mut responses = self.responses.lock().await;
            responses.pop_front().unwrap_or(Ok(None))
        }
    }

    struct TokenFetcher {
        expected_token: String,
        contents: String,
        request_count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl RequirementsFetcher for TokenFetcher {
        async fn fetch_requirements(
            &self,
            auth: &CodexAuth,
        ) -> Result<Option<String>, FetchAttemptError> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            if matches!(
                auth.get_token().as_deref(),
                Ok(token) if token == self.expected_token.as_str()
            ) {
                Ok(Some(self.contents.clone()))
            } else {
                Err(FetchAttemptError::Unauthorized {
                    status_code: Some(401),
                    message: "GET /config/requirements failed: 401".to_string(),
                })
            }
        }
    }

    struct UnauthorizedFetcher {
        message: String,
        request_count: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl RequirementsFetcher for UnauthorizedFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<Option<String>, FetchAttemptError> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            Err(FetchAttemptError::Unauthorized {
                status_code: Some(401),
                message: self.message.clone(),
            })
        }
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_chatgpt_auth() {
        let auth_manager = auth_manager_with_api_key();
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager,
            Arc::new(StaticFetcher { contents: None }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let result = service.fetch().await;
        assert_eq!(result, Ok(None));
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_business_or_enterprise_plan() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("pro"),
            Arc::new(StaticFetcher { contents: None }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let result = service.fetch().await;
        assert_eq!(result, Ok(None));
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_team_like_usage_based_plan() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("self_serve_business_usage_based"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(service.fetch().await, Ok(None));
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_allows_business_plan() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_allows_business_like_usage_based_plan() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise_cbp_usage_based"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_allows_hc_plan_as_enterprise() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("hc"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_missing_contents() {
        let result = parse_for_fetch(/*contents*/ None);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_empty_contents() {
        let result = parse_for_fetch(Some("   "));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_handles_invalid_toml() {
        let result = parse_for_fetch(Some("not = ["));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_empty_requirements() {
        let result = parse_for_fetch(Some("# comment"));
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_parses_valid_toml() {
        let result = parse_for_fetch(Some("allowed_approval_policies = [\"never\"]"));

        assert_eq!(
            result,
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            })
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_parses_apps_requirements_toml() {
        let result = parse_for_fetch(Some(
            r#"
[apps.connector_5f3c8c41a1e54ad7a76272c89e2554fa]
enabled = false
"#,
        ));

        assert_eq!(
            result,
            Some(ConfigRequirementsToml {
                apps: Some(codex_core::config_loader::AppsRequirementsToml {
                    apps: BTreeMap::from([(
                        "connector_5f3c8c41a1e54ad7a76272c89e2554fa".to_string(),
                        codex_core::config_loader::AppRequirementToml {
                            enabled: Some(false),
                        },
                    )]),
                }),
                ..Default::default()
            })
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_times_out() {
        let auth_manager = auth_manager_with_plan("enterprise");
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager,
            Arc::new(PendingFetcher),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let handle = tokio::spawn(async move { service.fetch_with_timeout().await });
        tokio::time::advance(CLOUD_REQUIREMENTS_TIMEOUT + Duration::from_millis(1)).await;

        let result = handle.await.expect("cloud requirements task");
        let err = result.expect_err("cloud requirements timeout should fail closed");
        assert!(
            err.to_string()
                .contains("timed out waiting for cloud requirements")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_retries_until_success() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Err(request_error()),
            Ok(Some("allowed_approval_policies = [\"never\"]".to_string())),
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let handle = tokio::spawn(async move { service.fetch().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(
            handle.await.expect("cloud requirements task"),
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_recovers_after_unauthorized_reload() {
        let auth_home = tempdir().expect("tempdir");
        write_auth_json(
            auth_home.path(),
            chatgpt_auth_json_with_last_refresh(
                "business",
                Some("user-12345"),
                Some("account-12345"),
                "stale-access-token",
                "test-refresh-token",
                // Keep auth "fresh" so the first request hits unauthorized recovery
                // instead of AuthManager::auth() proactively reloading from disk.
                "3025-01-01T00:00:00Z",
            ),
        )
        .expect("write initial auth");
        let auth_manager = Arc::new(AuthManager::new(
            auth_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        ));

        write_auth_json(
            auth_home.path(),
            chatgpt_auth_json_with_last_refresh(
                "business",
                Some("user-12345"),
                Some("account-12345"),
                "fresh-access-token",
                "test-refresh-token",
                "3025-01-01T00:00:00Z",
            ),
        )
        .expect("write refreshed auth");
        let auth = ManagedAuthContext {
            _home: auth_home,
            manager: auth_manager,
        };

        let fetcher = Arc::new(TokenFetcher {
            expected_token: "fresh-access-token".to_string(),
            contents: "allowed_approval_policies = [\"never\"]".to_string(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            Arc::clone(&auth.manager),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_recovers_after_unauthorized_reload_updates_cache_identity() {
        let auth_home = tempdir().expect("tempdir");
        write_auth_json(
            auth_home.path(),
            chatgpt_auth_json_with_last_refresh(
                "business",
                Some("user-12345"),
                Some("account-12345"),
                "stale-access-token",
                "test-refresh-token",
                "3025-01-01T00:00:00Z",
            ),
        )
        .expect("write initial auth");
        let auth_manager = Arc::new(AuthManager::new(
            auth_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        ));

        write_auth_json(
            auth_home.path(),
            chatgpt_auth_json_with_last_refresh(
                "business",
                Some("user-99999"),
                Some("account-12345"),
                "fresh-access-token",
                "test-refresh-token",
                "3025-01-01T00:00:00Z",
            ),
        )
        .expect("write refreshed auth");
        let auth = ManagedAuthContext {
            _home: auth_home,
            manager: auth_manager,
        };

        let fetcher = Arc::new(TokenFetcher {
            expected_token: "fresh-access-token".to_string(),
            contents: "allowed_approval_policies = [\"never\"]".to_string(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            Arc::clone(&auth.manager),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );

        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let cache_file: CloudRequirementsCacheFile =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read cache"))
                .expect("parse cache");
        assert_eq!(
            cache_file.signed_payload.chatgpt_user_id,
            Some("user-99999".to_string())
        );
        assert_eq!(
            cache_file.signed_payload.account_id,
            Some("account-12345".to_string())
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_surfaces_auth_recovery_message() {
        let auth = managed_auth_context(
            "enterprise",
            Some("user-12345"),
            Some("account-12345"),
            "stale-access-token",
            "test-refresh-token",
        );
        write_auth_json(
            auth._home.path(),
            chatgpt_auth_json(
                "enterprise",
                Some("user-12345"),
                Some("account-99999"),
                "fresh-access-token",
                "test-refresh-token",
            ),
        )
        .expect("write mismatched auth");

        let fetcher = Arc::new(UnauthorizedFetcher {
            message: "GET /config/requirements failed: 401".to_string(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            Arc::clone(&auth.manager),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let err = service
            .fetch()
            .await
            .expect_err("cloud requirements should surface auth recovery errors");
        assert_eq!(
            err.to_string(),
            "Your access token could not be refreshed because you have since logged out or signed in to another account. Please sign in again."
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_unauthorized_without_recovery_uses_generic_message() {
        let auth_home = tempdir().expect("tempdir");
        write_auth_json(
            auth_home.path(),
            chatgpt_auth_json_with_mode(
                "enterprise",
                Some("user-12345"),
                Some("account-12345"),
                "test-access-token",
                "test-refresh-token",
                "2025-01-01T00:00:00Z",
                Some("chatgptAuthTokens"),
            ),
        )
        .expect("write auth");
        let auth_manager = Arc::new(AuthManager::new(
            auth_home.path().to_path_buf(),
            /*enable_codex_api_key_env*/ false,
            AuthCredentialsStoreMode::File,
            /*chatgpt_base_url*/ None,
        ));

        let fetcher = Arc::new(UnauthorizedFetcher {
            message:
                "GET https://chatgpt.com/backend-api/wham/config/requirements failed: 401; content-type=text/html; body=<html>nope</html>"
                    .to_string(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let err = service
            .fetch()
            .await
            .expect_err("cloud requirements should fail closed");
        assert_eq!(
            err.to_string(),
            CLOUD_REQUIREMENTS_AUTH_RECOVERY_FAILED_MESSAGE
        );
        assert_eq!(err.code(), CloudRequirementsLoadErrorCode::Auth);
        assert_eq!(err.status_code(), Some(401));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_parse_error_does_not_retry() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Ok(Some("not = [".to_string())),
            Ok(Some("allowed_approval_policies = [\"never\"]".to_string())),
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let err = service
            .fetch()
            .await
            .expect_err("parse error should fail closed");
        let err_text = err.to_string();
        assert!(err_text.contains(CLOUD_REQUIREMENTS_PARSE_FAILED_MESSAGE));
        assert!(err_text.contains("Details:"));
        assert!(err_text.contains("not = ["));
        assert_eq!(err.code(), CloudRequirementsLoadErrorCode::Parse);
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_invalid_enum_value_surfaces_field_name() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"definitely-not-valid\"]".to_string(),
        ))]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            fetcher,
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let err = service
            .fetch()
            .await
            .expect_err("invalid enum value should fail closed");
        let err_text = err.to_string();
        assert!(err_text.contains(CLOUD_REQUIREMENTS_PARSE_FAILED_MESSAGE));
        assert!(err_text.contains("allowed_approval_policies"));
        assert!(err_text.contains("definitely-not-valid"));
        assert!(err_text.contains("unknown variant"));
        assert_eq!(err.code(), CloudRequirementsLoadErrorCode::Parse);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_uses_cache_when_valid() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let fetcher = Arc::new(SequenceFetcher::new(vec![Err(request_error())]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_writes_cache_when_identity_is_incomplete() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                /*chatgpt_user_id*/ None,
                Some("account-12345"),
            ),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );

        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let cache_file: CloudRequirementsCacheFile =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read cache"))
                .expect("parse cache");
        assert_eq!(cache_file.signed_payload.chatgpt_user_id, None);
        assert_eq!(
            cache_file.signed_payload.account_id,
            Some("account-12345".to_string())
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_does_not_use_cache_when_auth_identity_is_incomplete() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"on-request\"]".to_string(),
        ))]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                /*chatgpt_user_id*/ None,
                Some("account-12345"),
            ),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_cache_for_different_auth_identity() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                Some("user-12345"),
                Some("account-12345"),
            ),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"on-request\"]".to_string(),
        ))]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                Some("user-99999"),
                Some("account-12345"),
            ),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_tampered_cache() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let mut cache_file: CloudRequirementsCacheFile =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read cache"))
                .expect("parse cache");
        cache_file.signed_payload.contents =
            Some("allowed_approval_policies = [\"on-request\"]".to_string());
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&cache_file).expect("serialize cache"),
        )
        .expect("write cache");

        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"never\"]".to_string(),
        ))]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_expired_cache() {
        let codex_home = tempdir().expect("tempdir");
        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let cache_file = CloudRequirementsCacheFile {
            signed_payload: CloudRequirementsCacheSignedPayload {
                cached_at: Utc::now(),
                expires_at: Utc::now() - ChronoDuration::seconds(1),
                chatgpt_user_id: Some("user-12345".to_string()),
                account_id: Some("account-12345".to_string()),
                contents: Some("allowed_approval_policies = [\"on-request\"]".to_string()),
            },
            signature: String::new(),
        };
        let payload_bytes = cache_payload_bytes(&cache_file.signed_payload).expect("payload");
        let signature = sign_cache_payload(&payload_bytes).expect("sign payload");
        let cache_file = CloudRequirementsCacheFile {
            signature,
            ..cache_file
        };
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&cache_file).expect("serialize cache"),
        )
        .expect("write cache");

        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(Some(
            "allowed_approval_policies = [\"never\"]".to_string(),
        ))]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_writes_signed_cache() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            Arc::new(StaticFetcher {
                contents: Some("allowed_approval_policies = [\"never\"]".to_string()),
            }),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let _ = service.fetch().await;

        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let cache_file: CloudRequirementsCacheFile =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read cache"))
                .expect("parse cache");
        assert!(
            cache_file.signed_payload.expires_at
                <= cache_file.signed_payload.cached_at + ChronoDuration::minutes(30)
        );
        assert!(cache_file.signed_payload.expires_at > cache_file.signed_payload.cached_at);
        assert!(cache_file.signed_payload.cached_at <= Utc::now());
        assert_eq!(
            cache_file.signed_payload.chatgpt_user_id,
            Some("user-12345".to_string())
        );
        assert_eq!(
            cache_file.signed_payload.account_id,
            Some("account-12345".to_string())
        );
        assert_eq!(
            cache_file
                .signed_payload
                .contents
                .as_deref()
                .and_then(|contents| parse_cloud_requirements(contents).ok().flatten()),
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            })
        );
        let payload_bytes = cache_payload_bytes(&cache_file.signed_payload).expect("payload bytes");
        assert!(verify_cache_signature(
            &payload_bytes,
            &cache_file.signature
        ));
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_none_is_success_without_retry() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(None), Err(request_error())]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_stops_after_max_retries() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Err(request_error());
            CLOUD_REQUIREMENTS_MAX_ATTEMPTS
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise"),
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let handle = tokio::spawn(async move { service.fetch().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;

        let err = handle
            .await
            .expect("cloud requirements task")
            .expect_err("cloud requirements retry exhaustion should fail closed");
        assert_eq!(err.to_string(), CLOUD_REQUIREMENTS_LOAD_FAILED_MESSAGE);
        assert_eq!(err.code(), CloudRequirementsLoadErrorCode::RequestFailed);
        assert_eq!(
            fetcher.request_count.load(Ordering::SeqCst),
            CLOUD_REQUIREMENTS_MAX_ATTEMPTS
        );
    }

    #[tokio::test]
    async fn refresh_from_remote_updates_cached_cloud_requirements() {
        let codex_home = tempdir().expect("tempdir");
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Ok(Some("allowed_approval_policies = [\"never\"]".to_string())),
            Ok(Some(
                "allowed_approval_policies = [\"on-request\"]".to_string(),
            )),
        ]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business"),
            fetcher,
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(
            service.fetch().await,
            Ok(Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::Never]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            }))
        );

        assert!(service.refresh_cache().await);

        let path = codex_home.path().join(CLOUD_REQUIREMENTS_CACHE_FILENAME);
        let cache_file: CloudRequirementsCacheFile =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read cache"))
                .expect("parse cache");
        assert_eq!(
            cache_file
                .signed_payload
                .contents
                .as_deref()
                .and_then(|contents| parse_cloud_requirements(contents).ok().flatten()),
            Some(ConfigRequirementsToml {
                allowed_approval_policies: Some(vec![AskForApproval::OnRequest]),
                allowed_approvals_reviewers: None,
                allowed_sandbox_modes: None,
                remote_sandbox_config: None,
                allowed_web_search_modes: None,
                guardian_policy_config: None,
                feature_requirements: None,
                hooks: None,
                mcp_servers: None,
                apps: None,
                rules: None,
                enforce_residency: None,
                network: None,
                permissions: None,
            })
        );
    }
}
