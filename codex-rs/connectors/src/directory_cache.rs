use std::path::PathBuf;

use codex_app_server_protocol::AppInfo;
use serde::Deserialize;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;
use tracing::warn;

use crate::ConnectorDirectoryCacheKey;

pub(crate) const CONNECTOR_DIRECTORY_DISK_CACHE_SCHEMA_VERSION: u8 = 1;
const CONNECTOR_DIRECTORY_DISK_CACHE_DIR: &str = "cache/codex_app_directory";

#[derive(Clone)]
pub struct ConnectorDirectoryCacheContext {
    pub(crate) codex_home: PathBuf,
    pub(crate) cache_key: ConnectorDirectoryCacheKey,
}

impl ConnectorDirectoryCacheContext {
    pub fn new(codex_home: PathBuf, cache_key: ConnectorDirectoryCacheKey) -> Self {
        Self {
            codex_home,
            cache_key,
        }
    }

    pub(crate) fn cache_path(&self) -> PathBuf {
        let cache_key_json = serde_json::to_string(&self.cache_key).unwrap_or_default();
        let cache_key_hash = sha1_hex(&cache_key_json);
        self.codex_home
            .join(CONNECTOR_DIRECTORY_DISK_CACHE_DIR)
            .join(format!("{cache_key_hash}.json"))
    }
}

pub(crate) enum CachedConnectorDirectoryDiskLoad {
    Hit { connectors: Vec<AppInfo> },
    Missing,
    Invalid,
}

pub(crate) fn load_cached_directory_connectors_from_disk(
    cache_context: &ConnectorDirectoryCacheContext,
) -> CachedConnectorDirectoryDiskLoad {
    let cache_path = cache_context.cache_path();
    let bytes = match std::fs::read(&cache_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return CachedConnectorDirectoryDiskLoad::Missing;
        }
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to read connector directory disk cache: {err}"
            );
            return CachedConnectorDirectoryDiskLoad::Invalid;
        }
    };
    let cache: ConnectorDirectoryDiskCache = match serde_json::from_slice(&bytes) {
        Ok(cache) => cache,
        Err(err) => {
            warn!(
                cache_path = %cache_path.display(),
                "failed to parse connector directory disk cache: {err}"
            );
            let _ = std::fs::remove_file(cache_path);
            return CachedConnectorDirectoryDiskLoad::Invalid;
        }
    };
    if cache.schema_version != CONNECTOR_DIRECTORY_DISK_CACHE_SCHEMA_VERSION {
        let _ = std::fs::remove_file(cache_path);
        return CachedConnectorDirectoryDiskLoad::Invalid;
    }

    CachedConnectorDirectoryDiskLoad::Hit {
        connectors: cache.connectors,
    }
}

pub(crate) fn write_cached_directory_connectors_to_disk(
    cache_context: &ConnectorDirectoryCacheContext,
    connectors: &[AppInfo],
) {
    let cache_path = cache_context.cache_path();
    if let Some(parent) = cache_path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let Ok(bytes) = serde_json::to_vec_pretty(&ConnectorDirectoryDiskCache {
        schema_version: CONNECTOR_DIRECTORY_DISK_CACHE_SCHEMA_VERSION,
        connectors: connectors.to_vec(),
    }) else {
        return;
    };
    let _ = std::fs::write(cache_path, bytes);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConnectorDirectoryDiskCache {
    schema_version: u8,
    connectors: Vec<AppInfo>,
}

fn sha1_hex(value: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(value.as_bytes());
    let sha1 = hasher.finalize();
    format!("{sha1:x}")
}
