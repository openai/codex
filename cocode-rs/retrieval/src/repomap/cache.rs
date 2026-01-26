//! SQLite-based tag cache for repo map.
//!
//! Provides persistent tag cache: (filepath, mtime) -> Vec<CodeTag>

use std::sync::Arc;

use crate::error::Result;
use crate::storage::SqliteStore;
use crate::tags::extractor::CodeTag;
use crate::tags::extractor::TagKind;

/// SQLite-based tag cache for repo map operations.
pub struct RepoMapCache {
    /// SQLite store for persistent tag caching
    db: Arc<SqliteStore>,
}

impl RepoMapCache {
    /// Create a new tag cache.
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }

    /// Get cached tags for a file, validating mtime with optimistic locking.
    ///
    /// Uses double-check pattern to detect TOCTOU race conditions:
    /// 1. Check mtime before DB query
    /// 2. Query cached tags from DB
    /// 3. Check mtime again after query
    /// 4. Only return cache if both checks match
    ///
    /// Returns None if the file has been modified since caching.
    pub async fn get_tags(&self, filepath: &str) -> Result<Option<Vec<CodeTag>>> {
        // Step 1: Get mtime BEFORE database query (for optimistic lock check)
        let mtime_before = Self::get_file_mtime(filepath);

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
            // - Current file doesn't exist (mtime_before is None)
            // - No cached mtime record (cached_mtime is None)
            let skip_validation = mtime_before.is_none() || cached_mtime.is_none();

            // Special case: If cached mtime was 0 (file couldn't be read initially)
            // but now we can read it, force cache invalidation to get fresh tags
            if cached_mtime == Some(0) && mtime_before.is_some() {
                tracing::debug!(
                    filepath = fp_clone,
                    mtime_before = ?mtime_before,
                    "Cached mtime was 0, file now readable, invalidating cache"
                );
                self.invalidate_tags(&fp_clone).await?;
                return Ok(None);
            }

            // Step 3: Optimistic lock validation - check mtime AFTER query
            let mtime_after = Self::get_file_mtime(&fp_clone);

            // Validate with double-check pattern:
            // 1. cached_mtime must match mtime_before (cache is valid for the file state we saw)
            // 2. mtime_after must match mtime_before (file wasn't modified during query)
            let cache_valid = cached_mtime == mtime_before;
            let file_unchanged = mtime_after == mtime_before;

            if !skip_validation && (!cache_valid || !file_unchanged) {
                tracing::debug!(
                    filepath = fp_clone,
                    cached_mtime = ?cached_mtime,
                    mtime_before = ?mtime_before,
                    mtime_after = ?mtime_after,
                    cache_valid = cache_valid,
                    file_unchanged = file_unchanged,
                    "Cache validation failed (optimistic lock), invalidating"
                );
                self.invalidate_tags(&fp_clone).await?;
                return Ok(None);
            }

            Ok(Some(tags))
        } else {
            Ok(None)
        }
    }

    /// Get file mtime as unix timestamp in nanoseconds for high precision.
    ///
    /// Using nanosecond precision helps detect rapid file modifications
    /// that might occur within the same second.
    fn get_file_mtime(filepath: &str) -> Option<i64> {
        std::fs::metadata(filepath)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i64)
    }

    /// Get file mtime (public for callers to record before extraction).
    pub fn file_mtime(filepath: &str) -> Option<i64> {
        Self::get_file_mtime(filepath)
    }

    /// Store tags for a file with optimistic lock validation.
    ///
    /// Only writes if no newer version exists in DB (based on mtime comparison).
    ///
    /// # Arguments
    /// * `filepath` - Path to the file
    /// * `tags` - Extracted tags to cache
    /// * `expected_mtime` - The mtime recorded before extraction; if DB has newer, skip write
    ///
    /// # Returns
    /// * `Ok(true)` - Tags were written successfully
    /// * `Ok(false)` - Skipped due to newer version in DB (optimistic lock conflict)
    pub async fn put_tags(
        &self,
        filepath: &str,
        tags: &[CodeTag],
        expected_mtime: Option<i64>,
    ) -> Result<bool> {
        let fp = filepath.to_string();

        // Use expected_mtime if provided, otherwise get current mtime
        let file_mtime =
            expected_mtime.unwrap_or_else(|| Self::get_file_mtime(filepath).unwrap_or(0));

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
                // Optimistic lock: check if DB already has a newer version
                let existing_mtime: Option<i64> = conn
                    .query_row(
                        "SELECT mtime FROM repomap_tags WHERE filepath = ? LIMIT 1",
                        [&fp],
                        |row| row.get(0),
                    )
                    .ok();

                // If DB has a newer version (higher mtime), skip write
                if let Some(db_mtime) = existing_mtime {
                    if db_mtime > file_mtime {
                        tracing::debug!(
                            filepath = %fp,
                            db_mtime = db_mtime,
                            expected_mtime = file_mtime,
                            "Skipping put_tags: DB has newer version"
                        );
                        return Ok(false);
                    }
                }

                // Delete old entries and insert new ones
                conn.execute("DELETE FROM repomap_tags WHERE filepath = ?", [&fp])?;

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

                Ok(true)
            })
            .await
    }

    /// Invalidate cached tags for a file.
    ///
    /// Uses transaction for atomic deletion.
    pub async fn invalidate_tags(&self, filepath: &str) -> Result<()> {
        let fp = filepath.to_string();

        self.db
            .transaction(move |conn| {
                conn.execute("DELETE FROM repomap_tags WHERE filepath = ?", [&fp])?;
                Ok(())
            })
            .await
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
        let cache = RepoMapCache::new(store);
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

        // Store tags (None = no optimistic lock check)
        let tags = vec![make_tag("foo", 10, true), make_tag("bar", 20, false)];
        let written = cache.put_tags("test.rs", &tags, None).await.unwrap();
        assert!(written);

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
}
