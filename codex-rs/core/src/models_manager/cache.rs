use chrono::DateTime;
use chrono::Utc;
use codex_protocol::openai_models::ModelInfo;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;
use tokio::fs;

/// Serialized snapshot of models and metadata cached on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ModelsCache {
    pub(crate) fetched_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) etag: Option<String>,
    pub(crate) models: Vec<ModelInfo>,
}

impl ModelsCache {
    /// Returns `true` when the cache entry has not exceeded the configured TTL.
    pub(crate) fn is_fresh(&self, ttl: Duration) -> bool {
        if ttl.is_zero() {
            return false;
        }
        let Ok(ttl_duration) = chrono::Duration::from_std(ttl) else {
            return false;
        };
        let age = Utc::now().signed_duration_since(self.fetched_at);
        age <= ttl_duration
    }
}

/// Read and deserialize the cache file if it exists.
pub(crate) async fn load_cache(path: &Path) -> io::Result<Option<ModelsCache>> {
    match fs::read(path).await {
        Ok(contents) => {
            let cache = serde_json::from_slice(&contents)
                .map_err(|err| io::Error::new(ErrorKind::InvalidData, err.to_string()))?;
            Ok(Some(cache))
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Persist the cache contents to disk, creating parent directories as needed.
pub(crate) async fn save_cache(path: &Path, cache: &ModelsCache) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let json = serde_json::to_vec_pretty(cache)
        .map_err(|err| io::Error::new(ErrorKind::InvalidData, err.to_string()))?;
    fs::write(path, json).await
}
