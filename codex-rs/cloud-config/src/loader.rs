use crate::cache::CloudConfigBundleCache;
use codex_backend_client::Client as BackendClient;
use codex_backend_client::ConfigBundleResponse;
use codex_backend_client::DeliveredTomlFragment;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::CloudConfigBundleLoader;
use codex_config::CloudConfigFragment;
use codex_config::CloudConfigTomlBundle;
use codex_config::CloudRequirementsFragment;
use codex_config::CloudRequirementsTomlBundle;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::util::backoff;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::RefreshTokenError;
use codex_protocol::account::PlanType;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::timeout;

const CLOUD_CONFIG_BUNDLE_TIMEOUT: Duration = Duration::from_secs(15);
const CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS: usize = 5;
const CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);
const CLOUD_CONFIG_BUNDLE_FETCH_ATTEMPT_METRIC: &str = "codex.cloud_config_bundle.fetch_attempt";
const CLOUD_CONFIG_BUNDLE_FETCH_FINAL_METRIC: &str = "codex.cloud_config_bundle.fetch_final";
const CLOUD_CONFIG_BUNDLE_LOAD_METRIC: &str = "codex.cloud_config_bundle.load";
const CLOUD_CONFIG_BUNDLE_LOAD_FAILED_MESSAGE: &str =
    "Failed to load cloud config bundle (workspace-managed policies).";
const CLOUD_CONFIG_BUNDLE_AUTH_RECOVERY_FAILED_MESSAGE: &str = concat!(
    "Your authentication session could not be refreshed automatically. ",
    "Please log out and sign in again."
);
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

fn auth_identity(auth: &CodexAuth) -> (Option<String>, Option<String>) {
    (auth.get_chatgpt_user_id(), auth.get_account_id())
}

fn cloud_config_eligible_auth(auth: &CodexAuth) -> bool {
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

/// Fetches the raw cloud config bundle for an authenticated Codex account.
///
/// Implementations should return the backend-selected bundle exactly as delivered and leave
/// config/requirements TOML parsing to the config crate's typed composition layer.
trait BundleFetcher: Send + Sync {
    fn fetch_bundle(
        &self,
        auth: &CodexAuth,
    ) -> impl Future<Output = Result<CloudConfigBundle, FetchAttemptError>> + Send;
}

struct BackendBundleFetcher {
    base_url: String,
}

impl BackendBundleFetcher {
    fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

impl BundleFetcher for BackendBundleFetcher {
    async fn fetch_bundle(&self, auth: &CodexAuth) -> Result<CloudConfigBundle, FetchAttemptError> {
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

struct CloudConfigBundleService<F> {
    auth_manager: Arc<AuthManager>,
    fetcher: Arc<F>,
    cache: CloudConfigBundleCache,
    timeout: Duration,
}

impl<F> Clone for CloudConfigBundleService<F> {
    fn clone(&self) -> Self {
        Self {
            auth_manager: Arc::clone(&self.auth_manager),
            fetcher: Arc::clone(&self.fetcher),
            cache: self.cache.clone(),
            timeout: self.timeout,
        }
    }
}

impl<F> CloudConfigBundleService<F>
where
    F: BundleFetcher + 'static,
{
    fn new(
        auth_manager: Arc<AuthManager>,
        fetcher: Arc<F>,
        codex_home: PathBuf,
        timeout: Duration,
    ) -> Self {
        Self {
            auth_manager,
            fetcher,
            cache: CloudConfigBundleCache::new(codex_home),
            timeout,
        }
    }

    async fn fetch_with_timeout(
        &self,
    ) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_config_bundle.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let fetch_result = timeout(self.timeout, self.fetch())
            .await
            .inspect_err(|_| {
                let message = format!(
                    "Timed out waiting for cloud config bundle after {}s",
                    self.timeout.as_secs()
                );
                tracing::error!("{message}");
                emit_load_metric("startup", "error", None);
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

        let result = match fetch_result {
            Ok(result) => result,
            Err(err) => {
                emit_load_metric("startup", "error", None);
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
                emit_load_metric("startup", "success", None);
            }
        }

        Ok(result)
    }

    async fn fetch(&self) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        let Some(auth) = self.auth_manager.auth().await else {
            return Ok(None);
        };
        if !cloud_config_eligible_auth(&auth) {
            return Ok(None);
        }
        let (chatgpt_user_id, account_id) = auth_identity(&auth);

        match self
            .cache
            .load(chatgpt_user_id.as_deref(), account_id.as_deref())
            .await
        {
            Ok(signed_payload) => {
                tracing::info!(
                    path = %self.cache.path().display(),
                    "Using cached cloud config bundle"
                );
                return Ok(optional_bundle(signed_payload.bundle));
            }
            Err(cache_load_status) => {
                self.cache.log_load_status(&cache_load_status);
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

        while attempt <= CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS {
            let bundle = match self.fetcher.fetch_bundle(&auth).await {
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
                    if attempt < CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS {
                        tracing::warn!(
                            status = ?status,
                            attempt,
                            max_attempts = CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS,
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
                            max_attempts = CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS,
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
                                        None,
                                    );
                                    return Err(CloudConfigBundleLoadError::new(
                                        CloudConfigBundleLoadErrorCode::Auth,
                                        status_code,
                                        CLOUD_CONFIG_BUNDLE_AUTH_RECOVERY_FAILED_MESSAGE,
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
                                    None,
                                );
                                return Err(CloudConfigBundleLoadError::new(
                                    CloudConfigBundleLoadErrorCode::Auth,
                                    status_code,
                                    failed.message,
                                ));
                            }
                            Err(RefreshTokenError::Transient(recovery_err)) => {
                                if attempt < CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS {
                                    tracing::warn!(
                                        error = %recovery_err,
                                        attempt,
                                        max_attempts = CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS,
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
                        None,
                    );
                    return Err(CloudConfigBundleLoadError::new(
                        CloudConfigBundleLoadErrorCode::Auth,
                        status_code,
                        CLOUD_CONFIG_BUNDLE_AUTH_RECOVERY_FAILED_MESSAGE,
                    ));
                }
            };

            let (chatgpt_user_id, account_id) = auth_identity(&auth);
            if let Err(err) = self
                .cache
                .save(chatgpt_user_id, account_id, bundle.clone())
                .await
            {
                tracing::warn!(
                    error = %err,
                    "Failed to write cloud config bundle cache"
                );
            }

            emit_fetch_final_metric(trigger, "success", "none", attempt, None, Some(&bundle));
            return Ok(optional_bundle(bundle));
        }

        emit_fetch_final_metric(
            trigger,
            "error",
            "request_retry_exhausted",
            CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS,
            last_status_code,
            None,
        );
        tracing::error!(
            path = %self.cache.path().display(),
            "{CLOUD_CONFIG_BUNDLE_LOAD_FAILED_MESSAGE}"
        );
        Err(CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::RequestFailed,
            last_status_code,
            CLOUD_CONFIG_BUNDLE_LOAD_FAILED_MESSAGE,
        ))
    }

    async fn refresh_cache_in_background(&self) {
        loop {
            sleep(CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_INTERVAL).await;
            match timeout(self.timeout, self.refresh_cache()).await {
                Ok(true) => {}
                Ok(false) => break,
                Err(_) => {
                    tracing::error!(
                        "Timed out refreshing cloud config bundle cache from remote; keeping existing cache"
                    );
                    emit_load_metric("refresh", "error", None);
                }
            }
        }
    }

    async fn refresh_cache(&self) -> bool {
        let Some(auth) = self.auth_manager.auth().await else {
            return false;
        };
        if !cloud_config_eligible_auth(&auth) {
            return false;
        }

        match self.fetch_with_retries(auth, "refresh").await {
            Ok(bundle) => emit_load_metric("refresh", "success", bundle.as_ref()),
            Err(err) => {
                tracing::error!(
                    path = %self.cache.path().display(),
                    error = %err,
                    "Failed to refresh cloud config bundle cache from remote"
                );
                emit_load_metric("refresh", "error", None);
            }
        }
        true
    }
}

pub fn cloud_config_bundle_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
) -> CloudConfigBundleLoader {
    let service = CloudConfigBundleService::new(
        auth_manager,
        Arc::new(BackendBundleFetcher::new(chatgpt_base_url)),
        codex_home,
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
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

fn emit_fetch_attempt_metric(
    trigger: &str,
    attempt: usize,
    outcome: &str,
    status_code: Option<u16>,
) {
    let attempt_tag = attempt.to_string();
    let status_code_tag = status_code_tag(status_code);
    emit_metric(
        CLOUD_CONFIG_BUNDLE_FETCH_ATTEMPT_METRIC,
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
        CLOUD_CONFIG_BUNDLE_FETCH_FINAL_METRIC,
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
        CLOUD_CONFIG_BUNDLE_LOAD_METRIC,
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
    use crate::cache::CLOUD_CONFIG_BUNDLE_CACHE_FILENAME;
    use crate::cache::CloudConfigBundleCache;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
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

    fn request_error() -> FetchAttemptError {
        FetchAttemptError::Retryable(RetryableFailureKind::Request { status_code: None })
    }

    struct StaticBundleFetcher {
        bundle: CloudConfigBundle,
        request_count: AtomicUsize,
    }

    impl StaticBundleFetcher {
        fn new(bundle: CloudConfigBundle) -> Self {
            Self {
                bundle,
                request_count: AtomicUsize::new(0),
            }
        }
    }

    impl BundleFetcher for StaticBundleFetcher {
        async fn fetch_bundle(
            &self,
            _auth: &CodexAuth,
        ) -> Result<CloudConfigBundle, FetchAttemptError> {
            self.request_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.bundle.clone())
        }
    }

    struct PendingBundleFetcher;

    impl BundleFetcher for PendingBundleFetcher {
        async fn fetch_bundle(
            &self,
            _auth: &CodexAuth,
        ) -> Result<CloudConfigBundle, FetchAttemptError> {
            pending::<()>().await;
            Ok(CloudConfigBundle::default())
        }
    }

    struct SequenceBundleFetcher {
        responses: tokio::sync::Mutex<VecDeque<Result<CloudConfigBundle, FetchAttemptError>>>,
        request_count: AtomicUsize,
    }

    impl SequenceBundleFetcher {
        fn new(responses: Vec<Result<CloudConfigBundle, FetchAttemptError>>) -> Self {
            Self {
                responses: tokio::sync::Mutex::new(VecDeque::from(responses)),
                request_count: AtomicUsize::new(0),
            }
        }
    }

    impl BundleFetcher for SequenceBundleFetcher {
        async fn fetch_bundle(
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

    struct TokenBundleFetcher {
        expected_token: String,
        bundle: CloudConfigBundle,
        request_count: AtomicUsize,
    }

    impl BundleFetcher for TokenBundleFetcher {
        async fn fetch_bundle(
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

    struct UnauthorizedBundleFetcher {
        message: String,
        request_count: AtomicUsize,
    }

    impl BundleFetcher for UnauthorizedBundleFetcher {
        async fn fetch_bundle(
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
        assert_eq!(bundle_shape_tag(None), "none");
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
    async fn fetch_bundle_skips_non_chatgpt_auth() {
        let fetcher = Arc::new(StaticBundleFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_api_key().await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_bundle_skips_non_business_or_enterprise_plan() {
        let fetcher = Arc::new(StaticBundleFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("pro").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_bundle_skips_team_like_usage_based_plan() {
        let fetcher = Arc::new(StaticBundleFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("self_serve_business_usage_based").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_bundle_allows_business_plan_and_writes_cache() {
        let bundle = test_bundle();
        let codex_home = tempdir().expect("tempdir");
        let fetcher = Arc::new(StaticBundleFetcher::new(bundle.clone()));
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("business").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(bundle.clone())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
        assert!(
            codex_home
                .path()
                .join(CLOUD_CONFIG_BUNDLE_CACHE_FILENAME)
                .exists()
        );
    }

    #[tokio::test]
    async fn fetch_bundle_allows_business_like_usage_based_plan() {
        let fetcher = Arc::new(StaticBundleFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("enterprise_cbp_usage_based").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_bundle_allows_hc_plan_as_enterprise() {
        let fetcher = Arc::new(StaticBundleFetcher::new(test_bundle()));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("hc").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_bundle_empty_response_is_success_and_cached() {
        let codex_home = tempdir().expect("tempdir");
        let fetcher = Arc::new(StaticBundleFetcher::new(CloudConfigBundle::default()));
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("enterprise").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(None));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
        assert!(
            codex_home
                .path()
                .join(CLOUD_CONFIG_BUNDLE_CACHE_FILENAME)
                .exists()
        );
    }

    #[tokio::test]
    async fn fetch_bundle_uses_cache_when_valid() {
        let bundle = test_bundle();
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudConfigBundleService::new(
            auth_manager_with_plan("business").await,
            Arc::new(StaticBundleFetcher::new(bundle.clone())),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );
        let _ = prime_service.fetch().await;

        let fetcher = Arc::new(SequenceBundleFetcher::new(vec![Err(request_error())]));
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("business").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(bundle)));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_bundle_ignores_cache_for_different_auth_identity() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudConfigBundleService::new(
            auth_manager_with_plan_and_identity(
                "business",
                Some("user-12345"),
                Some("account-12345"),
            )
            .await,
            Arc::new(StaticBundleFetcher::new(test_bundle())),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
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
        let fetcher = Arc::new(SequenceBundleFetcher::new(vec![Ok(
            replacement_bundle.clone()
        )]));
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan_and_identity(
                "business",
                Some("user-99999"),
                Some("account-12345"),
            )
            .await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(replacement_bundle)));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_bundle_times_out() {
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("enterprise").await,
            Arc::new(PendingBundleFetcher),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );
        let handle = tokio::spawn(async move { service.fetch_with_timeout().await });
        tokio::time::advance(CLOUD_CONFIG_BUNDLE_TIMEOUT + Duration::from_millis(1)).await;

        let result = handle.await.expect("cloud config bundle task");
        let err = result.expect_err("cloud config bundle timeout should fail closed");
        assert!(
            err.to_string()
                .contains("timed out waiting for cloud config bundle")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_bundle_retries_until_success() {
        let fetcher = Arc::new(SequenceBundleFetcher::new(vec![
            Err(request_error()),
            Ok(test_bundle()),
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("business").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        let handle = tokio::spawn(async move { service.fetch().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(handle.await.expect("bundle task"), Ok(Some(test_bundle())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fetch_bundle_recovers_after_unauthorized_reload() {
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
        let fetcher = Arc::new(TokenBundleFetcher {
            expected_token: "fresh-access-token".to_string(),
            bundle: test_bundle(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fetch_bundle_recovers_after_unauthorized_reload_updates_cache_identity() {
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
        let fetcher = Arc::new(TokenBundleFetcher {
            expected_token: "fresh-access-token".to_string(),
            bundle: test_bundle(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());
        assert_eq!(
            cache
                .load(Some("user-99999"), Some("account-12345"))
                .await
                .expect("load cache")
                .bundle,
            test_bundle()
        );
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn fetch_bundle_surfaces_auth_recovery_message() {
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
        let fetcher = Arc::new(UnauthorizedBundleFetcher {
            message: "GET /config/bundle failed: 401".to_string(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
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
    async fn fetch_bundle_unauthorized_without_recovery_uses_generic_message() {
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

        let fetcher = Arc::new(UnauthorizedBundleFetcher {
            message:
                "GET https://chatgpt.com/backend-api/wham/config/bundle failed: 401; content-type=text/html; body=<html>nope</html>"
                    .to_string(),
            request_count: AtomicUsize::new(0),
        });
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        let err = service
            .fetch()
            .await
            .expect_err("cloud config bundle should fail closed");
        assert_eq!(
            err.to_string(),
            CLOUD_CONFIG_BUNDLE_AUTH_RECOVERY_FAILED_MESSAGE
        );
        assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::Auth);
        assert_eq!(err.status_code(), Some(401));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fetch_bundle_does_not_use_cache_when_auth_identity_is_incomplete() {
        let codex_home = tempdir().expect("tempdir");
        let prime_service = CloudConfigBundleService::new(
            auth_manager_with_plan("business").await,
            Arc::new(StaticBundleFetcher::new(test_bundle())),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
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
        let fetcher = Arc::new(SequenceBundleFetcher::new(vec![Ok(
            replacement_bundle.clone()
        )]));
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan_and_identity(
                "business",
                /*chatgpt_user_id*/ None,
                Some("account-12345"),
            )
            .await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(replacement_bundle)));
        assert_eq!(fetcher.request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn fetch_bundle_stops_after_max_retries() {
        let fetcher = Arc::new(SequenceBundleFetcher::new(vec![
            Err(request_error());
            CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS
        ]));
        let codex_home = tempdir().expect("tempdir");
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("enterprise").await,
            fetcher.clone(),
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        let handle = tokio::spawn(async move { service.fetch().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(5)).await;
        tokio::task::yield_now().await;

        let err = handle
            .await
            .expect("cloud config bundle task")
            .expect_err("cloud config bundle retry exhaustion should fail closed");
        assert_eq!(err.to_string(), CLOUD_CONFIG_BUNDLE_LOAD_FAILED_MESSAGE);
        assert_eq!(err.code(), CloudConfigBundleLoadErrorCode::RequestFailed);
        assert_eq!(
            fetcher.request_count.load(Ordering::SeqCst),
            CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS
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
        let fetcher = Arc::new(SequenceBundleFetcher::new(vec![
            Ok(test_bundle()),
            Ok(replacement_bundle.clone()),
        ]));
        let service = CloudConfigBundleService::new(
            auth_manager_with_plan("business").await,
            fetcher,
            codex_home.path().to_path_buf(),
            CLOUD_CONFIG_BUNDLE_TIMEOUT,
        );

        assert_eq!(service.fetch().await, Ok(Some(test_bundle())));
        assert!(service.refresh_cache().await);

        let cache = CloudConfigBundleCache::new(codex_home.path().to_path_buf());
        let signed_payload = cache
            .load(Some("user-12345"), Some("account-12345"))
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
