//! Core traits for the retrieval system.

use async_trait::async_trait;

use crate::error::Result;
use crate::types::CodeChunk;
use crate::types::IndexedFile;
use crate::types::SearchQuery;
use crate::types::SearchResult;

/// Trait for code indexing implementations.
#[async_trait]
pub trait Indexer: Send + Sync {
    /// Index a single file.
    async fn index_file(&self, path: &std::path::Path, content: &str) -> Result<IndexedFile>;

    /// Delete indexed data for a file.
    async fn delete_file(&self, path: &std::path::Path) -> Result<()>;

    /// Check if a file needs re-indexing.
    async fn needs_reindex(&self, path: &std::path::Path, content_hash: &str) -> Result<bool>;
}

/// Trait for code search implementations.
#[async_trait]
pub trait Searcher: Send + Sync {
    /// Search for code chunks matching the query.
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;

    /// Search using BM25 full-text search only.
    async fn search_bm25(&self, query: &str, limit: i32) -> Result<Vec<SearchResult>>;

    /// Search using vector similarity only.
    async fn search_vector(&self, embedding: &[f32], limit: i32) -> Result<Vec<SearchResult>>;
}

/// Trait for embedding providers.
///
/// Implementations must be Send + Sync for use with async runtime.
/// Provider implementations should be registered with EmbeddingRegistry
/// for runtime lookup by name.
#[async_trait]
pub trait EmbeddingProvider: Send + Sync + std::fmt::Debug {
    /// Provider name for registry lookup and logging.
    fn name(&self) -> &str;

    /// Get the embedding dimension.
    fn dimension(&self) -> i32;

    /// Embed a single text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Embed multiple texts in a batch.
    ///
    /// Default implementation calls embed() sequentially.
    /// Providers should override for efficient batching.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

/// Trait for code chunk storage.
#[async_trait]
pub trait ChunkStore: Send + Sync {
    /// Store a code chunk.
    async fn store(&self, chunk: &CodeChunk) -> Result<()>;

    /// Store multiple chunks.
    async fn store_batch(&self, chunks: &[CodeChunk]) -> Result<()>;

    /// Get a chunk by ID.
    async fn get(&self, id: &str) -> Result<Option<CodeChunk>>;

    /// Delete chunks by file path.
    async fn delete_by_path(&self, filepath: &str) -> Result<i32>;

    /// Count total chunks.
    async fn count(&self) -> Result<i64>;
}
