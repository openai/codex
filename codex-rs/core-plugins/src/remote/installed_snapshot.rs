use super::CODEX_PRODUCT_SKU;
use super::RemotePluginCatalogError;
use super::RemotePluginInstalledItem;
use super::RemotePluginScope;
use super::RemotePluginServiceConfig;
use super::get_remote_plugin_installed_page;
use codex_login::CodexAuth;
use codex_protocol::account::PlanType;
use codex_protocol::auth::AuthMode;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Semaphore;

const REMOTE_INSTALLED_PLUGIN_PAGE_LIMIT: u32 = 1000;
const REMOTE_INSTALLED_PLUGIN_SNAPSHOT_TTL: Duration = Duration::from_secs(30);
const REMOTE_INSTALLED_PLUGIN_FAILURE_TTL: Duration = Duration::from_secs(1);
const REMOTE_INSTALLED_PLUGIN_SNAPSHOT_INVALIDATED: &str =
    "remote installed plugin snapshot was invalidated while loading";

#[derive(Clone, Debug, PartialEq, Eq)]
struct InstalledSnapshotCacheKey {
    chatgpt_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    plan_type: Option<PlanType>,
    auth_mode: AuthMode,
    product_sku: &'static str,
}

impl InstalledSnapshotCacheKey {
    fn new(config: &RemotePluginServiceConfig, auth: &CodexAuth) -> Self {
        Self {
            chatgpt_base_url: config.chatgpt_base_url.trim_end_matches('/').to_string(),
            // ChatGPT-Account-ID is the active workspace/account selected for this request.
            account_id: auth.get_account_id(),
            chatgpt_user_id: auth.get_chatgpt_user_id(),
            plan_type: auth.account_plan_type(),
            auth_mode: auth.api_auth_mode(),
            product_sku: CODEX_PRODUCT_SKU,
        }
    }
}

struct CachedInstalledSnapshot {
    key: InstalledSnapshotCacheKey,
    value: CachedInstalledSnapshotValue,
    expires_at: Instant,
}

enum CachedInstalledSnapshotValue {
    Plugins(Arc<Vec<RemotePluginInstalledItem>>),
    Error(String),
}

pub(super) fn snapshot_invalidated_error() -> RemotePluginCatalogError {
    RemotePluginCatalogError::UnexpectedResponse(
        REMOTE_INSTALLED_PLUGIN_SNAPSHOT_INVALIDATED.to_string(),
    )
}

/// Identity-keyed installed-state cache shared by remote marketplace queries.
///
/// The cached response is raw so bundle sync and marketplace projection consume the same
/// download URLs, delivery overrides, policies, and scope metadata.
pub(crate) struct RemoteInstalledPluginSnapshotCache {
    cached: RwLock<Option<CachedInstalledSnapshot>>,
    fetch_semaphore: Semaphore,
    generation: AtomicU64,
}

impl Default for RemoteInstalledPluginSnapshotCache {
    fn default() -> Self {
        Self {
            cached: RwLock::new(None),
            fetch_semaphore: Semaphore::new(/*permits*/ 1),
            generation: AtomicU64::new(0),
        }
    }
}

impl RemoteInstalledPluginSnapshotCache {
    pub(super) async fn get_or_fetch(
        &self,
        config: &RemotePluginServiceConfig,
        auth: &CodexAuth,
    ) -> Result<Arc<Vec<RemotePluginInstalledItem>>, RemotePluginCatalogError> {
        let key = InstalledSnapshotCacheKey::new(config, auth);
        let generation = self.generation();
        if let Some(result) = self.cached_result_for_key(&key) {
            if generation == self.generation() {
                return result;
            }
            return Err(snapshot_invalidated_error());
        }

        let _fetch_permit = self.fetch_semaphore.acquire().await.map_err(|_| {
            RemotePluginCatalogError::UnexpectedResponse(
                "remote installed plugin snapshot fetch gate was closed".to_string(),
            )
        })?;
        if generation != self.generation() {
            return Err(snapshot_invalidated_error());
        }
        if let Some(result) = self.cached_result_for_key(&key) {
            if generation == self.generation() {
                return result;
            }
            return Err(snapshot_invalidated_error());
        }

        let result = fetch_unscoped_installed_plugins(config, auth)
            .await
            .map(Arc::new);
        let mut cached = match self.cached.write() {
            Ok(cached) => cached,
            Err(err) => err.into_inner(),
        };
        if generation != self.generation() {
            return Err(snapshot_invalidated_error());
        }
        let (value, ttl) = match &result {
            Ok(plugins) => (
                CachedInstalledSnapshotValue::Plugins(Arc::clone(plugins)),
                REMOTE_INSTALLED_PLUGIN_SNAPSHOT_TTL,
            ),
            Err(err) => (
                CachedInstalledSnapshotValue::Error(err.to_string()),
                REMOTE_INSTALLED_PLUGIN_FAILURE_TTL,
            ),
        };
        *cached = Some(CachedInstalledSnapshot {
            key,
            value,
            expires_at: Instant::now() + ttl,
        });
        result
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    pub(crate) fn invalidate(&self) -> bool {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let mut cached = match self.cached.write() {
            Ok(cached) => cached,
            Err(err) => err.into_inner(),
        };
        cached.take().is_some()
    }

    fn cached_result_for_key(
        &self,
        key: &InstalledSnapshotCacheKey,
    ) -> Option<Result<Arc<Vec<RemotePluginInstalledItem>>, RemotePluginCatalogError>> {
        let cached = match self.cached.read() {
            Ok(cached) => cached,
            Err(err) => err.into_inner(),
        };
        cached
            .as_ref()
            .filter(|cached| cached.key == *key && Instant::now() < cached.expires_at)
            .map(|cached| match &cached.value {
                CachedInstalledSnapshotValue::Plugins(plugins) => Ok(Arc::clone(plugins)),
                CachedInstalledSnapshotValue::Error(message) => Err(
                    RemotePluginCatalogError::UnexpectedResponse(message.clone()),
                ),
            })
    }
}

pub(super) async fn installed_plugins_for_scope(
    cache: &RemoteInstalledPluginSnapshotCache,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
) -> Result<Vec<RemotePluginInstalledItem>, RemotePluginCatalogError> {
    Ok(cache
        .get_or_fetch(config, auth)
        .await?
        .iter()
        .filter(|plugin| plugin.plugin.scope == scope)
        .cloned()
        .collect())
}

async fn fetch_unscoped_installed_plugins(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
) -> Result<Vec<RemotePluginInstalledItem>, RemotePluginCatalogError> {
    let mut plugins = Vec::new();
    let mut page_token = None;
    let mut seen_page_tokens = BTreeSet::new();
    let mut page_count = 0_u64;

    loop {
        let response = get_remote_plugin_installed_page(
            config,
            auth,
            /*scope*/ None,
            page_token.as_deref(),
            /*include_download_urls*/ true,
            Some(REMOTE_INSTALLED_PLUGIN_PAGE_LIMIT),
        )
        .await?;
        page_count += 1;
        plugins.extend(response.plugins);
        let Some(next_page_token) = response.pagination.next_page_token else {
            break;
        };
        if next_page_token.is_empty() || !seen_page_tokens.insert(next_page_token.clone()) {
            return Err(RemotePluginCatalogError::UnexpectedResponse(
                "remote installed plugin pagination returned an empty or repeated next page token"
                    .to_string(),
            ));
        }
        page_token = Some(next_page_token);
    }

    tracing::debug!(
        page_count,
        plugin_count = plugins.len(),
        "fetched unscoped remote installed plugin snapshot"
    );
    Ok(plugins)
}

#[cfg(test)]
#[path = "installed_snapshot_tests.rs"]
mod tests;
