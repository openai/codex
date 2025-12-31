//! Change detection for tweakcc indexing.
//!
//! Uses content hash (SHA256) to detect file modifications.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use sha2::Digest;
use sha2::Sha256;

use crate::error::Result;
use crate::storage::SqliteStore;

/// File change status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeStatus {
    /// File is new (not in catalog)
    Added,
    /// File content has changed
    Modified,
    /// File has been deleted
    Deleted,
    /// File is unchanged
    Unchanged,
}

/// Detected file change.
#[derive(Debug, Clone)]
pub struct FileChange {
    /// File path (relative to workspace)
    pub filepath: String,
    /// Change status
    pub status: ChangeStatus,
    /// Current content hash (None for deleted files)
    pub content_hash: Option<String>,
    /// Previous content hash (None for new files)
    pub previous_hash: Option<String>,
}

/// Catalog entry for a file.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    /// File path
    pub filepath: String,
    /// Content hash
    pub content_hash: String,
    /// Last modification time (unix timestamp)
    pub mtime: i64,
    /// When the file was indexed
    pub indexed_at: i64,
    /// Number of chunks
    pub chunks_count: i32,
    /// Number of failed chunks
    pub chunks_failed: i32,
}

/// Change detector for tweakcc indexing.
pub struct ChangeDetector {
    db: Arc<SqliteStore>,
}

impl ChangeDetector {
    /// Create a new change detector.
    pub fn new(db: Arc<SqliteStore>) -> Self {
        Self { db }
    }

    /// Compute content hash (SHA256, first 16 hex chars).
    pub fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = hasher.finalize();
        // Use first 16 hex chars (64 bits) for efficiency
        hex::encode(&result[..8])
    }

    /// Compute hash from file content bytes.
    pub fn compute_hash_bytes(content: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }

    /// Detect changes between current files and catalog.
    ///
    /// # Arguments
    /// * `workspace` - Workspace identifier
    /// * `current_files` - Map of filepath -> content hash
    ///
    /// # Returns
    /// Vector of file changes
    pub async fn detect_changes(
        &self,
        workspace: &str,
        current_files: &HashMap<String, String>,
    ) -> Result<Vec<FileChange>> {
        // Get catalog entries
        let catalog = self.get_catalog(workspace).await?;
        let catalog_map: HashMap<_, _> = catalog
            .into_iter()
            .map(|e| (e.filepath.clone(), e))
            .collect();

        let mut changes = Vec::new();

        // Check current files for added/modified
        for (filepath, hash) in current_files {
            match catalog_map.get(filepath) {
                Some(entry) => {
                    if &entry.content_hash != hash {
                        changes.push(FileChange {
                            filepath: filepath.clone(),
                            status: ChangeStatus::Modified,
                            content_hash: Some(hash.clone()),
                            previous_hash: Some(entry.content_hash.clone()),
                        });
                    }
                    // Unchanged files are not included
                }
                None => {
                    changes.push(FileChange {
                        filepath: filepath.clone(),
                        status: ChangeStatus::Added,
                        content_hash: Some(hash.clone()),
                        previous_hash: None,
                    });
                }
            }
        }

        // Check for deleted files
        let current_paths: HashSet<_> = current_files.keys().collect();
        for (filepath, entry) in &catalog_map {
            if !current_paths.contains(filepath) {
                changes.push(FileChange {
                    filepath: filepath.clone(),
                    status: ChangeStatus::Deleted,
                    content_hash: None,
                    previous_hash: Some(entry.content_hash.clone()),
                });
            }
        }

        Ok(changes)
    }

    /// Get catalog entries for a workspace.
    pub async fn get_catalog(&self, workspace: &str) -> Result<Vec<CatalogEntry>> {
        let ws = workspace.to_string();

        self.db
            .query(move |conn| {
                let mut stmt = conn.prepare(
                    "SELECT filepath, content_hash, mtime, indexed_at, chunks_count, chunks_failed
                     FROM catalog
                     WHERE workspace = ?",
                )?;

                let rows = stmt.query_map([&ws], |row| {
                    Ok(CatalogEntry {
                        filepath: row.get(0)?,
                        content_hash: row.get(1)?,
                        mtime: row.get(2)?,
                        indexed_at: row.get(3)?,
                        chunks_count: row.get(4)?,
                        chunks_failed: row.get(5)?,
                    })
                })?;

                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                Ok(results)
            })
            .await
    }

    /// Update catalog entry after indexing a file.
    pub async fn update_catalog(
        &self,
        workspace: &str,
        filepath: &str,
        content_hash: &str,
        mtime: i64,
        chunks_count: i32,
        chunks_failed: i32,
    ) -> Result<()> {
        let ws = workspace.to_string();
        let fp = filepath.to_string();
        let hash = content_hash.to_string();
        let now = chrono::Utc::now().timestamp();

        self.db
            .query(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO catalog (workspace, filepath, content_hash, mtime, indexed_at, chunks_count, chunks_failed)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    rusqlite::params![ws, fp, hash, mtime, now, chunks_count, chunks_failed],
                )?;
                Ok(())
            })
            .await
    }

    /// Remove catalog entry for a deleted file.
    pub async fn remove_from_catalog(&self, workspace: &str, filepath: &str) -> Result<()> {
        let ws = workspace.to_string();
        let fp = filepath.to_string();

        self.db
            .query(move |conn| {
                conn.execute(
                    "DELETE FROM catalog WHERE workspace = ? AND filepath = ?",
                    rusqlite::params![ws, fp],
                )?;
                Ok(())
            })
            .await
    }

    /// Get total chunk count for a workspace.
    ///
    /// Used for chunk limit checking before indexing.
    pub async fn get_total_chunks(&self, workspace: &str) -> Result<i64> {
        let ws = workspace.to_string();

        self.db
            .query(move |conn| {
                let count: i64 = conn
                    .query_row(
                        "SELECT COALESCE(SUM(chunks_count), 0) FROM catalog WHERE workspace = ?",
                        rusqlite::params![ws],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                Ok(count)
            })
            .await
    }

    /// Check if a file needs reindexing based on mtime.
    ///
    /// This is a fast check before computing the full hash.
    pub async fn needs_reindex(
        &self,
        workspace: &str,
        filepath: &str,
        current_mtime: i64,
    ) -> Result<bool> {
        let ws = workspace.to_string();
        let fp = filepath.to_string();

        self.db
            .query(move |conn| {
                let result: Option<i64> = conn
                    .query_row(
                        "SELECT mtime FROM catalog WHERE workspace = ? AND filepath = ?",
                        rusqlite::params![ws, fp],
                        |row| row.get(0),
                    )
                    .ok();

                match result {
                    Some(stored_mtime) => Ok(current_mtime > stored_mtime),
                    None => Ok(true), // Not in catalog, needs indexing
                }
            })
            .await
    }
}

/// Compute file hash from path.
pub fn hash_file(path: &Path) -> Result<String> {
    let content = std::fs::read(path).map_err(|e| crate::error::RetrievalErr::FileReadFailed {
        path: path.to_path_buf(),
        cause: e.to_string(),
    })?;
    Ok(ChangeDetector::compute_hash_bytes(&content))
}

/// Get file modification time.
pub fn get_mtime(path: &Path) -> Result<i64> {
    let metadata =
        std::fs::metadata(path).map_err(|e| crate::error::RetrievalErr::FileReadFailed {
            path: path.to_path_buf(),
            cause: e.to_string(),
        })?;

    let mtime = metadata
        .modified()
        .map_err(|e| crate::error::RetrievalErr::FileReadFailed {
            path: path.to_path_buf(),
            cause: e.to_string(),
        })?;

    Ok(mtime
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let hash1 = ChangeDetector::compute_hash("hello world");
        let hash2 = ChangeDetector::compute_hash("hello world");
        let hash3 = ChangeDetector::compute_hash("hello world!");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_compute_hash_bytes() {
        let hash = ChangeDetector::compute_hash_bytes(b"test content");
        assert_eq!(hash.len(), 16);
    }

    #[tokio::test]
    async fn test_detect_changes() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Arc::new(SqliteStore::open(&db_path).unwrap());
        let detector = ChangeDetector::new(store.clone());

        // Initially empty catalog
        let current = HashMap::from([
            ("file1.rs".to_string(), "hash1".to_string()),
            ("file2.rs".to_string(), "hash2".to_string()),
        ]);

        let changes = detector.detect_changes("ws", &current).await.unwrap();
        assert_eq!(changes.len(), 2);
        assert!(changes.iter().all(|c| c.status == ChangeStatus::Added));

        // Add to catalog
        detector
            .update_catalog("ws", "file1.rs", "hash1", 1000, 5, 0)
            .await
            .unwrap();
        detector
            .update_catalog("ws", "file2.rs", "hash2", 1000, 3, 0)
            .await
            .unwrap();

        // No changes now
        let changes = detector.detect_changes("ws", &current).await.unwrap();
        assert_eq!(changes.len(), 0);

        // Modify file1
        let current_modified = HashMap::from([
            ("file1.rs".to_string(), "hash1_new".to_string()),
            ("file2.rs".to_string(), "hash2".to_string()),
        ]);

        let changes = detector
            .detect_changes("ws", &current_modified)
            .await
            .unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].filepath, "file1.rs");
        assert_eq!(changes[0].status, ChangeStatus::Modified);

        // Delete file2 (file1 unchanged from catalog's hash1)
        let current_deleted = HashMap::from([("file1.rs".to_string(), "hash1".to_string())]);

        let changes = detector
            .detect_changes("ws", &current_deleted)
            .await
            .unwrap();
        // file1: catalog has hash1, current has hash1 -> unchanged (not in changes)
        // file2: in catalog but not in current -> deleted
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].filepath, "file2.rs");
        assert_eq!(changes[0].status, ChangeStatus::Deleted);
    }
}
