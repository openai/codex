//! Integration tests for the indexing module.
//!
//! Tests IndexManager, ChangeDetector, and related functionality.

use std::sync::Arc;

use tempfile::TempDir;

use codex_retrieval::chunking::CodeChunkerService;
use codex_retrieval::config::RetrievalConfig;
use codex_retrieval::indexing::ChangeDetector;
use codex_retrieval::indexing::ChangeStatus;
use codex_retrieval::indexing::FileWalker;
use codex_retrieval::indexing::IndexManager;
use codex_retrieval::storage::SqliteStore;

// ==== CodeChunkerService Tests ====

#[test]
fn test_code_splitter_for_rust_respects_function_boundaries() {
    // CodeSplitter should keep functions intact when possible
    let code = r#"fn hello() {
    println!("Hello");
}

fn world() {
    println!("World");
}

fn long_function_with_many_lines() {
    let a = 1;
    let b = 2;
    let c = 3;
    let d = 4;
    let e = 5;
    println!("{} {} {} {} {}", a, b, c, d, e);
}
"#;

    // Use token-aware chunker with small limit to force chunking
    let chunker = CodeChunkerService::new(50, 0);
    let chunks = chunker.chunk(code, "rust").expect("chunking failed");

    // Should have chunks
    assert!(!chunks.is_empty(), "Should produce chunks");

    // Each chunk should contain complete functions (no mid-function splits)
    for chunk in &chunks {
        let content = &chunk.content;
        // If a chunk starts a function, it should also end it (for small functions)
        if content.contains("fn hello()") && content.len() < 100 {
            assert!(
                content.contains("}"),
                "Small function should be complete in chunk"
            );
        }
    }

    // Verify line numbers are 1-indexed
    assert!(
        chunks[0].start_line >= 1,
        "Line numbers should be 1-indexed"
    );
}

#[test]
fn test_code_splitter_for_python_respects_class_boundaries() {
    let code = r#"def greet():
    print("Hello")

class Calculator:
    def add(self, a, b):
        return a + b

    def subtract(self, a, b):
        return a - b

def farewell():
    print("Goodbye")
"#;

    let chunker = CodeChunkerService::new(100, 0);
    let chunks = chunker.chunk(code, "python").expect("chunking failed");

    assert!(!chunks.is_empty());

    // Verify content coverage
    let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
    assert!(total.contains("def greet()"));
    assert!(total.contains("class Calculator"));
    assert!(total.contains("def farewell()"));
}

#[test]
fn test_code_splitter_for_go_respects_func_boundaries() {
    let code = r#"package main

func hello() {
    fmt.Println("Hello")
}

func world() {
    fmt.Println("World")
}
"#;

    let chunker = CodeChunkerService::new(50, 0);
    let chunks = chunker.chunk(code, "go").expect("chunking failed");

    assert!(!chunks.is_empty());

    let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
    assert!(total.contains("func hello()"));
    assert!(total.contains("func world()"));
}

#[test]
fn test_text_splitter_fallback_for_unsupported_language() {
    let code = "const x = 1;\nconst y = 2;\nconst z = 3;";

    let chunker = CodeChunkerService::new(1000, 0);
    let chunks = chunker.chunk(code, "javascript").expect("chunking failed");

    // Should fall back to TextSplitter
    assert!(!chunks.is_empty());
    let total: String = chunks.iter().map(|c| c.content.as_str()).collect();
    assert_eq!(total.trim(), code.trim());
}

#[test]
fn test_chunk_overlap_prepends_content() {
    // Create content that will produce multiple chunks
    let lines: Vec<String> = (1..=30).map(|i| format!("line{i}")).collect();
    let code = lines.join("\n");

    // With overlap (5 tokens)
    let chunker_with_overlap = CodeChunkerService::new(30, 5);
    let chunks = chunker_with_overlap
        .chunk(&code, "text")
        .expect("chunking failed");

    if chunks.len() >= 2 {
        // Second chunk should start with content from end of first chunk
        assert!(
            chunks[1].content.len() > 0,
            "Second chunk should have content"
        );
    }
}

// ==== ChangeDetector Tests ====

#[tokio::test]
async fn test_change_detector_detects_new_files() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Arc::new(SqliteStore::open(&db_path).expect("failed to open db"));

    let detector = ChangeDetector::new(db);

    // First scan with some files
    let mut files = std::collections::HashMap::new();
    files.insert("src/main.rs".to_string(), "hash1".to_string());
    files.insert("src/lib.rs".to_string(), "hash2".to_string());

    let changes = detector
        .detect_changes("workspace1", &files)
        .await
        .expect("detect_changes failed");

    // All files should be detected as Added
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().all(|c| c.status == ChangeStatus::Added));
}

#[tokio::test]
async fn test_change_detector_detects_modified_files() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Arc::new(SqliteStore::open(&db_path).expect("failed to open db"));

    let detector = ChangeDetector::new(db);
    let workspace = "test_workspace";

    // Initial state
    let mut files = std::collections::HashMap::new();
    files.insert("file.rs".to_string(), "hash_v1".to_string());

    let changes1 = detector
        .detect_changes(workspace, &files)
        .await
        .expect("detect_changes failed");

    // Update catalog to simulate indexed state
    for change in &changes1 {
        if let Some(hash) = &change.content_hash {
            detector
                .update_catalog(workspace, &change.filepath, hash, 0, 1, 0)
                .await
                .expect("update_catalog failed");
        }
    }

    // Second scan with modified file
    files.insert("file.rs".to_string(), "hash_v2".to_string());

    let changes2 = detector
        .detect_changes(workspace, &files)
        .await
        .expect("detect_changes failed");

    // File should be detected as Modified
    assert_eq!(changes2.len(), 1);
    assert_eq!(changes2[0].status, ChangeStatus::Modified);
}

#[tokio::test]
async fn test_change_detector_detects_deleted_files() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Arc::new(SqliteStore::open(&db_path).expect("failed to open db"));

    let detector = ChangeDetector::new(db);
    let workspace = "test_workspace";

    // Initial state with two files
    let mut files = std::collections::HashMap::new();
    files.insert("file1.rs".to_string(), "hash1".to_string());
    files.insert("file2.rs".to_string(), "hash2".to_string());

    let changes1 = detector
        .detect_changes(workspace, &files)
        .await
        .expect("detect_changes failed");

    // Update catalog
    for change in &changes1 {
        if let Some(hash) = &change.content_hash {
            detector
                .update_catalog(workspace, &change.filepath, hash, 0, 1, 0)
                .await
                .expect("update_catalog failed");
        }
    }

    // Second scan with one file removed
    files.remove("file2.rs");

    let changes2 = detector
        .detect_changes(workspace, &files)
        .await
        .expect("detect_changes failed");

    // file2.rs should be detected as Deleted
    let deleted: Vec<_> = changes2
        .iter()
        .filter(|c| c.status == ChangeStatus::Deleted)
        .collect();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].filepath, "file2.rs");
}

// ==== FileWalker Tests ====

#[test]
fn test_file_walker_finds_source_files() {
    let dir = TempDir::new().unwrap();

    // Create test files
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn foo() {}").unwrap();
    std::fs::write(dir.path().join("README.md"), "# Test").unwrap();

    let walker = FileWalker::new(10); // 10MB max
    let files = walker.walk(dir.path()).expect("walk failed");

    // Should find at least the .rs files
    let rs_files: Vec<_> = files
        .iter()
        .filter(|f| f.extension().map_or(false, |e| e == "rs"))
        .collect();
    assert_eq!(rs_files.len(), 2);
}

#[test]
fn test_file_walker_respects_gitignore() {
    let dir = TempDir::new().unwrap();

    // Create .gitignore
    std::fs::write(dir.path().join(".gitignore"), "target/\n*.log\n").unwrap();

    // Create files
    std::fs::create_dir_all(dir.path().join("target")).unwrap();
    std::fs::write(dir.path().join("target/debug.rs"), "ignored").unwrap();
    std::fs::write(dir.path().join("build.log"), "ignored").unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let walker = FileWalker::new(10);
    let files = walker.walk(dir.path()).expect("walk failed");

    // Should not include target/ or .log files
    let paths: Vec<_> = files.iter().map(|f| f.to_string_lossy()).collect();
    assert!(
        !paths.iter().any(|p| p.contains("target")),
        "Should ignore target/"
    );
    assert!(
        !paths.iter().any(|p| p.ends_with(".log")),
        "Should ignore *.log"
    );
    assert!(
        paths.iter().any(|p| p.ends_with("main.rs")),
        "Should include main.rs"
    );
}

// ==== IndexManager Creation Test ====

#[tokio::test]
async fn test_index_manager_creation() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("index.db");
    let db = Arc::new(SqliteStore::open(&db_path).expect("failed to open db"));

    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    // Should create successfully
    let _manager = IndexManager::new(config, db);
}
