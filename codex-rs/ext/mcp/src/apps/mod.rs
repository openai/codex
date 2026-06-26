use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::PoisonError;
use std::sync::RwLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use codex_analytics::AnalyticsEventsClient;
use codex_apps::CodexApps;
use codex_apps::CodexAppsConnectConfig;
use codex_apps::CodexAppsSnapshot;
use codex_connectors::CONNECTORS_CACHE_TTL;
use codex_connectors::ConnectorSnapshot;
use codex_core::config::Config;
use codex_core_plugins::PluginsManager;
use codex_login::AuthManager;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tokio_util::sync::CancellationToken;

use self::config::apps_connect_config;
use self::config::apps_inventory_eligible;
use self::config::apps_mcp_eligible;
use self::config::auth_revision_access_guard;
use self::config::current_auth_revision;

mod analytics;
mod config;
mod contributor;
mod install_verification;
mod policy;
mod presentation;

#[cfg(test)]
mod test_support;
#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

const APPS_RETRY_INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const APPS_RETRY_MAX_BACKOFF: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq)]
struct CodexAppsConnectionKey {
    config: CodexAppsConnectConfig,
    auth_revision: u64,
}

struct ConnectedCodexApps {
    key: CodexAppsConnectionKey,
    apps: Arc<CodexApps>,
    refresh_after: Option<Instant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppsRefreshRequirement {
    None,
    EnsureLive,
    Refresh,
}

struct AppsConnectionService {
    auth_manager: Arc<AuthManager>,
    environment_manager: Arc<codex_exec_server::EnvironmentManager>,
    current: RwLock<Option<ConnectedCodexApps>>,
    connect: Mutex<()>,
    publication_revision: Arc<AtomicU64>,
    background_initializations: StdMutex<Vec<AppsInitializationState>>,
    shutdown: CancellationToken,
}

struct AppsBackgroundInitialization {
    connection: Arc<AppsConnectionService>,
    key: CodexAppsConnectionKey,
    immediate_retry_available: bool,
    finished: bool,
}

struct AppsInitializationState {
    key: CodexAppsConnectionKey,
    phase: AppsInitializationPhase,
    consecutive_retry_failures: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppsInitializationPhase {
    InFlight,
    CoolingDown { retry_not_before: TokioInstant },
    RetryReady,
}

enum AppsBackgroundInitializationStart {
    Started(AppsBackgroundInitialization),
    Pending,
}

enum AppsBackgroundInitializationFailure {
    Abandon,
    RetryNow,
    RetryAfter(TokioInstant),
}

/// Contributes connector-scoped HTTP MCP servers from one shared Apps inventory owner.
pub struct CodexAppsMcpExtension {
    connection: Arc<AppsConnectionService>,
    initialization_tasks: StdMutex<JoinSet<()>>,
    plugins_manager: Arc<PluginsManager>,
    analytics_events_client: AnalyticsEventsClient,
}

impl CodexAppsMcpExtension {
    #[cfg(test)]
    fn new_for_tests(auth_manager: Arc<AuthManager>) -> Self {
        let codex_home = tempfile::tempdir().expect("temporary Codex home").keep();
        Self::new(
            auth_manager,
            Arc::new(codex_exec_server::EnvironmentManager::without_environments()),
            Arc::new(PluginsManager::new(codex_home)),
        )
    }

    pub fn new(
        auth_manager: Arc<AuthManager>,
        environment_manager: Arc<codex_exec_server::EnvironmentManager>,
        plugins_manager: Arc<PluginsManager>,
    ) -> Self {
        Self::new_with_analytics(
            auth_manager,
            environment_manager,
            plugins_manager,
            AnalyticsEventsClient::disabled(),
        )
    }

    pub fn new_with_analytics(
        auth_manager: Arc<AuthManager>,
        environment_manager: Arc<codex_exec_server::EnvironmentManager>,
        plugins_manager: Arc<PluginsManager>,
        analytics_events_client: AnalyticsEventsClient,
    ) -> Self {
        let connection = Arc::new(AppsConnectionService {
            auth_manager,
            environment_manager,
            current: RwLock::new(None),
            connect: Mutex::new(()),
            publication_revision: Arc::new(AtomicU64::new(0)),
            background_initializations: StdMutex::new(Vec::new()),
            shutdown: CancellationToken::new(),
        });
        Self {
            connection,
            initialization_tasks: StdMutex::new(JoinSet::new()),
            plugins_manager,
            analytics_events_client,
        }
    }

    async fn plugin_connector_snapshot(&self, config: &Config) -> ConnectorSnapshot {
        let loaded_plugins = self
            .plugins_manager
            .plugins_for_config(&config.plugins_config_input())
            .await;
        ConnectorSnapshot::from_plugin_capability_summaries(loaded_plugins.capability_summaries())
    }

    /// Returns the current connector inventory when Apps is eligible for this config.
    pub async fn snapshot(&self, config: &Config) -> anyhow::Result<Option<CodexAppsSnapshot>> {
        if !apps_inventory_eligible(config) {
            return Ok(None);
        }
        let Some((key, apps)) = self
            .connection
            .apps_for_config(config, /*refresh*/ false)
            .await?
        else {
            return Ok(None);
        };
        if let Err(error) = self.connection.refresh_if_stale(&key, &apps).await {
            tracing::warn!(%error, "failed to refresh stale Codex Apps inventory; using last-good snapshot");
        }
        Ok(Some(apps.snapshot()))
    }

    /// Returns the first available connector inventory without waiting for cached data to refresh.
    pub async fn snapshot_allowing_cached(
        &self,
        config: &Config,
    ) -> anyhow::Result<Option<CodexAppsSnapshot>> {
        if !apps_inventory_eligible(config) {
            return Ok(None);
        }
        Ok(self
            .connection
            .apps_for_config(config, /*refresh*/ false)
            .await?
            .map(|(_, apps)| apps.snapshot()))
    }

    /// Ensures connector-scoped MCP servers are ready to contribute for this config.
    pub async fn prepare_mcp_servers(&self, config: &Config) -> anyhow::Result<()> {
        if !apps_mcp_eligible(config) {
            return Ok(());
        }
        self.snapshot_allowing_cached(config).await?;
        Ok(())
    }

    /// Returns the already-connected snapshot without performing network discovery.
    pub async fn current_snapshot(&self, config: &Config) -> Option<CodexAppsSnapshot> {
        if !apps_inventory_eligible(config) {
            return None;
        }
        self.connection
            .current_snapshot_with_key(config)
            .await
            .map(|(_, snapshot)| snapshot)
    }

    fn initialize_in_background(
        &self,
        config: Config,
        connection_key: CodexAppsConnectionKey,
        thread_state: Option<(Arc<presentation::AppsThreadState>, u64)>,
    ) {
        if self.connection.shutdown.is_cancelled() {
            return;
        }
        let AppsBackgroundInitializationStart::Started(mut background_initialization) = self
            .connection
            .begin_background_initialization(connection_key.clone())
        else {
            return;
        };
        let connection = Arc::clone(&self.connection);
        let task = async move {
            loop {
                let result = tokio::select! {
                    _ = connection.shutdown.cancelled() => Ok(None),
                    result = connection.apps_for_config(&config, /*refresh*/ false) => result,
                };
                match result {
                    Ok(Some((connection_key, apps))) => {
                        background_initialization.succeeded();
                        if let Some((state, state_revision)) = thread_state {
                            let snapshot = apps.snapshot();
                            state.replace_apps_if_revision(
                                state_revision,
                                connection_key,
                                apps,
                                snapshot,
                                &config,
                            );
                        }
                        return;
                    }
                    Ok(None) => {
                        background_initialization.succeeded();
                        if let Some((state, state_revision)) = thread_state {
                            state.clear_if_revision(state_revision, &config);
                        }
                        return;
                    }
                    Err(error) => match background_initialization.failed() {
                        AppsBackgroundInitializationFailure::Abandon => return,
                        AppsBackgroundInitializationFailure::RetryNow => {
                            tracing::warn!(%error, "failed to initialize Codex Apps MCP; retrying");
                        }
                        AppsBackgroundInitializationFailure::RetryAfter(retry_not_before) => {
                            tracing::warn!(
                                %error,
                                retry_after_ms = retry_not_before
                                    .saturating_duration_since(TokioInstant::now())
                                    .as_millis(),
                                "failed to retry Codex Apps MCP initialization"
                            );
                            connection
                                .publish_retry_when_ready(&connection_key, retry_not_before)
                                .await;
                            return;
                        }
                    },
                }
            }
        };
        self.spawn_background_task(task);
    }

    fn refresh_in_background(&self, connection_key: CodexAppsConnectionKey, apps: Arc<CodexApps>) {
        if self.connection.shutdown.is_cancelled()
            || self.connection.refresh_requirement(&connection_key, &apps)
                == AppsRefreshRequirement::None
        {
            return;
        }
        let AppsBackgroundInitializationStart::Started(mut background_refresh) = self
            .connection
            .begin_background_initialization(connection_key.clone())
        else {
            return;
        };
        let connection = Arc::clone(&self.connection);
        let task = async move {
            loop {
                let result = tokio::select! {
                    _ = connection.shutdown.cancelled() => Ok(()),
                    result = connection.refresh_if_stale(&connection_key, &apps) => result,
                };
                match result {
                    Ok(()) => {
                        background_refresh.succeeded();
                        return;
                    }
                    Err(error) => match background_refresh.failed() {
                        AppsBackgroundInitializationFailure::Abandon => return,
                        AppsBackgroundInitializationFailure::RetryNow => {
                            tracing::warn!(%error, "failed to refresh stale Codex Apps MCP; retrying");
                        }
                        AppsBackgroundInitializationFailure::RetryAfter(retry_not_before) => {
                            tracing::warn!(
                                %error,
                                retry_after_ms = retry_not_before
                                    .saturating_duration_since(TokioInstant::now())
                                    .as_millis(),
                                "failed to retry stale Codex Apps MCP refresh"
                            );
                            connection
                                .publish_retry_when_ready(&connection_key, retry_not_before)
                                .await;
                            return;
                        }
                    },
                }
            }
        };
        self.spawn_background_task(task);
    }

    fn spawn_background_task(&self, task: impl Future<Output = ()> + Send + 'static) {
        let mut tasks = self
            .initialization_tasks
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        while let Some(result) = tasks.try_join_next() {
            log_initialization_join_result(result);
        }
        if !self.connection.shutdown.is_cancelled() {
            tasks.spawn(task);
        }
    }

    /// Prevents new background initialization and cancels any initialization in progress.
    pub fn begin_shutdown(&self) {
        self.connection.shutdown.cancel();
    }

    /// Cancels and joins background initialization, then stops the connected Apps runtime.
    pub async fn shutdown(&self) {
        self.begin_shutdown();
        let mut tasks = {
            let mut tasks = self
                .initialization_tasks
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            std::mem::take(&mut *tasks)
        };
        while let Some(result) = tasks.join_next().await {
            log_initialization_join_result(result);
        }
        self.connection
            .background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        let connected = self
            .connection
            .current
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(connected) = connected {
            connected.apps.shutdown().await;
        }
    }

    /// Refreshes and returns the connector inventory when Apps is eligible for this config.
    pub async fn refresh_snapshot(
        &self,
        config: &Config,
    ) -> anyhow::Result<Option<CodexAppsSnapshot>> {
        if !apps_inventory_eligible(config) {
            return Ok(None);
        }
        Ok(self
            .connection
            .apps_for_config(config, /*refresh*/ true)
            .await?
            .map(|(_, apps)| apps.snapshot()))
    }
}

impl AppsConnectionService {
    async fn connection_key(&self, config: &Config) -> Option<CodexAppsConnectionKey> {
        if self.shutdown.is_cancelled() {
            return None;
        }
        let (auth, auth_revision) = self.current_auth().await;
        let Some(auth) = auth else {
            self.clear_connected_through(auth_revision);
            return None;
        };
        Some(CodexAppsConnectionKey {
            config: apps_connect_config(config, &auth),
            auth_revision,
        })
    }

    async fn current_snapshot_with_key(
        &self,
        config: &Config,
    ) -> Option<(CodexAppsConnectionKey, CodexAppsSnapshot)> {
        let key = self.connection_key(config).await?;
        self.current_apps_for_key(&key)
            .map(|apps| (key, apps.snapshot()))
    }

    fn current_apps_for_key(&self, key: &CodexAppsConnectionKey) -> Option<Arc<CodexApps>> {
        let current = self.current.read().unwrap_or_else(PoisonError::into_inner);
        current
            .as_ref()
            .filter(|connected| &connected.key == key)
            .map(|connected| Arc::clone(&connected.apps))
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "Apps refreshes for one connection must remain serialized"
    )]
    async fn refresh_if_stale(
        &self,
        key: &CodexAppsConnectionKey,
        apps: &Arc<CodexApps>,
    ) -> anyhow::Result<()> {
        if self.refresh_requirement(key, apps) == AppsRefreshRequirement::None {
            return Ok(());
        }

        let _refresh = self.connect.lock().await;
        match self.refresh_requirement(key, apps) {
            AppsRefreshRequirement::None => return Ok(()),
            AppsRefreshRequirement::EnsureLive => {
                apps.ensure_live().await?;
            }
            AppsRefreshRequirement::Refresh => {
                apps.refresh().await?;
            }
        }
        self.mark_refresh_succeeded(key, apps);
        Ok(())
    }

    fn refresh_requirement(
        &self,
        key: &CodexAppsConnectionKey,
        apps: &Arc<CodexApps>,
    ) -> AppsRefreshRequirement {
        self.current
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|connected| connected.key == *key && Arc::ptr_eq(&connected.apps, apps))
            .map_or(AppsRefreshRequirement::None, |connected| {
                match connected.refresh_after {
                    None => AppsRefreshRequirement::EnsureLive,
                    Some(refresh_after) if Instant::now() >= refresh_after => {
                        AppsRefreshRequirement::Refresh
                    }
                    Some(_) => AppsRefreshRequirement::None,
                }
            })
    }

    fn mark_refresh_succeeded(&self, key: &CodexAppsConnectionKey, apps: &Arc<CodexApps>) {
        {
            let mut current = self.current.write().unwrap_or_else(PoisonError::into_inner);
            if let Some(connected) = current.as_mut()
                && connected.key == *key
                && Arc::ptr_eq(&connected.apps, apps)
            {
                connected.refresh_after = Some(Instant::now() + CONNECTORS_CACHE_TTL);
            }
        }
        self.clear_idle_initialization(key);
    }

    async fn current_auth(&self) -> (Option<codex_login::CodexAuth>, u64) {
        let (auth, revision) = self.auth_manager.auth_with_revision().await;
        (
            auth.filter(codex_login::CodexAuth::uses_codex_backend),
            revision,
        )
    }

    async fn apps_for_config(
        self: &Arc<Self>,
        config: &Config,
        refresh: bool,
    ) -> anyhow::Result<Option<(CodexAppsConnectionKey, Arc<CodexApps>)>> {
        loop {
            let (auth, auth_revision) = self.current_auth().await;
            let Some(auth) = auth else {
                self.clear_connected_through(auth_revision);
                return Ok(None);
            };

            let connect_config = apps_connect_config(config, &auth);
            let key = CodexAppsConnectionKey {
                config: connect_config.clone(),
                auth_revision,
            };
            let auth_provider = codex_model_provider::auth_provider_from_auth(&auth);
            let access_guard = auth_revision_access_guard(&self.auth_manager, auth_revision);
            let environment_manager = Arc::clone(&self.environment_manager);
            let publication_revision = Arc::clone(&self.publication_revision);
            let apps = tokio::select! {
                biased;
                _ = self.shutdown.cancelled() => return Ok(None),
                apps = self.apps_for_key(key.clone(), refresh, move || async move {
                    let on_change: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                        publication_revision.fetch_add(1, Ordering::AcqRel);
                    });
                    Ok(Arc::new(
                        CodexApps::connect_with_environment(
                            &connect_config,
                            auth_provider,
                            environment_manager,
                            on_change,
                            access_guard,
                        )
                        .await?,
                    ))
                }) => apps,
            };
            if current_auth_revision(&self.auth_manager) != auth_revision {
                continue;
            }
            let Some(apps) = apps? else {
                continue;
            };
            return Ok(Some((key, apps)));
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "Apps connection setup and publication must remain serialized"
    )]
    async fn apps_for_key<F, Fut>(
        &self,
        key: CodexAppsConnectionKey,
        refresh: bool,
        connect: F,
    ) -> anyhow::Result<Option<Arc<CodexApps>>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = anyhow::Result<Arc<CodexApps>>>,
    {
        let existing = {
            let current = self.current.read().unwrap_or_else(PoisonError::into_inner);
            if current
                .as_ref()
                .is_some_and(|current| current.key.auth_revision > key.auth_revision)
            {
                return Ok(None);
            }
            current
                .as_ref()
                .filter(|current| current.key == key)
                .map(|current| Arc::clone(&current.apps))
        };
        if let Some(apps) = existing {
            if refresh {
                self.refresh_existing(&key, &apps).await?;
            }
            return Ok(Some(apps));
        }

        // Serialize cold setup without locking the published snapshot. Contributors can continue
        // to use the process-current generation while direct callers await a replacement.
        let _connect = self.connect.lock().await;
        let existing = {
            let current = self.current.read().unwrap_or_else(PoisonError::into_inner);
            if current
                .as_ref()
                .is_some_and(|current| current.key.auth_revision > key.auth_revision)
            {
                return Ok(None);
            }
            current
                .as_ref()
                .filter(|current| current.key == key)
                .map(|current| Arc::clone(&current.apps))
        };
        if let Some(existing) = existing {
            if refresh {
                self.refresh_existing(&key, &existing).await?;
            }
            return Ok(Some(existing));
        }
        let apps = connect().await?;
        if refresh {
            apps.ensure_live().await?;
        }
        let refresh_after = apps
            .snapshot()
            .is_live_inventory()
            .then(|| Instant::now() + CONNECTORS_CACHE_TTL);
        self.clear_idle_initialization(&key);
        *self.current.write().unwrap_or_else(PoisonError::into_inner) = Some(ConnectedCodexApps {
            key,
            apps: Arc::clone(&apps),
            refresh_after,
        });
        self.publication_revision.fetch_add(1, Ordering::AcqRel);
        Ok(Some(apps))
    }

    async fn refresh_existing(
        &self,
        key: &CodexAppsConnectionKey,
        apps: &Arc<CodexApps>,
    ) -> anyhow::Result<()> {
        if apps.snapshot().is_live_inventory() {
            apps.refresh().await?;
        } else {
            apps.ensure_live().await?;
        }
        self.mark_refresh_succeeded(key, apps);
        Ok(())
    }

    fn begin_background_initialization(
        self: &Arc<Self>,
        key: CodexAppsConnectionKey,
    ) -> AppsBackgroundInitializationStart {
        if self.shutdown.is_cancelled() {
            return AppsBackgroundInitializationStart::Pending;
        }
        let mut initializations = self
            .background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let immediate_retry_available = match initializations
            .iter_mut()
            .find(|initialization| initialization.key == key)
        {
            Some(initialization) => match initialization.phase {
                AppsInitializationPhase::InFlight => {
                    return AppsBackgroundInitializationStart::Pending;
                }
                AppsInitializationPhase::CoolingDown { retry_not_before }
                    if TokioInstant::now() < retry_not_before =>
                {
                    return AppsBackgroundInitializationStart::Pending;
                }
                AppsInitializationPhase::CoolingDown { .. }
                | AppsInitializationPhase::RetryReady => {
                    initialization.phase = AppsInitializationPhase::InFlight;
                    false
                }
            },
            None => {
                initializations.push(AppsInitializationState {
                    key: key.clone(),
                    phase: AppsInitializationPhase::InFlight,
                    consecutive_retry_failures: 0,
                });
                true
            }
        };
        AppsBackgroundInitializationStart::Started(AppsBackgroundInitialization {
            connection: Arc::clone(self),
            key,
            immediate_retry_available,
            finished: false,
        })
    }

    async fn publish_retry_when_ready(
        &self,
        key: &CodexAppsConnectionKey,
        retry_not_before: TokioInstant,
    ) {
        tokio::select! {
            _ = self.shutdown.cancelled() => return,
            _ = tokio::time::sleep_until(retry_not_before) => {}
        }
        let publish = self
            .background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter_mut()
            .find(|initialization| initialization.key == *key)
            .is_some_and(|initialization| {
                if initialization.phase
                    != (AppsInitializationPhase::CoolingDown { retry_not_before })
                {
                    return false;
                }
                initialization.phase = AppsInitializationPhase::RetryReady;
                true
            });
        if publish {
            self.publication_revision.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn clear_idle_initialization(&self, key: &CodexAppsConnectionKey) {
        let mut initializations = self
            .background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = initializations.iter().position(|state| state.key == *key)
            && initializations[index].phase != AppsInitializationPhase::InFlight
        {
            initializations.swap_remove(index);
        }
    }

    #[cfg(test)]
    fn background_initialization_is_active(&self) -> bool {
        self.background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|state| state.phase == AppsInitializationPhase::InFlight)
    }

    #[cfg(test)]
    fn background_initialization_is_active_for(&self, key: &CodexAppsConnectionKey) -> bool {
        self.background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|state| state.key == *key && state.phase == AppsInitializationPhase::InFlight)
    }

    fn clear_connected_through(&self, auth_revision: u64) {
        {
            let mut current = self.current.write().unwrap_or_else(PoisonError::into_inner);
            if current
                .as_ref()
                .is_some_and(|current| current.key.auth_revision <= auth_revision)
            {
                *current = None;
            }
        }
        self.background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|initialization| {
                initialization.key.auth_revision > auth_revision
                    || initialization.phase == AppsInitializationPhase::InFlight
            });
    }
}

impl AppsBackgroundInitialization {
    fn succeeded(&mut self) {
        self.remove_state();
        self.finished = true;
    }

    fn failed(&mut self) -> AppsBackgroundInitializationFailure {
        if self.immediate_retry_available {
            self.immediate_retry_available = false;
            return AppsBackgroundInitializationFailure::RetryNow;
        }
        let mut initializations = self
            .connection
            .background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let Some(initialization) = initializations
            .iter_mut()
            .find(|initialization| initialization.key == self.key)
        else {
            self.finished = true;
            return AppsBackgroundInitializationFailure::Abandon;
        };
        initialization.consecutive_retry_failures =
            initialization.consecutive_retry_failures.saturating_add(1);
        let retry_not_before =
            TokioInstant::now() + apps_retry_backoff(initialization.consecutive_retry_failures);
        initialization.phase = AppsInitializationPhase::CoolingDown { retry_not_before };
        self.finished = true;
        AppsBackgroundInitializationFailure::RetryAfter(retry_not_before)
    }

    fn remove_state(&self) {
        let mut initializations = self
            .connection
            .background_initializations
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = initializations
            .iter()
            .position(|initialization| initialization.key == self.key)
        {
            initializations.swap_remove(index);
        }
    }
}

impl Drop for AppsBackgroundInitialization {
    fn drop(&mut self) {
        if !self.finished {
            self.remove_state();
        }
    }
}

fn apps_retry_backoff(consecutive_retry_failures: u32) -> Duration {
    let exponent = consecutive_retry_failures.saturating_sub(1).min(5);
    APPS_RETRY_INITIAL_BACKOFF
        .saturating_mul(1 << exponent)
        .min(APPS_RETRY_MAX_BACKOFF)
}

impl Drop for CodexAppsMcpExtension {
    fn drop(&mut self) {
        self.begin_shutdown();
    }
}

fn log_initialization_join_result(result: Result<(), tokio::task::JoinError>) {
    if let Err(error) = result
        && !error.is_cancelled()
    {
        tracing::warn!(%error, "Codex Apps background initialization task failed");
    }
}
