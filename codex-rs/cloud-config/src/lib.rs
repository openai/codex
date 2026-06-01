//! Cloud-hosted configuration data for Codex.
//!
//! This crate owns transport, caching, and refresh behavior for cloud-delivered
//! config data. Parsing and composition remain in `codex-config`.

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use chrono::DateTime;
use chrono::Duration as ChronoDuration;
use chrono::Utc;
use codex_backend_client::Client as BackendClient;
use codex_backend_client::ConfigBundleResponse;
use codex_backend_client::DeliveredTomlFragment;
use codex_config::AbsolutePathBuf;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigBundleLayers;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::CloudConfigBundleLoader;
use codex_config::CloudConfigFragment;
use codex_config::CloudConfigTomlBundle;
use codex_config::CloudRequirementsFragment;
use codex_config::CloudRequirementsTomlBundle;
use codex_config::compose_requirements;
use codex_config::types::AuthCredentialsStoreMode;
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
use std::path::Path;
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
const CLOUD_CONFIG_BUNDLE_CACHE_VERSION: u32 = 1;
const CLOUD_REQUIREMENTS_CACHE_FILENAME: &str = "cloud-config-bundle-cache.json";
const CLOUD_REQUIREMENTS_CACHE_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const CLOUD_REQUIREMENTS_CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const CLOUD_REQUIREMENTS_FETCH_ATTEMPT_METRIC: &str = "codex.cloud_config_bundle.fetch_attempt";
const CLOUD_REQUIREMENTS_FETCH_FINAL_METRIC: &str = "codex.cloud_config_bundle.fetch_final";
const CLOUD_REQUIREMENTS_LOAD_METRIC: &str = "codex.cloud_config_bundle.load";
const CLOUD_REQUIREMENTS_LOAD_FAILED_MESSAGE: &str =
    "Failed to load cloud config bundle (workspace-managed policies).";
const CLOUD_REQUIREMENTS_AUTH_RECOVERY_FAILED_MESSAGE: &str = concat!(
    "Your authentication session could not be refreshed automatically. ",
    "Please log out and sign in again."
);
const CLOUD_REQUIREMENTS_CACHE_WRITE_HMAC_KEY: &[u8] =
    b"codex-cloud-config-bundle-cache-v1-6160ae70-bcfd-4ca8-a99b-40f73b3b072e";
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
    #[error("Skipping cloud config bundle cache read because auth identity is incomplete.")]
    AuthIdentityIncomplete,
    #[error("Cloud config bundle cache file not found.")]
    CacheFileNotFound,
    #[error("Failed to read cloud config bundle cache: {0}.")]
    CacheReadFailed(String),
    #[error("Failed to parse cloud config bundle cache: {0}.")]
    CacheParseFailed(String),
    #[error("Cloud config bundle cache failed signature verification.")]
    CacheSignatureInvalid,
    #[error("Ignoring cloud config bundle cache because cached identity is incomplete.")]
    CacheIdentityIncomplete,
    #[error("Ignoring cloud config bundle cache for different auth identity.")]
    CacheIdentityMismatch,
    #[error("Ignoring cloud config bundle cache with unsupported version {0}.")]
    CacheVersionUnsupported(u32),
    #[error("Cloud config bundle cache expired.")]
    CacheExpired,
    #[error("Ignoring cloud config bundle cache because the cached bundle is invalid.")]
    CacheInvalidBundle,
}

#[derive(Debug, Error)]
enum CloudRequirementsError {
    #[error("failed to write cloud config bundle cache")]
    CacheWrite,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CloudRequirementsCacheFile {
    signed_payload: CloudRequirementsCacheSignedPayload,
    signature: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CloudRequirementsCacheSignedPayload {
    version: u32,
    cached_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    chatgpt_user_id: Option<String>,
    account_id: Option<String>,
    bundle: CloudConfigBundle,
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
    (auth.get_chatgpt_user_id(), auth.get_account_id())
}

fn cloud_requirements_eligible_auth(auth: &CodexAuth) -> bool {
    let Some(plan_type) = auth.account_plan_type() else {
        return false;
    };
    auth.uses_codex_backend()
        && (plan_type.is_business_like() || matches!(plan_type, PlanType::Enterprise))
}

fn optional_bundle(bundle: CloudConfigBundle) -> Option<CloudConfigBundle> {
    if bundle.is_empty() {
        None
    } else {
        Some(bundle)
    }
}

fn cache_payload_bytes(payload: &CloudRequirementsCacheSignedPayload) -> Option<Vec<u8>> {
    serde_json::to_vec(&payload).ok()
}

/// Retrieves one cloud config bundle from the backend.
///
/// Implementations should return the backend-selected bundle exactly as delivered and leave
/// validation, caching, and config/requirements parsing decisions to the service layer.
#[async_trait]
trait RequirementsFetcher: Send + Sync {
    async fn fetch_requirements(
        &self,
        auth: &CodexAuth,
    ) -> Result<CloudConfigBundle, FetchAttemptError>;
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
    ) -> Result<CloudConfigBundle, FetchAttemptError> {
        let client = BackendClient::from_auth(self.base_url.clone(), auth)
            .inspect_err(|err| {
                tracing::warn!(
                    error = %err,
                    "Failed to construct backend client for cloud config bundle"
                );
            })
            .map_err(|_| FetchAttemptError::Retryable(RetryableFailureKind::BackendClientInit))?;

        let response = client
            .get_config_bundle()
            .await
            .inspect_err(|err| {
                tracing::warn!(error = %err, "Failed to fetch cloud config bundle");
            })
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

        Ok(bundle_from_response(response))
    }
}

fn bundle_from_response(response: ConfigBundleResponse) -> CloudConfigBundle {
    let config_toml = response
        .config_toml
        .flatten()
        .map(|config_toml| *config_toml)
        .and_then(|config_toml| config_toml.enterprise_managed.flatten())
        .unwrap_or_default()
        .into_iter()
        .map(config_fragment_from_delivered)
        .collect();
    let requirements_toml = response
        .requirements_toml
        .flatten()
        .map(|requirements_toml| *requirements_toml)
        .and_then(|requirements_toml| requirements_toml.enterprise_managed.flatten())
        .unwrap_or_default()
        .into_iter()
        .map(requirements_fragment_from_delivered)
        .collect();

    CloudConfigBundle {
        config_toml: CloudConfigTomlBundle {
            enterprise_managed: config_toml,
        },
        requirements_toml: CloudRequirementsTomlBundle {
            enterprise_managed: requirements_toml,
        },
    }
}

fn config_fragment_from_delivered(fragment: DeliveredTomlFragment) -> CloudConfigFragment {
    CloudConfigFragment {
        id: fragment.id,
        name: fragment.name,
        contents: fragment.contents,
    }
}

fn requirements_fragment_from_delivered(
    fragment: DeliveredTomlFragment,
) -> CloudRequirementsFragment {
    CloudRequirementsFragment {
        id: fragment.id,
        name: fragment.name,
        contents: fragment.contents,
    }
}

#[derive(Clone)]
struct CloudRequirementsService {
    auth_manager: Arc<AuthManager>,
    fetcher: Arc<dyn RequirementsFetcher>,
    cache_path: PathBuf,
    codex_home: PathBuf,
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
            codex_home,
            timeout,
        }
    }

    async fn fetch_with_timeout(
        &self,
    ) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_config_bundle.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let load_result = timeout(self.timeout, self.fetch())
            .await
            .inspect_err(|_| {
                let message = format!(
                    "Timed out waiting for cloud config bundle after {}s",
                    self.timeout.as_secs()
                );
                tracing::error!("{message}");
                emit_load_metric("startup", "error", /*bundle*/ None);
            })
            .map_err(|_| {
                CloudConfigBundleLoadError::new(
                    CloudConfigBundleLoadErrorCode::Timeout,
                    /*status_code*/ None,
                    format!(
                        "timed out waiting for cloud config bundle after {}s",
                        self.timeout.as_secs()
                    ),
                )
            })?;

        let result = match load_result {
            Ok(result) => result,
            Err(err) => {
                emit_load_metric("startup", "error", /*bundle*/ None);
                return Err(err);
            }
        };

        match result.as_ref() {
            Some(bundle) => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    config_fragments = bundle.config_toml.enterprise_managed.len(),
                    requirements_fragments = bundle.requirements_toml.enterprise_managed.len(),
                    "Cloud config bundle load completed"
                );
                emit_load_metric("startup", "success", Some(bundle));
            }
            None => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "Cloud config bundle load completed (none)"
                );
                emit_load_metric("startup", "success", /*bundle*/ None);
            }
        }

        Ok(result)
    }

    async fn fetch(&self) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        let Some(auth) = self.auth_manager.auth().await else {
            return Ok(None);
        };
        if !cloud_requirements_eligible_auth(&auth) {
            return Ok(None);
        }

        let (chatgpt_user_id, account_id) = auth_identity(&auth);
        match self
            .load_cache(chatgpt_user_id.as_deref(), account_id.as_deref())
            .await
        {
            Ok(signed_payload) => {
                if let Err(err) = validate_bundle(&signed_payload.bundle, &self.codex_home) {
                    tracing::warn!(
                        path = %self.cache_path.display(),
                        error = %err,
                        "Ignoring invalid cached cloud config bundle"
                    );
                    self.log_cache_load_status(&CacheLoadStatus::CacheInvalidBundle);
                } else {
                    tracing::info!(
                        path = %self.cache_path.display(),
                        "Using cached cloud config bundle"
                    );
                    return Ok(optional_bundle(signed_payload.bundle));
                }
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
    ) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        let mut attempt = 1;
        let mut last_status_code: Option<u16> = None;
        let mut auth_recovery = self.auth_manager.unauthorized_recovery();

        while attempt <= CLOUD_REQUIREMENTS_MAX_ATTEMPTS {
            let bundle = match self.fetcher.fetch_requirements(&auth).await {
                Ok(bundle) => {
                    emit_fetch_attempt_metric(
                        trigger, attempt, "success", /*status_code*/ None,
                    );
                    bundle
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
                            "Failed to fetch cloud config bundle; retrying"
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
                            "Cloud config bundle request was unauthorized; attempting auth recovery"
                        );
                        match auth_recovery.next().await {
                            Ok(_) => {
                                let Some(refreshed_auth) = self.auth_manager.auth().await else {
                                    tracing::error!(
                                        "Auth recovery succeeded but no auth is available for cloud config bundle"
                                    );
                                    emit_fetch_final_metric(
                                        trigger,
                                        "error",
                                        "auth_recovery_missing_auth",
                                        attempt,
                                        status_code,
                                        /*bundle*/ None,
                                    );
                                    return Err(CloudConfigBundleLoadError::new(
                                        CloudConfigBundleLoadErrorCode::Auth,
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
                                    "Failed to recover from unauthorized cloud config bundle request"
                                );
                                emit_fetch_final_metric(
                                    trigger,
                                    "error",
                                    "auth_recovery_unrecoverable",
                                    attempt,
                                    status_code,
                                    /*bundle*/ None,
                                );
                                return Err(CloudConfigBundleLoadError::new(
                                    CloudConfigBundleLoadErrorCode::Auth,
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
                                        "Failed to recover from unauthorized cloud config bundle request; retrying"
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
                        "Cloud config bundle request was unauthorized and no auth recovery is available"
                    );
                    emit_fetch_final_metric(
                        trigger,
                        "error",
                        "auth_recovery_unavailable",
                        attempt,
                        status_code,
                        /*bundle*/ None,
                    );
                    return Err(CloudConfigBundleLoadError::new(
                        CloudConfigBundleLoadErrorCode::Auth,
                        status_code,
                        CLOUD_REQUIREMENTS_AUTH_RECOVERY_FAILED_MESSAGE,
                    ));
                }
            };

            if let Err(err) = validate_bundle(&bundle, &self.codex_home) {
                emit_fetch_final_metric(
                    trigger,
                    "error",
                    "invalid_bundle",
                    attempt,
                    /*status_code*/ None,
                    /*bundle*/ None,
                );
                return Err(err);
            }

            let (chatgpt_user_id, account_id) = auth_identity(&auth);
            if let Err(err) = self
                .save_cache(chatgpt_user_id, account_id, bundle.clone())
                .await
            {
                tracing::warn!(
                    error = %err,
                    "Failed to write cloud config bundle cache"
                );
            }

            emit_fetch_final_metric(
                trigger,
                "success",
                "none",
                attempt,
                /*status_code*/ None,
                Some(&bundle),
            );
            return Ok(optional_bundle(bundle));
        }

        emit_fetch_final_metric(
            trigger,
            "error",
            "request_retry_exhausted",
            CLOUD_REQUIREMENTS_MAX_ATTEMPTS,
            last_status_code,
            /*bundle*/ None,
        );
        tracing::error!(
            path = %self.cache_path.display(),
            "{CLOUD_REQUIREMENTS_LOAD_FAILED_MESSAGE}"
        );
        Err(CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::RequestFailed,
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
                        "Timed out refreshing cloud config bundle cache from remote; keeping existing cache"
                    );
                    emit_load_metric("refresh", "error", /*bundle*/ None);
                }
            }
        }
    }

    async fn refresh_cache(&self) -> bool {
        let Some(auth) = self.auth_manager.auth().await else {
            return false;
        };
        if !cloud_requirements_eligible_auth(&auth) {
            return false;
        }

        match self.fetch_with_retries(auth, "refresh").await {
            Ok(bundle) => emit_load_metric("refresh", "success", bundle.as_ref()),
            Err(err) => {
                tracing::error!(
                    path = %self.cache_path.display(),
                    error = %err,
                    "Failed to refresh cloud config bundle cache from remote"
                );
                emit_load_metric("refresh", "error", /*bundle*/ None);
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
        if cache_file.signed_payload.version != CLOUD_CONFIG_BUNDLE_CACHE_VERSION {
            return Err(CacheLoadStatus::CacheVersionUnsupported(
                cache_file.signed_payload.version,
            ));
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
        bundle: CloudConfigBundle,
    ) -> Result<(), CloudRequirementsError> {
        let now = Utc::now();
        let expires_at = now
            .checked_add_signed(
                ChronoDuration::from_std(CLOUD_REQUIREMENTS_CACHE_TTL)
                    .map_err(|_| CloudRequirementsError::CacheWrite)?,
            )
            .ok_or(CloudRequirementsError::CacheWrite)?;
        let signed_payload = CloudRequirementsCacheSignedPayload {
            version: CLOUD_CONFIG_BUNDLE_CACHE_VERSION,
            cached_at: now,
            expires_at,
            chatgpt_user_id,
            account_id,
            bundle,
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

pub fn cloud_config_bundle_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
) -> CloudConfigBundleLoader {
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
        tracing::warn!("cloud config bundle refresher task slot was poisoned");
        err.into_inner()
    });
    if let Some(existing_task) = refresher_guard.replace(refresh_task) {
        existing_task.abort();
    }
    CloudConfigBundleLoader::new(async move {
        task.await.map_err(|err| {
            tracing::error!(error = %err, "Cloud config bundle task failed");
            CloudConfigBundleLoadError::new(
                CloudConfigBundleLoadErrorCode::Internal,
                /*status_code*/ None,
                format!("cloud config bundle load failed: {err}"),
            )
        })?
    })
}

pub async fn cloud_config_bundle_loader_for_storage(
    codex_home: PathBuf,
    enable_codex_api_key_env: bool,
    credentials_store_mode: AuthCredentialsStoreMode,
    chatgpt_base_url: String,
) -> CloudConfigBundleLoader {
    let auth_manager = AuthManager::shared(
        codex_home.clone(),
        enable_codex_api_key_env,
        credentials_store_mode,
        Some(chatgpt_base_url.clone()),
    )
    .await;
    cloud_config_bundle_loader(auth_manager, chatgpt_base_url, codex_home)
}

fn validate_bundle(
    bundle: &CloudConfigBundle,
    codex_home: &Path,
) -> Result<(), CloudConfigBundleLoadError> {
    let base_dir = AbsolutePathBuf::from_absolute_path(codex_home).map_err(|err| {
        CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::Internal,
            /*status_code*/ None,
            format!("failed to validate cloud config bundle base path: {err}"),
        )
    })?;
    let bundle_layers =
        CloudConfigBundleLayers::from_bundle(bundle.clone(), &base_dir).map_err(|err| {
            CloudConfigBundleLoadError::new(
                CloudConfigBundleLoadErrorCode::InvalidBundle,
                /*status_code*/ None,
                format!("invalid cloud config bundle: {err}"),
            )
        })?;
    let CloudConfigBundleLayers {
        enterprise_managed_config: _,
        enterprise_managed_requirements,
    } = bundle_layers;

    compose_requirements(enterprise_managed_requirements).map_err(|err| {
        CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::InvalidBundle,
            /*status_code*/ None,
            format!("invalid cloud config bundle: {err}"),
        )
    })?;

    Ok(())
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
    bundle: Option<&CloudConfigBundle>,
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
            ("bundle_shape", bundle_shape_tag(bundle)),
        ],
    );
}

fn emit_load_metric(trigger: &str, outcome: &str, bundle: Option<&CloudConfigBundle>) {
    emit_metric(
        CLOUD_REQUIREMENTS_LOAD_METRIC,
        vec![
            ("trigger", trigger.to_string()),
            ("outcome", outcome.to_string()),
            ("bundle_shape", bundle_shape_tag(bundle)),
        ],
    );
}

fn bundle_shape_tag(bundle: Option<&CloudConfigBundle>) -> String {
    let Some(bundle) = bundle else {
        return "none".to_string();
    };

    let mut sources = Vec::new();
    if !bundle.config_toml.enterprise_managed.is_empty() {
        sources.push("enterprise_config");
    }
    if !bundle.requirements_toml.enterprise_managed.is_empty() {
        sources.push("enterprise_requirements");
    }

    if sources.is_empty() {
        "empty".to_string()
    } else {
        sources.sort_unstable();
        sources.join(",")
    }
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
    use codex_backend_client::ConfigBundleResponse;
    use codex_backend_client::DeliveredTomlFragment;
    use codex_config::CloudConfigFragment;
    use codex_config::CloudConfigTomlBundle;
    use codex_config::CloudRequirementsFragment;
    use codex_config::CloudRequirementsTomlBundle;
    use codex_config::types::AuthCredentialsStoreMode;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::future::pending;
    use std::path::Path;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;
    use tempfile::tempdir;

    fn write_auth_json(codex_home: &Path, value: serde_json::Value) -> std::io::Result<()> {
        std::fs::write(codex_home.join("auth.json"), serde_json::to_string(&value)?)?;
        Ok(())
    }

    async fn auth_manager_with_api_key() -> Arc<AuthManager> {
        let tmp = tempdir().expect("tempdir");
        let auth_json = json!({
            "OPENAI_API_KEY": "sk-test-key",
            "tokens": null,
            "last_refresh": null,
        });
        write_auth_json(tmp.path(), auth_json).expect("write auth");
        Arc::new(
            AuthManager::new(
                tmp.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                /*chatgpt_base_url*/ None,
            )
            .await,
        )
    }

    async fn auth_manager_with_plan_and_identity(
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
        Arc::new(
            AuthManager::new(
                tmp.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                /*chatgpt_base_url*/ None,
            )
            .await,
        )
    }

    async fn auth_manager_with_plan(plan_type: &str) -> Arc<AuthManager> {
        auth_manager_with_plan_and_identity(plan_type, Some("user-12345"), Some("account-12345"))
            .await
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

    fn test_bundle() -> CloudConfigBundle {
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                enterprise_managed: vec![test_config_fragment()],
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![test_requirements_fragment()],
            },
        }
    }

    fn test_config_fragment() -> CloudConfigFragment {
        CloudConfigFragment {
            id: "cfg_1".to_string(),
            name: "Base config".to_string(),
            contents: "model = \"gpt-5\"".to_string(),
        }
    }

    fn test_requirements_fragment() -> CloudRequirementsFragment {
        CloudRequirementsFragment {
            id: "req_1".to_string(),
            name: "Base requirements".to_string(),
            contents: "allowed_approval_policies = [\"never\"]".to_string(),
        }
    }

    fn invalid_config_bundle() -> CloudConfigBundle {
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                enterprise_managed: vec![CloudConfigFragment {
                    id: "cfg_invalid".to_string(),
                    name: "Invalid config".to_string(),
                    contents: "model = [".to_string(),
                }],
            },
            requirements_toml: CloudRequirementsTomlBundle::default(),
        }
    }

    fn request_error() -> FetchAttemptError {
        FetchAttemptError::Retryable(RetryableFailureKind::Request { status_code: None })
    }

    struct StaticFetcher {
        bundle: CloudConfigBundle,
        request_count: AtomicUsize,
    }

    impl StaticFetcher {
        fn new(bundle: CloudConfigBundle) -> Self {
            Self {
                bundle,
                request_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl RequirementsFetcher for StaticFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<CloudConfigBundle, FetchAttemptError> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.bundle.clone())
        }
    }

    struct PendingFetcher;

    #[async_trait]
    impl RequirementsFetcher for PendingFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<CloudConfigBundle, FetchAttemptError> {
            pending::<()>().await;
            Ok(CloudConfigBundle::default())
        }
    }

    struct SequenceFetcher {
        responses: tokio::sync::Mutex<VecDeque<Result<CloudConfigBundle, FetchAttemptError>>>,
        request_count: AtomicUsize,
    }

    impl SequenceFetcher {
        fn new(responses: Vec<Result<CloudConfigBundle, FetchAttemptError>>) -> Self {
            Self {
                responses: tokio::sync::Mutex::new(VecDeque::from(responses)),
                request_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl RequirementsFetcher for SequenceFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<CloudConfigBundle, FetchAttemptError> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            let mut responses = self.responses.lock().await;
            responses
                .pop_front()
                .unwrap_or_else(|| Ok(CloudConfigBundle::default()))
        }
    }

    struct TokenFetcher {
        expected_token: String,
        bundle: CloudConfigBundle,
        request_count: AtomicUsize,
    }

    #[async_trait]
    impl RequirementsFetcher for TokenFetcher {
        async fn fetch_requirements(
            &self,
            auth: &CodexAuth,
        ) -> Result<CloudConfigBundle, FetchAttemptError> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            if matches!(
                auth.get_token().as_deref(),
                Ok(token) if token == self.expected_token.as_str()
            ) {
                Ok(self.bundle.clone())
            } else {
                Err(FetchAttemptError::Unauthorized {
                    status_code: Some(401),
                    message: "GET /config/bundle failed: 401".to_string(),
                })
            }
        }
    }

    struct UnauthorizedFetcher {
        message: String,
        request_count: AtomicUsize,
    }

    #[async_trait]
    impl RequirementsFetcher for UnauthorizedFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<CloudConfigBundle, FetchAttemptError> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            Err(FetchAttemptError::Unauthorized {
                status_code: Some(401),
                message: self.message.clone(),
            })
        }
    }

    #[test]
    fn bundle_shape_tag_describes_sorted_enterprise_sources() {
        assert_eq!(bundle_shape_tag(/*bundle*/ None), "none");
        assert_eq!(
            bundle_shape_tag(Some(&CloudConfigBundle::default())),
            "empty"
        );
        assert_eq!(
            bundle_shape_tag(Some(&CloudConfigBundle {
                config_toml: CloudConfigTomlBundle {
                    enterprise_managed: vec![test_config_fragment()],
                },
                requirements_toml: CloudRequirementsTomlBundle::default(),
            })),
            "enterprise_config"
        );
        assert_eq!(
            bundle_shape_tag(Some(&CloudConfigBundle {
                config_toml: CloudConfigTomlBundle::default(),
                requirements_toml: CloudRequirementsTomlBundle {
                    enterprise_managed: vec![test_requirements_fragment()],
                },
            })),
            "enterprise_requirements"
        );
        assert_eq!(
            bundle_shape_tag(Some(&CloudConfigBundle {
                config_toml: CloudConfigTomlBundle {
                    enterprise_managed: vec![test_config_fragment()],
                },
                requirements_toml: CloudRequirementsTomlBundle {
                    enterprise_managed: vec![test_requirements_fragment()],
                },
            })),
            "enterprise_config,enterprise_requirements"
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_chatgpt_auth() {
        let fetcher = Arc::new(StaticFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_api_key().await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_non_business_or_enterprise_plan() {
        let fetcher = Arc::new(StaticFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("pro").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_skips_team_like_usage_based_plan() {
        let fetcher = Arc::new(StaticFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("self_serve_business_usage_based").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_allows_business_plan() {
        let bundle = test_bundle();
        let codex_home = tempdir().expect("tempdir");
        let fetcher = Arc::new(StaticFetcher::new(bundle.clone()));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(bundle.clone())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
        assert!(
            codex_home
                .path()
                .join(CLOUD_REQUIREMENTS_CACHE_FILENAME)
                .exists()
        );
    }

    #[tokio::test]
    async fn fetch_requirements_rejects_invalid_remote_bundle_before_cache_write() {
        let codex_home = tempdir().expect("tempdir");
        let fetcher = Arc::new(StaticFetcher::new(invalid_config_bundle()));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let err = service
            .fetch()
            .await
            .expect_err("invalid remote bundle should fail closed");

        assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::InvalidBundle);
        assert!(err.to_string().contains("invalid cloud config bundle"));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
        assert!(
            !codex_home
                .path()
                .join(CLOUD_REQUIREMENTS_CACHE_FILENAME)
                .exists()
        );
    }

    #[tokio::test]
    async fn fetch_requirements_ignores_invalid_cache_and_refetches() {
        let codex_home = tempdir().expect("tempdir");
        let replacement_bundle = test_bundle();
        let fetcher = Arc::new(StaticFetcher::new(replacement_bundle.clone()));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        service
            .save_cache(
                Some("user-12345".to_string()),
                Some("account-12345".to_string()),
                invalid_config_bundle(),
            )
            .await
            .expect("write invalid cache");

        assert_eq!(service.fetch().await, Ok(Some(replacement_bundle.clone())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            service
                .load_cache(Some("user-12345"), Some("account-12345"))
                .await
                .expect("load refreshed cache")
                .bundle,
            replacement_bundle
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_allows_business_like_usage_based_plan() {
        let fetcher = Arc::new(StaticFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise_cbp_usage_based").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_allows_hc_plan_as_enterprise() {
        let fetcher = Arc::new(StaticFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("hc").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_requirements_empty_response_is_success_and_cached() {
        let codex_home = tempdir().expect("tempdir");
        let fetcher = Arc::new(StaticFetcher::new(CloudConfigBundle::default()));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
        assert!(
            codex_home
                .path()
                .join(CLOUD_REQUIREMENTS_CACHE_FILENAME)
                .exists()
        );
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_uses_cache_when_valid() {
        let bundle = test_bundle();
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan("business").await,
            Arc::new(StaticFetcher::new(bundle.clone())),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let fetcher = Arc::new(SequenceFetcher::new(vec![Err(request_error())]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(bundle)));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_ignores_cache_for_different_auth_identity() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                Some("user-12345"),
                Some("account-12345"),
            )
            .await,
            Arc::new(StaticFetcher::new(test_bundle())),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let replacement_bundle = CloudConfigBundle {
            config_toml: CloudConfigTomlBundle::default(),
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![CloudRequirementsFragment {
                    id: "req_2".to_string(),
                    name: "Replacement requirements".to_string(),
                    contents: "allowed_approval_policies = [\"on-request\"]".to_string(),
                }],
            },
        };
        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(replacement_bundle.clone())]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                Some("user-99999"),
                Some("account-12345"),
            )
            .await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(replacement_bundle)));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_times_out() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("enterprise").await,
            Arc::new(PendingFetcher),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let handle = tokio::spawn(async move { service.fetch_with_timeout().await });
        tokio::time::advance(CLOUD_REQUIREMENTS_TIMEOUT + Duration::from_millis(1)).await;

        let result = handle.await.expect("cloud config bundle task");
        let err = result.expect_err("cloud config bundle timeout should fail closed");
        assert!(
            err.to_string()
                .contains("timed out waiting for cloud config bundle")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_cloud_requirements_retries_until_success() {
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Err(request_error()),
            Ok(test_bundle()),
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        let handle = tokio::spawn(async move { service.fetch().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(handle.await.expect("bundle task"), Ok(Some(test_bundle())));
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
        let auth_manager = Arc::new(
            AuthManager::new(
                auth_home.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                /*chatgpt_base_url*/ None,
            )
            .await,
        );

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
        let fetcher = Arc::new(TokenFetcher {
            expected_token: "fresh-access-token".to_string(),
            bundle: test_bundle(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
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
        let auth_manager = Arc::new(
            AuthManager::new(
                auth_home.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                /*chatgpt_base_url*/ None,
            )
            .await,
        );

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
        let fetcher = Arc::new(TokenFetcher {
            expected_token: "fresh-access-token".to_string(),
            bundle: test_bundle(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudRequirementsService::new(
            auth_manager,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        assert_eq!(
            service
                .load_cache(Some("user-99999"), Some("account-12345"))
                .await
                .expect("load cache")
                .bundle,
            test_bundle()
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_surfaces_auth_recovery_message() {
        let auth_home = tempdir().expect("tempdir");
        write_auth_json(
            auth_home.path(),
            chatgpt_auth_json(
                "enterprise",
                Some("user-12345"),
                Some("account-12345"),
                "stale-access-token",
                "test-refresh-token",
            ),
        )
        .expect("write auth");
        let auth_manager = Arc::new(
            AuthManager::new(
                auth_home.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                /*chatgpt_base_url*/ None,
            )
            .await,
        );

        write_auth_json(
            auth_home.path(),
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
            message: "GET /config/bundle failed: 401".to_string(),
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
            .expect_err("cloud config bundle should surface auth recovery errors");
        assert_eq!(
            err.to_string(),
            "Your access token could not be refreshed because you have since logged out or signed in to another account. Please sign in again."
        );
        assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::Auth);
        assert_eq!(err.status_code(), Some(401));
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
        let auth_manager = Arc::new(
            AuthManager::new(
                auth_home.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                /*chatgpt_base_url*/ None,
            )
            .await,
        );

        let fetcher = Arc::new(UnauthorizedFetcher {
            message:
                "GET https://chatgpt.com/backend-api/wham/config/bundle failed: 401; content-type=text/html; body=<html>nope</html>"
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
            .expect_err("cloud config bundle should fail closed");
        assert_eq!(
            err.to_string(),
            CLOUD_REQUIREMENTS_AUTH_RECOVERY_FAILED_MESSAGE
        );
        assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::Auth);
        assert_eq!(err.status_code(), Some(401));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_cloud_requirements_does_not_use_cache_when_auth_identity_is_incomplete() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudRequirementsService::new(
            auth_manager_with_plan("business").await,
            Arc::new(StaticFetcher::new(test_bundle())),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let replacement_bundle = CloudConfigBundle {
            config_toml: CloudConfigTomlBundle::default(),
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![CloudRequirementsFragment {
                    id: "req_2".to_string(),
                    name: "Replacement requirements".to_string(),
                    contents: "allowed_approval_policies = [\"on-request\"]".to_string(),
                }],
            },
        };
        let fetcher = Arc::new(SequenceFetcher::new(vec![Ok(replacement_bundle.clone())]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan_and_identity(
                "business",
                /*chatgpt_user_id*/ None,
                Some("account-12345"),
            )
            .await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(replacement_bundle)));
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
            auth_manager_with_plan("enterprise").await,
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
            .expect("cloud config bundle task")
            .expect_err("cloud config bundle retry exhaustion should fail closed");
        assert_eq!(err.to_string(), CLOUD_REQUIREMENTS_LOAD_FAILED_MESSAGE);
        assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::RequestFailed);
        assert_eq!(
            fetcher.request_count.load(Ordering::SeqCst),
            CLOUD_REQUIREMENTS_MAX_ATTEMPTS
        );
    }

    #[tokio::test]
    async fn refresh_from_remote_updates_cached_bundle() {
        let replacement_bundle = CloudConfigBundle {
            config_toml: CloudConfigTomlBundle::default(),
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![CloudRequirementsFragment {
                    id: "req_2".to_string(),
                    name: "Replacement requirements".to_string(),
                    contents: "allowed_approval_policies = [\"on-request\"]".to_string(),
                }],
            },
        };
        let codex_home = tempdir().expect("tempdir");
        let fetcher = Arc::new(SequenceFetcher::new(vec![
            Ok(test_bundle()),
            Ok(replacement_bundle.clone()),
        ]));
        let service = CloudRequirementsService::new(
            auth_manager_with_plan("business").await,
            fetcher,
            codex_home.path().to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        assert!(service.refresh_cache().await);

        let signed_payload = service
            .load_cache(Some("user-12345"), Some("account-12345"))
            .await
            .expect("load cache");
        assert_eq!(signed_payload.bundle, replacement_bundle);
    }

    #[test]
    fn bundle_response_conversion_preserves_fragment_order() {
        let response = ConfigBundleResponse {
            config_toml: Some(Some(Box::new(codex_backend_client::DeliveredConfigToml {
                enterprise_managed: Some(Some(vec![
                    DeliveredTomlFragment::new(
                        "cfg_high".to_string(),
                        "High config".to_string(),
                        "model = \"high\"".to_string(),
                    ),
                    DeliveredTomlFragment::new(
                        "cfg_low".to_string(),
                        "Low config".to_string(),
                        "model = \"low\"".to_string(),
                    ),
                ])),
            }))),
            requirements_toml: Some(Some(Box::new(
                codex_backend_client::DeliveredRequirementsToml {
                    enterprise_managed: Some(Some(vec![DeliveredTomlFragment::new(
                        "req_high".to_string(),
                        "High requirements".to_string(),
                        "allowed_approval_policies = [\"never\"]".to_string(),
                    )])),
                },
            ))),
        };

        assert_eq!(
            bundle_from_response(response),
            CloudConfigBundle {
                config_toml: CloudConfigTomlBundle {
                    enterprise_managed: vec![
                        CloudConfigFragment {
                            id: "cfg_high".to_string(),
                            name: "High config".to_string(),
                            contents: "model = \"high\"".to_string(),
                        },
                        CloudConfigFragment {
                            id: "cfg_low".to_string(),
                            name: "Low config".to_string(),
                            contents: "model = \"low\"".to_string(),
                        },
                    ],
                },
                requirements_toml: CloudRequirementsTomlBundle {
                    enterprise_managed: vec![CloudRequirementsFragment {
                        id: "req_high".to_string(),
                        name: "High requirements".to_string(),
                        contents: "allowed_approval_policies = [\"never\"]".to_string(),
                    }],
                },
            }
        );
    }

    #[test]
    fn bundle_response_conversion_treats_missing_sections_as_empty() {
        assert_eq!(
            bundle_from_response(ConfigBundleResponse::new()),
            CloudConfigBundle::default()
        );
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use codex_config::CloudConfigFragment;
    use codex_config::CloudConfigTomlBundle;
    use codex_config::CloudRequirementsFragment;
    use codex_config::CloudRequirementsTomlBundle;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn test_bundle() -> CloudConfigBundle {
        CloudConfigBundle {
            config_toml: CloudConfigTomlBundle {
                enterprise_managed: vec![CloudConfigFragment {
                    id: "cfg_1".to_string(),
                    name: "Base config".to_string(),
                    contents: "model = \"gpt-5\"".to_string(),
                }],
            },
            requirements_toml: CloudRequirementsTomlBundle {
                enterprise_managed: vec![CloudRequirementsFragment {
                    id: "req_1".to_string(),
                    name: "Base requirements".to_string(),
                    contents: "allowed_approval_policies = [\"never\"]".to_string(),
                }],
            },
        }
    }

    struct NeverFetcher;

    #[async_trait]
    impl RequirementsFetcher for NeverFetcher {
        async fn fetch_requirements(
            &self,
            _auth: &CodexAuth,
        ) -> Result<CloudConfigBundle, FetchAttemptError> {
            panic!("cache tests should not fetch from remote");
        }
    }

    async fn test_service(codex_home: &Path) -> CloudRequirementsService {
        let auth_home = tempdir().expect("tempdir");
        let auth_manager = Arc::new(
            AuthManager::new(
                auth_home.path().to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                AuthCredentialsStoreMode::File,
                /*chatgpt_base_url*/ None,
            )
            .await,
        );
        CloudRequirementsService::new(
            auth_manager,
            Arc::new(NeverFetcher),
            codex_home.to_path_buf(),
            CLOUD_REQUIREMENTS_TIMEOUT,
        )
    }

    fn signed_cache_file(
        signed_payload: CloudRequirementsCacheSignedPayload,
    ) -> CloudRequirementsCacheFile {
        let payload_bytes = cache_payload_bytes(&signed_payload).expect("payload bytes");
        CloudRequirementsCacheFile {
            signature: sign_cache_payload(&payload_bytes).expect("signature"),
            signed_payload,
        }
    }

    fn valid_signed_payload() -> CloudRequirementsCacheSignedPayload {
        let cached_at = Utc::now();
        CloudRequirementsCacheSignedPayload {
            version: CLOUD_CONFIG_BUNDLE_CACHE_VERSION,
            cached_at,
            expires_at: cached_at + ChronoDuration::minutes(30),
            chatgpt_user_id: Some("user-12345".to_string()),
            account_id: Some("account-12345".to_string()),
            bundle: test_bundle(),
        }
    }

    fn write_cache_file(cache_path: &Path, cache_file: &CloudRequirementsCacheFile) {
        std::fs::write(
            cache_path,
            serde_json::to_vec_pretty(cache_file).expect("serialize cache"),
        )
        .expect("write cache");
    }

    #[tokio::test]
    async fn save_writes_signed_payload_and_loads_for_matching_identity() {
        let codex_home = tempdir().expect("tempdir");
        let service = test_service(codex_home.path()).await;
        let bundle = test_bundle();

        service
            .save_cache(
                Some("user-12345".to_string()),
                Some("account-12345".to_string()),
                bundle.clone(),
            )
            .await
            .expect("save cache");

        let cache_file: CloudRequirementsCacheFile =
            serde_json::from_slice(&std::fs::read(&service.cache_path).expect("read cache"))
                .expect("parse cache");
        assert_eq!(cache_file.signed_payload.version, 1);
        assert_eq!(
            cache_file.signed_payload.chatgpt_user_id,
            Some("user-12345".to_string())
        );
        assert_eq!(
            cache_file.signed_payload.account_id,
            Some("account-12345".to_string())
        );
        assert_eq!(cache_file.signed_payload.bundle, bundle);
        assert!(
            cache_file.signed_payload.expires_at
                <= cache_file.signed_payload.cached_at + ChronoDuration::minutes(30)
        );
        assert!(cache_file.signed_payload.expires_at > cache_file.signed_payload.cached_at);
        assert!(verify_cache_signature(
            &cache_payload_bytes(&cache_file.signed_payload).expect("payload bytes"),
            &cache_file.signature,
        ));

        assert_eq!(
            service
                .load_cache(Some("user-12345"), Some("account-12345"))
                .await,
            Ok(cache_file.signed_payload)
        );
    }

    #[tokio::test]
    async fn load_rejects_missing_request_identity_before_reading_cache_file() {
        let codex_home = tempdir().expect("tempdir");
        let service = test_service(codex_home.path()).await;

        assert_eq!(
            service
                .load_cache(/*chatgpt_user_id*/ None, Some("account-12345"))
                .await,
            Err(CacheLoadStatus::AuthIdentityIncomplete)
        );
        assert_eq!(
            service
                .load_cache(Some("user-12345"), /*account_id*/ None)
                .await,
            Err(CacheLoadStatus::AuthIdentityIncomplete)
        );
    }

    #[tokio::test]
    async fn load_reports_missing_and_malformed_cache_files() {
        let codex_home = tempdir().expect("tempdir");
        let service = test_service(codex_home.path()).await;

        assert_eq!(
            service
                .load_cache(Some("user-12345"), Some("account-12345"))
                .await,
            Err(CacheLoadStatus::CacheFileNotFound)
        );

        std::fs::write(&service.cache_path, "{").expect("write malformed cache");
        assert!(matches!(
            service
                .load_cache(Some("user-12345"), Some("account-12345"))
                .await,
            Err(CacheLoadStatus::CacheParseFailed(_))
        ));
    }

    #[tokio::test]
    async fn load_rejects_tampered_payload() {
        let codex_home = tempdir().expect("tempdir");
        let service = test_service(codex_home.path()).await;
        let mut cache_file = signed_cache_file(valid_signed_payload());
        cache_file
            .signed_payload
            .bundle
            .requirements_toml
            .enterprise_managed[0]
            .contents = "allowed_approval_policies = [\"on-request\"]".to_string();
        write_cache_file(&service.cache_path, &cache_file);

        assert_eq!(
            service
                .load_cache(Some("user-12345"), Some("account-12345"))
                .await,
            Err(CacheLoadStatus::CacheSignatureInvalid)
        );
    }

    #[tokio::test]
    async fn load_rejects_cache_for_incomplete_or_different_identity() {
        let codex_home = tempdir().expect("tempdir");
        let service = test_service(codex_home.path()).await;
        let cache_file = signed_cache_file(valid_signed_payload());
        write_cache_file(&service.cache_path, &cache_file);

        assert_eq!(
            service
                .load_cache(Some("user-99999"), Some("account-12345"))
                .await,
            Err(CacheLoadStatus::CacheIdentityMismatch)
        );

        let mut signed_payload = valid_signed_payload();
        signed_payload.chatgpt_user_id = None;
        write_cache_file(&service.cache_path, &signed_cache_file(signed_payload));

        assert_eq!(
            service
                .load_cache(Some("user-12345"), Some("account-12345"))
                .await,
            Err(CacheLoadStatus::CacheIdentityIncomplete)
        );
    }

    #[tokio::test]
    async fn load_rejects_expired_cache() {
        let codex_home = tempdir().expect("tempdir");
        let service = test_service(codex_home.path()).await;
        let mut signed_payload = valid_signed_payload();
        signed_payload.expires_at = Utc::now() - ChronoDuration::seconds(1);
        write_cache_file(&service.cache_path, &signed_cache_file(signed_payload));

        assert_eq!(
            service
                .load_cache(Some("user-12345"), Some("account-12345"))
                .await,
            Err(CacheLoadStatus::CacheExpired)
        );
    }

    #[tokio::test]
    async fn load_rejects_unsupported_cache_version() {
        let codex_home = tempdir().expect("tempdir");
        let service = test_service(codex_home.path()).await;
        let mut signed_payload = valid_signed_payload();
        signed_payload.version = 2;
        write_cache_file(&service.cache_path, &signed_cache_file(signed_payload));

        assert_eq!(
            service
                .load_cache(Some("user-12345"), Some("account-12345"))
                .await,
            Err(CacheLoadStatus::CacheVersionUnsupported(2))
        );
    }
}
