use super::RemotePluginDirectoryItem;
use super::RemotePluginServiceConfig;
use codex_login::CodexAuth;
use serde::Deserialize;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;
use tracing::warn;

const REMOTE_PLUGIN_CATALOG_DISK_CACHE_SCHEMA_VERSION: u8 = 1;
const REMOTE_PLUGIN_CATALOG_DISK_CACHE_DIR: &str = "cache/remote_plugin_catalog";

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
    let _ = std::fs::write(cache_path, bytes);
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
