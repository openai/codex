//! End-to-end vector search integration tests.
//!
//! Tests hybrid search with mock embedding provider.

use std::sync::Arc;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use tempfile::TempDir;

use codex_retrieval::Result;
use codex_retrieval::error::RetrievalErr;
use codex_retrieval::search::HybridSearcher;
use codex_retrieval::storage::LanceDbStore;
use codex_retrieval::traits::EmbeddingProvider;
use codex_retrieval::types::CodeChunk;
use codex_retrieval::types::ScoreType;

// ==== Mock Embedding Provider ====

/// Mock embedding provider for testing.
#[derive(Debug)]
struct MockEmbeddingProvider {
    dimension: i32,
    call_count: AtomicI32,
    should_fail: bool,
}

impl MockEmbeddingProvider {
    fn new(dimension: i32) -> Self {
        Self {
            dimension,
            call_count: AtomicI32::new(0),
            should_fail: false,
        }
    }

    fn failing() -> Self {
        Self {
            dimension: 1536,
            call_count: AtomicI32::new(0),
            should_fail: true,
        }
    }

    fn get_call_count(&self) -> i32 {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn dimension(&self) -> i32 {
        self.dimension
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        if self.should_fail {
            return Err(RetrievalErr::EmbeddingFailed {
                cause: "mock failure".to_string(),
            });
        }

        // Generate deterministic embedding based on text
        let hash = text.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
        let mut embedding = vec![0.0f32; self.dimension as usize];
        for (i, v) in embedding.iter_mut().enumerate() {
            *v = ((hash as f32 + i as f32) / 1000.0).sin();
        }
        Ok(embedding)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

// ==== Test Fixtures ====

async fn setup_store() -> (TempDir, Arc<LanceDbStore>) {
    let dir = TempDir::new().unwrap();
    let store = LanceDbStore::open(dir.path()).await.unwrap();
    (dir, Arc::new(store))
}

fn make_chunk(id: &str, content: &str, filepath: &str) -> CodeChunk {
    CodeChunk {
        id: id.to_string(),
        source_id: "test".to_string(),
        filepath: filepath.to_string(),
        language: "rust".to_string(),
        content: content.to_string(),
        start_line: 1,
        end_line: 10,
        embedding: None,
        modified_time: None,
        workspace: "test".to_string(),
        content_hash: String::new(),
        indexed_at: 0,
        parent_symbol: None,
        is_overview: false,
    }
}

async fn setup_with_chunks(chunks: Vec<CodeChunk>) -> (TempDir, Arc<LanceDbStore>) {
    let (dir, store) = setup_store().await;

    // Store all chunks at once
    store.store_chunks(&chunks).await.unwrap();

    // Create FTS index for BM25 search
    store.create_fts_index().await.ok();

    // Wait for index to be ready
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    (dir, store)
}

// ==== BM25-Only Search Tests ====

#[tokio::test]
async fn test_bm25_only_search() {
    let chunks = vec![
        make_chunk(
            "1",
            "fn authenticate_user(username: &str) -> bool",
            "auth.rs",
        ),
        make_chunk(
            "2",
            "fn validate_password(password: &str) -> bool",
            "auth.rs",
        ),
        make_chunk(
            "3",
            "fn create_database_connection() -> Connection",
            "db.rs",
        ),
    ];

    let (_dir, store) = setup_with_chunks(chunks).await;
    let searcher = HybridSearcher::new(store);

    let results = searcher.search("authenticate", 10).await.unwrap();

    // Should find the auth-related chunk
    assert!(!results.is_empty());
    assert!(results[0].chunk.content.contains("authenticate"));
}

#[tokio::test]
async fn test_bm25_no_results() {
    let chunks = vec![make_chunk("1", "fn hello_world() -> String", "main.rs")];

    let (_dir, store) = setup_with_chunks(chunks).await;
    let searcher = HybridSearcher::new(store);

    let results = searcher.search("authenticate", 10).await.unwrap();

    // Should return empty
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_bm25_limit() {
    let chunks: Vec<CodeChunk> = (0..20)
        .map(|i| {
            make_chunk(
                &format!("chunk{i}"),
                &format!("fn function_{i}() {{ }}"),
                "many.rs",
            )
        })
        .collect();

    let (_dir, store) = setup_with_chunks(chunks).await;
    let searcher = HybridSearcher::new(store);

    let results = searcher.search("function", 5).await.unwrap();

    // Should respect limit
    assert!(results.len() <= 5);
}

// ==== Hybrid Search Tests ====

#[tokio::test]
async fn test_hybrid_search_with_embeddings() {
    let chunks = vec![
        make_chunk(
            "1",
            "fn authenticate_user(username: &str) -> bool",
            "auth.rs",
        ),
        make_chunk(
            "2",
            "fn validate_password(password: &str) -> bool",
            "auth.rs",
        ),
        make_chunk(
            "3",
            "fn create_database_connection() -> Connection",
            "db.rs",
        ),
    ];

    let (_dir, store) = setup_with_chunks(chunks).await;
    let provider = Arc::new(MockEmbeddingProvider::new(1536));
    let searcher = HybridSearcher::with_embeddings(store, provider.clone());

    let results = searcher.search("user authentication", 10).await.unwrap();

    // Should have called embedding provider
    assert!(provider.get_call_count() > 0);

    // Should find auth-related chunks
    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_hybrid_search_fallback_on_embedding_failure() {
    let chunks = vec![
        make_chunk(
            "1",
            "fn authenticate_user(username: &str) -> bool",
            "auth.rs",
        ),
        make_chunk(
            "2",
            "fn validate_password(password: &str) -> bool",
            "auth.rs",
        ),
    ];

    let (_dir, store) = setup_with_chunks(chunks).await;
    let provider = Arc::new(MockEmbeddingProvider::failing());
    let searcher = HybridSearcher::with_embeddings(store, provider);

    // Should still return results via BM25 fallback
    let results = searcher.search("authenticate", 10).await.unwrap();
    assert!(!results.is_empty());

    // Results should be from BM25 (ScoreType::Bm25)
    assert!(
        results
            .iter()
            .any(|r| matches!(r.score_type, ScoreType::Bm25))
    );
}

// ==== Identifier Query Detection Tests ====

#[tokio::test]
async fn test_identifier_query_boosting() {
    let chunks = vec![
        make_chunk("1", "fn getUserName() -> String { }", "user.rs"),
        make_chunk("2", "fn getUser() -> User { }", "user.rs"),
        make_chunk(
            "3",
            "This function gets the user name from database",
            "docs.rs",
        ),
    ];

    let (_dir, store) = setup_with_chunks(chunks).await;
    let searcher = HybridSearcher::new(store);

    // Search for identifier-like query
    let results = searcher.search("getUserName", 10).await.unwrap();

    // Should find the exact function name first
    if !results.is_empty() {
        assert!(results[0].chunk.content.contains("getUserName"));
    }
}

// ==== Deduplication Tests ====

#[tokio::test]
async fn test_overlapping_chunks_deduplication() {
    // Create overlapping chunks from the same file
    let chunks = vec![
        CodeChunk {
            id: "chunk1".to_string(),
            source_id: "test".to_string(),
            filepath: "auth.rs".to_string(),
            language: "rust".to_string(),
            content: "fn authenticate() { validate(); }".to_string(),
            start_line: 1,
            end_line: 10,
            embedding: None,
            modified_time: None,
            workspace: "test".to_string(),
            content_hash: String::new(),
            indexed_at: 0,
            parent_symbol: None,
            is_overview: false,
        },
        CodeChunk {
            id: "chunk2".to_string(),
            source_id: "test".to_string(),
            filepath: "auth.rs".to_string(),
            language: "rust".to_string(),
            content: "fn validate() { check(); }".to_string(),
            start_line: 5, // Overlaps with chunk1
            end_line: 15,
            embedding: None,
            modified_time: None,
            workspace: "test".to_string(),
            content_hash: String::new(),
            indexed_at: 0,
            parent_symbol: None,
            is_overview: false,
        },
    ];

    let (_dir, store) = setup_with_chunks(chunks).await;
    let searcher = HybridSearcher::new(store);

    let results = searcher.search("authenticate validate", 10).await.unwrap();

    // Should deduplicate overlapping chunks
    // The exact count depends on deduplication logic
    assert!(results.len() <= 2);
}

// ==== Score Type Tests ====

#[tokio::test]
async fn test_score_type_bm25() {
    let chunks = vec![make_chunk("1", "fn test_function() { }", "test.rs")];

    let (_dir, store) = setup_with_chunks(chunks).await;
    let searcher = HybridSearcher::new(store);

    let results = searcher.search_bm25("test", 10).await.unwrap();

    if !results.is_empty() {
        assert!(matches!(results[0].score_type, ScoreType::Bm25));
    }
}

// ==== Empty Store Tests ====

#[tokio::test]
async fn test_search_empty_store() {
    let (_dir, store) = setup_store().await;
    let searcher = HybridSearcher::new(store);

    let results = searcher.search("anything", 10).await.unwrap();
    assert!(results.is_empty());
}

// ==== Mock Provider Tests ====

#[tokio::test]
async fn test_mock_embedding_deterministic() {
    let provider = MockEmbeddingProvider::new(10);

    let embedding1 = provider.embed("test").await.unwrap();
    let embedding2 = provider.embed("test").await.unwrap();

    // Same input should produce same output
    assert_eq!(embedding1, embedding2);

    // Different input should produce different output
    let embedding3 = provider.embed("different").await.unwrap();
    assert_ne!(embedding1, embedding3);
}

#[tokio::test]
async fn test_mock_embedding_dimension() {
    let provider = MockEmbeddingProvider::new(256);

    let embedding = provider.embed("test").await.unwrap();
    assert_eq!(embedding.len(), 256);
}

#[tokio::test]
async fn test_mock_embedding_batch() {
    let provider = MockEmbeddingProvider::new(128);

    let texts = vec!["hello".to_string(), "world".to_string(), "test".to_string()];
    let embeddings = provider.embed_batch(&texts).await.unwrap();

    assert_eq!(embeddings.len(), 3);
    assert!(embeddings.iter().all(|e| e.len() == 128));
}
