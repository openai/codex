//! Embedding cache to avoid recomputing embeddings for unchanged content.
//!
//! Uses SQLite to persist embeddings keyed by (filepath, content_hash, artifact_id).
//! This allows precise deletion by filepath when files are modified or deleted.
//!
//! Reference: Continue `core/indexing/LanceDbIndex.ts`

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use rusqlite::params;

use crate::error::Result;
use crate::error::RetrievalErr;

/// Result of a bulk cache lookup.
///
/// Separates cache hits from misses for efficient processing.
#[derive(Debug, Default)]
pub struct CacheLookupResult {
    /// Found entries: (filepath, content_hash, embedding)
    pub hits: Vec<(String, String, Vec<f32>)>,
    /// Missing entries: (filepath, content_hash)
    pub misses: Vec<(String, String)>,
}

impl CacheLookupResult {
    /// Returns true if all entries were found in cache.
    pub fn all_hits(&self) -> bool {
        self.misses.is_empty()
    }

    /// Returns true if no entries were found in cache.
    pub fn all_misses(&self) -> bool {
        self.hits.is_empty()
    }

    /// Total number of entries queried.
    pub fn total(&self) -> usize {
        self.hits.len() + self.misses.len()
    }

    /// Cache hit ratio (0.0 to 1.0).
    pub fn hit_ratio(&self) -> f32 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.hits.len() as f32 / total as f32
        }
    }
}

/// Embedding cache backed by SQLite.
///
/// Stores embeddings keyed by (filepath, content_hash), with artifact ID versioning
/// to invalidate cache when the embedding model changes.
///
/// Using filepath as part of the key allows:
/// - Precise deletion when a file is modified or deleted
/// - Simple cache management without reference counting
pub struct EmbeddingCache {
    conn: Mutex<Connection>,
    artifact_id: String,
}

impl EmbeddingCache {
    /// Open or create an embedding cache at the given path.
    ///
    /// # Arguments
    /// * `path` - Path to the SQLite database file
    /// * `artifact_id` - Identifier for the embedding model (e.g., "text-embedding-3-small-v1")
    pub fn open(path: &Path, artifact_id: &str) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| RetrievalErr::SqliteFailed {
            operation: "open embedding cache".to_string(),
            cause: e.to_string(),
        })?;

        // Create embeddings table with (filepath, content_hash, artifact_id) as composite key
        conn.execute(
            "CREATE TABLE IF NOT EXISTS embeddings (
                filepath TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                artifact_id TEXT NOT NULL,
                embedding BLOB NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                PRIMARY KEY (filepath, content_hash, artifact_id)
            )",
            [],
        )
        .map_err(|e| RetrievalErr::SqliteFailed {
            operation: "create embeddings table".to_string(),
            cause: e.to_string(),
        })?;

        // Create index for efficient filepath lookups (for delete_by_filepath)
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_embeddings_filepath ON embeddings(filepath)",
            [],
        )
        .map_err(|e| RetrievalErr::SqliteFailed {
            operation: "create filepath index".to_string(),
            cause: e.to_string(),
        })?;

        // Create index for efficient artifact_id lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_embeddings_artifact ON embeddings(artifact_id)",
            [],
        )
        .map_err(|e| RetrievalErr::SqliteFailed {
            operation: "create artifact index".to_string(),
            cause: e.to_string(),
        })?;

        Ok(Self {
            conn: Mutex::new(conn),
            artifact_id: artifact_id.to_string(),
        })
    }

    /// Get a cached embedding for the given filepath and content hash.
    ///
    /// Returns `None` if the embedding is not cached or was created with
    /// a different artifact ID.
    pub fn get(&self, filepath: &str, content_hash: &str) -> Option<Vec<f32>> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT embedding FROM embeddings WHERE filepath = ? AND content_hash = ? AND artifact_id = ?",
            params![filepath, content_hash, self.artifact_id],
            |row| {
                let bytes: Vec<u8> = row.get(0)?;
                Ok(bytes_to_f32_vec(&bytes))
            },
        )
        .ok()
    }

    /// Store an embedding in the cache.
    ///
    /// Overwrites any existing entry with the same (filepath, content_hash).
    pub fn put(&self, filepath: &str, content_hash: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.conn.lock().map_err(|_| RetrievalErr::SqliteFailed {
            operation: "lock embedding cache".to_string(),
            cause: "mutex poisoned".to_string(),
        })?;

        let bytes = f32_vec_to_bytes(embedding);
        conn.execute(
            "INSERT OR REPLACE INTO embeddings (filepath, content_hash, artifact_id, embedding) VALUES (?, ?, ?, ?)",
            params![filepath, content_hash, self.artifact_id, bytes],
        )
        .map_err(|e| RetrievalErr::SqliteFailed {
            operation: "insert embedding".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Delete all cached embeddings for a filepath.
    ///
    /// Call this when a file is modified or deleted to clean up stale cache entries.
    pub fn delete_by_filepath(&self, filepath: &str) -> Result<i32> {
        let conn = self.conn.lock().map_err(|_| RetrievalErr::SqliteFailed {
            operation: "lock embedding cache".to_string(),
            cause: "mutex poisoned".to_string(),
        })?;

        let count = conn
            .execute(
                "DELETE FROM embeddings WHERE filepath = ?",
                params![filepath],
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "delete embeddings by filepath".to_string(),
                cause: e.to_string(),
            })?;

        Ok(count as i32)
    }

    /// Get multiple cached embeddings at once.
    ///
    /// Returns a vector of (filepath, content_hash, embedding) tuples for found entries.
    pub fn get_batch(&self, entries: &[(String, String)]) -> Vec<(String, String, Vec<f32>)> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();
        for (filepath, hash) in entries {
            if let Ok(embedding) = conn.query_row(
                "SELECT embedding FROM embeddings WHERE filepath = ? AND content_hash = ? AND artifact_id = ?",
                params![filepath, hash, self.artifact_id],
                |row| {
                    let bytes: Vec<u8> = row.get(0)?;
                    Ok(bytes_to_f32_vec(&bytes))
                },
            ) {
                results.push((filepath.clone(), hash.clone(), embedding));
            }
        }

        results
    }

    /// Store multiple embeddings in the cache.
    ///
    /// Each entry is (filepath, content_hash, embedding).
    pub fn put_batch(&self, entries: &[(String, String, Vec<f32>)]) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| RetrievalErr::SqliteFailed {
            operation: "lock embedding cache".to_string(),
            cause: "mutex poisoned".to_string(),
        })?;

        let tx = conn.transaction().map_err(|e| RetrievalErr::SqliteFailed {
            operation: "begin transaction".to_string(),
            cause: e.to_string(),
        })?;

        for (filepath, hash, embedding) in entries {
            let bytes = f32_vec_to_bytes(embedding);
            tx.execute(
                "INSERT OR REPLACE INTO embeddings (filepath, content_hash, artifact_id, embedding) VALUES (?, ?, ?, ?)",
                params![filepath, hash, self.artifact_id, bytes],
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "insert embedding batch".to_string(),
                cause: e.to_string(),
            })?;
        }

        tx.commit().map_err(|e| RetrievalErr::SqliteFailed {
            operation: "commit transaction".to_string(),
            cause: e.to_string(),
        })?;

        Ok(())
    }

    /// Remove all embeddings with a different artifact ID.
    ///
    /// Useful for cleaning up stale cache entries after model upgrade.
    pub fn prune_stale(&self) -> Result<i32> {
        let conn = self.conn.lock().map_err(|_| RetrievalErr::SqliteFailed {
            operation: "lock embedding cache".to_string(),
            cause: "mutex poisoned".to_string(),
        })?;

        let count = conn
            .execute(
                "DELETE FROM embeddings WHERE artifact_id != ?",
                params![self.artifact_id],
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "prune stale embeddings".to_string(),
                cause: e.to_string(),
            })?;

        Ok(count as i32)
    }

    /// Get the total number of cached embeddings.
    pub fn count(&self) -> Result<i32> {
        let conn = self.conn.lock().map_err(|_| RetrievalErr::SqliteFailed {
            operation: "lock embedding cache".to_string(),
            cause: "mutex poisoned".to_string(),
        })?;

        let count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM embeddings WHERE artifact_id = ?",
                params![self.artifact_id],
                |row| row.get(0),
            )
            .map_err(|e| RetrievalErr::SqliteFailed {
                operation: "count embeddings".to_string(),
                cause: e.to_string(),
            })?;

        Ok(count)
    }

    /// Get the artifact ID used by this cache.
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// Execute a function with the connection.
    ///
    /// Internal API for cache_ext bulk operations.
    pub(crate) fn with_conn<F, T>(&self, f: F) -> crate::error::Result<T>
    where
        F: FnOnce(&rusqlite::Connection, &str) -> crate::error::Result<T>,
    {
        let conn = self.conn.lock().map_err(|_| RetrievalErr::SqliteFailed {
            operation: "lock embedding cache".to_string(),
            cause: "mutex poisoned".to_string(),
        })?;
        f(&conn, &self.artifact_id)
    }

    /// Bulk lookup using SQL WHERE IN clause.
    ///
    /// More efficient than sequential queries for large batches.
    /// Returns both hits and misses for efficient downstream processing.
    pub fn get_batch_bulk(&self, entries: &[(String, String)]) -> Result<CacheLookupResult> {
        if entries.is_empty() {
            return Ok(CacheLookupResult::default());
        }

        let entries_clone: Vec<(String, String)> = entries.to_vec();

        self.with_conn(|conn, artifact_id| {
            let mut conditions = Vec::with_capacity(entries_clone.len());
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            for (filepath, hash) in &entries_clone {
                conditions.push("(filepath = ? AND content_hash = ?)");
                params_vec.push(Box::new(filepath.clone()));
                params_vec.push(Box::new(hash.clone()));
            }

            let query = format!(
                "SELECT filepath, content_hash, embedding FROM embeddings WHERE artifact_id = ? AND ({})",
                conditions.join(" OR ")
            );

            let mut all_params: Vec<Box<dyn rusqlite::ToSql>> =
                vec![Box::new(artifact_id.to_string())];
            all_params.extend(params_vec);

            let params_refs: Vec<&dyn rusqlite::ToSql> =
                all_params.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn
                .prepare(&query)
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "prepare bulk lookup".to_string(),
                    cause: e.to_string(),
                })?;

            let rows = stmt
                .query_map(params_refs.as_slice(), |row| {
                    let filepath: String = row.get(0)?;
                    let content_hash: String = row.get(1)?;
                    let bytes: Vec<u8> = row.get(2)?;
                    Ok((filepath, content_hash, bytes_to_f32_vec(&bytes)))
                })
                .map_err(|e| RetrievalErr::SqliteFailed {
                    operation: "execute bulk lookup".to_string(),
                    cause: e.to_string(),
                })?;

            let mut hits = Vec::new();
            let mut found_keys: HashSet<(String, String)> = HashSet::new();

            for row_result in rows {
                if let Ok((filepath, hash, embedding)) = row_result {
                    found_keys.insert((filepath.clone(), hash.clone()));
                    hits.push((filepath, hash, embedding));
                }
            }

            let misses: Vec<(String, String)> = entries_clone
                .iter()
                .filter(|(f, h)| !found_keys.contains(&(f.clone(), h.clone())))
                .cloned()
                .collect();

            Ok(CacheLookupResult { hits, misses })
        })
    }

    /// Lookup with deduplication by content hash.
    ///
    /// When multiple files have identical content, only one embedding needs to
    /// be computed. Returns unique content hashes that need embedding.
    pub fn get_batch_deduplicated(
        &self,
        entries: &[(String, String)],
    ) -> Result<(Vec<(String, String, Vec<f32>)>, Vec<String>)> {
        let result = self.get_batch_bulk(entries)?;

        let mut unique_hashes: HashSet<String> = HashSet::new();
        for (_, hash) in &result.misses {
            unique_hashes.insert(hash.clone());
        }

        Ok((result.hits, unique_hashes.into_iter().collect()))
    }
}

/// Convert a byte slice to a Vec<f32>.
pub(crate) fn bytes_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Convert a Vec<f32> to bytes.
fn f32_vec_to_bytes(floats: &[f32]) -> Vec<u8> {
    floats.iter().flat_map(|f| f.to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_put_and_get() {
        let dir = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(&dir.path().join("cache.db"), "test-model-v1").unwrap();

        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        cache.put("src/main.rs", "hash123", &embedding).unwrap();

        let retrieved = cache.get("src/main.rs", "hash123").unwrap();
        assert_eq!(retrieved.len(), 4);
        assert!((retrieved[0] - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_artifact_id_isolation() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.db");

        // Store with model v1
        let cache_v1 = EmbeddingCache::open(&path, "model-v1").unwrap();
        cache_v1
            .put("src/lib.rs", "hash123", &[1.0, 2.0, 3.0])
            .unwrap();

        // Try to retrieve with model v2 - should not find it
        let cache_v2 = EmbeddingCache::open(&path, "model-v2").unwrap();
        assert!(cache_v2.get("src/lib.rs", "hash123").is_none());

        // Original model should still work
        assert!(cache_v1.get("src/lib.rs", "hash123").is_some());
    }

    #[test]
    fn test_delete_by_filepath() {
        let dir = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(&dir.path().join("cache.db"), "test-model").unwrap();

        // Store embeddings for multiple files
        cache.put("file_a.rs", "hash_a1", &[1.0, 2.0]).unwrap();
        cache.put("file_a.rs", "hash_a2", &[3.0, 4.0]).unwrap(); // same file, different hash
        cache.put("file_b.rs", "hash_b1", &[5.0, 6.0]).unwrap();

        assert_eq!(cache.count().unwrap(), 3);

        // Delete all embeddings for file_a.rs
        let deleted = cache.delete_by_filepath("file_a.rs").unwrap();
        assert_eq!(deleted, 2);

        // Verify file_a.rs entries are gone
        assert!(cache.get("file_a.rs", "hash_a1").is_none());
        assert!(cache.get("file_a.rs", "hash_a2").is_none());

        // file_b.rs should still exist
        assert!(cache.get("file_b.rs", "hash_b1").is_some());
        assert_eq!(cache.count().unwrap(), 1);
    }

    #[test]
    fn test_batch_operations() {
        let dir = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(&dir.path().join("cache.db"), "test-model").unwrap();

        let entries = vec![
            ("file1.rs".to_string(), "hash1".to_string(), vec![0.1, 0.2]),
            ("file2.rs".to_string(), "hash2".to_string(), vec![0.3, 0.4]),
            ("file3.rs".to_string(), "hash3".to_string(), vec![0.5, 0.6]),
        ];

        cache.put_batch(&entries).unwrap();

        let queries: Vec<(String, String)> = vec![
            ("file1.rs".to_string(), "hash1".to_string()),
            ("file2.rs".to_string(), "hash2".to_string()),
            ("missing.rs".to_string(), "missing".to_string()),
        ];
        let results = cache.get_batch(&queries);

        assert_eq!(results.len(), 2); // Only file1 and file2 found
    }

    #[test]
    fn test_prune_stale() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("cache.db");

        // Store with old model
        let cache_old = EmbeddingCache::open(&path, "model-old").unwrap();
        cache_old.put("file1.rs", "hash1", &[1.0]).unwrap();
        cache_old.put("file2.rs", "hash2", &[2.0]).unwrap();

        // Store with new model
        let cache_new = EmbeddingCache::open(&path, "model-new").unwrap();
        cache_new.put("file3.rs", "hash3", &[3.0]).unwrap();

        // Prune old entries
        let pruned = cache_new.prune_stale().unwrap();
        assert_eq!(pruned, 2);

        // Verify old entries are gone
        assert!(cache_old.get("file1.rs", "hash1").is_none());
        // New entries remain
        assert!(cache_new.get("file3.rs", "hash3").is_some());
    }

    #[test]
    fn test_count() {
        let dir = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(&dir.path().join("cache.db"), "test-model").unwrap();

        assert_eq!(cache.count().unwrap(), 0);

        cache.put("file1.rs", "hash1", &[1.0]).unwrap();
        cache.put("file2.rs", "hash2", &[2.0]).unwrap();

        assert_eq!(cache.count().unwrap(), 2);
    }

    #[test]
    fn test_same_content_different_files() {
        // Test that same content in different files are stored separately
        let dir = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(&dir.path().join("cache.db"), "test-model").unwrap();

        let same_hash = "same_content_hash";
        cache.put("file_a.rs", same_hash, &[1.0, 2.0]).unwrap();
        cache.put("file_b.rs", same_hash, &[1.0, 2.0]).unwrap();

        assert_eq!(cache.count().unwrap(), 2);

        // Delete file_a.rs should not affect file_b.rs
        cache.delete_by_filepath("file_a.rs").unwrap();

        assert!(cache.get("file_a.rs", same_hash).is_none());
        assert!(cache.get("file_b.rs", same_hash).is_some());
    }

    #[test]
    fn test_byte_conversion() {
        let original = vec![0.1234, 5.6789, -1.0, 0.0];
        let bytes = f32_vec_to_bytes(&original);
        let converted = bytes_to_f32_vec(&bytes);

        assert_eq!(original.len(), converted.len());
        for (a, b) in original.iter().zip(converted.iter()) {
            assert!((a - b).abs() < 0.0001);
        }
    }

    // Bulk lookup tests (from cache_ext.rs)

    fn create_test_cache() -> (TempDir, EmbeddingCache) {
        let dir = TempDir::new().unwrap();
        let cache = EmbeddingCache::open(&dir.path().join("cache.db"), "test-model").unwrap();
        (dir, cache)
    }

    #[test]
    fn test_bulk_lookup_empty() {
        let (_dir, cache) = create_test_cache();
        let result = cache.get_batch_bulk(&[]).unwrap();
        assert!(result.all_hits()); // Empty is considered all hits
        assert_eq!(result.total(), 0);
    }

    #[test]
    fn test_bulk_lookup_all_hits() {
        let (_dir, cache) = create_test_cache();

        // Insert some entries
        cache.put("file1.rs", "hash1", &[0.1, 0.2]).unwrap();
        cache.put("file2.rs", "hash2", &[0.3, 0.4]).unwrap();

        let entries = vec![
            ("file1.rs".to_string(), "hash1".to_string()),
            ("file2.rs".to_string(), "hash2".to_string()),
        ];

        let result = cache.get_batch_bulk(&entries).unwrap();
        assert!(result.all_hits());
        assert_eq!(result.hits.len(), 2);
        assert_eq!(result.misses.len(), 0);
        assert!((result.hit_ratio() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_bulk_lookup_all_misses() {
        let (_dir, cache) = create_test_cache();

        let entries = vec![
            ("missing1.rs".to_string(), "hash1".to_string()),
            ("missing2.rs".to_string(), "hash2".to_string()),
        ];

        let result = cache.get_batch_bulk(&entries).unwrap();
        assert!(result.all_misses());
        assert_eq!(result.hits.len(), 0);
        assert_eq!(result.misses.len(), 2);
        assert!(result.hit_ratio() < 0.001);
    }

    #[test]
    fn test_bulk_lookup_mixed() {
        let (_dir, cache) = create_test_cache();

        // Insert only one entry
        cache.put("found.rs", "hash1", &[0.1, 0.2]).unwrap();

        let entries = vec![
            ("found.rs".to_string(), "hash1".to_string()),
            ("missing.rs".to_string(), "hash2".to_string()),
        ];

        let result = cache.get_batch_bulk(&entries).unwrap();
        assert!(!result.all_hits());
        assert!(!result.all_misses());
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.misses.len(), 1);
        assert!((result.hit_ratio() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_deduplicated_lookup() {
        let (_dir, cache) = create_test_cache();

        // Two files with same content hash
        let entries = vec![
            ("file_a.rs".to_string(), "same_hash".to_string()),
            ("file_b.rs".to_string(), "same_hash".to_string()),
            ("file_c.rs".to_string(), "different_hash".to_string()),
        ];

        let (hits, unique_hashes) = cache.get_batch_deduplicated(&entries).unwrap();
        assert!(hits.is_empty()); // Nothing cached
        assert_eq!(unique_hashes.len(), 2); // Only 2 unique hashes
        assert!(unique_hashes.contains(&"same_hash".to_string()));
        assert!(unique_hashes.contains(&"different_hash".to_string()));
    }

    #[test]
    fn test_hit_ratio() {
        let result = CacheLookupResult {
            hits: vec![
                ("a".to_string(), "h1".to_string(), vec![0.1]),
                ("b".to_string(), "h2".to_string(), vec![0.2]),
            ],
            misses: vec![("c".to_string(), "h3".to_string())],
        };
        // 2 hits, 1 miss = 2/3 ≈ 0.667
        assert!((result.hit_ratio() - 0.667).abs() < 0.01);
    }
}
