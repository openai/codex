//! BM25 full-text search with tunable k1/b parameters.
//!
//! Uses a custom BM25 implementation via the `bm25` crate for code-optimized search.
//! Falls back to FTS5 if the custom index is not available.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use tokio::sync::Mutex;
use tokio::sync::RwLock;

use super::bm25_index::Bm25Config;
use super::bm25_index::Bm25Index;
use super::bm25_index::Bm25Metadata;
use super::bm25_index::SparseEmbedding;
use crate::config::SearchConfig;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::storage::VectorStore;
use crate::types::CodeChunk;
use crate::types::ScoreType;
use crate::types::SearchQuery;
use crate::types::SearchResult;

/// BM25 searcher with tunable parameters.
///
/// Uses a custom BM25 implementation for better code search quality.
/// Parameters k1 and b are optimized for code:
/// - k1 = 0.8 (lower than default 1.2, reduces repeated keyword weight)
/// - b = 0.5 (lower than default 0.75, less length normalization)
///
/// Supports lazy loading from storage on first search with exponential backoff retry.
pub struct Bm25Searcher {
    /// Vector store for chunk retrieval
    store: Arc<dyn VectorStore>,
    /// Custom BM25 index
    index: Arc<RwLock<Bm25Index>>,
    /// Chunk cache for fast retrieval
    chunk_cache: Arc<RwLock<HashMap<String, CodeChunk>>>,
    /// BM25 configuration for lazy loading
    config: Bm25Config,
    /// Workspace root for reading content from files during index loading.
    /// Uses std Mutex for interior mutability (set after Arc construction).
    workspace_root: std::sync::Mutex<Option<PathBuf>>,
    /// Whether the index has been loaded from storage
    loaded: AtomicBool,
    /// Whether loading is currently in progress (prevents double-load race)
    loading: AtomicBool,
    /// Number of failed load attempts (for exponential backoff)
    load_attempts: AtomicU32,
    /// Last load attempt time (for exponential backoff)
    last_load_attempt: Mutex<Option<Instant>>,
}

impl Bm25Searcher {
    /// Create a new BM25 searcher with default configuration.
    pub fn new(store: Arc<dyn VectorStore>) -> Self {
        Self {
            store,
            index: Arc::new(RwLock::new(Bm25Index::new())),
            chunk_cache: Arc::new(RwLock::new(HashMap::new())),
            config: Bm25Config::default(),
            workspace_root: std::sync::Mutex::new(None),
            loaded: AtomicBool::new(false),
            loading: AtomicBool::new(false),
            load_attempts: AtomicU32::new(0),
            last_load_attempt: Mutex::new(None),
        }
    }

    /// Create a new BM25 searcher with custom configuration.
    pub fn with_config(store: Arc<dyn VectorStore>, config: &SearchConfig) -> Self {
        Self {
            store,
            index: Arc::new(RwLock::new(Bm25Index::from_search_config(config))),
            chunk_cache: Arc::new(RwLock::new(HashMap::new())),
            config: Bm25Config::from_search_config(config),
            workspace_root: std::sync::Mutex::new(None),
            loaded: AtomicBool::new(false),
            loading: AtomicBool::new(false),
            load_attempts: AtomicU32::new(0),
            last_load_attempt: Mutex::new(None),
        }
    }

    /// Create a BM25 searcher with a pre-loaded index.
    pub fn with_index(
        store: Arc<dyn VectorStore>,
        index: Arc<RwLock<Bm25Index>>,
        chunk_cache: Arc<RwLock<HashMap<String, CodeChunk>>>,
    ) -> Self {
        Self {
            store,
            index,
            chunk_cache,
            config: Bm25Config::default(),
            workspace_root: std::sync::Mutex::new(None),
            loaded: AtomicBool::new(true), // Already loaded
            loading: AtomicBool::new(false),
            load_attempts: AtomicU32::new(0),
            last_load_attempt: Mutex::new(None),
        }
    }

    /// Set the workspace root for reading content from files during index loading.
    ///
    /// Can be called after Arc construction (uses interior mutability).
    pub fn set_workspace_root(&self, root: impl Into<PathBuf>) {
        if let Ok(mut guard) = self.workspace_root.lock() {
            *guard = Some(root.into());
        }
    }

    /// Load the BM25 index from storage.
    ///
    /// Loads metadata and BM25 embeddings from the database, then reads fresh
    /// content from the file system to restore the scorer and populate the cache.
    ///
    /// Requires `workspace_root` to be set for reading file content.
    /// Chunks whose files are unreadable are silently skipped.
    pub async fn load_from_storage(&self, config: &Bm25Config) -> Result<()> {
        let metadata = self.store.load_bm25_metadata().await?;
        let embeddings = self.store.load_all_bm25_embeddings().await?;
        let chunk_refs = self.store.load_all_chunk_refs().await?;

        // Read content from files for each chunk
        let mut contents = HashMap::new();
        let workspace_root = self.workspace_root.lock().ok().and_then(|g| g.clone());
        if let Some(ref workspace_root) = workspace_root {
            for (id, chunk_ref) in &chunk_refs {
                match chunk_ref.read_content(workspace_root) {
                    Ok(hydrated) => {
                        contents.insert(id.clone(), hydrated.content);
                    }
                    Err(e) => {
                        tracing::warn!(
                            id = %id,
                            filepath = %chunk_ref.filepath,
                            error = %e,
                            "Skipping chunk during BM25 load: file unreadable"
                        );
                    }
                }
            }
        } else {
            tracing::warn!(
                "BM25 load_from_storage called without workspace_root; \
                 index will be empty (no content to re-embed)"
            );
        }

        let new_index =
            Bm25Index::load_with_contents(embeddings, contents.clone(), metadata, config.clone());

        // Populate chunk cache with content read from files
        let mut cache = self.chunk_cache.write().await;
        for (id, chunk_ref) in &chunk_refs {
            if let Some(content) = contents.get(id) {
                cache.insert(id.clone(), chunk_ref.to_code_chunk_with_content(content));
            }
        }

        let mut index = self.index.write().await;
        *index = new_index;

        self.loaded.store(true, Ordering::SeqCst);
        tracing::debug!(
            chunk_refs = chunk_refs.len(),
            cache_size = cache.len(),
            "BM25 index loaded from storage with content from files"
        );
        Ok(())
    }

    /// Ensure the index is loaded from storage (lazy loading with exponential backoff).
    ///
    /// Called automatically before search if the index hasn't been loaded yet.
    /// Uses atomic CAS to prevent double-load race condition where multiple
    /// concurrent searchers could all load from storage simultaneously.
    /// Uses exponential backoff to avoid hammering storage on repeated failures.
    /// After max retries (10), falls back to empty index.
    async fn ensure_loaded(&self) -> Result<()> {
        // Fast path: already loaded
        if self.loaded.load(Ordering::Acquire) {
            return Ok(());
        }

        // Try to claim loading responsibility using atomic CAS
        // This prevents multiple concurrent searchers from all loading simultaneously
        if self
            .loading
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // We claimed loading responsibility
            let result = self.do_load().await;

            // Release loading flag regardless of outcome
            self.loading.store(false, Ordering::Release);

            result
        } else {
            // Another task is loading, wait for completion
            let mut wait_count = 0;
            const MAX_WAIT_ITERATIONS: u32 = 1000; // 10 seconds max

            while self.loading.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(10)).await;
                wait_count += 1;

                if wait_count >= MAX_WAIT_ITERATIONS {
                    return Err(RetrievalErr::NotReady {
                        workspace: "bm25".to_string(),
                        reason: "Timeout waiting for BM25 index load".to_string(),
                    });
                }
            }

            // Check if loading succeeded
            if self.loaded.load(Ordering::Acquire) {
                Ok(())
            } else {
                Err(RetrievalErr::NotReady {
                    workspace: "bm25".to_string(),
                    reason: "BM25 index load failed by another task".to_string(),
                })
            }
        }
    }

    /// Internal loading logic with retry and backoff.
    async fn do_load(&self) -> Result<()> {
        let attempts = self.load_attempts.load(Ordering::SeqCst);
        const MAX_RETRIES: u32 = 10;

        // Check exponential backoff timing
        if attempts > 0 && attempts < MAX_RETRIES {
            let last = self.last_load_attempt.lock().await;
            if let Some(last_time) = *last {
                // Backoff: 100ms, 200ms, 400ms, ... up to ~102 seconds
                let backoff = Duration::from_millis(100 * 2u64.pow(attempts.min(10)));
                if last_time.elapsed() < backoff {
                    return Err(RetrievalErr::NotReady {
                        workspace: "bm25".to_string(),
                        reason: format!(
                            "BM25 index loading, retry after {:?}",
                            backoff.saturating_sub(last_time.elapsed())
                        ),
                    });
                }
            }
        }

        // Max retries reached - fall back to empty index
        if attempts >= MAX_RETRIES {
            tracing::warn!(
                attempts = attempts,
                "Max BM25 load retries reached, using empty index. \
                Text search will be unavailable."
            );
            self.loaded.store(true, Ordering::Release);
            return Ok(());
        }

        // Try to load from storage
        match self.load_from_storage(&self.config).await {
            Ok(()) => {
                // Reset retry state on success
                self.load_attempts.store(0, Ordering::SeqCst);
                self.loaded.store(true, Ordering::Release);
                tracing::debug!("BM25 index loaded from storage");
                Ok(())
            }
            Err(e) => {
                // Increment retry counter and record time
                let new_attempts = self.load_attempts.fetch_add(1, Ordering::SeqCst) + 1;
                *self.last_load_attempt.lock().await = Some(Instant::now());

                tracing::warn!(
                    error = %e,
                    attempt = new_attempts,
                    max_retries = MAX_RETRIES,
                    "Failed to load BM25 index, will retry with backoff"
                );

                Err(RetrievalErr::NotReady {
                    workspace: "bm25".to_string(),
                    reason: format!(
                        "BM25 index load failed (attempt {new_attempts}/{MAX_RETRIES}): {e}"
                    ),
                })
            }
        }
    }

    /// Save the BM25 index to storage.
    pub async fn save_to_storage(&self) -> Result<()> {
        let index = self.index.read().await;
        let metadata = index.metadata();
        self.store.save_bm25_metadata(&metadata).await?;
        Ok(())
    }

    /// Pre-load the BM25 index from storage to avoid first-search latency spike.
    ///
    /// This is optional - the index will be loaded lazily on first search if
    /// warmup is not called. However, calling warmup during service initialization
    /// can improve the user experience by eliminating the cold-start delay.
    pub async fn warmup(&self) -> Result<()> {
        self.ensure_loaded().await
    }

    /// Get a reference to the index.
    pub fn index(&self) -> &Arc<RwLock<Bm25Index>> {
        &self.index
    }

    /// Get a reference to the chunk cache.
    pub fn chunk_cache(&self) -> &Arc<RwLock<HashMap<String, CodeChunk>>> {
        &self.chunk_cache
    }

    /// Index a chunk.
    pub async fn index_chunk(&self, chunk: &CodeChunk) -> SparseEmbedding {
        // Mark as loaded since we're building the index
        self.loaded.store(true, Ordering::SeqCst);

        let mut index = self.index.write().await;
        let embedding = index.upsert_chunk(chunk);

        // Update cache
        let mut cache = self.chunk_cache.write().await;
        cache.insert(chunk.id.clone(), chunk.clone());

        embedding
    }

    /// Index multiple chunks.
    pub async fn index_chunks(&self, chunks: &[CodeChunk]) -> Vec<SparseEmbedding> {
        // Mark as loaded since we're building the index
        self.loaded.store(true, Ordering::SeqCst);

        let mut index = self.index.write().await;
        let embeddings = index.upsert_chunks(chunks);

        // Update cache
        let mut cache = self.chunk_cache.write().await;
        for chunk in chunks {
            cache.insert(chunk.id.clone(), chunk.clone());
        }

        embeddings
    }

    /// Remove a chunk from the index.
    pub async fn remove_chunk(&self, chunk_id: &str) {
        let mut index = self.index.write().await;
        index.remove_chunk(chunk_id);

        let mut cache = self.chunk_cache.write().await;
        cache.remove(chunk_id);
    }

    /// Remove all chunks for a given filepath from the index.
    ///
    /// This is used when a file is deleted to clean up the BM25 index.
    pub async fn remove_chunks_by_filepath(&self, filepath: &str) {
        let mut index = self.index.write().await;
        let mut cache = self.chunk_cache.write().await;

        // Find all chunk IDs with matching filepath
        let ids_to_remove: Vec<String> = cache
            .iter()
            .filter(|(_, chunk)| chunk.filepath == filepath)
            .map(|(id, _)| id.clone())
            .collect();

        // Remove from both index and cache
        for id in &ids_to_remove {
            index.remove_chunk(id);
            cache.remove(id);
        }
    }

    /// Recalculate avgdl if needed.
    pub async fn recalculate_avgdl_if_needed(&self, previous_count: i64) {
        let mut index = self.index.write().await;
        if index.needs_avgdl_update(previous_count) {
            index.recalculate_avgdl();
        }
    }

    /// Search for code chunks matching the query.
    ///
    /// Uses the custom BM25 index for scoring, then retrieves full chunks
    /// from cache or storage.
    ///
    /// On first call, lazily loads the index from storage if available.
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        tracing::trace!(
            query = %query.text,
            limit = query.limit,
            "BM25 search started"
        );

        // Ensure index is loaded from storage (lazy loading)
        self.ensure_loaded().await?;

        let index = self.index.read().await;
        let results = index.search(&query.text, query.limit);
        tracing::trace!(raw_results = results.len(), "BM25 index search completed");

        if results.is_empty() {
            // Fall back to FTS5 if no results from custom index
            tracing::debug!(
                query = %query.text,
                "BM25 index returned no results, falling back to FTS5"
            );
            return self.search_fallback(query).await;
        }

        let cache = self.chunk_cache.read().await;
        let mut search_results = Vec::with_capacity(results.len());

        for (chunk_id, score) in results {
            if let Some(chunk) = cache.get(&chunk_id) {
                search_results.push(SearchResult {
                    chunk: chunk.clone(),
                    score,
                    score_type: ScoreType::Bm25,
                    is_stale: None,
                });
            } else {
                // Log warning for stale index detection - chunk in BM25 but not in cache
                tracing::warn!(
                    chunk_id = %chunk_id,
                    score = score,
                    "Chunk in BM25 index but missing from cache - stale index detected"
                );
            }
        }

        Ok(search_results)
    }

    /// Fallback search using FTS5.
    async fn search_fallback(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let chunks = self.store.search_fts(&query.text, query.limit).await?;

        Ok(chunks
            .into_iter()
            .enumerate()
            .map(|(i, chunk)| SearchResult {
                chunk,
                score: 1.0 / (1.0 + i as f32), // Simple ranking for fallback
                score_type: ScoreType::Bm25,
                is_stale: None,
            })
            .collect())
    }

    /// Get the current document count in the index.
    pub async fn doc_count(&self) -> i64 {
        let index = self.index.read().await;
        index.doc_count()
    }

    /// Get metadata from the index.
    pub async fn metadata(&self) -> Bm25Metadata {
        let index = self.index.read().await;
        index.metadata()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_chunk(id: &str, content: &str) -> CodeChunk {
        CodeChunk {
            id: id.to_string(),
            source_id: "test".to_string(),
            filepath: "test.rs".to_string(),
            language: "rust".to_string(),
            content: content.to_string(),
            start_line: 1,
            end_line: 3,
            embedding: None,
            modified_time: None,
            workspace: "test".to_string(),
            content_hash: String::new(),
            indexed_at: 0,
            parent_symbol: None,
            is_overview: false,
        }
    }

    #[tokio::test]
    async fn test_index_and_search() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let store = Arc::new(crate::storage::SqliteVecStore::open(dir.path()).unwrap());
        let searcher = Bm25Searcher::new(store);

        // Index some chunks
        let chunk1 = make_test_chunk("1", "fn get_user_by_id(id: i32) -> User");
        let chunk2 = make_test_chunk("2", "fn delete_user(id: i32) -> Result<()>");
        let chunk3 = make_test_chunk("3", "struct DatabaseConnection { pool: Pool }");

        searcher.index_chunk(&chunk1).await;
        searcher.index_chunk(&chunk2).await;
        searcher.index_chunk(&chunk3).await;

        // Search
        let query = SearchQuery {
            text: "get user".to_string(),
            limit: 10,
            ..Default::default()
        };

        let results = searcher.search(&query).await.unwrap();

        // Should find results
        assert!(!results.is_empty());
        // First result should be chunk1
        assert_eq!(results[0].chunk.id, "1");
    }

    #[tokio::test]
    async fn test_doc_count() {
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let store = Arc::new(crate::storage::SqliteVecStore::open(dir.path()).unwrap());
        let searcher = Bm25Searcher::new(store);

        assert_eq!(searcher.doc_count().await, 0);

        let chunk = make_test_chunk("1", "fn test() {}");
        searcher.index_chunk(&chunk).await;

        assert_eq!(searcher.doc_count().await, 1);
    }
}
