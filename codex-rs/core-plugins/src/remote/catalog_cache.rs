use super::RemotePluginDirectoryItem;
use super::RemotePluginServiceConfig;
use codex_login::CodexAuth;
use serde::Deserialize;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::RwLock;
use tracing::warn;

const REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION: u8 = 1;
const REMOTE_PLUGIN_CATALOG_DISK_CACHE_DIR: &str = "cache/remote_plugin_catalog";
const REMOTE_PLUGIN_CATALOG_MEMORY_CACHE_CAPACITY: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct RemotePluginCatalogCacheKey {
    chatgpt_base_url: String,
    account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    is_workspace_account: bool,
}

impl RemotePluginCatalogCacheKey {
    fn global(config: &RemotePluginServiceConfig, auth: &CodexAuth) -> Self {
        Self {
            chatgpt_base_url: config.chatgpt_base_url.clone(),
            account_id: auth.get_account_id(),
            chatgpt_user_id: auth.get_chatgpt_user_id(),
            is_workspace_account: auth.is_workspace_account(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemotePluginCatalogDiskCache {
    schema_version: u8,
    plugins: Vec<RemotePluginDirectoryItem>,
}

#[derive(Clone)]
struct MemoryCacheEntry {
    path: PathBuf,
    bytes: Vec<u8>,
    plugins: Vec<RemotePluginDirectoryItem>,
}

#[derive(Default)]
struct MemoryCache {
    entries: VecDeque<MemoryCacheEntry>,
}

impl MemoryCache {
    fn get(&self, path: &Path, bytes: &[u8]) -> Option<Vec<RemotePluginDirectoryItem>> {
        self.entries
            .iter()
            .find(|entry| entry.path == path && entry.bytes == bytes)
            .map(|entry| entry.plugins.clone())
    }

    fn insert(&mut self, path: PathBuf, bytes: Vec<u8>, plugins: Vec<RemotePluginDirectoryItem>) {
        self.entries.retain(|entry| entry.path != path);
        if self.entries.len() == REMOTE_PLUGIN_CATALOG_MEMORY_CACHE_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(MemoryCacheEntry {
            path,
            bytes,
            plugins,
        });
    }

    fn remove(&mut self, path: &Path) {
        self.entries.retain(|entry| entry.path != path);
    }
}

fn memory_cache() -> &'static RwLock<MemoryCache> {
    static CACHE: OnceLock<RwLock<MemoryCache>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(MemoryCache::default()))
}

pub(crate) fn load_cached_global_directory_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
) -> Option<Vec<RemotePluginDirectoryItem>> {
    let cache_path = cache_path(
        codex_home,
        &RemotePluginCatalogCacheKey::global(config, auth),
    );
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
    if let Some(plugins) = memory_cache()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&cache_path, &bytes)
    {
        return Some(plugins);
    }
    let mut memory_cache = memory_cache()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(plugins) = memory_cache.get(&cache_path, &bytes) {
        return Some(plugins);
    }
    let cache: RemotePluginCatalogDiskCache = match serde_json::from_slice(&bytes) {
        Ok(cache) => cache,
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to parse remote plugin catalog disk cache: {err}"
            );
            memory_cache.remove(&cache_path);
            drop(memory_cache);
            let _ = std::fs::remove_file(cache_path);
            return None;
        }
    };
    if cache.schema_version != REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION {
        memory_cache.remove(&cache_path);
        drop(memory_cache);
        let _ = std::fs::remove_file(cache_path);
        return None;
    }

    memory_cache.insert(cache_path, bytes, cache.plugins.clone());
    Some(cache.plugins)
}

pub(crate) fn write_cached_global_directory_plugins(
    codex_home: &Path,
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    plugins: &[RemotePluginDirectoryItem],
) {
    let cache_path = cache_path(
        codex_home,
        &RemotePluginCatalogCacheKey::global(config, auth),
    );
    if let Some(parent) = cache_path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let Ok(bytes) = serde_json::to_vec_pretty(&RemotePluginCatalogDiskCache {
        schema_version: REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION,
        plugins: plugins.to_vec(),
    }) else {
        return;
    };
    if std::fs::write(&cache_path, &bytes).is_ok() {
        memory_cache()
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(cache_path, bytes, plugins.to_vec());
    }
}

#[cfg(test)]
#[path = "catalog_cache_tests.rs"]
mod tests;

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
