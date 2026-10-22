//! Cloud config bundle lifecycle orchestration.
//!
//! Startup loads a shared bundle from cache or backend, and background refresh
//! updates both the on-disk cache and the bundle observed by future config loads.
//! One-shot network loads can disable disk-cache reads and writes.

use crate::backend::BundleClient;
use crate::backend::BundleRequestError;
use crate::backend::RetryableFailureKind;
use crate::cache::CacheLoadStatus;
use crate::cache::CloudConfigBundleCache;
use crate::chatgpt::ChatGptSource;
use crate::config_source::BundleSource;
use crate::metrics::emit_fetch_attempt_metric;
use crate::metrics::emit_fetch_final_metric;
use crate::metrics::emit_load_metric;
use crate::validation::validate_bundle;
use codex_async_utils::backoff;
use codex_config::AbsolutePathBuf;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::CloudConfigBundlePolicy;
use codex_config::CloudConfigBundleSnapshot;
use codex_login::AuthManager;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tokio::time::sleep;
use tokio::time::timeout;

pub(crate) const CLOUD_CONFIG_BUNDLE_TIMEOUT: Duration = Duration::from_secs(20);
const CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS: usize = 5;
const CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_INTERVAL: Duration = Duration::from_secs(15 * 60);
const CLOUD_CONFIG_BUNDLE_TIMEOUT_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const CLOUD_CONFIG_BUNDLE_LOAD_FAILED_MESSAGE: &str =
    "Failed to load cloud config bundle (workspace-managed policies).";
fn optional_bundle(bundle: CloudConfigBundle) -> Option<CloudConfigBundle> {
    if bundle.is_empty() {
        None
    } else {
        Some(bundle)
    }
}

enum CachedBundleLookup {
    Hit(Option<CloudConfigBundle>),
    Miss,
}

pub(crate) struct CloudConfigBundleService<S: BundleSource> {
    source: S,
    cache: CloudConfigBundleCache,
    cache_enabled: bool,
    codex_home: AbsolutePathBuf,
    timeout: Duration,
    latest_bundle: OnceCell<Mutex<CloudConfigBundleSnapshot>>,
    pub(crate) policy: CloudConfigBundlePolicy,
}

impl<C: BundleClient + 'static> CloudConfigBundleService<ChatGptSource<C>> {
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        client: Arc<C>,
        codex_home: PathBuf,
        timeout: Duration,
    ) -> Self {
        Self::from_source(
            ChatGptSource {
                auth_manager,
                client,
            },
            codex_home,
            timeout,
        )
    }
}

impl<S: BundleSource + 'static> CloudConfigBundleService<S> {
    pub(crate) fn from_source(source: S, codex_home: PathBuf, timeout: Duration) -> Self {
        let codex_home = AbsolutePathBuf::resolve_path_against_base(codex_home, "/");
        let cache = match source.cache_namespace() {
            Some(namespace) => CloudConfigBundleCache::for_source(codex_home.clone(), &namespace),
            None => CloudConfigBundleCache::new(codex_home.clone()),
        };
        Self {
            source,
            cache,
            cache_enabled: true,
            codex_home,
            timeout,
            latest_bundle: OnceCell::new(),
            policy: CloudConfigBundlePolicy::default(),
        }
    }

    pub(crate) fn without_cache(mut self) -> Self {
        self.cache_enabled = false;
        self
    }

    pub(crate) async fn get_latest(
        &self,
    ) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        self.get_latest_snapshot().await.bundle
    }

    pub(crate) async fn get_latest_snapshot(&self) -> CloudConfigBundleSnapshot {
        self.latest_snapshot().await.lock().await.clone()
    }

    async fn latest_snapshot(&self) -> &Mutex<CloudConfigBundleSnapshot> {
        self.latest_bundle
            .get_or_init(|| async {
                let mut snapshot = CloudConfigBundleSnapshot {
                    config_server: None,
                    bundle: self.load_startup_bundle_with_timeout().await,
                    binding: None,
                };
                self.policy.publish_snapshot(&mut snapshot);
                Mutex::new(snapshot)
            })
            .await
    }

    pub(crate) async fn load_startup_bundle_with_timeout(
        &self,
    ) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_config_bundle.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let load_result = timeout(self.timeout, self.load_startup_bundle())
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

    async fn load_startup_bundle(
        &self,
    ) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        let Some(auth) = self.source.auth().await? else {
            return Ok(None);
        };

        if self.cache_enabled {
            // Startup prefers a valid, identity-matched cache entry. The backend is
            // only consulted on cache miss or invalid cache contents.
            let (chatgpt_user_id, account_id) = self.source.identity(&auth);
            match self
                .load_valid_cached_bundle(chatgpt_user_id.as_deref(), account_id.as_deref())
                .await
            {
                CachedBundleLookup::Hit(bundle) => return Ok(bundle),
                CachedBundleLookup::Miss => {}
            }
        }

        self.fetch_remote_bundle_and_update_cache_with_retries(auth, "startup")
            .await
    }

    async fn load_valid_cached_bundle(
        &self,
        chatgpt_user_id: Option<&str>,
        account_id: Option<&str>,
    ) -> CachedBundleLookup {
        match self.cache.load(chatgpt_user_id, account_id).await {
            Ok(signed_payload) => {
                if let Err(err) = validate_bundle(&signed_payload.bundle, &self.codex_home) {
                    tracing::warn!(
                        path = %self.cache.path().display(),
                        error = %err,
                        "Ignoring invalid cached cloud config bundle"
                    );
                    self.cache
                        .log_load_status(&CacheLoadStatus::CacheInvalidBundle);
                    CachedBundleLookup::Miss
                } else {
                    tracing::info!(
                        path = %self.cache.path().display(),
                        "Using cached cloud config bundle"
                    );
                    CachedBundleLookup::Hit(optional_bundle(signed_payload.bundle))
                }
            }
            Err(cache_load_status) => {
                self.cache.log_load_status(&cache_load_status);
                CachedBundleLookup::Miss
            }
        }
    }

    async fn fetch_remote_bundle_and_update_cache_with_retries(
        &self,
        mut auth: S::Auth,
        trigger: &'static str,
    ) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        let mut context = self.source.begin_fetch();
        let mut attempt = 1;
        let mut last_status_code: Option<u16> = None;

        while attempt <= CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS {
            match self
                .fetch_with_auth_recovery(&mut auth, &mut context, trigger, attempt)
                .await
            {
                Ok(bundle) => {
                    return self
                        .validate_and_cache_remote_bundle(&auth, trigger, attempt, bundle)
                        .await;
                }
                Err(BundleRequestError::Policy(denied)) => {
                    return Err(CloudConfigBundleLoadError::new(
                        CloudConfigBundleLoadErrorCode::RequestFailed,
                        /*status_code*/ None,
                        denied.to_string(),
                    ));
                }
                Err(BundleRequestError::Retryable(status)) => {
                    last_status_code = status.status_code();
                    if self
                        .retry_after_request_failure(trigger, attempt, status)
                        .await
                    {
                        attempt += 1;
                        continue;
                    }
                }
                Err(BundleRequestError::Fatal {
                    error,
                    metric_reason,
                }) => {
                    if let Some(reason) = metric_reason {
                        emit_fetch_final_metric(
                            trigger,
                            "error",
                            reason,
                            attempt,
                            error.status_code(),
                            /*bundle*/ None,
                        );
                    }
                    return Err(error);
                }
                Err(BundleRequestError::Unauthorized {
                    status_code,
                    message,
                }) => {
                    emit_fetch_final_metric(
                        trigger,
                        "error",
                        "auth",
                        attempt,
                        status_code,
                        /*bundle*/ None,
                    );
                    return Err(CloudConfigBundleLoadError::new(
                        CloudConfigBundleLoadErrorCode::Auth,
                        status_code,
                        message,
                    ));
                }
            }

            break;
        }

        emit_fetch_final_metric(
            trigger,
            "error",
            "request_retry_exhausted",
            CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS,
            last_status_code,
            /*bundle*/ None,
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

    async fn fetch_with_auth_recovery(
        &self,
        auth: &mut S::Auth,
        context: &mut S::FetchContext,
        trigger: &'static str,
        attempt: usize,
    ) -> Result<CloudConfigBundle, BundleRequestError> {
        let mut recovery_attempt = 0;
        loop {
            let result = self.source.get_bundle(auth, context).await;
            let Err(BundleRequestError::Unauthorized { status_code, .. }) = &result else {
                return result;
            };
            emit_fetch_attempt_metric(trigger, attempt, "unauthorized", *status_code);
            if !self
                .source
                .recover_auth(auth, context, *status_code, recovery_attempt)
                .await?
            {
                return result;
            }
            recovery_attempt += 1;
        }
    }

    async fn validate_and_cache_remote_bundle(
        &self,
        auth: &S::Auth,
        trigger: &'static str,
        attempt: usize,
        bundle: CloudConfigBundle,
    ) -> Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError> {
        emit_fetch_attempt_metric(trigger, attempt, "success", /*status_code*/ None);
        let revision = self.policy.observe_remote_bundle(&bundle);
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

        let (chatgpt_user_id, account_id) = self.source.identity(auth);
        if self.cache_enabled
            && let Some(revision) = revision
        {
            let result = match self
                .cache
                .prepare(chatgpt_user_id, account_id, bundle.clone())
                .await
            {
                Ok(staged) => staged.publish_if_current(revision).await,
                Err(err) => Err(err),
            };
            if let Err(err) = result {
                tracing::warn!(error = %err, "Failed to write cloud config bundle cache");
            }
        }

        emit_fetch_final_metric(
            trigger,
            "success",
            "none",
            attempt,
            /*status_code*/ None,
            Some(&bundle),
        );
        Ok(optional_bundle(bundle))
    }

    async fn retry_after_request_failure(
        &self,
        trigger: &'static str,
        attempt: usize,
        status: RetryableFailureKind,
    ) -> bool {
        let status_code = status.status_code();
        if matches!(status, RetryableFailureKind::Request { .. }) {
            emit_fetch_attempt_metric(trigger, attempt, "error", status_code);
        }
        if attempt < CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS {
            tracing::warn!(
                status = ?status,
                attempt,
                max_attempts = CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS,
                "Failed to fetch cloud config bundle; retrying"
            );
            sleep(backoff(attempt as u64)).await;
            true
        } else {
            false
        }
    }

    pub(crate) async fn refresh_cache_in_background(&self) {
        loop {
            let mut refresh_interval = CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_INTERVAL;
            if let Some(latest_bundle) = self.latest_bundle.get()
                && matches!(
                    &latest_bundle.lock().await.bundle,
                    Err(error) if error.code() == CloudConfigBundleLoadErrorCode::Timeout
                )
            {
                // Recover startup timeouts through this worker without making
                // readers fetch concurrently or extending the startup deadline.
                refresh_interval = CLOUD_CONFIG_BUNDLE_TIMEOUT_RETRY_INTERVAL;
            }
            sleep(refresh_interval).await;
            match timeout(self.timeout, self.refresh_cache_once()).await {
                Ok(true) => {}
                Ok(false) => break,
                Err(_) => {
                    tracing::error!(
                        "Timed out refreshing cloud config bundle cache from remote; keeping existing cache"
                    );
                    emit_load_metric("refresh", "error", /*bundle*/ None);
                    self.publish_refresh_result(Err(CloudConfigBundleLoadError::new(
                        CloudConfigBundleLoadErrorCode::Timeout,
                        /*status_code*/ None,
                        "timed out refreshing cloud config bundle",
                    )))
                    .await;
                }
            }
        }
    }

    async fn refresh_cache_once(&self) -> bool {
        let auth = match self.source.auth().await {
            Ok(Some(auth)) => auth,
            Ok(None) => return false,
            Err(error) => {
                self.publish_refresh_result(Err(error)).await;
                return true;
            }
        };

        match self
            .fetch_remote_bundle_and_update_cache_with_retries(auth, "refresh")
            .await
        {
            Ok(bundle) => {
                emit_load_metric("refresh", "success", bundle.as_ref());
                self.publish_refresh_result(Ok(bundle)).await;
            }
            Err(err) => {
                tracing::error!(
                    path = %self.cache.path().display(),
                    error = %err,
                    "Failed to refresh cloud config bundle cache from remote"
                );
                emit_load_metric("refresh", "error", /*bundle*/ None);
                self.publish_refresh_result(Err(err)).await;
            }
        }
        true
    }

    async fn publish_refresh_result(
        &self,
        result: Result<Option<CloudConfigBundle>, CloudConfigBundleLoadError>,
    ) {
        let Some(latest) = self.latest_bundle.get() else {
            return;
        };
        let mut latest = latest.lock().await;
        if result.is_ok() {
            *latest = CloudConfigBundleSnapshot {
                config_server: None,
                bundle: result,
                binding: None,
            };
            self.policy.publish_snapshot(&mut latest);
        } else if latest.bundle.is_err() {
            latest.bundle = result;
            latest.binding = None;
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
