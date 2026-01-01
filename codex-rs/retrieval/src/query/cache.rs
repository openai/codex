//! Query rewrite cache.
//!
//! Provides SQLite-based caching for query rewrite results to avoid
//! redundant LLM calls for frequently used queries.

use std::sync::Arc;

use crate::config::RewriteCacheConfig;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::query::RewrittenQuery;
use crate::storage::SqliteStore;

/// Query rewrite cache.
///
/// Stores rewritten query results in SQLite with TTL support.
/// Includes a config hash in the cache key to invalidate cache
/// when rewrite settings (LLM model, provider) change.
pub struct RewriteCache {
    db: Arc<SqliteStore>,
    config: RewriteCacheConfig,
    /// Hash of LLM config to invalidate cache on config change
    config_hash: String,
}

impl RewriteCache {
    /// Create a new rewrite cache.
    ///
    /// Note: This is async because schema initialization requires async DB access.
    ///
    /// # Arguments
    /// * `db` - SQLite store for persistence
    /// * `config` - Cache configuration (TTL, max entries, etc.)
    /// * `llm_config_hash` - Hash of LLM config (provider + model) to invalidate
    ///   cache when rewrite settings change
    pub async fn new(
        db: Arc<SqliteStore>,
        config: RewriteCacheConfig,
        llm_config_hash: &str,
    ) -> Result<Self> {
        let cache = Self {
            db,
            config,
            config_hash: llm_config_hash.to_string(),
        };
        cache.init_schema().await?;
        Ok(cache)
    }

    /// Compute a hash from LLM configuration for cache key versioning.
    ///
    /// Use this to generate the `llm_config_hash` parameter for `new()`.
    pub fn compute_llm_config_hash(provider: &str, model: &str) -> String {
        use sha2::Digest;
        use sha2::Sha256;

        let config_str = format!("{provider}:{model}");
        let mut hasher = Sha256::new();
        hasher.update(config_str.as_bytes());
        let result = hasher.finalize();
        // Use first 8 bytes for compact key
        hex::encode(&result[..8])
    }

    /// Initialize the cache schema.
    async fn init_schema(&self) -> Result<()> {
        self.db
            .query(|conn| {
                conn.execute_batch(CACHE_SCHEMA)?;
                Ok(())
            })
            .await
    }

    /// Get a cached rewrite result.
    pub async fn get(&self, query: &str) -> Result<Option<RewrittenQuery>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let cache_key = compute_cache_key(query, &self.config_hash);
        let now = chrono::Utc::now().timestamp();

        let db = self.db.clone();
        db.query(move |conn| {
            let mut stmt = conn.prepare(
                "SELECT result_json FROM query_rewrite_cache
                 WHERE cache_key = ? AND expires_at > ?",
            )?;

            let result: Option<String> = stmt
                .query_row(rusqlite::params![cache_key, now], |row| row.get(0))
                .ok();

            if let Some(json) = result {
                // Update hit count
                let _ = conn.execute(
                    "UPDATE query_rewrite_cache SET hit_count = hit_count + 1 WHERE cache_key = ?",
                    rusqlite::params![cache_key],
                );

                // Parse the cached result
                serde_json::from_str(&json)
                    .map(Some)
                    .map_err(|e| RetrievalErr::json_parse("rewrite cache result", e))
            } else {
                Ok(None)
            }
        })
        .await
    }

    /// Store a rewrite result in the cache.
    pub async fn put(&self, query: &str, result: &RewrittenQuery) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let cache_key = compute_cache_key(query, &self.config_hash);
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + self.config.ttl_secs;
        let original = query.to_string();

        let result_json = serde_json::to_string(result)
            .map_err(|e| RetrievalErr::json_parse("rewrite result serialization", e))?;

        let db = self.db.clone();
        let max_entries = self.config.max_entries;

        db.query(move |conn| {
            // Insert or replace the cache entry
            conn.execute(
                "INSERT OR REPLACE INTO query_rewrite_cache
                 (cache_key, original_query, result_json, created_at, expires_at, hit_count)
                 VALUES (?, ?, ?, ?, ?, 0)",
                rusqlite::params![cache_key, original, result_json, now, expires_at],
            )?;

            // Prune old entries if over limit
            let count: i32 =
                conn.query_row("SELECT COUNT(*) FROM query_rewrite_cache", [], |row| {
                    row.get(0)
                })?;

            if count > max_entries {
                let to_delete = count - max_entries;
                conn.execute(
                    "DELETE FROM query_rewrite_cache WHERE cache_key IN (
                        SELECT cache_key FROM query_rewrite_cache
                        ORDER BY hit_count ASC, created_at ASC
                        LIMIT ?
                    )",
                    rusqlite::params![to_delete],
                )?;
            }

            Ok(())
        })
        .await
    }

    /// Clear expired entries from the cache.
    pub async fn prune_expired(&self) -> Result<i32> {
        let now = chrono::Utc::now().timestamp();
        let db = self.db.clone();

        db.query(move |conn| {
            let deleted = conn.execute(
                "DELETE FROM query_rewrite_cache WHERE expires_at < ?",
                rusqlite::params![now],
            )?;
            Ok(deleted as i32)
        })
        .await
    }

    /// Get cache statistics.
    pub async fn stats(&self) -> Result<CacheStats> {
        let now = chrono::Utc::now().timestamp();
        let db = self.db.clone();

        db.query(move |conn| {
            let total: i32 =
                conn.query_row("SELECT COUNT(*) FROM query_rewrite_cache", [], |row| {
                    row.get(0)
                })?;

            let valid: i32 = conn.query_row(
                "SELECT COUNT(*) FROM query_rewrite_cache WHERE expires_at > ?",
                rusqlite::params![now],
                |row| row.get(0),
            )?;

            let total_hits: i64 = conn.query_row(
                "SELECT COALESCE(SUM(hit_count), 0) FROM query_rewrite_cache",
                [],
                |row| row.get(0),
            )?;

            Ok(CacheStats {
                total_entries: total,
                valid_entries: valid,
                expired_entries: total - valid,
                total_hits,
            })
        })
        .await
    }

    /// Clear all cache entries.
    pub async fn clear(&self) -> Result<()> {
        let db = self.db.clone();
        db.query(|conn| {
            conn.execute("DELETE FROM query_rewrite_cache", [])?;
            Ok(())
        })
        .await
    }
}

/// Cache statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Total entries in cache
    pub total_entries: i32,
    /// Valid (non-expired) entries
    pub valid_entries: i32,
    /// Expired entries (pending cleanup)
    pub expired_entries: i32,
    /// Total cache hits
    pub total_hits: i64,
}

/// Compute a cache key for a query.
///
/// Uses SHA-256 (truncated to 128 bits) for better collision resistance
/// compared to DefaultHasher. This provides ~2^64 collision resistance
/// via birthday paradox, sufficient for millions of cached queries.
///
/// Includes the config hash to invalidate cache when LLM settings change.
fn compute_cache_key(query: &str, config_hash: &str) -> String {
    use sha2::Digest;
    use sha2::Sha256;

    let normalized = query.trim().to_lowercase();
    let mut hasher = Sha256::new();
    // Include config hash to invalidate on config change
    hasher.update(config_hash.as_bytes());
    hasher.update(b":");
    hasher.update(normalized.as_bytes());
    let result = hasher.finalize();
    // Use first 16 bytes (128 bits) for key - sufficient for cache
    hex::encode(&result[..16])
}

/// SQLite schema for the rewrite cache.
const CACHE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS query_rewrite_cache (
    cache_key TEXT PRIMARY KEY,
    original_query TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    hit_count INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_cache_expires ON query_rewrite_cache(expires_at);
CREATE INDEX IF NOT EXISTS idx_cache_hits ON query_rewrite_cache(hit_count);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::QueryIntent;
    use crate::query::RewriteSource;
    use tempfile::TempDir;

    async fn create_test_cache() -> (RewriteCache, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Arc::new(SqliteStore::open(&db_path).unwrap());

        let config = RewriteCacheConfig {
            enabled: true,
            ttl_secs: 3600,
            max_entries: 100,
        };

        let config_hash = RewriteCache::compute_llm_config_hash("openai", "gpt-4o-mini");
        let cache = RewriteCache::new(db, config, &config_hash).await.unwrap();
        (cache, dir)
    }

    #[tokio::test]
    async fn test_cache_put_and_get() {
        let (cache, _dir) = create_test_cache().await;

        let query = "test query";
        let result = RewrittenQuery::unchanged(query)
            .with_intent(QueryIntent::Definition)
            .with_source(RewriteSource::Rule);

        // Put into cache
        cache.put(query, &result).await.unwrap();

        // Get from cache
        let cached = cache.get(query).await.unwrap();
        assert!(cached.is_some());
        let cached = cached.unwrap();
        assert_eq!(cached.original, query);
        assert_eq!(cached.intent, QueryIntent::Definition);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let (cache, _dir) = create_test_cache().await;

        let cached = cache.get("nonexistent").await.unwrap();
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Arc::new(SqliteStore::open(&db_path).unwrap());

        let config = RewriteCacheConfig {
            enabled: false,
            ttl_secs: 3600,
            max_entries: 100,
        };

        let config_hash = RewriteCache::compute_llm_config_hash("openai", "gpt-4o-mini");
        let cache = RewriteCache::new(db, config, &config_hash).await.unwrap();

        let result = RewrittenQuery::unchanged("test");
        cache.put("test", &result).await.unwrap();

        let cached = cache.get("test").await.unwrap();
        assert!(cached.is_none()); // Cache disabled
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let (cache, _dir) = create_test_cache().await;

        // Add some entries
        for i in 0..5 {
            let query = format!("query {i}");
            let result = RewrittenQuery::unchanged(&query);
            cache.put(&query, &result).await.unwrap();
        }

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total_entries, 5);
        assert_eq!(stats.valid_entries, 5);
        assert_eq!(stats.expired_entries, 0);
    }

    #[tokio::test]
    async fn test_cache_hit_count() {
        let (cache, _dir) = create_test_cache().await;

        let query = "test";
        let result = RewrittenQuery::unchanged(query);
        cache.put(query, &result).await.unwrap();

        // Get multiple times
        for _ in 0..3 {
            let _ = cache.get(query).await.unwrap();
        }

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total_hits, 3);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let (cache, _dir) = create_test_cache().await;

        let result = RewrittenQuery::unchanged("test");
        cache.put("test", &result).await.unwrap();

        cache.clear().await.unwrap();

        let stats = cache.stats().await.unwrap();
        assert_eq!(stats.total_entries, 0);
    }
}
