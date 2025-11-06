//! AI response caching to reduce redundant API calls

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Cache for AI responses
pub struct AICache {
    /// In-memory cache
    cache: Arc<RwLock<HashMap<String, CachedResponse>>>,
    /// Maximum cache entries
    max_entries: usize,
    /// Cache file path (optional persistent cache)
    cache_file: Option<PathBuf>,
    /// Enable persistent cache
    persistent: bool,
}

impl AICache {
    /// Create a new AI cache
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
            cache_file: None,
            persistent: false,
        }
    }

    /// Enable persistent cache with file
    pub fn with_persistent_cache(mut self, cache_file: PathBuf) -> Self {
        self.cache_file = Some(cache_file);
        self.persistent = true;
        self
    }

    /// Load cache from disk
    pub async fn load(&self) -> Result<()> {
        if !self.persistent {
            return Ok(());
        }

        let cache_file = match &self.cache_file {
            Some(f) => f,
            None => return Ok(()),
        };

        if !cache_file.exists() {
            debug!("Cache file does not exist, starting with empty cache");
            return Ok(());
        }

        let content = tokio::fs::read_to_string(cache_file).await?;
        let disk_cache: HashMap<String, CachedResponse> = serde_json::from_str(&content)?;

        let mut cache = self.cache.write().await;
        *cache = disk_cache;

        info!("Loaded {} entries from cache file", cache.len());
        Ok(())
    }

    /// Save cache to disk
    pub async fn save(&self) -> Result<()> {
        if !self.persistent {
            return Ok(());
        }

        let cache_file = match &self.cache_file {
            Some(f) => f,
            None => return Ok(()),
        };

        let cache = self.cache.read().await;
        let content = serde_json::to_string_pretty(&*cache)?;

        // Create parent directory if needed
        if let Some(parent) = cache_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(cache_file, content).await?;

        info!("Saved {} entries to cache file", cache.len());
        Ok(())
    }

    /// Get cached response
    pub async fn get(&self, key: &str) -> Option<String> {
        let cache = self.cache.read().await;

        if let Some(cached) = cache.get(key) {
            // Check if expired
            if cached.is_expired() {
                debug!("Cache hit but expired: {}", Self::truncate_key(key));
                return None;
            }

            debug!("Cache hit: {}", Self::truncate_key(key));
            return Some(cached.response.clone());
        }

        debug!("Cache miss: {}", Self::truncate_key(key));
        None
    }

    /// Put response in cache
    pub async fn put(&self, key: String, response: String, ttl_secs: u64) {
        let mut cache = self.cache.write().await;

        // Evict oldest if at capacity
        if cache.len() >= self.max_entries {
            self.evict_oldest(&mut cache);
        }

        let cached = CachedResponse {
            response,
            timestamp: chrono::Utc::now(),
            ttl_secs,
        };

        cache.insert(key.clone(), cached);
        debug!("Cached response: {}", Self::truncate_key(&key));
    }

    /// Clear all cache entries
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        info!("Cache cleared");
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let cache = self.cache.read().await;

        let total_entries = cache.len();
        let expired_entries = cache.values().filter(|v| v.is_expired()).count();

        CacheStats {
            total_entries,
            expired_entries,
            active_entries: total_entries - expired_entries,
            max_entries: self.max_entries,
        }
    }

    /// Remove expired entries
    pub async fn cleanup_expired(&self) {
        let mut cache = self.cache.write().await;
        let before = cache.len();
        cache.retain(|_, v| !v.is_expired());
        let removed = before - cache.len();

        if removed > 0 {
            info!("Cleaned up {} expired cache entries", removed);
        }
    }

    /// Evict oldest entry
    fn evict_oldest(&self, cache: &mut HashMap<String, CachedResponse>) {
        if let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, v)| v.timestamp)
            .map(|(k, _)| k.clone())
        {
            cache.remove(&oldest_key);
            debug!("Evicted oldest cache entry");
        }
    }

    /// Truncate key for logging
    fn truncate_key(key: &str) -> String {
        if key.len() > 100 {
            format!("{}...", &key[..100])
        } else {
            key.to_string()
        }
    }

    /// Generate cache key from code content and prompt
    pub fn generate_key(content: &str, prompt: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        prompt.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Generate cache key from file path and metadata
    pub async fn generate_file_key(file_path: &Path, operation: &str) -> Result<String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Include file modification time in key
        let metadata = tokio::fs::metadata(file_path).await?;
        let modified = metadata.modified()?;

        let mut hasher = DefaultHasher::new();
        file_path.to_string_lossy().hash(&mut hasher);
        operation.hash(&mut hasher);
        format!("{:?}", modified).hash(&mut hasher);

        Ok(format!("{:x}", hasher.finish()))
    }
}

/// Cached AI response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedResponse {
    /// The cached response
    response: String,
    /// When it was cached
    timestamp: chrono::DateTime<chrono::Utc>,
    /// Time-to-live in seconds
    ttl_secs: u64,
}

impl CachedResponse {
    /// Check if cache entry is expired
    fn is_expired(&self) -> bool {
        let now = chrono::Utc::now();
        let age = now.signed_duration_since(self.timestamp);
        age.num_seconds() >= self.ttl_secs as i64
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total entries in cache
    pub total_entries: usize,
    /// Expired entries
    pub expired_entries: usize,
    /// Active (non-expired) entries
    pub active_entries: usize,
    /// Maximum allowed entries
    pub max_entries: usize,
}

impl CacheStats {
    /// Get hit rate estimate (active/max)
    pub fn utilization(&self) -> f32 {
        if self.max_entries == 0 {
            0.0
        } else {
            self.active_entries as f32 / self.max_entries as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_put_get() {
        let cache = AICache::new(10);

        cache.put(
            "test_key".to_string(),
            "test_response".to_string(),
            3600,
        ).await;

        let result = cache.get("test_key").await;
        assert_eq!(result, Some("test_response".to_string()));
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = AICache::new(10);
        let result = cache.get("nonexistent").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = AICache::new(10);

        // Cache with 0 TTL (immediately expired)
        cache.put(
            "expired_key".to_string(),
            "expired_response".to_string(),
            0,
        ).await;

        // Sleep a bit to ensure expiration
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let result = cache.get("expired_key").await;
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let cache = AICache::new(3);

        // Add 4 entries (should evict oldest)
        for i in 0..4 {
            cache.put(
                format!("key_{}", i),
                format!("response_{}", i),
                3600,
            ).await;

            // Small delay to ensure different timestamps
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 3);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let cache = AICache::new(10);

        cache.put("key1".to_string(), "response1".to_string(), 3600).await;
        cache.put("key2".to_string(), "response2".to_string(), 3600).await;

        cache.clear().await;

        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 0);
    }

    #[test]
    fn test_generate_key() {
        let key1 = AICache::generate_key("code content", "analyze this");
        let key2 = AICache::generate_key("code content", "analyze this");
        let key3 = AICache::generate_key("different code", "analyze this");

        // Same input should produce same key
        assert_eq!(key1, key2);
        // Different input should produce different key
        assert_ne!(key1, key3);
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let cache = AICache::new(10);

        // Add expired entry
        cache.put("expired".to_string(), "data".to_string(), 0).await;
        // Add valid entry
        cache.put("valid".to_string(), "data".to_string(), 3600).await;

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        cache.cleanup_expired().await;

        let stats = cache.stats().await;
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.active_entries, 1);
    }
}
