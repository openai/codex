//! Structured error types for the retrieval system.
//!
//! Design principles:
//! - Each variant contains sufficient context information
//! - Avoid String-based errors (e.g., `Storage(String)`)
//! - Support From conversion for common dependency errors
//!
//! At core boundaries, convert to `CodexErr::Fatal(e.to_string())`

use std::path::PathBuf;
use thiserror::Error;

/// Structured error type for retrieval operations.
#[derive(Error, Debug)]
pub enum RetrievalErr {
    // Storage errors
    #[error("LanceDB connection failed: uri={uri}, cause={cause}")]
    LanceDbConnectionFailed { uri: String, cause: String },

    #[error("LanceDB query failed: table={table}, cause={cause}")]
    LanceDbQueryFailed { table: String, cause: String },

    #[error("SQLite lock timeout: path={path:?}, waited={waited_ms}ms")]
    SqliteLockedTimeout { path: PathBuf, waited_ms: u64 },

    #[error("SQLite error: path={path:?}, cause={cause}")]
    SqliteError { path: PathBuf, cause: String },

    #[error("SQLite operation failed: operation={operation}, cause={cause}")]
    SqliteFailed { operation: String, cause: String },

    // Indexing errors
    #[error(
        "Indexing already in progress: workspace={workspace}, phase={phase}, running for {started_secs_ago}s"
    )]
    IndexingInProgress {
        workspace: String,
        phase: String,
        started_secs_ago: i64,
    },

    #[error("Index corrupted: workspace={workspace}, reason={reason}")]
    IndexCorrupted { workspace: String, reason: String },

    #[error("Content hash mismatch: expected={expected}, actual={actual}")]
    ContentHashMismatch { expected: String, actual: String },

    #[error("File not indexable: path={path:?}, reason={reason}")]
    FileNotIndexable { path: PathBuf, reason: String },

    #[error("File read failed: path={path:?}, cause={cause}")]
    FileReadFailed { path: PathBuf, cause: String },

    #[error("Unsupported language: extension={extension}")]
    UnsupportedLanguage { extension: String },

    #[error("Tag extraction failed: {cause}")]
    TagExtractionFailed { cause: String },

    // Search errors
    #[error("Search failed: query={query}, cause={cause}")]
    SearchFailed { query: String, cause: String },

    #[error("Embedding dimension mismatch: expected={expected}, actual={actual}")]
    EmbeddingDimensionMismatch { expected: i32, actual: i32 },

    // Embedding errors
    #[error("Embedding failed: {cause}")]
    EmbeddingFailed { cause: String },

    // Reranker errors
    #[error("Reranker error: provider={provider}, cause={cause}")]
    RerankerError { provider: String, cause: String },

    #[error("Reranker API error: provider={provider}, status={status}, body={body}")]
    RerankerApiError {
        provider: String,
        status: u16,
        body: String,
    },

    // Feature errors
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(String),

    // Config errors
    #[error("Config error: field={field}, cause={cause}")]
    ConfigError { field: String, cause: String },

    #[error("Config file parse error: path={path:?}, cause={cause}")]
    ConfigParseError { path: PathBuf, cause: String },

    #[error("Retrieval is not enabled. Create ~/.codex/retrieval.toml or .codex/retrieval.toml")]
    NotEnabled,

    /// Index is not ready yet (building or not initialized).
    /// Caller should retry after a short delay.
    #[error("Index not ready: workspace={workspace}, reason={reason}. Please retry later.")]
    NotReady { workspace: String, reason: String },

    /// Chunk limit exceeded - prevents runaway indexing on large codebases.
    #[error("Chunk limit exceeded: {current} chunks (limit: {limit}). {hint}")]
    ChunkLimitExceeded {
        current: i64,
        limit: i64,
        hint: String,
    },

    // Generic errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: context={context}, cause={cause}")]
    JsonParse { context: String, cause: String },

    #[error("Tokio join error: {0}")]
    TokioJoin(#[from] tokio::task::JoinError),
}

/// Result type alias for retrieval operations.
pub type Result<T> = std::result::Result<T, RetrievalErr>;

// Convert from rusqlite errors.
// NOTE: This loses path context. Prefer explicit map_err with path when available.
impl From<rusqlite::Error> for RetrievalErr {
    fn from(e: rusqlite::Error) -> Self {
        Self::SqliteError {
            path: PathBuf::new(),
            cause: e.to_string(),
        }
    }
}

impl RetrievalErr {
    /// Create a SQLite error with path context.
    pub fn sqlite_error(path: &std::path::Path, e: rusqlite::Error) -> Self {
        Self::SqliteError {
            path: path.to_path_buf(),
            cause: e.to_string(),
        }
    }

    /// Create a JSON parse error with context.
    pub fn json_parse(context: &str, e: impl std::fmt::Display) -> Self {
        Self::JsonParse {
            context: context.to_string(),
            cause: e.to_string(),
        }
    }

    /// Create a NotReady error for index not initialized.
    pub fn index_not_initialized(workspace: &str) -> Self {
        Self::NotReady {
            workspace: workspace.to_string(),
            reason: "index not initialized".to_string(),
        }
    }

    /// Create a NotReady error for index building in progress.
    pub fn index_building(workspace: &str) -> Self {
        Self::NotReady {
            workspace: workspace.to_string(),
            reason: "index building in progress".to_string(),
        }
    }

    /// Check if this error is retryable.
    ///
    /// Returns true for transient errors where retry may succeed:
    /// - NotReady (index building)
    /// - SqliteLockedTimeout (concurrent access)
    /// - IndexingInProgress (another indexing job running)
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::NotReady { .. }
                | Self::SqliteLockedTimeout { .. }
                | Self::IndexingInProgress { .. }
        )
    }

    /// Suggested retry delay in milliseconds for retryable errors.
    pub fn suggested_retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::NotReady { .. } => Some(1000),           // 1 second
            Self::SqliteLockedTimeout { .. } => Some(100), // 100ms
            Self::IndexingInProgress { .. } => Some(5000), // 5 seconds
            _ => None,
        }
    }
}
