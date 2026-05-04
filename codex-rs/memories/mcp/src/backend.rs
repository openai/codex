use serde::Serialize;
use std::future::Future;

pub const DEFAULT_LIST_MAX_RESULTS: usize = 2_000;
pub const MAX_LIST_RESULTS: usize = 2_000;
pub const DEFAULT_SEARCH_MAX_RESULTS: usize = 200;
pub const MAX_SEARCH_RESULTS: usize = 200;
pub const DEFAULT_READ_MAX_TOKENS: usize = 20_000;

/// Storage interface behind the memories MCP tools.
///
/// Implementations should return paths relative to the memory store and enforce
/// their own storage-specific access rules. The local implementation uses the
/// filesystem today; a later implementation can satisfy the same contract from a
/// remote backend.
pub trait MemoriesBackend: Clone + Send + Sync + 'static {
    fn list(
        &self,
        request: ListMemoriesRequest,
    ) -> impl Future<Output = Result<ListMemoriesResponse, MemoriesBackendError>> + Send;

    fn read(
        &self,
        request: ReadMemoryRequest,
    ) -> impl Future<Output = Result<ReadMemoryResponse, MemoriesBackendError>> + Send;

    fn search(
        &self,
        request: SearchMemoriesRequest,
    ) -> impl Future<Output = Result<SearchMemoriesResponse, MemoriesBackendError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListMemoriesRequest {
    pub path: Option<String>,
    pub cursor: Option<String>,
    pub max_results: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListMemoriesResponse {
    pub path: Option<String>,
    pub entries: Vec<MemoryEntry>,
    pub next_cursor: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadMemoryRequest {
    pub path: String,
    pub line_offset: usize,
    pub max_lines: Option<usize>,
    pub max_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadMemoryResponse {
    pub path: String,
    pub start_line_number: usize,
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMemoriesRequest {
    pub query: String,
    pub path: Option<String>,
    pub max_results: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchMemoriesResponse {
    pub query: String,
    pub path: Option<String>,
    pub matches: Vec<MemorySearchMatch>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemoryEntry {
    pub path: String,
    pub entry_type: MemoryEntryType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEntryType {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MemorySearchMatch {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MemoriesBackendError {
    #[error("path '{path}' {reason}")]
    InvalidPath { path: String, reason: String },
    #[error("cursor '{cursor}' {reason}")]
    InvalidCursor { cursor: String, reason: String },
    #[error("line_offset must be a 1-indexed line number")]
    InvalidLineOffset,
    #[error("max_lines must be a positive integer")]
    InvalidMaxLines,
    #[error("line_offset exceeds file length")]
    LineOffsetExceedsFileLength,
    #[error("path '{path}' is not a file")]
    NotFile { path: String },
    #[error("query must not be empty")]
    EmptyQuery,
    #[error("I/O error while reading memories: {0}")]
    Io(#[from] std::io::Error),
}

impl MemoriesBackendError {
    pub fn invalid_path(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidPath {
            path: path.into(),
            reason: reason.into(),
        }
    }

    pub fn invalid_cursor(cursor: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidCursor {
            cursor: cursor.into(),
            reason: reason.into(),
        }
    }
}
