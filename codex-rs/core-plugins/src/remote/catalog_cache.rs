use super::RemotePluginDirectoryItem;
use super::RemotePluginScope;
use super::RemotePluginServiceConfig;
use chrono::DateTime;
use chrono::Utc;
use codex_login::CodexAuth;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tracing::warn;

// `plugin/list` and other callers that assume all plugins are available locally will be
// deprecated soon. Remove this catalog cache when those callers migrate to on-demand fetching.
const REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION: u8 = 1;
const REMOTE_PLUGIN_CATALOG_DISK_CACHE_DIR: &str = "cache/remote_plugin_catalog";
const REMOTE_PLUGIN_CATALOG_DISK_CACHE_TTL: Duration = Duration::from_secs(60 * 60 * 3);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct RemotePluginCatalogCacheKey {
    chatgpt_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
    // Global catalogs predate scoped cache keys and must keep their existing filenames.
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<RemotePluginScope>,
}

impl RemotePluginCatalogCacheKey {
    fn new(
        config: &RemotePluginServiceConfig,
        auth: &CodexAuth,
        scope: RemotePluginScope,
    ) -> Option<Self> {
        let cache_key = Self {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            account_id: auth.get_account_id(),
            chatgpt_user_id: auth.get_chatgpt_user_id(),
            is_workspace_account: auth.is_workspace_account(),
            scope: (scope != RemotePluginScope::Global).then_some(scope),
        };
        // Preserve global catalog caching for existing header-auth clients, but never share
        // user or workspace catalogs when the auth mode cannot identify their owner.
        if !matches!(scope, RemotePluginScope::Global)
            && cache_key.account_id.is_none()
            && cache_key.chatgpt_user_id.is_none()
        {
            return None;
        }

        Some(cache_key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemotePluginCatalogDiskCache {
    schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fetched_at: Option<DateTime<Utc>>,
    plugins: Vec<RemotePluginDirectoryItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemotePluginCatalogCacheFreshness {
    Fresh,
    Stale,
}

#[derive(Debug, Clone)]
pub(super) struct CachedDirectoryPlugins {
    pub plugins: Vec<RemotePluginDirectoryItem>,
    pub freshness: RemotePluginCatalogCacheFreshness,
}

pub(crate) fn load_cached_directory_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
) -> Option<CachedDirectoryPlugins> {
    let cache_key = RemotePluginCatalogCacheKey::new(config, auth, scope)?;
    let cache_path = cache_path(codex_home, &cache_key);
    let bytes = match std::fs::read(&cache_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to read remote plugin catalog disk cache: {err}"
            );
            return None;
        }
    };
    let cache: RemotePluginCatalogDiskCache = match serde_json::from_slice(&bytes) {
        Ok(cache) => cache,
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to parse remote plugin catalog disk cache: {err}"
            );
            let _ = std::fs::remove_file(cache_path);
            return None;
        }
    };
    if cache.schema_version != REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION {
        let _ = std::fs::remove_file(cache_path);
        return None;
    }

    let freshness = if is_fresh(cache.fetched_at, Utc::now()) {
        RemotePluginCatalogCacheFreshness::Fresh
    } else {
        RemotePluginCatalogCacheFreshness::Stale
    };
    Some(CachedDirectoryPlugins {
        plugins: cache.plugins,
        freshness,
    })
}

pub(crate) fn write_cached_directory_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
    plugins: &[RemotePluginDirectoryItem],
) {
    let Some(cache_key) = RemotePluginCatalogCacheKey::new(config, auth, scope) else {
        return;
    };
    let cache_path = cache_path(codex_home, &cache_key);
    let Ok(contents) = serde_json::to_string_pretty(&RemotePluginCatalogDiskCache {
        schema_version: REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION,
        fetched_at: Some(Utc::now()),
        plugins: plugins.to_vec(),
    }) else {
        return;
    };
    let _ = codex_utils_path::write_atomically(&cache_path, &contents);
}

pub(crate) fn remove_cached_directory_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    scope: RemotePluginScope,
) {
    let Some(cache_key) = RemotePluginCatalogCacheKey::new(config, auth, scope) else {
        return;
    };
    let cache_path = cache_path(codex_home, &cache_key);
    match std::fs::remove_file(&cache_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to remove remote plugin catalog disk cache: {err}"
            );
        }
    }
}

fn cache_path(codex_home: &Path, cache_key: &RemotePluginCatalogCacheKey) -> PathBuf {
    let cache_key_json = serde_json::to_vec(cache_key).unwrap_or_default();
    let mut cache_key_hash = 0xcbf29ce484222325_u64;
    for byte in cache_key_json {
        cache_key_hash ^= u64::from(byte);
        cache_key_hash = cache_key_hash.wrapping_mul(0x100000001b3);
    }
    codex_home
        .join(REMOTE_PLUGIN_CATALOG_DISK_CACHE_DIR)
        .join(format!("{cache_key_hash:016x}.json"))
}

fn is_fresh(fetched_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    let Some(fetched_at) = fetched_at else {
        return false;
    };
    let Ok(ttl) = chrono::Duration::from_std(REMOTE_PLUGIN_CATALOG_DISK_CACHE_TTL) else {
        return false;
    };
    let age = now.signed_duration_since(fetched_at);
    age >= chrono::Duration::zero() && age <= ttl
}

#[cfg(test)]
#[path = "catalog_cache_tests.rs"]
mod tests;
