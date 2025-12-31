//! 2-level cache for repo map.
//!
//! Level 1: SQLite - persistent tag cache (filepath, mtime) -> Vec<CodeTag>
//! Level 2: In-memory TTL - full map result cache

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::error::Result;
use crate::storage::SqliteStore;
use crate::tags::extractor::CodeTag;
use crate::tags::extractor::TagKind;

/// 2-level cache for repo map operations.
pub struct RepoMapCache {
    /// SQLite store for persistent tag caching
    db: Arc<SqliteStore>,
    /// In-memory TTL cache for full map results
    map_cache: HashMap<MapCacheKey, (MapCacheEntry, Instant)>,
    /// TTL for map cache entries in seconds
    cache_ttl_secs: i64,
}

/// Key for map cache (TTL level 2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MapCacheKey {
    request_hash: String,
}

/// Entry for map cache.
#[derive(Debug, Clone)]
struct MapCacheEntry {
    content: String,
    tokens: i32,
    files_included: i32,
}

impl RepoMapCache {
    /// Create a new 2-level cache.
    pub fn new(db: Arc<SqliteStore>, cache_ttl_secs: i64) -> Self {
        Self {
            db,
            map_cache: HashMap::new(),
            cache_ttl_secs,
        }
    }

    // ========== Level 1: SQLite Tag Cache ==========

    /// Get cached tags for a file, validating mtime.
    ///
    /// Returns None if the file has been modified since caching.
    pub async fn get_tags(&self, filepath: &str) -> Result<Option<Vec<CodeTag>>> {
        // Get current file mtime for validation
        let current_mtime = Self::get_file_mtime(filepath);

        let fp = filepath.to_string();
        let fp_clone = fp.clone();

        // Query both mtime and tags
        let result = self
            .db
            .query(move |conn| {
                // First check if we have cached tags and get the stored mtime
                let cached_mtime: Option<i64> = conn
                    .query_row(
                        "SELECT mtime FROM repomap_tags WHERE filepath = ? LIMIT 1",
                        [&fp],
                        |row| row.get(0),
                    )
                    .ok();

                let mut stmt = conn.prepare(
                    "SELECT name, is_definition, tag_kind, start_line, end_line,
                            start_byte, end_byte, signature, docs
                     FROM repomap_tags WHERE filepath = ? ORDER BY start_line",
                )?;

                let rows = stmt.query_map([&fp], |row| {
                    let name: String = row.get(0)?;
                    let is_def: i32 = row.get(1)?;
                    let tag_kind: String = row.get(2)?;
                    let start_line: i32 = row.get(3)?;
                    let end_line: i32 = row.get(4)?;
                    let start_byte: i32 = row.get(5)?;
                    let end_byte: i32 = row.get(6)?;
                    let signature: Option<String> = row.get(7)?;
                    let docs: Option<String> = row.get(8)?;

                    Ok((
                        name, is_def, tag_kind, start_line, end_line, start_byte, end_byte,
                        signature, docs,
                    ))
                })?;

                let mut tags = Vec::new();
                for row in rows {
                    let (
                        name,
                        is_def,
                        tag_kind,
                        start_line,
                        end_line,
                        start_byte,
                        end_byte,
                        signature,
                        docs,
                    ) = row?;

                    tags.push(CodeTag {
                        name,
                        kind: TagKind::from_syntax_type(&tag_kind),
                        start_line,
                        end_line,
                        start_byte,
                        end_byte,
                        signature,
                        docs,
                        is_definition: is_def == 1,
                    });
                }

                if tags.is_empty() {
                    Ok((None, None))
                } else {
                    Ok((Some(tags), cached_mtime))
                }
            })
            .await?;

        let (tags, cached_mtime) = result;

        // Validate mtime if we have cached tags
        if let Some(tags) = tags {
            // Skip mtime validation only if:
            // - Current file doesn't exist (current_mtime is None)
            // - No cached mtime record (cached_mtime is None)
            let skip_validation = current_mtime.is_none() || cached_mtime.is_none();

            // Special case: If cached mtime was 0 (file couldn't be read initially)
            // but now we can read it, force cache invalidation to get fresh tags
            if cached_mtime == Some(0) && current_mtime.is_some() {
                tracing::debug!(
                    filepath = fp_clone,
                    current_mtime = ?current_mtime,
                    "Cached mtime was 0, file now readable, invalidating cache"
                );
                self.invalidate_tags(&fp_clone).await?;
                return Ok(None);
            }

            if !skip_validation && cached_mtime != current_mtime {
                tracing::debug!(
                    filepath = fp_clone,
                    cached_mtime = ?cached_mtime,
                    current_mtime = ?current_mtime,
                    "File mtime changed, invalidating cache"
                );
                self.invalidate_tags(&fp_clone).await?;
                return Ok(None);
            }
            Ok(Some(tags))
        } else {
            Ok(None)
        }
    }

    /// Get file mtime as unix timestamp.
    fn get_file_mtime(filepath: &str) -> Option<i64> {
        std::fs::metadata(filepath)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
    }

    /// Store tags for a file.
    ///
    /// Stores the file's current mtime for later validation.
    pub async fn put_tags(&self, filepath: &str, tags: &[CodeTag]) -> Result<()> {
        let fp = filepath.to_string();

        // Get file mtime for cache validation (use 0 if file doesn't exist or can't get mtime)
        let file_mtime = Self::get_file_mtime(filepath).unwrap_or(0);

        // Clone all tag data for the async closure
        let tags_clone: Vec<_> = tags
            .iter()
            .map(|t| {
                (
                    t.name.clone(),
                    if t.is_definition { 1_i32 } else { 0_i32 },
                    t.kind.as_str().to_string(),
                    t.start_line,
                    t.end_line,
                    t.start_byte,
                    t.end_byte,
                    t.signature.clone(),
                    t.docs.clone(),
                )
            })
            .collect();

        self.db
            .transaction(move |conn| {
                // Delete old entries
                conn.execute("DELETE FROM repomap_tags WHERE filepath = ?", [&fp])?;

                // Insert new entries with file's actual mtime (not current time)
                let mut stmt = conn.prepare(
                    "INSERT INTO repomap_tags (workspace, filepath, mtime, name, is_definition,
                        tag_kind, start_line, end_line, start_byte, end_byte, signature, docs)
                     VALUES ('default', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )?;

                for (
                    name,
                    is_def,
                    tag_kind,
                    start_line,
                    end_line,
                    start_byte,
                    end_byte,
                    signature,
                    docs,
                ) in &tags_clone
                {
                    stmt.execute(rusqlite::params![
                        fp, file_mtime, name, is_def, tag_kind, start_line, end_line, start_byte,
                        end_byte, signature, docs
                    ])?;
                }

                Ok(())
            })
            .await
    }

    /// Invalidate cached tags for a file.
    pub async fn invalidate_tags(&self, filepath: &str) -> Result<()> {
        let fp = filepath.to_string();

        self.db
            .query(move |conn| {
                conn.execute("DELETE FROM repomap_tags WHERE filepath = ?", [&fp])?;
                Ok(())
            })
            .await
    }

    // ========== Level 2: In-Memory TTL Map Cache ==========

    /// Get cached map result by request hash.
    pub fn get_map(&mut self, request_hash: &str) -> Option<(String, i32, i32)> {
        self.cleanup_expired();

        let key = MapCacheKey {
            request_hash: request_hash.to_string(),
        };

        self.map_cache
            .get(&key)
            .map(|(entry, _)| (entry.content.clone(), entry.tokens, entry.files_included))
    }

    /// Store map result.
    pub fn put_map(
        &mut self,
        request_hash: &str,
        content: String,
        tokens: i32,
        files_included: i32,
    ) {
        let key = MapCacheKey {
            request_hash: request_hash.to_string(),
        };

        let entry = MapCacheEntry {
            content,
            tokens,
            files_included,
        };

        self.map_cache.insert(key, (entry, Instant::now()));
    }

    /// Invalidate all map cache entries.
    #[allow(dead_code)]
    pub fn invalidate_all_maps(&mut self) {
        self.map_cache.clear();
    }

    /// Clean up expired TTL entries.
    fn cleanup_expired(&mut self) {
        let ttl = std::time::Duration::from_secs(self.cache_ttl_secs as u64);
        let now = Instant::now();

        self.map_cache
            .retain(|_, (_, created_at)| now.duration_since(*created_at) < ttl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, RepoMapCache) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let cache = RepoMapCache::new(store, 3600);
        (dir, cache)
    }

    fn make_tag(name: &str, line: i32, is_def: bool) -> CodeTag {
        CodeTag {
            name: name.to_string(),
            kind: TagKind::Function,
            start_line: line,
            end_line: line + 10,
            start_byte: line * 100,
            end_byte: (line + 10) * 100,
            signature: Some(format!("fn {}()", name)),
            docs: None,
            is_definition: is_def,
        }
    }

    #[tokio::test]
    async fn test_tag_cache() {
        let (_dir, cache) = setup().await;

        // Initially empty
        let result = cache.get_tags("test.rs").await.unwrap();
        assert!(result.is_none());

        // Store tags
        let tags = vec![make_tag("foo", 10, true), make_tag("bar", 20, false)];
        cache.put_tags("test.rs", &tags).await.unwrap();

        // Retrieve tags
        let result = cache.get_tags("test.rs").await.unwrap();
        assert!(result.is_some());
        let cached_tags = result.unwrap();
        assert_eq!(cached_tags.len(), 2);

        // Verify all fields round-trip correctly for first tag
        assert_eq!(cached_tags[0].name, "foo");
        assert!(cached_tags[0].is_definition);
        assert_eq!(cached_tags[0].kind, TagKind::Function);
        assert_eq!(cached_tags[0].start_line, 10);
        assert_eq!(cached_tags[0].end_line, 20);
        assert_eq!(cached_tags[0].start_byte, 1000);
        assert_eq!(cached_tags[0].end_byte, 2000);
        assert_eq!(cached_tags[0].signature, Some("fn foo()".to_string()));

        // Verify second tag
        assert_eq!(cached_tags[1].name, "bar");
        assert!(!cached_tags[1].is_definition);
        assert_eq!(cached_tags[1].signature, Some("fn bar()".to_string()));

        // Invalidate
        cache.invalidate_tags("test.rs").await.unwrap();
        let result = cache.get_tags("test.rs").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_map_cache() {
        let (_dir, mut cache) = setup().await;

        // Initially empty
        let result = cache.get_map("hash123");
        assert!(result.is_none());

        // Store map
        cache.put_map("hash123", "map content".to_string(), 100, 5);

        // Retrieve map
        let result = cache.get_map("hash123");
        assert!(result.is_some());
        let (content, tokens, files) = result.unwrap();
        assert_eq!(content, "map content");
        assert_eq!(tokens, 100);
        assert_eq!(files, 5);

        // Invalidate all
        cache.invalidate_all_maps();
        let result = cache.get_map("hash123");
        assert!(result.is_none());
    }
}
