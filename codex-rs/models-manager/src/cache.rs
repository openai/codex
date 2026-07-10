use chrono::DateTime;
use chrono::Utc;
use codex_protocol::openai_models::ModelInfo;
use serde::Deserialize;
use serde::Serialize;
use std::io;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::error;
use tracing::info;

/// Tracks in-process freshness or loads and saves model cache snapshots on disk.
#[derive(Debug)]
pub(crate) struct ModelsCacheManager {
    cache_path: Option<PathBuf>,
    memory_cache_fetched_at: RwLock<Option<Instant>>,
    cache_ttl: Duration,
}

pub(crate) enum ModelsCacheHit {
    InMemory,
    Persisted(ModelsCache),
}

impl ModelsCacheManager {
    /// Create a new cache manager with the given path and TTL.
    pub(crate) fn new(cache_path: PathBuf, cache_ttl: Duration) -> Self {
        Self {
            cache_path: Some(cache_path),
            memory_cache_fetched_at: RwLock::new(None),
            cache_ttl,
        }
    }

    /// Create a cache manager that retains model state in memory only.
    pub(crate) fn without_disk_cache(cache_ttl: Duration) -> Self {
        Self {
            cache_path: None,
            memory_cache_fetched_at: RwLock::new(None),
            cache_ttl,
        }
    }

    /// Attempt to load a fresh cache entry. Returns `None` if the cache doesn't exist or is stale.
    pub(crate) async fn load_fresh(&self, expected_version: &str) -> Option<ModelsCacheHit> {
        info!(
            cache_path = ?self.cache_path.as_ref(),
            expected_version,
            "models cache: attempting load_fresh"
        );
        if self.cache_path.is_none() {
            // An in-memory entry cannot outlive this process or its fixed client version. The
            // model manager already owns the corresponding models and ETag.
            let fetched_at = *self.memory_cache_fetched_at.read().await;
            let is_fresh = fetched_at.is_some_and(|fetched_at| {
                !self.cache_ttl.is_zero() && fetched_at.elapsed() <= self.cache_ttl
            });
            info!(
                cache_ttl_secs = self.cache_ttl.as_secs(),
                is_fresh, "models cache: checked in-memory freshness"
            );
            return is_fresh.then_some(ModelsCacheHit::InMemory);
        }
        let cache = match self.load().await {
            Ok(cache) => cache?,
            Err(err) => {
                error!("failed to load models cache: {err}");
                return None;
            }
        };
        info!(
            cache_path = ?self.cache_path.as_ref(),
            cached_version = ?cache.client_version,
            fetched_at = %cache.fetched_at,
            "models cache: loaded cache entry"
        );
        if cache.client_version.as_deref() != Some(expected_version) {
            info!(
                cache_path = ?self.cache_path.as_ref(),
                expected_version,
                cached_version = ?cache.client_version,
                "models cache: cache version mismatch"
            );
            return None;
        }
        if !cache.is_fresh(self.cache_ttl) {
            info!(
                cache_path = ?self.cache_path.as_ref(),
                cache_ttl_secs = self.cache_ttl.as_secs(),
                fetched_at = %cache.fetched_at,
                "models cache: cache is stale"
            );
            return None;
        }
        info!(
            cache_path = ?self.cache_path.as_ref(),
            cache_ttl_secs = self.cache_ttl.as_secs(),
            "models cache: cache hit"
        );
        Some(ModelsCacheHit::Persisted(cache))
    }

    /// Record fresh cache state, serializing it when disk persistence is configured.
    pub(crate) async fn persist_cache(
        &self,
        models: &[ModelInfo],
        etag: Option<String>,
        client_version: String,
    ) {
        if self.cache_path.is_none() {
            *self.memory_cache_fetched_at.write().await = Some(Instant::now());
            return;
        }
        let cache = ModelsCache {
            fetched_at: Utc::now(),
            etag,
            client_version: Some(client_version),
            models: models.to_vec(),
        };
        if let Err(err) = self.save_internal(&cache).await {
            error!("failed to write models cache: {err}");
        }
    }

    /// Renew the cache TTL by updating the fetched_at timestamp to now.
    pub(crate) async fn renew_cache_ttl(&self) -> io::Result<()> {
        if self.cache_path.is_none() {
            let mut fetched_at = self.memory_cache_fetched_at.write().await;
            if fetched_at.is_none() {
                return Err(io::Error::new(ErrorKind::NotFound, "cache not found"));
            }
            *fetched_at = Some(Instant::now());
            return Ok(());
        }
        let mut cache = match self.load().await? {
            Some(cache) => cache,
            None => return Err(io::Error::new(ErrorKind::NotFound, "cache not found")),
        };
        cache.fetched_at = Utc::now();
        self.save_internal(&cache).await
    }

    async fn load(&self) -> io::Result<Option<ModelsCache>> {
        let Some(cache_path) = self.cache_path.as_ref() else {
            return Ok(None);
        };
        match fs::read(cache_path).await {
            Ok(contents) => {
                let cache = serde_json::from_slice(&contents)
                    .map_err(|err| io::Error::new(ErrorKind::InvalidData, err.to_string()))?;
                Ok(Some(cache))
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    async fn save_internal(&self, cache: &ModelsCache) -> io::Result<()> {
        let Some(cache_path) = self.cache_path.as_ref() else {
            return Err(io::Error::new(
                ErrorKind::Unsupported,
                "memory-only cache does not serialize model data",
            ));
        };
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_vec_pretty(cache)
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err.to_string()))?;
        fs::write(cache_path, json).await
    }

    #[cfg(test)]
    /// Set the cache TTL.
    pub(crate) fn set_ttl(&mut self, ttl: Duration) {
        self.cache_ttl = ttl;
    }

    #[cfg(test)]
    /// Manipulate cache file for testing. Allows setting a custom fetched_at timestamp.
    pub(crate) async fn manipulate_cache_for_test<F>(&self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut DateTime<Utc>),
    {
        let mut cache = match self.load().await? {
            Some(cache) => cache,
            None => return Err(io::Error::new(ErrorKind::NotFound, "cache not found")),
        };
        f(&mut cache.fetched_at);
        self.save_internal(&cache).await
    }

    #[cfg(test)]
    /// Mutate the full cache contents for testing.
    pub(crate) async fn mutate_cache_for_test<F>(&self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut ModelsCache),
    {
        let mut cache = match self.load().await? {
            Some(cache) => cache,
            None => return Err(io::Error::new(ErrorKind::NotFound, "cache not found")),
        };
        f(&mut cache);
        self.save_internal(&cache).await
    }
}

/// Serialized snapshot of models and metadata cached on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ModelsCache {
    pub(crate) fetched_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) etag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) client_version: Option<String>,
    pub(crate) models: Vec<ModelInfo>,
}

impl ModelsCache {
    /// Returns `true` when the cache entry has not exceeded the configured TTL.
    fn is_fresh(&self, ttl: Duration) -> bool {
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
