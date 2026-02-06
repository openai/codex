//! Vector store abstraction.
//!
//! Defines the `VectorStore` trait for backend-agnostic vector storage and search.
//!
//! Content is NOT stored in the database — only metadata (filepath, line range, hash).
//! Actual code content is read from the file system on demand.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::Result;
use crate::search::Bm25Metadata;
use crate::search::SparseEmbedding;
use crate::storage::chunk_types::FileMetadata;
use crate::storage::chunk_types::IndexPolicy;
use crate::storage::chunk_types::IndexStatus;
use crate::types::ChunkRef;
use crate::types::CodeChunk;

/// Backend-agnostic vector storage and search.
///
/// Implementations provide chunk storage, vector KNN search, full-text search,
/// CRUD operations, index management, and BM25 metadata persistence.
#[async_trait]
pub trait VectorStore: Send + Sync {
    // ========== Chunk Storage ==========

    /// Store a batch of code chunks (without BM25 embeddings).
    async fn store_chunks(&self, chunks: &[CodeChunk]) -> Result<()>;

    /// Store a batch of code chunks with optional BM25 embeddings.
    async fn store_chunks_with_bm25(
        &self,
        chunks: &[CodeChunk],
        bm25_embeddings: Option<&[String]>,
    ) -> Result<()>;

    // ========== Vector Search ==========

    /// Search using vector similarity.
    async fn search_vector(&self, embedding: &[f32], limit: i32) -> Result<Vec<CodeChunk>>;

    /// Search using vector similarity, returning chunks with distance scores.
    async fn search_vector_with_distance(
        &self,
        embedding: &[f32],
        limit: i32,
    ) -> Result<Vec<(CodeChunk, f32)>>;

    /// Search using vector similarity, returning ChunkRefs.
    async fn search_vector_refs(&self, embedding: &[f32], limit: i32) -> Result<Vec<ChunkRef>>;

    // ========== Full-Text Search ==========

    /// Search using full-text search.
    async fn search_fts(&self, query: &str, limit: i32) -> Result<Vec<CodeChunk>>;

    /// Search using full-text search, returning ChunkRefs.
    async fn search_fts_refs(&self, query: &str, limit: i32) -> Result<Vec<ChunkRef>>;

    // ========== CRUD ==========

    /// Delete chunks by file path.
    async fn delete_by_path(&self, filepath: &str) -> Result<i32>;

    /// Delete all chunks for a workspace.
    async fn delete_workspace(&self, workspace: &str) -> Result<i32>;

    /// Count total chunks.
    async fn count(&self) -> Result<i64>;

    /// Check if the chunks table exists.
    async fn table_exists(&self) -> Result<bool>;

    /// List all chunks with a default safety limit (100k).
    async fn list_all_chunks(&self) -> Result<Vec<CodeChunk>>;

    /// List chunks with a configurable limit.
    async fn list_all_chunks_with_limit(&self, limit: Option<i32>) -> Result<Vec<CodeChunk>>;

    // ========== File Metadata ==========

    /// Get file metadata for a specific file in a workspace.
    async fn get_file_metadata(
        &self,
        workspace: &str,
        filepath: &str,
    ) -> Result<Option<FileMetadata>>;

    /// Get all file metadata in a workspace.
    async fn get_workspace_files(&self, workspace: &str) -> Result<Vec<FileMetadata>>;

    // ========== Index Management ==========

    /// Create a vector index (no-op for brute-force backends).
    async fn create_vector_index(&self) -> Result<()>;

    /// Create a full-text search index (no-op for trigger-based backends).
    async fn create_fts_index(&self) -> Result<()>;

    /// Get current index status.
    async fn get_index_status(&self, policy: &IndexPolicy) -> Result<IndexStatus>;

    /// Apply index policy - create indexes if thresholds are met.
    async fn apply_index_policy(&self, policy: &IndexPolicy) -> Result<bool>;

    /// Check if index creation is needed based on policy.
    async fn needs_index(&self, policy: &IndexPolicy) -> Result<bool>;

    // ========== BM25 Metadata ==========

    /// Save BM25 metadata.
    async fn save_bm25_metadata(&self, metadata: &Bm25Metadata) -> Result<()>;

    /// Load BM25 metadata.
    async fn load_bm25_metadata(&self) -> Result<Option<Bm25Metadata>>;

    /// Check if BM25 metadata exists.
    async fn bm25_metadata_exists(&self) -> Result<bool>;

    // ========== Bulk Load ==========

    /// Load all chunk references (metadata without content).
    ///
    /// Returns a map from chunk ID to `ChunkRef`. Used by BM25 loader to
    /// read fresh content from the file system during index restoration.
    async fn load_all_chunk_refs(&self) -> Result<HashMap<String, ChunkRef>>;

    /// Load all BM25 embeddings from chunks.
    async fn load_all_bm25_embeddings(&self) -> Result<HashMap<String, SparseEmbedding>>;
}
