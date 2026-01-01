//! BM25 full-text search with tunable k1/b parameters.
//!
//! Uses a custom BM25 implementation via the `bm25` crate for code-optimized search.
//! Falls back to LanceDB FTS if the custom index is not available.

use std::collections::HashMap;
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
use crate::storage::LanceDbStore;
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
    /// LanceDB store for chunk retrieval
    store: Arc<LanceDbStore>,
    /// Custom BM25 index
    index: Arc<RwLock<Bm25Index>>,
    /// Chunk cache for fast retrieval
    chunk_cache: Arc<RwLock<HashMap<String, CodeChunk>>>,
    /// BM25 configuration for lazy loading
    config: Bm25Config,
    /// Whether the index has been loaded from storage
    loaded: AtomicBool,
    /// Number of failed load attempts (for exponential backoff)
    load_attempts: AtomicU32,
    /// Last load attempt time (for exponential backoff)
    last_load_attempt: Mutex<Option<Instant>>,
}

impl Bm25Searcher {
    /// Create a new BM25 searcher with default configuration.
    pub fn new(store: Arc<LanceDbStore>) -> Self {
        Self {
            store,
            index: Arc::new(RwLock::new(Bm25Index::new())),
            chunk_cache: Arc::new(RwLock::new(HashMap::new())),
            config: Bm25Config::default(),
            loaded: AtomicBool::new(false),
            load_attempts: AtomicU32::new(0),
            last_load_attempt: Mutex::new(None),
        }
    }

    /// Create a new BM25 searcher with custom configuration.
    pub fn with_config(store: Arc<LanceDbStore>, config: &SearchConfig) -> Self {
        Self {
            store,
            index: Arc::new(RwLock::new(Bm25Index::from_search_config(config))),
            chunk_cache: Arc::new(RwLock::new(HashMap::new())),
            config: Bm25Config::from_search_config(config),
            loaded: AtomicBool::new(false),
            load_attempts: AtomicU32::new(0),
            last_load_attempt: Mutex::new(None),
        }
    }

    /// Create a BM25 searcher with a pre-loaded index.
    pub fn with_index(
        store: Arc<LanceDbStore>,
        index: Arc<RwLock<Bm25Index>>,
        chunk_cache: Arc<RwLock<HashMap<String, CodeChunk>>>,
    ) -> Self {
        Self {
            store,
            index,
            chunk_cache,
            config: Bm25Config::default(),
            loaded: AtomicBool::new(true), // Already loaded
            load_attempts: AtomicU32::new(0),
            last_load_attempt: Mutex::new(None),
        }
    }

    /// Load the BM25 index from storage.
    ///
    /// Loads metadata, embeddings, contents, and populates the chunk cache from LanceDB.
    /// Contents are re-embedded to restore the scorer for search.
    ///
    /// **Note:** This can be memory-intensive for large indices since all chunks
    /// are loaded to rebuild the scorer and populate the cache.
    pub async fn load_from_storage(&self, config: &Bm25Config) -> Result<()> {
        let metadata = self.store.load_bm25_metadata().await?;
        let embeddings = self.store.load_all_bm25_embeddings().await?;
        let contents = self.store.load_all_chunk_contents().await?;

        let new_index =
            Bm25Index::load_with_contents(embeddings, contents.clone(), metadata, config.clone());

        // Populate chunk cache for search result hydration
        // This prevents search results from being skipped due to missing cache entries
        let chunks = self.store.list_all_chunks().await?;
        let mut cache = self.chunk_cache.write().await;
        for chunk in &chunks {
            cache.insert(chunk.id.clone(), chunk.clone());
        }

        let mut index = self.index.write().await;
        *index = new_index;

        self.loaded.store(true, Ordering::SeqCst);
        tracing::debug!(
            chunks = chunks.len(),
            cache_size = cache.len(),
            "BM25 index loaded from storage with chunk cache populated"
        );
        Ok(())
    }

    /// Ensure the index is loaded from storage (lazy loading with exponential backoff).
    ///
    /// Called automatically before search if the index hasn't been loaded yet.
    /// Uses exponential backoff to avoid hammering storage on repeated failures.
    /// After max retries (10), falls back to empty index.
    async fn ensure_loaded(&self) -> Result<()> {
        // Fast path: already loaded
        if self.loaded.load(Ordering::SeqCst) {
            return Ok(());
        }

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
                Search will use LanceDB FTS fallback which may have lower quality."
            );
            self.loaded.store(true, Ordering::SeqCst);
            return Ok(());
        }

        // Try to load from storage
        match self.load_from_storage(&self.config).await {
            Ok(()) => {
                // Reset retry state on success
                self.load_attempts.store(0, Ordering::SeqCst);
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
        // Ensure index is loaded from storage (lazy loading)
        self.ensure_loaded().await?;

        let index = self.index.read().await;
        let results = index.search(&query.text, query.limit);

        if results.is_empty() {
            // Fall back to LanceDB FTS if no results from custom index
            tracing::debug!(
                query = %query.text,
                "BM25 index returned no results, falling back to LanceDB FTS"
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
            }
            // Note: If chunk not in cache, skip it. The chunk cache should be
            // populated during indexing. Missing chunks indicate stale index.
        }

        Ok(search_results)
    }

    /// Fallback search using LanceDB FTS.
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
        let store = Arc::new(LanceDbStore::open(dir.path()).await.unwrap());
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
        let store = Arc::new(LanceDbStore::open(dir.path()).await.unwrap());
        let searcher = Bm25Searcher::new(store);

        assert_eq!(searcher.doc_count().await, 0);

        let chunk = make_test_chunk("1", "fn test() {}");
        searcher.index_chunk(&chunk).await;

        assert_eq!(searcher.doc_count().await, 1);
    }
}
