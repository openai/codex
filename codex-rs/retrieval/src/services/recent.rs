//! Recently accessed files service.
//!
//! Tracks recently accessed/edited files for temporal relevance in search.
//! Provides chunking support for integrating recent files into search results.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::chunking::CodeChunkerService;
use crate::config::ChunkingConfig;
use crate::context::RetrievalContext;
use crate::error::Result;
use crate::search::RecentFilesCache;
use crate::types::CodeChunk;
use crate::types::ScoreType;
use crate::types::SearchResult;

/// Default capacity for recent files cache.
const DEFAULT_RECENT_FILES_CAPACITY: usize = 50;

/// Service for tracking recently accessed files.
///
/// Provides temporal relevance signal for search results by boosting
/// recently accessed files. Content is read fresh from disk on demand
/// to avoid stale cached chunks.
pub struct RecentFilesService {
    /// Shared context (optional for backward compat).
    #[allow(dead_code)]
    ctx: Option<Arc<RetrievalContext>>,
    /// LRU cache for recently accessed file paths.
    cache: RwLock<RecentFilesCache>,
    /// Configuration for chunking recent files (fallback when no context).
    chunking_config: ChunkingConfig,
}

impl RecentFilesService {
    /// Create a new recent files service with context.
    ///
    /// This is the recommended constructor that uses shared context.
    pub fn new(ctx: Arc<RetrievalContext>) -> Self {
        Self {
            chunking_config: ctx.config().chunking.clone(),
            ctx: Some(ctx),
            cache: RwLock::new(RecentFilesCache::new(DEFAULT_RECENT_FILES_CAPACITY)),
        }
    }

    /// Create with explicit capacity and chunking config.
    ///
    /// Use `new(ctx)` instead when you have a RetrievalContext.
    pub fn with_config(capacity: usize, chunking_config: ChunkingConfig) -> Self {
        Self {
            ctx: None,
            cache: RwLock::new(RecentFilesCache::new(capacity)),
            chunking_config,
        }
    }

    /// Create with default capacity (backward compat).
    pub fn with_default_capacity(chunking_config: ChunkingConfig) -> Self {
        Self::with_config(DEFAULT_RECENT_FILES_CAPACITY, chunking_config)
    }

    // ========== File Tracking ==========

    /// Notify that a file has been accessed or edited.
    ///
    /// Updates the LRU cache for temporal relevance in search results.
    /// Only the path is stored; content is read fresh on demand.
    pub async fn notify_file_accessed(&self, path: impl AsRef<Path>) {
        self.cache.write().await.notify_file_accessed(path);
    }

    /// Remove a file from the cache.
    ///
    /// Call this when a file is closed or deleted.
    pub async fn remove_file(&self, path: impl AsRef<Path>) {
        self.cache.write().await.remove(path);
    }

    /// Clear all recent files from the cache.
    pub async fn clear(&self) {
        self.cache.write().await.clear();
    }

    // ========== Queries ==========

    /// Get paths of recently accessed files.
    ///
    /// Returns up to `limit` file paths, ordered by most recently accessed first.
    pub async fn get_recent_paths(&self, limit: usize) -> Vec<PathBuf> {
        self.cache.read().await.get_recent_paths(limit)
    }

    /// Check if a file is in the recent files cache.
    pub async fn is_recent_file(&self, path: impl AsRef<Path>) -> bool {
        self.cache.read().await.contains(path)
    }

    /// Get the number of files in the recent files cache.
    pub async fn count(&self) -> usize {
        self.cache.read().await.len()
    }

    // ========== Search Integration ==========

    /// Get SearchResults from recently accessed files for RRF fusion.
    ///
    /// Reads and chunks files on demand to ensure fresh content.
    /// Results are scored by recency rank (most recent = highest score).
    pub async fn get_recent_search_results(&self, limit: usize) -> Vec<SearchResult> {
        let paths = self.cache.read().await.get_recent_paths(limit);

        if paths.is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        for (rank, path) in paths.iter().enumerate() {
            match self.chunk_file(path).await {
                Ok(chunks) => {
                    for chunk in chunks {
                        results.push(SearchResult {
                            chunk,
                            // Score based on recency rank (most recent = highest)
                            score: 1.0 / (rank as f32 + 1.0),
                            score_type: ScoreType::Recent,
                            is_stale: None,
                        });
                    }
                }
                Err(e) => {
                    tracing::debug!(path = ?path, error = %e, "Failed to chunk recent file");
                }
            }
        }

        results
    }

    /// Get chunks from recently accessed files.
    ///
    /// Reads files from disk and chunks them on demand to ensure fresh content.
    pub async fn get_recent_chunks(&self, limit: usize) -> Vec<CodeChunk> {
        let paths = self.cache.read().await.get_recent_paths(limit);
        let mut all_chunks = Vec::new();

        for path in paths {
            match self.chunk_file(&path).await {
                Ok(chunks) => all_chunks.extend(chunks),
                Err(e) => {
                    tracing::debug!(path = ?path, error = %e, "Failed to chunk recent file");
                }
            }
        }

        all_chunks
    }

    // ========== Internal ==========

    /// Read and chunk a file.
    ///
    /// Returns empty vec if file is not readable or not a supported language.
    async fn chunk_file(&self, path: &Path) -> Result<Vec<CodeChunk>> {
        // Read file content
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(path = ?path, error = %e, "Failed to read file for chunking");
                return Ok(Vec::new());
            }
        };

        // Get file extension for language detection
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
            .to_string();

        // Get filepath string
        let filepath_str = path.to_string_lossy().to_string();

        // Chunking config
        let max_tokens = self.chunking_config.max_tokens as usize;
        let overlap_tokens = self.chunking_config.overlap_tokens as usize;

        // Clone extension for use in closure
        let ext_for_chunk = extension.clone();

        // Run CPU-intensive chunking in a blocking thread pool
        let spans = match tokio::task::spawn_blocking(move || {
            let chunker = CodeChunkerService::new(max_tokens, overlap_tokens);
            chunker.chunk(&content, &ext_for_chunk)
        })
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                tracing::debug!(path = ?path, error = %e, "Failed to chunk file");
                return Ok(Vec::new());
            }
            Err(e) => {
                tracing::warn!(path = ?path, error = %e, "Chunking task panicked");
                return Ok(Vec::new());
            }
        };

        // Convert spans to CodeChunks
        let workspace = "recent"; // Mark as from recent files
        let chunks: Vec<CodeChunk> = spans
            .into_iter()
            .enumerate()
            .map(|(i, span)| CodeChunk {
                id: format!("{}:{}:{}", workspace, filepath_str, i),
                source_id: workspace.to_string(),
                filepath: filepath_str.clone(),
                language: extension.to_string(),
                content: span.content,
                start_line: span.start_line,
                end_line: span.end_line,
                embedding: None,
                modified_time: None,
                workspace: workspace.to_string(),
                content_hash: String::new(),
                indexed_at: 0,
                parent_symbol: None,
                is_overview: span.is_overview,
            })
            .collect();

        Ok(chunks)
    }
}

impl Default for RecentFilesService {
    fn default() -> Self {
        Self::with_config(DEFAULT_RECENT_FILES_CAPACITY, ChunkingConfig::default())
    }
}

impl std::fmt::Debug for RecentFilesService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecentFilesService").finish()
    }
}

/// Create a shared RecentFilesService.
pub fn shared_recent_files_service(chunking_config: ChunkingConfig) -> Arc<RecentFilesService> {
    Arc::new(RecentFilesService::with_default_capacity(chunking_config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_recent_files_empty_on_creation() {
        let service = RecentFilesService::default();
        assert_eq!(service.count().await, 0);
    }

    #[tokio::test]
    async fn test_notify_file_accessed() {
        let service = RecentFilesService::default();
        let path = Path::new("src/main.rs");

        service.notify_file_accessed(path).await;

        assert!(service.is_recent_file(path).await);
        assert_eq!(service.count().await, 1);
    }

    #[tokio::test]
    async fn test_get_recent_paths() {
        let service = RecentFilesService::default();

        service.notify_file_accessed("a.rs").await;
        service.notify_file_accessed("b.rs").await;
        service.notify_file_accessed("c.rs").await;

        let paths = service.get_recent_paths(10).await;
        assert_eq!(paths.len(), 3);
        // Most recent first
        assert_eq!(paths[0], PathBuf::from("c.rs"));
        assert_eq!(paths[1], PathBuf::from("b.rs"));
        assert_eq!(paths[2], PathBuf::from("a.rs"));
    }

    #[tokio::test]
    async fn test_get_recent_chunks() {
        let dir = TempDir::new().unwrap();
        let service = RecentFilesService::default();

        // Create a temporary file
        let file_path = dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {\n    println!(\"hello\");\n}").unwrap();

        service.notify_file_accessed(&file_path).await;

        let chunks = service.get_recent_chunks(100).await;
        assert!(!chunks.is_empty());
        assert!(chunks[0].content.contains("fn main()"));
    }

    #[tokio::test]
    async fn test_remove_file() {
        let service = RecentFilesService::default();
        let path = Path::new("src/main.rs");

        service.notify_file_accessed(path).await;
        assert!(service.is_recent_file(path).await);

        service.remove_file(path).await;
        assert!(!service.is_recent_file(path).await);
    }

    #[tokio::test]
    async fn test_clear() {
        let service = RecentFilesService::default();

        service.notify_file_accessed("a.rs").await;
        service.notify_file_accessed("b.rs").await;
        assert_eq!(service.count().await, 2);

        service.clear().await;
        assert_eq!(service.count().await, 0);
    }

    #[tokio::test]
    async fn test_get_recent_chunks_nonexistent_file() {
        let service = RecentFilesService::default();

        // Notify with non-existent file
        let path = Path::new("/nonexistent/file.rs");
        service.notify_file_accessed(path).await;

        // File is tracked
        assert!(service.is_recent_file(path).await);

        // But get_recent_chunks returns empty (file doesn't exist)
        let chunks = service.get_recent_chunks(100).await;
        assert!(chunks.is_empty());
    }

    #[tokio::test]
    async fn test_get_recent_search_results() {
        let dir = TempDir::new().unwrap();
        let service = RecentFilesService::default();

        // Create test files
        let file1 = dir.path().join("test1.rs");
        let file2 = dir.path().join("test2.rs");
        std::fs::write(&file1, "fn foo() {}").unwrap();
        std::fs::write(&file2, "fn bar() {}").unwrap();

        service.notify_file_accessed(&file1).await;
        service.notify_file_accessed(&file2).await;

        let results = service.get_recent_search_results(10).await;
        assert!(!results.is_empty());

        // Check score type
        for result in &results {
            assert_eq!(result.score_type, ScoreType::Recent);
        }
    }
}
