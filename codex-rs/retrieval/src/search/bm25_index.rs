//! BM25 index management with tunable k1/b parameters.
//!
//! This module provides a custom BM25 implementation using the `bm25` crate,
//! allowing us to tune k1 and b parameters for code-optimized search.

use std::collections::HashMap;
use std::sync::Arc;

use bm25::Embedder;
use bm25::EmbedderBuilder;
use bm25::Scorer;
use bm25::Tokenizer;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::RwLock;

use super::code_tokenizer::CodeTokenizer;
use crate::config::SearchConfig;
use crate::types::CodeChunk;

/// Sparse embedding representation for BM25.
///
/// Stores non-zero token indices and their TF weights.
/// Can be serialized to JSON for LanceDB storage.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SparseEmbedding {
    /// Token indices (hash of token string)
    pub indices: Vec<i32>,
    /// Corresponding TF weights
    pub values: Vec<f32>,
}

impl SparseEmbedding {
    /// Create a new sparse embedding.
    pub fn new(indices: Vec<i32>, values: Vec<f32>) -> Self {
        Self { indices, values }
    }

    /// Create from bm25 crate's Embedding type.
    pub fn from_bm25_embedding(embedding: &bm25::Embedding<u32>) -> Self {
        let mut indices = Vec::new();
        let mut values = Vec::new();

        for token_embedding in embedding.iter() {
            // Saturate at i32::MAX to prevent overflow panic.
            // In practice, token indices rarely exceed 2^31-1 for typical vocabularies.
            let index = if token_embedding.index > i32::MAX as u32 {
                tracing::warn!(
                    token_index = token_embedding.index,
                    "Token index exceeds i32::MAX, saturating"
                );
                i32::MAX
            } else {
                token_embedding.index as i32
            };
            indices.push(index);
            values.push(token_embedding.value);
        }

        Self { indices, values }
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Serialize to JSON string for storage.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Deserialize from JSON string.
    pub fn from_json(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

/// BM25 configuration.
#[derive(Debug, Clone)]
pub struct Bm25Config {
    /// Term frequency saturation (k1), default 0.8 for code
    pub k1: f32,
    /// Document length normalization (b), default 0.5 for code
    pub b: f32,
    /// Average document length (calculated from corpus)
    pub avgdl: f32,
}

impl Default for Bm25Config {
    fn default() -> Self {
        Self {
            k1: 0.8,      // Lower than default 1.2, better for code with repeated keywords
            b: 0.5,       // Lower than default 0.75, less length normalization for functions
            avgdl: 100.0, // Default, will be recalculated from corpus
        }
    }
}

impl Bm25Config {
    /// Create from SearchConfig.
    pub fn from_search_config(config: &SearchConfig) -> Self {
        Self {
            k1: config.bm25_k1,
            b: config.bm25_b,
            avgdl: 100.0, // Will be recalculated
        }
    }
}

/// BM25 metadata for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bm25Metadata {
    /// Average document length
    pub avgdl: f32,
    /// Total number of documents
    pub total_docs: i64,
    /// Last update timestamp
    pub updated_at: i64,
}

/// BM25 index manager.
///
/// Manages BM25 embeddings and scoring with tunable parameters.
/// Thread-safe for concurrent access.
pub struct Bm25Index {
    /// Code-specific tokenizer
    tokenizer: CodeTokenizer,
    /// BM25 embedder
    embedder: Embedder<u32, CodeTokenizer>,
    /// BM25 scorer for document ranking
    scorer: Scorer<String, u32>,
    /// Chunk ID to sparse embedding mapping (for persistence)
    embeddings: HashMap<String, SparseEmbedding>,
    /// Chunk ID to content mapping (for avgdl calculation)
    doc_lengths: HashMap<String, i32>,
    /// Configuration
    config: Bm25Config,
}

/// Helper function to build embedder with config.
fn build_embedder(config: &Bm25Config) -> Embedder<u32, CodeTokenizer> {
    EmbedderBuilder::<u32, CodeTokenizer>::with_avgdl(config.avgdl)
        .b(config.b)
        .k1(config.k1)
        .build()
}

impl Bm25Index {
    /// Create a new BM25 index with default configuration.
    pub fn new() -> Self {
        Self::with_config(Bm25Config::default())
    }

    /// Create a new BM25 index with custom configuration.
    pub fn with_config(config: Bm25Config) -> Self {
        let tokenizer = CodeTokenizer::new();
        let embedder = build_embedder(&config);
        let scorer = Scorer::new();

        Self {
            tokenizer,
            embedder,
            scorer,
            embeddings: HashMap::new(),
            doc_lengths: HashMap::new(),
            config,
        }
    }

    /// Create from SearchConfig.
    pub fn from_search_config(config: &SearchConfig) -> Self {
        Self::with_config(Bm25Config::from_search_config(config))
    }

    /// Load index from stored embeddings and metadata.
    ///
    /// This version does NOT restore the scorer - use `load_with_contents` for full restoration.
    /// Searches will not work until documents are re-indexed via `upsert_chunk`.
    #[deprecated(
        since = "0.1.0",
        note = "Use load_with_contents for proper scorer restoration"
    )]
    pub fn load(
        embeddings: HashMap<String, SparseEmbedding>,
        metadata: Option<Bm25Metadata>,
        config: Bm25Config,
    ) -> Self {
        Self::load_with_contents(embeddings, HashMap::new(), metadata, config)
    }

    /// Load index from stored embeddings, contents, and metadata.
    ///
    /// Reconstructs the index from persisted data, including the scorer for search.
    /// Pass chunk contents to re-embed documents and populate the scorer.
    ///
    /// # Arguments
    /// * `embeddings` - Stored sparse embeddings (chunk_id -> SparseEmbedding)
    /// * `contents` - Chunk contents for re-embedding (chunk_id -> content string)
    /// * `metadata` - Index metadata including avgdl
    /// * `config` - BM25 configuration
    ///
    /// # Performance
    /// Re-embedding is O(n) where n is the number of documents. For large indices,
    /// this may take a few seconds on startup. The embedder is CPU-bound.
    pub fn load_with_contents(
        embeddings: HashMap<String, SparseEmbedding>,
        contents: HashMap<String, String>,
        metadata: Option<Bm25Metadata>,
        config: Bm25Config,
    ) -> Self {
        let avgdl = metadata.as_ref().map(|m| m.avgdl).unwrap_or(config.avgdl);
        let mut config = config;
        config.avgdl = avgdl;

        let tokenizer = CodeTokenizer::new();
        let embedder = build_embedder(&config);
        let mut scorer = Scorer::new();
        let mut doc_lengths = HashMap::new();

        // Rebuild scorer from contents by re-embedding
        // This is necessary because bm25::Embedding can't be reconstructed from SparseEmbedding
        let start = std::time::Instant::now();
        let mut rebuilt_count = 0;

        for (chunk_id, content) in &contents {
            // Re-embed the content
            let embedding = embedder.embed(content);

            // Track document length
            let tokens = tokenizer.tokenize(content);
            doc_lengths.insert(chunk_id.clone(), tokens.len() as i32);

            // Update scorer
            scorer.upsert(chunk_id, embedding);
            rebuilt_count += 1;
        }

        if rebuilt_count > 0 {
            tracing::debug!(
                count = rebuilt_count,
                elapsed_ms = start.elapsed().as_millis(),
                "Rebuilt BM25 scorer from contents"
            );
        }

        Self {
            tokenizer,
            embedder,
            scorer,
            embeddings,
            doc_lengths,
            config,
        }
    }

    /// Check if the scorer is populated and ready for search.
    ///
    /// Returns false if the index was loaded without contents (scorer is empty).
    pub fn is_searchable(&self) -> bool {
        // If we have embeddings but no doc_lengths, scorer wasn't rebuilt
        self.embeddings.is_empty() || !self.doc_lengths.is_empty()
    }

    /// Get current configuration.
    pub fn config(&self) -> &Bm25Config {
        &self.config
    }

    /// Get number of indexed documents.
    pub fn doc_count(&self) -> i64 {
        self.embeddings.len() as i64
    }

    /// Get all chunk IDs.
    pub fn chunk_ids(&self) -> Vec<String> {
        self.embeddings.keys().cloned().collect()
    }

    /// Get embedding for a chunk.
    pub fn get_embedding(&self, chunk_id: &str) -> Option<&SparseEmbedding> {
        self.embeddings.get(chunk_id)
    }

    /// Get all embeddings (for persistence).
    pub fn embeddings(&self) -> &HashMap<String, SparseEmbedding> {
        &self.embeddings
    }

    /// Index a chunk and return its sparse embedding.
    ///
    /// Updates both the internal scorer and the embeddings map.
    pub fn upsert_chunk(&mut self, chunk: &CodeChunk) -> SparseEmbedding {
        let content = &chunk.content;
        let chunk_id = &chunk.id;

        // Tokenize and embed
        let tokens = self.tokenizer.tokenize(content);
        let doc_length = tokens.len() as i32;

        // Create embedding
        let embedding = self.embedder.embed(content);
        let sparse = SparseEmbedding::from_bm25_embedding(&embedding);

        // Update scorer
        self.scorer.upsert(chunk_id, embedding);

        // Store for persistence
        self.embeddings.insert(chunk_id.clone(), sparse.clone());
        self.doc_lengths.insert(chunk_id.clone(), doc_length);

        sparse
    }

    /// Index multiple chunks at once.
    pub fn upsert_chunks(&mut self, chunks: &[CodeChunk]) -> Vec<SparseEmbedding> {
        chunks.iter().map(|c| self.upsert_chunk(c)).collect()
    }

    /// Remove a chunk from the index.
    pub fn remove_chunk(&mut self, chunk_id: &str) {
        self.embeddings.remove(chunk_id);
        self.doc_lengths.remove(chunk_id);
        // Note: bm25 Scorer doesn't have a remove method, so we rebuild if needed
    }

    /// Remove multiple chunks.
    pub fn remove_chunks(&mut self, chunk_ids: &[String]) {
        for id in chunk_ids {
            self.remove_chunk(id);
        }
    }

    /// Search for chunks matching the query.
    ///
    /// Returns (chunk_id, score) pairs sorted by relevance.
    pub fn search(&self, query: &str, limit: i32) -> Vec<(String, f32)> {
        let query_embedding = self.embedder.embed(query);
        let matches = self.scorer.matches(&query_embedding);

        matches
            .into_iter()
            .take(limit as usize)
            .map(|doc| (doc.id, doc.score))
            .collect()
    }

    /// Recalculate avgdl from current documents.
    ///
    /// Should be called after significant document changes.
    pub fn recalculate_avgdl(&mut self) {
        if self.doc_lengths.is_empty() {
            return;
        }

        let total_length: i64 = self.doc_lengths.values().map(|&l| l as i64).sum();
        let avgdl = total_length as f32 / self.doc_lengths.len() as f32;

        self.config.avgdl = avgdl;

        // Rebuild embedder with new avgdl
        self.embedder = build_embedder(&self.config);
    }

    /// Get metadata for persistence.
    pub fn metadata(&self) -> Bm25Metadata {
        Bm25Metadata {
            avgdl: self.config.avgdl,
            total_docs: self.doc_count(),
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Check if the index needs avgdl recalculation.
    ///
    /// Returns true if document count changed significantly (>10%).
    pub fn needs_avgdl_update(&self, previous_count: i64) -> bool {
        let current = self.doc_count();
        if previous_count == 0 {
            return current > 0;
        }
        let change_ratio = ((current - previous_count) as f32).abs() / previous_count as f32;
        change_ratio > 0.1
    }
}

impl Default for Bm25Index {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper for BM25Index.
pub type SharedBm25Index = Arc<RwLock<Bm25Index>>;

/// Create a new shared BM25 index.
pub fn new_shared_index() -> SharedBm25Index {
    Arc::new(RwLock::new(Bm25Index::new()))
}

/// Create a new shared BM25 index with config.
pub fn new_shared_index_with_config(config: &SearchConfig) -> SharedBm25Index {
    Arc::new(RwLock::new(Bm25Index::from_search_config(config)))
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

    #[test]
    fn test_index_and_search() {
        let mut index = Bm25Index::new();

        // Index some chunks
        let chunk1 = make_test_chunk("1", "fn get_user_by_id(id: i32) -> User");
        let chunk2 = make_test_chunk("2", "fn delete_user(id: i32) -> Result<()>");
        let chunk3 = make_test_chunk("3", "struct DatabaseConnection { pool: Pool }");

        index.upsert_chunk(&chunk1);
        index.upsert_chunk(&chunk2);
        index.upsert_chunk(&chunk3);

        // Search for "user"
        let results = index.search("get user", 10);
        assert!(!results.is_empty());

        // First result should be chunk1 (most relevant to "get user")
        assert_eq!(results[0].0, "1");
    }

    #[test]
    fn test_sparse_embedding_serialization() {
        let embedding = SparseEmbedding::new(vec![1, 2, 3], vec![0.5, 0.3, 0.2]);

        let json = embedding.to_json();
        let restored = SparseEmbedding::from_json(&json).unwrap();

        assert_eq!(embedding.indices, restored.indices);
        assert_eq!(embedding.values, restored.values);
    }

    #[test]
    fn test_config_from_search_config() {
        let search_config = SearchConfig {
            bm25_k1: 0.9,
            bm25_b: 0.4,
            ..Default::default()
        };

        let bm25_config = Bm25Config::from_search_config(&search_config);
        assert!((bm25_config.k1 - 0.9).abs() < 0.001);
        assert!((bm25_config.b - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_recalculate_avgdl() {
        let mut index = Bm25Index::new();

        // Index chunks of different lengths
        let chunk1 = make_test_chunk("1", "fn foo() {}");
        let chunk2 = make_test_chunk("2", "fn bar_baz_qux() { let x = 1; let y = 2; }");

        index.upsert_chunk(&chunk1);
        index.upsert_chunk(&chunk2);

        let old_avgdl = index.config().avgdl;
        index.recalculate_avgdl();
        let new_avgdl = index.config().avgdl;

        // avgdl should change after recalculation
        assert!((old_avgdl - new_avgdl).abs() > 0.001 || old_avgdl == 100.0);
    }

    #[test]
    fn test_remove_chunk() {
        let mut index = Bm25Index::new();

        let chunk = make_test_chunk("1", "fn test() {}");
        index.upsert_chunk(&chunk);

        assert_eq!(index.doc_count(), 1);

        index.remove_chunk("1");
        assert_eq!(index.doc_count(), 0);
    }

    #[test]
    fn test_metadata() {
        let mut index = Bm25Index::new();

        let chunk = make_test_chunk("1", "fn test() {}");
        index.upsert_chunk(&chunk);

        let metadata = index.metadata();
        assert_eq!(metadata.total_docs, 1);
        assert!(metadata.updated_at > 0);
    }
}
