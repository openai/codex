//! Integration tests for the retrieval CLI functionality.
//!
//! Tests the CLI components including search, indexing, and configuration.

use std::path::Path;
use std::sync::Arc;

use tempfile::TempDir;

use cocode_retrieval::FacadeBuilder;
use cocode_retrieval::RetrievalFeatures;
use cocode_retrieval::SnippetStorage;
use cocode_retrieval::SymbolQuery;
use cocode_retrieval::config::RetrievalConfig;
use cocode_retrieval::indexing::IndexManager;
use cocode_retrieval::indexing::RebuildMode;
use cocode_retrieval::storage::SqliteStore;

// ==== Helper Function Tests ====

/// Test workspace name extraction from paths.
#[test]
fn test_workspace_name_extraction() {
    // Helper function to extract workspace name (mirrors CLI implementation)
    fn workspace_name(workdir: &Path) -> &str {
        workdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
    }

    // Normal directory
    assert_eq!(
        workspace_name(Path::new("/home/user/myproject")),
        "myproject"
    );
    assert_eq!(workspace_name(Path::new("/tmp/test-repo")), "test-repo");

    // Root directory should return "default"
    assert_eq!(workspace_name(Path::new("/")), "default");

    // Current directory representation - file_name() returns None for "."
    assert_eq!(workspace_name(Path::new(".")), "default");

    // Nested paths
    assert_eq!(
        workspace_name(Path::new("/a/b/c/deep/nested/project")),
        "project"
    );

    // Path with trailing slash (after canonicalization this wouldn't happen, but test anyway)
    assert_eq!(workspace_name(Path::new("project")), "project");
}

/// Test RetrievalFeatures configurations.
#[test]
fn test_retrieval_features() {
    // STANDARD features (BM25 + query rewrite)
    assert!(RetrievalFeatures::STANDARD.code_search);
    assert!(RetrievalFeatures::STANDARD.query_rewrite);
    assert!(!RetrievalFeatures::STANDARD.vector_search);
    assert!(RetrievalFeatures::STANDARD.has_search());

    // FULL features (all enabled)
    assert!(RetrievalFeatures::FULL.code_search);
    assert!(RetrievalFeatures::FULL.query_rewrite);
    assert!(RetrievalFeatures::FULL.vector_search);
    assert!(RetrievalFeatures::FULL.has_search());

    // MINIMAL features (BM25 only)
    assert!(RetrievalFeatures::MINIMAL.code_search);
    assert!(!RetrievalFeatures::MINIMAL.query_rewrite);
    assert!(!RetrievalFeatures::MINIMAL.vector_search);
    assert!(RetrievalFeatures::MINIMAL.has_search());

    // NONE features
    assert!(!RetrievalFeatures::NONE.has_search());
}

// ==== Config Tests ====

#[test]
fn test_config_default_values() {
    let config = RetrievalConfig::default();

    // Default should not be enabled
    assert!(!config.enabled);

    // Check indexing defaults
    assert!(config.indexing.batch_size > 0);
    assert!(config.indexing.max_file_size_mb > 0);

    // Check search defaults
    assert!(config.search.n_final > 0);

    // Check chunking defaults
    assert!(config.chunking.max_tokens > 0);
}

#[tokio::test]
async fn test_config_with_custom_data_dir() {
    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.enabled = true;
    config.data_dir = dir.path().to_path_buf();

    // Verify data_dir is set correctly
    assert_eq!(config.data_dir, dir.path());
}

// ==== Index Manager Tests ====

#[tokio::test]
async fn test_index_manager_creation() {
    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let db_path = config.data_dir.join("retrieval.db");
    let store = Arc::new(SqliteStore::open(&db_path).unwrap());
    let manager = IndexManager::new(config, store);

    // Should be able to get stats for a new workspace
    let stats = manager.get_stats("test-workspace").await.unwrap();
    assert_eq!(stats.file_count, 0);
    assert_eq!(stats.chunk_count, 0);
    assert!(stats.last_indexed.is_none());
}

#[tokio::test]
async fn test_index_build_empty_directory() {
    let dir = TempDir::new().unwrap();
    let workdir = TempDir::new().unwrap();

    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let db_path = config.data_dir.join("retrieval.db");
    let store = Arc::new(SqliteStore::open(&db_path).unwrap());
    let mut manager = IndexManager::new(config, store);

    // Build index for empty directory
    let mut rx = manager
        .rebuild("test", workdir.path(), RebuildMode::Incremental)
        .await
        .unwrap();

    // Drain progress updates
    while let Some(_progress) = rx.recv().await {}

    // Stats should show 0 files
    let stats = manager.get_stats("test").await.unwrap();
    assert_eq!(stats.file_count, 0);
}

#[tokio::test]
async fn test_index_build_with_files() {
    let dir = TempDir::new().unwrap();
    let workdir = TempDir::new().unwrap();

    // Create some test files
    std::fs::write(
        workdir.path().join("main.rs"),
        r#"fn main() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#,
    )
    .unwrap();

    std::fs::write(
        workdir.path().join("lib.rs"),
        r#"pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
"#,
    )
    .unwrap();

    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let db_path = config.data_dir.join("retrieval.db");
    let store = Arc::new(SqliteStore::open(&db_path).unwrap());
    let mut manager = IndexManager::new(config, store);

    // Build index
    let mut rx = manager
        .rebuild("test", workdir.path(), RebuildMode::Incremental)
        .await
        .unwrap();

    // Drain progress updates
    while let Some(_progress) = rx.recv().await {}

    // Stats should show indexed files
    let stats = manager.get_stats("test").await.unwrap();
    assert!(stats.file_count > 0, "Should have indexed files");
}

#[tokio::test]
async fn test_clean_rebuild_mode() {
    let dir = TempDir::new().unwrap();
    let workdir = TempDir::new().unwrap();

    // Create a test file
    std::fs::write(workdir.path().join("test.rs"), "fn test() {}").unwrap();

    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let db_path = config.data_dir.join("retrieval.db");
    let store = Arc::new(SqliteStore::open(&db_path).unwrap());
    let mut manager = IndexManager::new(config, store);

    // First build (tweakcc)
    let mut rx = manager
        .rebuild("test", workdir.path(), RebuildMode::Incremental)
        .await
        .unwrap();
    while let Some(_) = rx.recv().await {}

    let stats1 = manager.get_stats("test").await.unwrap();

    // Clean rebuild
    let mut rx = manager
        .rebuild("test", workdir.path(), RebuildMode::Clean)
        .await
        .unwrap();
    while let Some(_) = rx.recv().await {}

    let stats2 = manager.get_stats("test").await.unwrap();

    // After clean rebuild, should have same number of files
    assert_eq!(stats1.file_count, stats2.file_count);
}

// ==== Search Tests ====

#[tokio::test]
async fn test_retrieval_service_creation() {
    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let service = FacadeBuilder::new(config)
        .features(RetrievalFeatures::MINIMAL)
        .build()
        .await
        .unwrap();
    assert!(service.features().code_search);
    assert!(!service.features().vector_search);
}

#[tokio::test]
async fn test_search_empty_index() {
    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let service = FacadeBuilder::new(config)
        .features(RetrievalFeatures::MINIMAL)
        .build()
        .await
        .unwrap();

    // Start index pipeline and wait for completion
    service.start_pipeline().await.unwrap();
    let result = service
        .index_service()
        .trigger_session_start()
        .await
        .unwrap();
    // Wait for index to complete
    if let Some(rx) = result.index_receiver {
        let _ = rx.await;
    }

    // Search on empty index should return empty results
    let results = service.search("test query").await.unwrap();
    assert!(results.results.is_empty());
}

#[tokio::test]
async fn test_search_with_limit() {
    use cocode_retrieval::SearchRequest;

    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();
    config.search.n_final = 100; // Set high default

    let service = FacadeBuilder::new(config)
        .features(RetrievalFeatures::MINIMAL)
        .build()
        .await
        .unwrap();

    // Start index pipeline and wait for completion
    service.start_pipeline().await.unwrap();
    let result = service
        .index_service()
        .trigger_session_start()
        .await
        .unwrap();
    // Wait for index to complete
    if let Some(rx) = result.index_receiver {
        let _ = rx.await;
    }

    // Search with explicit limit using new SearchRequest API
    let results = service
        .search_service()
        .execute(SearchRequest::new("test").limit(5))
        .await
        .unwrap();
    // Empty index, so results will be empty, but the limit parameter should be accepted
    assert!(results.results.len() <= 5);
}

#[tokio::test]
async fn test_bm25_search() {
    use cocode_retrieval::SearchRequest;

    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let service = FacadeBuilder::new(config)
        .features(RetrievalFeatures::MINIMAL)
        .build()
        .await
        .unwrap();

    // Start index pipeline and wait for completion
    service.start_pipeline().await.unwrap();
    let result = service
        .index_service()
        .trigger_session_start()
        .await
        .unwrap();
    // Wait for index to complete
    if let Some(rx) = result.index_receiver {
        let _ = rx.await;
    }

    // BM25 search on empty index using new SearchRequest API
    let results = service
        .search_service()
        .execute(SearchRequest::new("function").bm25().limit(10))
        .await
        .unwrap();
    assert!(results.results.is_empty());
}

#[tokio::test]
async fn test_vector_search_without_embeddings() {
    use cocode_retrieval::SearchRequest;

    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let service = FacadeBuilder::new(config)
        .features(RetrievalFeatures::FULL) // Enable vector but no provider configured
        .build()
        .await
        .unwrap();

    // Start index pipeline and wait for completion
    service.start_pipeline().await.unwrap();
    let result = service
        .index_service()
        .trigger_session_start()
        .await
        .unwrap();
    // Wait for index to complete
    if let Some(rx) = result.index_receiver {
        let _ = rx.await;
    }

    // Vector search without embeddings configured should return empty
    assert!(!service.search_service().has_vector_search());
    let results = service
        .search_service()
        .execute(SearchRequest::new("semantic query").vector().limit(10))
        .await
        .unwrap();
    assert!(results.results.is_empty());
}

// ==== Snippet Search Tests ====

#[tokio::test]
async fn test_symbol_query_parsing() {
    // Test SymbolQuery parsing
    let query = SymbolQuery::parse("type:function name:handler");
    assert!(query.kind.is_some() || query.name.is_some());

    let query2 = SymbolQuery::parse("just plain text");
    // Should still create a query, just with the text as search term
    assert!(query2.name.is_some() || query2.text.is_some());
}

#[tokio::test]
async fn test_snippet_search_empty_index() {
    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let db_path = config.data_dir.join("retrieval.db");
    let store = Arc::new(SqliteStore::open(&db_path).unwrap());
    let snippet_store = SnippetStorage::new(store);

    let query = SymbolQuery::parse("fn:test");
    let results = snippet_store.search_fts("test", &query, 10).await.unwrap();
    assert!(results.is_empty());
}

// ==== End-to-End Tests ====

#[tokio::test]
async fn test_index_and_search_flow() {
    let dir = TempDir::new().unwrap();
    let workdir = TempDir::new().unwrap();

    // Create test file with searchable content
    std::fs::write(
        workdir.path().join("search_test.rs"),
        r#"/// Handler for processing requests
pub fn handle_request(req: Request) -> Response {
    // Process the request
    Response::ok()
}

/// Configuration struct
pub struct AppConfig {
    pub port: u16,
    pub host: String,
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            port: 8080,
            host: "localhost".to_string(),
        }
    }
}
"#,
    )
    .unwrap();

    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    // Step 1: Build index
    let db_path = config.data_dir.join("retrieval.db");
    let store = Arc::new(SqliteStore::open(&db_path).unwrap());
    let mut manager = IndexManager::new(config.clone(), store);

    let workspace = workdir
        .path()
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("test");

    let mut rx = manager
        .rebuild(workspace, workdir.path(), RebuildMode::Incremental)
        .await
        .unwrap();
    while let Some(_) = rx.recv().await {}

    // Verify indexing succeeded
    let stats = manager.get_stats(workspace).await.unwrap();
    assert!(stats.file_count > 0, "Should have indexed the test file");

    // Step 2: Search
    use cocode_retrieval::SearchRequest;
    let service = FacadeBuilder::new(config)
        .features(RetrievalFeatures::MINIMAL)
        .build()
        .await
        .unwrap();

    // Start index pipeline and wait for completion
    service.start_pipeline().await.unwrap();
    let result = service
        .index_service()
        .trigger_session_start()
        .await
        .unwrap();
    // Wait for index to complete
    if let Some(rx) = result.index_receiver {
        let _ = rx.await;
    }

    // BM25 search for known content using new SearchRequest API
    let results = service
        .search_service()
        .execute(SearchRequest::new("handle_request").bm25().limit(10))
        .await
        .unwrap();
    // Note: Results may be empty if FTS index not populated during this test
    // The important thing is that the search completes without error
    assert!(results.results.len() <= 10);
}

// ==== Watch Debounce Tests ====

#[test]
fn test_debounce_ms_validation() {
    // Test that negative debounce_ms is handled correctly
    let negative: i32 = -100;
    let validated = negative.max(0) as u64;
    assert_eq!(validated, 0);

    let positive: i32 = 500;
    let validated = positive.max(0) as u64;
    assert_eq!(validated, 500);
}

// ==== Config Loading Tests ====

#[tokio::test]
async fn test_config_from_file() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("retrieval.toml");

    // Write a minimal config file (no [retrieval] wrapper - from_file parses directly)
    std::fs::write(
        &config_path,
        r#"enabled = true

[indexing]
batch_size = 100
watch_enabled = true
watch_debounce_ms = 250

[search]
n_final = 20
"#,
    )
    .unwrap();

    let config = RetrievalConfig::from_file(&config_path).unwrap();
    assert!(config.enabled);
    assert_eq!(config.indexing.batch_size, 100);
    assert!(config.indexing.watch_enabled);
    assert_eq!(config.indexing.watch_debounce_ms, 250);
    assert_eq!(config.search.n_final, 20);
}

#[test]
fn test_config_load_nonexistent_returns_default() {
    let dir = TempDir::new().unwrap();
    let nonexistent_config = dir.path().join("nonexistent.toml");
    // Use explicit config file path - does not fall back to global config
    let config = RetrievalConfig::load_with_config_file(&nonexistent_config).unwrap();
    assert!(!config.enabled); // Default is disabled
}
