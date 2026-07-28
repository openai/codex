//! Cloud config bundle lifecycle orchestration.
//!
//! Startup loads a shared bundle from cache or backend. A background refresher
//! updates both the on-disk cache and the bundle used by future config loads.

use crate::backend::BundleClient;
use crate::backend::BundleRequestError;
use crate::backend::RetryableFailureKind;
use crate::cache::CacheFreshness;
use crate::cache::CacheLoadStatus;
use crate::cache::CacheLockAttempt;
use crate::cache::CloudConfigBundleCache;
use crate::metrics::emit_fetch_attempt_metric;
use crate::metrics::emit_fetch_final_metric;
use crate::metrics::emit_load_metric;
use crate::validation::validate_bundle;
use codex_config::AbsolutePathBuf;
use codex_config::CloudConfigBundle;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::CloudConfigBundlePublisher;
use codex_core::util::backoff;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::RefreshTokenError;
use codex_login::UnauthorizedRecovery;
use codex_protocol::account::PlanType;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;

pub(crate) const CLOUD_CONFIG_BUNDLE_TIMEOUT: Duration = Duration::from_secs(15);
const CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS: usize = 5;
pub(crate) const CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_RETRY_INTERVAL: Duration =
    Duration::from_secs(60);
const CLOUD_CONFIG_BUNDLE_CACHE_LOCK_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const CLOUD_CONFIG_BUNDLE_LOAD_FAILED_MESSAGE: &str =
    "Failed to load cloud config bundle (workspace-managed policies).";
const CLOUD_CONFIG_BUNDLE_AUTH_RECOVERY_FAILED_MESSAGE: &str = concat!(
    "Your authentication session could not be refreshed automatically. ",
    "Please log out and sign in again."
);

fn auth_identity(auth: &CodexAuth) -> (Option<String>, Option<String>) {
    (auth.get_chatgpt_user_id(), auth.get_account_id())
}

fn cloud_config_eligible_auth(auth: &CodexAuth) -> bool {
    let Some(plan_type) = auth.account_plan_type() else {
        return false;
    };
    auth.uses_codex_backend()
        && (plan_type.is_business_like()
            || matches!(plan_type, PlanType::Enterprise | PlanType::Edu))
}

fn optional_bundle(bundle: CloudConfigBundle) -> Option<CloudConfigBundle> {
    if bundle.is_empty() {
        None
    } else {
        Some(bundle)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct LoadedBundle {
    pub(crate) bundle: Option<CloudConfigBundle>,
    pub(crate) refresh_in: Duration,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum StartupLoad {
    /// Cloud config does not apply to the current auth, so no refresh is needed.
    Inactive,
    /// Cloud config applies and must refresh, even when the current bundle is empty.
    Active(LoadedBundle),
}

#[derive(Debug, Eq, PartialEq)]
enum CacheRefreshSchedule {
    Stop,
    ContinueAfter(Duration),
}

enum CachedBundle {
    Fresh(LoadedBundle),
    Fallback(LoadedBundle),
    Miss,
}

#[derive(Clone, Copy)]
enum StaleCachePolicy {
    FallbackOnError,
    RefreshRequired,
}

enum UnauthorizedRecoveryAction {
    RetrySameAttempt,
    RetryNextAttempt,
}

pub(crate) struct CloudConfigBundleService<C> {
    auth_manager: Arc<AuthManager>,
    client: Arc<C>,
    cache: CloudConfigBundleCache,
    codex_home: AbsolutePathBuf,
    timeout: Duration,
}

impl<C> CloudConfigBundleService<C>
where
    C: BundleClient + 'static,
{
    pub(crate) fn new(
        auth_manager: Arc<AuthManager>,
        client: Arc<C>,
        codex_home: PathBuf,
        timeout: Duration,
    ) -> Self {
        let codex_home = AbsolutePathBuf::resolve_path_against_base(codex_home, "/");
        Self {
            auth_manager,
            client,
            cache: CloudConfigBundleCache::new(codex_home.clone()),
            codex_home,
            timeout,
        }
    }

    pub(crate) async fn load_startup_bundle(
        &self,
    ) -> Result<StartupLoad, CloudConfigBundleLoadError> {
        let _timer =
            codex_otel::start_global_timer("codex.cloud_config_bundle.fetch.duration_ms", &[]);
        let started_at = Instant::now();
        let load_result = match timeout(self.timeout, async {
            let Some(auth) = self.auth_manager.auth().await else {
                return Ok(StartupLoad::Inactive);
            };
            if !cloud_config_eligible_auth(&auth) {
                return Ok(StartupLoad::Inactive);
            }

            self.load_bundle(auth, "startup", StaleCachePolicy::FallbackOnError)
                .await
                .map(StartupLoad::Active)
        })
        .await
        {
            Ok(load_result) => load_result,
            Err(_) => {
                let fallback = self.load_cached_fallback_for_current_auth().await;
                if let Some(loaded) = fallback {
                    tracing::warn!(
                        path = %self.cache.path().display(),
                        "Timed out refreshing cloud config bundle; using cached fallback"
                    );
                    Ok(StartupLoad::Active(loaded))
                } else {
                    let message = format!(
                        "timed out waiting for cloud config bundle after {}s",
                        self.timeout.as_secs()
                    );
                    tracing::error!("{message}");
                    Err(CloudConfigBundleLoadError::new(
                        CloudConfigBundleLoadErrorCode::Timeout,
                        /*status_code*/ None,
                        message,
                    ))
                }
            }
        };

        let result = match load_result {
            Ok(result) => result,
            Err(err) => {
                emit_load_metric("startup", "error", /*bundle*/ None);
                return Err(err);
            }
        };

        match &result {
            StartupLoad::Active(LoadedBundle {
                bundle: Some(bundle),
                ..
            }) => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    config_fragments = bundle.config_toml.enterprise_managed.len(),
                    requirements_fragments = bundle.requirements_toml.enterprise_managed.len(),
                    "Cloud config bundle load completed"
                );
                emit_load_metric("startup", "success", Some(bundle));
            }
            StartupLoad::Inactive | StartupLoad::Active(LoadedBundle { bundle: None, .. }) => {
                tracing::info!(
                    elapsed_ms = started_at.elapsed().as_millis(),
                    "Cloud config bundle load completed (none)"
                );
                emit_load_metric("startup", "success", /*bundle*/ None);
            }
        }

        Ok(result)
    }

    async fn load_cached_bundle(&self, auth: &CodexAuth) -> CachedBundle {
        let (chatgpt_user_id, account_id) = auth_identity(auth);
        match self
            .cache
            .load(chatgpt_user_id.as_deref(), account_id.as_deref())
            .await
        {
            Ok(loaded_cache) => {
                if let Err(err) =
                    validate_bundle(&loaded_cache.signed_payload.bundle, &self.codex_home)
                {
                    tracing::warn!(
                        path = %self.cache.path().display(),
                        error = %err,
                        "Ignoring invalid cached cloud config bundle"
                    );
                    self.cache
                        .log_load_status(&CacheLoadStatus::CacheInvalidBundle);
                    CachedBundle::Miss
                } else {
                    let bundle = optional_bundle(loaded_cache.signed_payload.bundle);
                    match loaded_cache.freshness {
                        CacheFreshness::Fresh { refresh_in } => {
                            tracing::info!(
                                path = %self.cache.path().display(),
                                "Using cached cloud config bundle"
                            );
                            CachedBundle::Fresh(LoadedBundle { bundle, refresh_in })
                        }
                        CacheFreshness::Stale => CachedBundle::Fallback(LoadedBundle {
                            bundle,
                            refresh_in: CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_RETRY_INTERVAL,
                        }),
                    }
                }
            }
            Err(cache_load_status) => {
                self.cache.log_load_status(&cache_load_status);
                CachedBundle::Miss
            }
        }
    }

    async fn load_cached_fallback_for_current_auth(&self) -> Option<LoadedBundle> {
        let auth = self.auth_manager.auth_cached()?;
        if !cloud_config_eligible_auth(&auth) {
            return None;
        }
        match self.load_cached_bundle(&auth).await {
            CachedBundle::Fresh(loaded) | CachedBundle::Fallback(loaded) => Some(loaded),
            CachedBundle::Miss => None,
        }
    }

    async fn load_bundle(
        &self,
        auth: CodexAuth,
        trigger: &'static str,
        stale_cache_policy: StaleCachePolicy,
    ) -> Result<LoadedBundle, CloudConfigBundleLoadError> {
        loop {
            // Fresh cache entries satisfy the load immediately. A soft-stale
            // entry continues through coordination and a blocking fetch; only
            // startup may use it after that refresh fails.
            match self.load_cached_bundle(&auth).await {
                CachedBundle::Fresh(loaded) => return Ok(loaded),
                CachedBundle::Fallback(_) | CachedBundle::Miss => {}
            }

            // This is a cross-process single-flight lock, not a cache-file
            // integrity lock. One process fetches while contenders wait and
            // recheck the shared cache for its result.
            match self.cache.try_acquire_lock().await {
                Ok(CacheLockAttempt::Acquired(_cache_lock)) => {
                    // Close the race between the cache read and lock acquisition.
                    match self.load_cached_bundle(&auth).await {
                        CachedBundle::Fresh(loaded) => return Ok(loaded),
                        CachedBundle::Fallback(_) | CachedBundle::Miss => {}
                    }
                    return self
                        .fetch_remote_bundle_with_fallback(auth, trigger, stale_cache_policy)
                        .await;
                }
                Ok(CacheLockAttempt::Contended) => {
                    sleep(CLOUD_CONFIG_BUNDLE_CACHE_LOCK_RETRY_INTERVAL).await;
                }
                Err(err) => {
                    tracing::warn!(
                        path = %self.cache.path().display(),
                        error = %err,
                        "Failed to acquire cloud config bundle cache lock; fetching without coordination"
                    );
                    return self
                        .fetch_remote_bundle_with_fallback(auth, trigger, stale_cache_policy)
                        .await;
                }
            }
        }
    }

    async fn fetch_remote_bundle_with_fallback(
        &self,
        auth: CodexAuth,
        trigger: &'static str,
        stale_cache_policy: StaleCachePolicy,
    ) -> Result<LoadedBundle, CloudConfigBundleLoadError> {
        match self
            .fetch_remote_bundle_and_update_cache_with_retries(auth, trigger)
            .await
        {
            Ok(loaded) => Ok(loaded),
            Err(err)
                if matches!(stale_cache_policy, StaleCachePolicy::FallbackOnError)
                    && err.code() != CloudConfigBundleLoadErrorCode::Auth =>
            {
                // Auth recovery may have changed identities during the fetch.
                // Re-read the cache against the current identity before using it.
                let fallback = self.load_cached_fallback_for_current_auth().await;
                match fallback {
                    Some(loaded) => {
                        tracing::warn!(
                            path = %self.cache.path().display(),
                            error = %err,
                            "Failed to refresh cloud config bundle; using cached fallback"
                        );
                        Ok(loaded)
                    }
                    None => Err(err),
                }
            }
            Err(err) => Err(err),
        }
    }

    async fn fetch_remote_bundle_and_update_cache_with_retries(
        &self,
        mut auth: CodexAuth,
        trigger: &'static str,
    ) -> Result<LoadedBundle, CloudConfigBundleLoadError> {
        let mut attempt = 1;
        let mut last_status_code: Option<u16> = None;
        let mut auth_recovery = self.auth_manager.unauthorized_recovery();

        while attempt <= CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS {
            match self.client.get_bundle(&auth).await {
                Ok(bundle) => {
                    return self
                        .validate_and_cache_remote_bundle(&auth, trigger, attempt, bundle)
                        .await;
                }
                Err(BundleRequestError::Retryable(status)) => {
                    // Transient request and server failures use bounded backoff
                    // and consume the next retry-budget position.
                    last_status_code = status.status_code();
                    if self
                        .retry_after_request_failure(trigger, attempt, status)
                        .await
                    {
                        attempt += 1;
                        continue;
                    }
                }
                Err(BundleRequestError::Unauthorized {
                    status_code,
                    message,
                }) => {
                    // Unauthorized responses first run the AuthManager recovery
                    // sequence. A successful recovery retries the same logical
                    // attempt; transient recovery failures consume an attempt.
                    last_status_code = status_code;
                    match self
                        .handle_unauthorized(
                            &mut auth,
                            &mut auth_recovery,
                            trigger,
                            attempt,
                            status_code,
                            &message,
                        )
                        .await?
                    {
                        UnauthorizedRecoveryAction::RetrySameAttempt => continue,
                        UnauthorizedRecoveryAction::RetryNextAttempt => {
                            attempt += 1;
                            continue;
                        }
                    }
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

    async fn validate_and_cache_remote_bundle(
        &self,
        auth: &CodexAuth,
        trigger: &'static str,
        attempt: usize,
        bundle: CloudConfigBundle,
    ) -> Result<LoadedBundle, CloudConfigBundleLoadError> {
        emit_fetch_attempt_metric(trigger, attempt, "success", /*status_code*/ None);
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

        let (chatgpt_user_id, account_id) = auth_identity(auth);
        let refresh_in = match self
            .cache
            .save(chatgpt_user_id, account_id, bundle.clone())
            .await
        {
            Ok(refresh_in) => refresh_in,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "Failed to write cloud config bundle cache"
                );
                CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_RETRY_INTERVAL
            }
        };

        emit_fetch_final_metric(
            trigger,
            "success",
            "none",
            attempt,
            /*status_code*/ None,
            Some(&bundle),
        );
        Ok(LoadedBundle {
            bundle: optional_bundle(bundle),
            refresh_in,
        })
    }

    async fn retry_after_request_failure(
        &self,
        trigger: &'static str,
        attempt: usize,
        status: RetryableFailureKind,
    ) -> bool {
        let status_code = status.status_code();
        emit_fetch_attempt_metric(trigger, attempt, "error", status_code);
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

    async fn handle_unauthorized(
        &self,
        auth: &mut CodexAuth,
        auth_recovery: &mut UnauthorizedRecovery,
        trigger: &'static str,
        attempt: usize,
        status_code: Option<u16>,
        message: &str,
    ) -> Result<UnauthorizedRecoveryAction, CloudConfigBundleLoadError> {
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
                            /*bundle*/ None,
                        );
                        return Err(CloudConfigBundleLoadError::new(
                            CloudConfigBundleLoadErrorCode::Auth,
                            status_code,
                            CLOUD_CONFIG_BUNDLE_AUTH_RECOVERY_FAILED_MESSAGE,
                        ));
                    };
                    *auth = refreshed_auth;
                    return Ok(UnauthorizedRecoveryAction::RetrySameAttempt);
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
                    if attempt < CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS {
                        tracing::warn!(
                            error = %recovery_err,
                            attempt,
                            max_attempts = CLOUD_CONFIG_BUNDLE_MAX_ATTEMPTS,
                            "Failed to recover from unauthorized cloud config bundle request; retrying"
                        );
                        sleep(backoff(attempt as u64)).await;
                    }
                    return Ok(UnauthorizedRecoveryAction::RetryNextAttempt);
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
        Err(CloudConfigBundleLoadError::new(
            CloudConfigBundleLoadErrorCode::Auth,
            status_code,
            CLOUD_CONFIG_BUNDLE_AUTH_RECOVERY_FAILED_MESSAGE,
        ))
    }

    pub(crate) async fn refresh_cache_in_background(
        &self,
        mut refresh_in: Duration,
        publisher: CloudConfigBundlePublisher,
    ) {
        loop {
            tokio::select! {
                biased;
                _ = publisher.closed() => break,
                _ = sleep(refresh_in) => {}
            }
            // A peer may have replaced the shared cache while we slept.
            // load_bundle() rechecks it before attempting the lock, then
            // returns the peer's later deadline without making a remote call.
            let refresh_result = timeout(self.timeout, self.refresh_cache_once(&publisher)).await;
            refresh_in = match refresh_result {
                Ok(CacheRefreshSchedule::ContinueAfter(refresh_in)) => refresh_in,
                Ok(CacheRefreshSchedule::Stop) => break,
                Err(_) => {
                    tracing::error!(
                        "Timed out refreshing cloud config bundle cache; keeping existing cache"
                    );
                    emit_load_metric("refresh", "error", /*bundle*/ None);
                    CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_RETRY_INTERVAL
                }
            };
        }
    }

    async fn refresh_cache_once(
        &self,
        publisher: &CloudConfigBundlePublisher,
    ) -> CacheRefreshSchedule {
        let Some(auth) = self.auth_manager.auth().await else {
            return CacheRefreshSchedule::Stop;
        };
        if !cloud_config_eligible_auth(&auth) {
            return CacheRefreshSchedule::Stop;
        }

        match self
            .load_bundle(auth, "refresh", StaleCachePolicy::RefreshRequired)
            .await
        {
            Ok(loaded) => {
                emit_load_metric("refresh", "success", loaded.bundle.as_ref());
                publisher.publish(Ok(loaded.bundle));
                CacheRefreshSchedule::ContinueAfter(loaded.refresh_in)
            }
            Err(err) => {
                tracing::error!(
                    path = %self.cache.path().display(),
                    error = %err,
                    "Failed to refresh cloud config bundle cache"
                );
                emit_load_metric("refresh", "error", /*bundle*/ None);
                CacheRefreshSchedule::ContinueAfter(
                    CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_RETRY_INTERVAL,
                )
            }
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
