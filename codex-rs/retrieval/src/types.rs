//! Core data types for the retrieval system.

use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use std::path::Path;
use std::path::PathBuf;

/// Source file unique identifier.
///
/// Uses SHA256 content hash (first 16 chars) to detect changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceFileId {
    /// File path (relative to workspace)
    pub path: PathBuf,
    /// Detected programming language
    pub language: String,
    /// SHA256 content hash (first 16 characters)
    pub content_hash: String,
}

impl SourceFileId {
    /// Compute source file ID from path and content.
    pub fn compute(path: &Path, content: &str) -> Self {
        let hash = Sha256::digest(content.as_bytes());
        Self {
            path: path.to_path_buf(),
            language: detect_language(path).unwrap_or_default(),
            content_hash: format!("{:x}", hash)[..16].to_string(),
        }
    }
}

/// Detect programming language from file extension.
pub fn detect_language(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext {
            "rs" => "rust",
            "go" => "go",
            "py" => "python",
            "java" => "java",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" => "javascript",
            "c" | "h" => "c",
            "cpp" | "cc" | "cxx" | "hpp" => "cpp",
            "cs" => "csharp",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" | "kts" => "kotlin",
            "scala" => "scala",
            "lua" => "lua",
            "sh" | "bash" => "bash",
            "sql" => "sql",
            "md" => "markdown",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "xml" => "xml",
            "html" | "htm" => "html",
            "css" => "css",
            _ => ext,
        })
        .map(String::from)
}

/// Code chunk - a segment of source code.
///
/// Extended with metadata fields for tweakcc indexing support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeChunk {
    /// Unique ID: "{workspace}:{filepath}:{chunk_idx}"
    pub id: String,
    /// Workspace identifier
    pub source_id: String,
    /// Relative file path
    pub filepath: String,
    /// Programming language
    pub language: String,
    /// Chunk content
    pub content: String,
    /// Start line number (1-indexed)
    pub start_line: i32,
    /// End line number (1-indexed)
    pub end_line: i32,
    /// Optional embedding vector
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    /// File modification time (Unix timestamp in seconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub modified_time: Option<i64>,
    // Extended metadata fields for tweakcc indexing
    /// Workspace identifier (same as source_id for backward compatibility)
    #[serde(default)]
    pub workspace: String,
    /// Content hash for change detection
    #[serde(default)]
    pub content_hash: String,
    /// Index timestamp (Unix timestamp in seconds)
    #[serde(default)]
    pub indexed_at: i64,
    /// Parent symbol context (e.g., "class UserService" or "impl UserRepo")
    ///
    /// Provides class/struct context for methods embedded inside larger structures.
    /// This allows embedding models to understand the full semantic context.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub parent_symbol: Option<String>,
    /// Whether this is an overview chunk (class/struct structure with collapsed method bodies)
    ///
    /// Overview chunks show the structure of a class/struct with method signatures
    /// but bodies collapsed to `{ ... }`. This helps search queries like
    /// "what methods does UserService have" return a single comprehensive result.
    #[serde(default)]
    pub is_overview: bool,
}

impl CodeChunk {
    /// Prepare content for embedding with filepath and parent symbol context.
    ///
    /// Wraps the chunk content with filepath and optional parent symbol information
    /// so that embeddings can understand the full context. This is inspired by
    /// Continue's approach of adding class headers to method chunks.
    ///
    /// Format without parent: ```{filepath}\n{content}\n```
    /// Format with parent: ```{filepath}\n{parent_symbol} ...\n\n{content}\n```
    ///
    /// This helps the embedding model understand that:
    /// - Code from test files relates to testing
    /// - Code from specific directories has certain purposes
    /// - Methods belong to their parent class/struct/impl
    /// - Similar code in different files may have different contexts
    pub fn embedding_content(&self) -> String {
        match &self.parent_symbol {
            Some(parent) => {
                format!(
                    "```{}\n{} ...\n\n{}\n```",
                    self.filepath, parent, self.content
                )
            }
            None => {
                format!("```{}\n{}\n```", self.filepath, self.content)
            }
        }
    }
}

/// Wrap content with filepath context for embedding.
///
/// Use this when preparing code snippets for embedding generation.
/// The format matches what `CodeChunk::embedding_content()` produces.
pub fn wrap_content_for_embedding(filepath: &str, content: &str) -> String {
    format!("```{filepath}\n{content}\n```")
}

/// Chunk reference - stores file location instead of content.
///
/// Unlike `CodeChunk`, this struct does NOT store the actual code content.
/// Instead, it stores a reference (filepath + line range) and reads fresh
/// content from the file system on demand via `read_content()`.
///
/// **Benefits over CodeChunk:**
/// - Always returns current file content (no staleness)
/// - Less storage (no code duplication)
/// - Consistent with agent's file operations
///
/// **Industry practice:** Continue Dev, Cursor, GitHub Copilot all use this approach.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRef {
    /// Unique ID: "{workspace}:{filepath}:{chunk_idx}"
    pub id: String,
    /// Workspace identifier
    pub source_id: String,
    /// Relative file path
    pub filepath: String,
    /// Programming language
    pub language: String,
    /// Start line number (1-indexed)
    pub start_line: i32,
    /// End line number (1-indexed)
    pub end_line: i32,
    /// Optional embedding vector
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    /// Workspace identifier
    #[serde(default)]
    pub workspace: String,
    /// Content hash for staleness detection (SHA256 of original content)
    #[serde(default)]
    pub content_hash: String,
    /// Index timestamp (Unix timestamp in seconds)
    #[serde(default)]
    pub indexed_at: i64,
    /// Parent symbol context (e.g., "class UserService" or "impl UserRepo")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub parent_symbol: Option<String>,
    /// Whether this is an overview chunk
    #[serde(default)]
    pub is_overview: bool,
}

/// Result of reading chunk content from file system.
#[derive(Debug, Clone)]
pub struct HydratedChunk {
    /// The chunk reference
    pub chunk_ref: ChunkRef,
    /// Fresh content read from file
    pub content: String,
    /// Whether content matches the indexed hash (false = file was modified)
    pub is_fresh: bool,
}

impl ChunkRef {
    /// Read fresh content from file system.
    ///
    /// Reads lines `start_line..end_line` from the file and validates
    /// against the stored content_hash to detect staleness.
    ///
    /// # Arguments
    /// * `workspace_root` - Root directory of the workspace
    ///
    /// # Returns
    /// * `Ok(HydratedChunk)` - Content read successfully, `is_fresh` indicates hash match
    /// * `Err` - File not found or read error
    pub fn read_content(&self, workspace_root: &Path) -> std::io::Result<HydratedChunk> {
        let file_path = workspace_root.join(&self.filepath);
        let file_content = std::fs::read_to_string(&file_path)?;
        let lines: Vec<&str> = file_content.lines().collect();

        // Convert 1-indexed lines to 0-indexed array indices
        let start_idx = (self.start_line - 1).max(0) as usize;
        let end_idx = (self.end_line as usize).min(lines.len());

        let content = if start_idx < lines.len() {
            lines[start_idx..end_idx].join("\n")
        } else {
            String::new()
        };

        // Check if content matches stored hash
        // Handle both 16-char (SourceFileId format) and 64-char (full SHA256) hashes
        let current_hash = compute_chunk_hash(&content);
        let is_fresh =
            self.content_hash.is_empty() || hashes_match(&current_hash, &self.content_hash);

        Ok(HydratedChunk {
            chunk_ref: self.clone(),
            content,
            is_fresh,
        })
    }

    /// Prepare content for embedding with filepath and parent symbol context.
    ///
    /// Note: This reads the file synchronously. For async operations,
    /// use `read_content()` and then `wrap_content_for_embedding()`.
    pub fn embedding_content(&self, workspace_root: &Path) -> std::io::Result<String> {
        let hydrated = self.read_content(workspace_root)?;
        Ok(match &self.parent_symbol {
            Some(parent) => {
                format!(
                    "```{}\n{} ...\n\n{}\n```",
                    self.filepath, parent, hydrated.content
                )
            }
            None => {
                format!("```{}\n{}\n```", self.filepath, hydrated.content)
            }
        })
    }

    /// Convert to CodeChunk by reading content from file.
    ///
    /// Use this for backward compatibility with code that expects CodeChunk.
    pub fn to_code_chunk(&self, workspace_root: &Path) -> std::io::Result<CodeChunk> {
        let hydrated = self.read_content(workspace_root)?;
        Ok(CodeChunk {
            id: self.id.clone(),
            source_id: self.source_id.clone(),
            filepath: self.filepath.clone(),
            language: self.language.clone(),
            content: hydrated.content,
            start_line: self.start_line,
            end_line: self.end_line,
            embedding: self.embedding.clone(),
            modified_time: None,
            workspace: self.workspace.clone(),
            content_hash: self.content_hash.clone(),
            indexed_at: self.indexed_at,
            parent_symbol: self.parent_symbol.clone(),
            is_overview: self.is_overview,
        })
    }
}

impl From<CodeChunk> for ChunkRef {
    /// Convert CodeChunk to ChunkRef (drops content).
    fn from(chunk: CodeChunk) -> Self {
        Self {
            id: chunk.id,
            source_id: chunk.source_id,
            filepath: chunk.filepath,
            language: chunk.language,
            start_line: chunk.start_line,
            end_line: chunk.end_line,
            embedding: chunk.embedding,
            workspace: chunk.workspace,
            content_hash: chunk.content_hash,
            indexed_at: chunk.indexed_at,
            parent_symbol: chunk.parent_symbol,
            is_overview: chunk.is_overview,
        }
    }
}

impl From<&CodeChunk> for ChunkRef {
    /// Convert &CodeChunk to ChunkRef (clones required fields, drops content).
    fn from(chunk: &CodeChunk) -> Self {
        Self {
            id: chunk.id.clone(),
            source_id: chunk.source_id.clone(),
            filepath: chunk.filepath.clone(),
            language: chunk.language.clone(),
            start_line: chunk.start_line,
            end_line: chunk.end_line,
            embedding: chunk.embedding.clone(),
            workspace: chunk.workspace.clone(),
            content_hash: chunk.content_hash.clone(),
            indexed_at: chunk.indexed_at,
            parent_symbol: chunk.parent_symbol.clone(),
            is_overview: chunk.is_overview,
        }
    }
}

/// Code tag extracted by tree-sitter-tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeTag {
    /// Symbol name (function, class, etc.)
    pub name: String,
    /// Syntax type
    pub syntax_type: SyntaxType,
    /// Start line number
    pub start_line: i32,
    /// End line number
    pub end_line: i32,
    /// Function signature (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Documentation comment (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
}

/// Syntax type for code symbols.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyntaxType {
    Function,
    Method,
    Class,
    Struct,
    Trait,
    Interface,
    Enum,
    Constant,
    Variable,
}

/// Search result with scoring information.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matched code chunk
    pub chunk: CodeChunk,
    /// Relevance score
    pub score: f32,
    /// Score type (how it was computed)
    pub score_type: ScoreType,
    /// Whether the content is stale (file modified since indexing).
    ///
    /// None = freshness not checked (no hydration performed)
    /// Some(true) = content was stale but hydration refreshed it
    /// Some(false) = content is fresh
    pub is_stale: Option<bool>,
}

/// Type of score for search results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreType {
    /// BM25 full-text search score
    Bm25,
    /// Vector similarity score
    Vector,
    /// Hybrid (RRF fused) score
    Hybrid,
    /// Snippet exact match score
    Snippet,
    /// Recently accessed file score
    Recent,
}

impl std::fmt::Display for ScoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScoreType::Bm25 => write!(f, "BM25"),
            ScoreType::Vector => write!(f, "Vector"),
            ScoreType::Hybrid => write!(f, "Hybrid"),
            ScoreType::Snippet => write!(f, "Snippet"),
            ScoreType::Recent => write!(f, "Recent"),
        }
    }
}

/// Index tag for workspace tracking.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IndexTag {
    /// Workspace identifier
    pub workspace: String,
}

/// Chunk span with line number information.
#[derive(Debug, Clone)]
pub struct ChunkSpan {
    /// Chunk content
    pub content: String,
    /// Start line number (1-indexed, matches CodeChunk)
    pub start_line: i32,
    /// End line number (1-indexed, matches CodeChunk)
    pub end_line: i32,
    /// Whether this is an overview chunk
    pub is_overview: bool,
}

/// Indexed file with metadata.
#[derive(Debug, Clone)]
pub struct IndexedFile {
    /// File path
    pub path: PathBuf,
    /// Content hash
    pub content_hash: String,
    /// Language
    pub language: String,
    /// Number of chunks created
    pub chunks_count: i32,
    /// Number of chunks that failed to process
    pub chunks_failed: i32,
    /// Modification time
    pub mtime: i64,
    /// Index timestamp
    pub indexed_at: i64,
}

/// Default maximum results to return.
pub const DEFAULT_N_FINAL: i32 = 20;

/// Default tokens per chunk for context budget calculation.
pub const DEFAULT_TOKENS_PER_CHUNK: i32 = 512;

/// Search query with options.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Query text
    pub text: String,
    /// Maximum results to return
    pub limit: i32,
    /// Workspace filter
    pub workspace: Option<String>,
    /// Path prefix filter
    pub path_filter: Option<Vec<String>>,
    /// Language filter
    pub language_filter: Option<Vec<String>>,
    /// Optional context length (tokens) for dynamic result limiting.
    ///
    /// When provided, the search will dynamically adjust the number of results
    /// to fit within ~50% of the context window, reserving space for reasoning.
    pub context_length: Option<i32>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            limit: DEFAULT_N_FINAL,
            workspace: None,
            path_filter: None,
            language_filter: None,
            context_length: None,
        }
    }
}

/// Compute SHA256 hash of chunk content for embedding cache.
///
/// Returns the full 64-character hex string to avoid key collisions.
/// Used for cache lookup: `cache.get(filepath, chunk_hash, artifact_id)`.
pub fn compute_chunk_hash(content: &str) -> String {
    let hash = Sha256::digest(content.as_bytes());
    format!("{:x}", hash)
}

/// Compare hashes that may have different lengths.
///
/// Supports comparing:
/// - 16-char truncated hashes (from SourceFileId)
/// - 64-char full SHA256 hashes (from compute_chunk_hash)
///
/// If lengths differ, compares the shorter prefix of the longer hash.
fn hashes_match(hash_a: &str, hash_b: &str) -> bool {
    if hash_a.len() == hash_b.len() {
        hash_a == hash_b
    } else {
        // Compare using the shorter length
        let min_len = hash_a.len().min(hash_b.len());
        hash_a[..min_len] == hash_b[..min_len]
    }
}

/// Calculate the optimal number of results based on available context.
///
/// Returns the smaller of `DEFAULT_N_FINAL` or what fits in 50% of the context.
pub fn calculate_n_final(context_length: Option<i32>) -> i32 {
    match context_length {
        Some(ctx_len) if ctx_len > 0 => {
            // Reserve 50% of context for reasoning
            let max_retrieval_tokens = ctx_len / 2;
            let n = max_retrieval_tokens / DEFAULT_TOKENS_PER_CHUNK;
            n.min(DEFAULT_N_FINAL).max(1)
        }
        _ => DEFAULT_N_FINAL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test chunk with default metadata.
    fn make_test_chunk(id: &str, filepath: &str, content: &str) -> CodeChunk {
        CodeChunk {
            id: id.to_string(),
            source_id: "test".to_string(),
            filepath: filepath.to_string(),
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

    /// Helper to create a test chunk with parent symbol context.
    fn make_test_chunk_with_parent(
        id: &str,
        filepath: &str,
        content: &str,
        parent: &str,
    ) -> CodeChunk {
        CodeChunk {
            id: id.to_string(),
            source_id: "test".to_string(),
            filepath: filepath.to_string(),
            language: "rust".to_string(),
            content: content.to_string(),
            start_line: 1,
            end_line: 3,
            embedding: None,
            modified_time: None,
            workspace: "test".to_string(),
            content_hash: String::new(),
            indexed_at: 0,
            parent_symbol: Some(parent.to_string()),
            is_overview: false,
        }
    }

    #[test]
    fn test_code_chunk_embedding_content() {
        let chunk = make_test_chunk(
            "test:src/main.rs:0",
            "src/main.rs",
            "fn main() {\n    println!(\"Hello\");\n}",
        );

        let embedding_content = chunk.embedding_content();
        assert!(embedding_content.starts_with("```src/main.rs\n"));
        assert!(embedding_content.ends_with("\n```"));
        assert!(embedding_content.contains("fn main()"));
    }

    #[test]
    fn test_code_chunk_embedding_content_test_file() {
        // Test that test files are properly wrapped
        let chunk = make_test_chunk(
            "test:tests/integration.rs:0",
            "tests/integration.rs",
            "#[test]\nfn test_something() {}",
        );

        let embedding_content = chunk.embedding_content();
        assert!(embedding_content.starts_with("```tests/integration.rs\n"));
        // The embedding model can now understand this is test code
    }

    #[test]
    fn test_wrap_content_for_embedding() {
        let content = wrap_content_for_embedding("src/lib.rs", "pub fn foo() {}");
        assert_eq!(content, "```src/lib.rs\npub fn foo() {}\n```");
    }

    #[test]
    fn test_wrap_content_preserves_multiline() {
        let content =
            wrap_content_for_embedding("src/utils.rs", "fn helper() {\n    // do something\n}");
        assert_eq!(
            content,
            "```src/utils.rs\nfn helper() {\n    // do something\n}\n```"
        );
    }

    #[test]
    fn test_code_chunk_with_metadata() {
        let chunk = CodeChunk {
            id: "ws:file.rs:0".to_string(),
            source_id: "ws".to_string(),
            filepath: "file.rs".to_string(),
            language: "rust".to_string(),
            content: "fn test() {}".to_string(),
            start_line: 1,
            end_line: 1,
            embedding: None,
            modified_time: Some(1700000000),
            workspace: "ws".to_string(),
            content_hash: "abc123".to_string(),
            indexed_at: 1700000100,
            parent_symbol: None,
            is_overview: false,
        };

        assert_eq!(chunk.workspace, "ws");
        assert_eq!(chunk.content_hash, "abc123");
        assert_eq!(chunk.indexed_at, 1700000100);
    }

    #[test]
    fn test_embedding_content_with_parent_symbol() {
        // Test method inside a class/impl
        let chunk = make_test_chunk_with_parent(
            "test:src/user_service.rs:0",
            "src/user_service.rs",
            "fn get_user(&self, id: i64) -> User {\n    self.repo.find(id)\n}",
            "impl UserService",
        );

        let embedding_content = chunk.embedding_content();
        assert!(embedding_content.starts_with("```src/user_service.rs\nimpl UserService ..."));
        assert!(embedding_content.contains("fn get_user(&self"));
        assert!(embedding_content.ends_with("\n```"));
    }

    #[test]
    fn test_embedding_content_without_parent_symbol() {
        // Test top-level function
        let chunk = make_test_chunk(
            "test:src/main.rs:0",
            "src/main.rs",
            "fn main() {\n    println!(\"Hello\");\n}",
        );

        let embedding_content = chunk.embedding_content();
        // Should not have the "..." parent marker
        assert!(!embedding_content.contains("..."));
        assert!(embedding_content.starts_with("```src/main.rs\nfn main()"));
    }

    #[test]
    fn test_calculate_n_final_none() {
        // No context length -> use default
        assert_eq!(calculate_n_final(None), DEFAULT_N_FINAL);
    }

    #[test]
    fn test_calculate_n_final_large_context() {
        // 128k tokens = 64k for retrieval / 512 per chunk = 125 chunks
        // But capped at DEFAULT_N_FINAL (20)
        assert_eq!(calculate_n_final(Some(128_000)), DEFAULT_N_FINAL);
    }

    #[test]
    fn test_calculate_n_final_small_context() {
        // 4k tokens = 2k for retrieval / 512 per chunk = 3 chunks
        assert_eq!(calculate_n_final(Some(4_000)), 3);
    }

    #[test]
    fn test_calculate_n_final_very_small_context() {
        // 512 tokens = 256 for retrieval / 512 per chunk = 0, but min is 1
        assert_eq!(calculate_n_final(Some(512)), 1);
    }

    #[test]
    fn test_calculate_n_final_zero_context() {
        // Zero context -> use default
        assert_eq!(calculate_n_final(Some(0)), DEFAULT_N_FINAL);
    }

    #[test]
    fn test_calculate_n_final_negative_context() {
        // Negative context -> use default
        assert_eq!(calculate_n_final(Some(-1000)), DEFAULT_N_FINAL);
    }

    #[test]
    fn test_search_query_with_context_length() {
        let query = SearchQuery {
            text: "test query".to_string(),
            limit: calculate_n_final(Some(8000)),
            context_length: Some(8000),
            ..Default::default()
        };
        // 8000 tokens = 4000 for retrieval / 512 = 7 chunks
        assert_eq!(query.limit, 7);
        assert_eq!(query.context_length, Some(8000));
    }

    #[test]
    fn test_compute_chunk_hash() {
        let hash1 = compute_chunk_hash("fn main() {}");
        let hash2 = compute_chunk_hash("fn main() {}");
        let hash3 = compute_chunk_hash("fn main() { }"); // different content

        // Same content = same hash
        assert_eq!(hash1, hash2);
        // Different content = different hash
        assert_ne!(hash1, hash3);
        // Full 64-char SHA256 hex
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_hashes_match() {
        let full_hash = compute_chunk_hash("fn main() {}");
        let short_hash = &full_hash[..16]; // 16-char truncated hash

        // Same length hashes
        assert!(hashes_match(&full_hash, &full_hash));
        assert!(hashes_match(short_hash, short_hash));

        // Different length hashes (16 vs 64 chars)
        assert!(hashes_match(&full_hash, short_hash));
        assert!(hashes_match(short_hash, &full_hash));

        // Non-matching hashes
        let other_hash = compute_chunk_hash("fn bar() {}");
        assert!(!hashes_match(&full_hash, &other_hash));
        assert!(!hashes_match(&full_hash[..16], &other_hash[..16]));
    }

    #[test]
    fn test_chunk_ref_staleness_with_short_hash() {
        use std::io::Write;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "fn foo() {{}}").unwrap();

        // Use 16-char hash (SourceFileId format)
        let expected_content = "fn foo() {}";
        let full_hash = compute_chunk_hash(expected_content);
        let short_hash = full_hash[..16].to_string();

        let chunk_ref = ChunkRef {
            id: "test:test.rs:0".to_string(),
            source_id: "test".to_string(),
            filepath: "test.rs".to_string(),
            language: "rust".to_string(),
            start_line: 1,
            end_line: 1,
            embedding: None,
            workspace: "test".to_string(),
            content_hash: short_hash, // 16-char hash
            indexed_at: 0,
            parent_symbol: None,
            is_overview: false,
        };

        let hydrated = chunk_ref.read_content(dir.path()).unwrap();
        assert!(
            hydrated.is_fresh,
            "16-char hash should match 64-char computed hash"
        );

        // Modify file
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "fn bar() {{}}").unwrap();

        let hydrated = chunk_ref.read_content(dir.path()).unwrap();
        assert!(
            !hydrated.is_fresh,
            "Should detect stale content with 16-char hash"
        );
    }

    #[test]
    fn test_chunk_ref_read_content() {
        use std::io::Write;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "line1").unwrap();
        writeln!(file, "line2").unwrap();
        writeln!(file, "line3").unwrap();
        writeln!(file, "line4").unwrap();

        let chunk_ref = ChunkRef {
            id: "test:test.rs:0".to_string(),
            source_id: "test".to_string(),
            filepath: "test.rs".to_string(),
            language: "rust".to_string(),
            start_line: 2,
            end_line: 3,
            embedding: None,
            workspace: "test".to_string(),
            content_hash: String::new(),
            indexed_at: 0,
            parent_symbol: None,
            is_overview: false,
        };

        let hydrated = chunk_ref.read_content(dir.path()).unwrap();
        assert_eq!(hydrated.content, "line2\nline3");
        assert!(hydrated.is_fresh); // Empty hash = always fresh
    }

    #[test]
    fn test_chunk_ref_staleness_detection() {
        use std::io::Write;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "fn foo() {{}}").unwrap();

        // Compute hash of expected content
        let expected_content = "fn foo() {}";
        let expected_hash = compute_chunk_hash(expected_content);

        let chunk_ref = ChunkRef {
            id: "test:test.rs:0".to_string(),
            source_id: "test".to_string(),
            filepath: "test.rs".to_string(),
            language: "rust".to_string(),
            start_line: 1,
            end_line: 1,
            embedding: None,
            workspace: "test".to_string(),
            content_hash: expected_hash,
            indexed_at: 0,
            parent_symbol: None,
            is_overview: false,
        };

        let hydrated = chunk_ref.read_content(dir.path()).unwrap();
        assert!(hydrated.is_fresh);

        // Now modify the file
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "fn bar() {{}}").unwrap();

        let hydrated = chunk_ref.read_content(dir.path()).unwrap();
        assert!(!hydrated.is_fresh); // Content changed, hash doesn't match
    }

    #[test]
    fn test_chunk_ref_to_code_chunk() {
        use std::io::Write;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("main.rs");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "fn main() {{").unwrap();
        writeln!(file, "    println!(\"hello\");").unwrap();
        writeln!(file, "}}").unwrap();

        let chunk_ref = ChunkRef {
            id: "test:main.rs:0".to_string(),
            source_id: "test".to_string(),
            filepath: "main.rs".to_string(),
            language: "rust".to_string(),
            start_line: 1,
            end_line: 3,
            embedding: None,
            workspace: "test".to_string(),
            content_hash: String::new(),
            indexed_at: 12345,
            parent_symbol: Some("mod main".to_string()),
            is_overview: false,
        };

        let code_chunk = chunk_ref.to_code_chunk(dir.path()).unwrap();
        assert_eq!(code_chunk.id, "test:main.rs:0");
        assert_eq!(code_chunk.filepath, "main.rs");
        assert!(code_chunk.content.contains("fn main()"));
        assert_eq!(code_chunk.parent_symbol, Some("mod main".to_string()));
    }

    #[test]
    fn test_code_chunk_to_chunk_ref() {
        let chunk = CodeChunk {
            id: "ws:file.rs:0".to_string(),
            source_id: "ws".to_string(),
            filepath: "file.rs".to_string(),
            language: "rust".to_string(),
            content: "fn test() {}".to_string(),
            start_line: 1,
            end_line: 1,
            embedding: Some(vec![0.1, 0.2]),
            modified_time: Some(1700000000),
            workspace: "ws".to_string(),
            content_hash: "abc123".to_string(),
            indexed_at: 1700000100,
            parent_symbol: Some("impl Foo".to_string()),
            is_overview: false,
        };

        let chunk_ref: ChunkRef = chunk.into();
        assert_eq!(chunk_ref.id, "ws:file.rs:0");
        assert_eq!(chunk_ref.filepath, "file.rs");
        assert_eq!(chunk_ref.content_hash, "abc123");
        assert_eq!(chunk_ref.embedding, Some(vec![0.1, 0.2]));
        assert_eq!(chunk_ref.parent_symbol, Some("impl Foo".to_string()));
        // Note: content is NOT in ChunkRef
    }
}
