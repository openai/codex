//! Structured event protocol for the retrieval system.
//!
//! Provides a unified event system that all operations emit, allowing consumers
//! (TUI, CLI, external tools) to adapt to the event stream.
//!
//! # Event Flow
//!
//! ```text
//! Operation (search/index/repomap)
//!     │
//!     ▼
//! EventEmitter::emit(RetrievalEvent)
//!     │
//!     ├──► TuiConsumer (updates TUI state)
//!     ├──► CliConsumer (prints to stdout)
//!     ├──► JsonLinesConsumer (writes JSON-lines)
//!     └──► LoggingConsumer (writes to tracing)
//! ```

use std::collections::HashMap;
use std::fmt;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde::Serialize;

use crate::indexing::IndexStats;
use crate::indexing::RebuildMode;
use crate::types::ScoreType;
use crate::types::SyntaxType;

// ============================================================================
// Core Event Types
// ============================================================================

/// Unified event enum for all retrieval operations.
///
/// Events are designed to be:
/// - Serializable to JSON for external consumers
/// - Self-contained with all necessary context
/// - Timestamped for ordering and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RetrievalEvent {
    // ========================================================================
    // Lifecycle Events
    // ========================================================================
    /// Session started with configuration summary.
    SessionStarted {
        session_id: String,
        config: ConfigSummary,
    },

    /// Session ended.
    SessionEnded {
        session_id: String,
        duration_ms: i64,
    },

    // ========================================================================
    // Search Pipeline Events
    // ========================================================================
    /// Search operation started.
    SearchStarted {
        query_id: String,
        query: String,
        mode: SearchMode,
        limit: i32,
    },

    /// Query preprocessing completed.
    QueryPreprocessed {
        query_id: String,
        tokens: Vec<String>,
        language: String,
        duration_ms: i64,
    },

    /// Query rewriting completed.
    QueryRewritten {
        query_id: String,
        original: String,
        rewritten: String,
        expansions: Vec<String>,
        translated: bool,
        duration_ms: i64,
    },

    /// BM25 search started.
    Bm25SearchStarted {
        query_id: String,
        query_terms: Vec<String>,
    },

    /// BM25 search completed.
    Bm25SearchCompleted {
        query_id: String,
        results: Vec<ChunkSummary>,
        duration_ms: i64,
    },

    /// Vector search started.
    VectorSearchStarted {
        query_id: String,
        embedding_dim: i32,
    },

    /// Vector search completed.
    VectorSearchCompleted {
        query_id: String,
        results: Vec<ChunkSummary>,
        duration_ms: i64,
    },

    /// Snippet/symbol search started.
    SnippetSearchStarted {
        query_id: String,
        symbol_query: SymbolQueryInfo,
    },

    /// Snippet/symbol search completed.
    SnippetSearchCompleted {
        query_id: String,
        results: Vec<SnippetSummary>,
        duration_ms: i64,
    },

    /// RRF fusion started.
    FusionStarted {
        query_id: String,
        bm25_count: i32,
        vector_count: i32,
        snippet_count: i32,
    },

    /// RRF fusion completed.
    FusionCompleted {
        query_id: String,
        merged_count: i32,
        duration_ms: i64,
    },

    /// Reranking started.
    RerankingStarted {
        query_id: String,
        backend: String,
        input_count: i32,
    },

    /// Reranking completed.
    RerankingCompleted {
        query_id: String,
        adjustments: Vec<ScoreAdjustment>,
        duration_ms: i64,
    },

    /// Search operation completed successfully.
    SearchCompleted {
        query_id: String,
        results: Vec<SearchResultSummary>,
        total_duration_ms: i64,
    },

    /// Search operation failed.
    SearchError {
        query_id: String,
        error: String,
        retryable: bool,
    },

    // ========================================================================
    // Index Events
    // ========================================================================
    /// Index build started.
    IndexBuildStarted {
        workspace: String,
        mode: RebuildModeInfo,
        estimated_files: i32,
    },

    /// Index phase changed.
    IndexPhaseChanged {
        workspace: String,
        phase: IndexPhaseInfo,
        progress: f32,
        description: String,
    },

    /// Individual file processed.
    IndexFileProcessed {
        workspace: String,
        path: String,
        chunks: i32,
        status: FileProcessStatus,
    },

    /// Index build completed successfully.
    IndexBuildCompleted {
        workspace: String,
        stats: IndexStatsSummary,
        duration_ms: i64,
    },

    /// Index build failed.
    IndexBuildFailed { workspace: String, error: String },

    // ========================================================================
    // Watch Events
    // ========================================================================
    /// File watching started.
    WatchStarted {
        workspace: String,
        paths: Vec<String>,
    },

    /// File changed detected.
    FileChanged {
        workspace: String,
        path: String,
        kind: FileChangeKind,
    },

    /// Incremental index triggered by file changes.
    IncrementalIndexTriggered {
        workspace: String,
        changed_files: i32,
    },

    /// File watching stopped.
    WatchStopped { workspace: String },

    // ========================================================================
    // RepoMap Events
    // ========================================================================
    /// RepoMap generation started.
    RepoMapStarted {
        request_id: String,
        max_tokens: i32,
        chat_files: i32,
        other_files: i32,
    },

    /// PageRank computation completed.
    PageRankComputed {
        request_id: String,
        iterations: i32,
        top_files: Vec<RankedFileSummary>,
    },

    /// RepoMap generation completed.
    RepoMapGenerated {
        request_id: String,
        tokens: i32,
        files: i32,
        duration_ms: i64,
    },

    /// RepoMap cache hit.
    RepoMapCacheHit {
        request_id: String,
        cache_key: String,
    },

    // ========================================================================
    // Debug/Diagnostic Events
    // ========================================================================
    /// Diagnostic log entry.
    DiagnosticLog {
        level: LogLevel,
        module: String,
        message: String,
        #[serde(skip_serializing_if = "HashMap::is_empty")]
        fields: HashMap<String, serde_json::Value>,
    },
}

impl RetrievalEvent {
    /// Get the event type name.
    pub fn event_type(&self) -> &'static str {
        match self {
            RetrievalEvent::SessionStarted { .. } => "session_started",
            RetrievalEvent::SessionEnded { .. } => "session_ended",
            RetrievalEvent::SearchStarted { .. } => "search_started",
            RetrievalEvent::QueryPreprocessed { .. } => "query_preprocessed",
            RetrievalEvent::QueryRewritten { .. } => "query_rewritten",
            RetrievalEvent::Bm25SearchStarted { .. } => "bm25_search_started",
            RetrievalEvent::Bm25SearchCompleted { .. } => "bm25_search_completed",
            RetrievalEvent::VectorSearchStarted { .. } => "vector_search_started",
            RetrievalEvent::VectorSearchCompleted { .. } => "vector_search_completed",
            RetrievalEvent::SnippetSearchStarted { .. } => "snippet_search_started",
            RetrievalEvent::SnippetSearchCompleted { .. } => "snippet_search_completed",
            RetrievalEvent::FusionStarted { .. } => "fusion_started",
            RetrievalEvent::FusionCompleted { .. } => "fusion_completed",
            RetrievalEvent::RerankingStarted { .. } => "reranking_started",
            RetrievalEvent::RerankingCompleted { .. } => "reranking_completed",
            RetrievalEvent::SearchCompleted { .. } => "search_completed",
            RetrievalEvent::SearchError { .. } => "search_error",
            RetrievalEvent::IndexBuildStarted { .. } => "index_build_started",
            RetrievalEvent::IndexPhaseChanged { .. } => "index_phase_changed",
            RetrievalEvent::IndexFileProcessed { .. } => "index_file_processed",
            RetrievalEvent::IndexBuildCompleted { .. } => "index_build_completed",
            RetrievalEvent::IndexBuildFailed { .. } => "index_build_failed",
            RetrievalEvent::WatchStarted { .. } => "watch_started",
            RetrievalEvent::FileChanged { .. } => "file_changed",
            RetrievalEvent::IncrementalIndexTriggered { .. } => "incremental_index_triggered",
            RetrievalEvent::WatchStopped { .. } => "watch_stopped",
            RetrievalEvent::RepoMapStarted { .. } => "repomap_started",
            RetrievalEvent::PageRankComputed { .. } => "pagerank_computed",
            RetrievalEvent::RepoMapGenerated { .. } => "repomap_generated",
            RetrievalEvent::RepoMapCacheHit { .. } => "repomap_cache_hit",
            RetrievalEvent::DiagnosticLog { .. } => "diagnostic_log",
        }
    }

    /// Get current timestamp in milliseconds since Unix epoch.
    pub fn timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }

    /// Serialize to JSON line (with timestamp wrapper).
    pub fn to_json_line(&self) -> String {
        let wrapper = TimestampedEvent {
            timestamp: Self::timestamp(),
            event: self,
        };
        serde_json::to_string(&wrapper).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Wrapper for adding timestamp to events.
#[derive(Serialize)]
struct TimestampedEvent<'a> {
    timestamp: i64,
    #[serde(flatten)]
    event: &'a RetrievalEvent,
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Search mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    /// Hybrid search (BM25 + vector + snippet).
    #[default]
    Hybrid,
    /// BM25 full-text only.
    Bm25,
    /// Vector similarity only.
    Vector,
    /// Symbol/snippet search.
    Snippet,
}

impl fmt::Display for SearchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchMode::Hybrid => write!(f, "hybrid"),
            SearchMode::Bm25 => write!(f, "bm25"),
            SearchMode::Vector => write!(f, "vector"),
            SearchMode::Snippet => write!(f, "snippet"),
        }
    }
}

/// Configuration summary for session events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummary {
    pub enabled: bool,
    pub data_dir: String,
    pub bm25_enabled: bool,
    pub vector_enabled: bool,
    pub query_rewrite_enabled: bool,
    pub reranker_backend: Option<String>,
}

/// Chunk summary for search result events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkSummary {
    pub id: String,
    pub filepath: String,
    pub start_line: i32,
    pub end_line: i32,
    pub score: f32,
    pub language: String,
}

impl From<crate::types::SearchResult> for ChunkSummary {
    fn from(result: crate::types::SearchResult) -> Self {
        Self {
            id: result.chunk.id,
            filepath: result.chunk.filepath,
            start_line: result.chunk.start_line,
            end_line: result.chunk.end_line,
            score: result.score,
            language: result.chunk.language,
        }
    }
}

/// Snippet/symbol summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetSummary {
    pub name: String,
    pub filepath: String,
    pub start_line: i32,
    pub end_line: i32,
    pub syntax_type: SyntaxType,
    pub score: f32,
}

/// Symbol query information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolQueryInfo {
    pub name_pattern: Option<String>,
    pub type_filter: Option<String>,
    pub file_pattern: Option<String>,
}

/// Search result summary for final results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultSummary {
    pub filepath: String,
    pub start_line: i32,
    pub end_line: i32,
    pub score: f32,
    pub score_type: ScoreType,
    pub language: String,
    /// First few lines of content (for preview).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    /// Whether the content is stale (file modified since indexing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_stale: Option<bool>,
}

impl From<crate::types::SearchResult> for SearchResultSummary {
    fn from(result: crate::types::SearchResult) -> Self {
        // Extract first 3 lines for preview
        let preview = result
            .chunk
            .content
            .lines()
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");

        Self {
            filepath: result.chunk.filepath,
            start_line: result.chunk.start_line,
            end_line: result.chunk.end_line,
            score: result.score,
            score_type: result.score_type,
            language: result.chunk.language,
            preview: if preview.is_empty() {
                None
            } else {
                Some(preview)
            },
            is_stale: result.is_stale,
        }
    }
}

/// Score adjustment from reranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreAdjustment {
    pub filepath: String,
    pub original_score: f32,
    pub adjusted_score: f32,
    pub reason: String,
}

/// Rebuild mode information.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RebuildModeInfo {
    /// Clean rebuild (delete existing).
    Clean,
    /// Incremental update.
    Incremental,
}

impl From<RebuildMode> for RebuildModeInfo {
    fn from(mode: RebuildMode) -> Self {
        match mode {
            RebuildMode::Clean => RebuildModeInfo::Clean,
            RebuildMode::Incremental => RebuildModeInfo::Incremental,
        }
    }
}

/// Index phase information.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IndexPhaseInfo {
    /// Scanning files.
    Scanning,
    /// Computing hashes.
    Hashing,
    /// Detecting changes.
    Detecting,
    /// Chunking files.
    Chunking,
    /// Computing embeddings.
    Embedding,
    /// Building BM25 index.
    Bm25Indexing,
    /// Storing to database.
    Storing,
    /// Cleanup and finalization.
    Finalizing,
}

impl IndexPhaseInfo {
    /// Create from a phase description string.
    pub fn from_description(desc: &str) -> Self {
        let desc_lower = desc.to_lowercase();
        if desc_lower.contains("scan") {
            IndexPhaseInfo::Scanning
        } else if desc_lower.contains("hash") {
            IndexPhaseInfo::Hashing
        } else if desc_lower.contains("detect") || desc_lower.contains("change") {
            IndexPhaseInfo::Detecting
        } else if desc_lower.contains("chunk") || desc_lower.contains("split") {
            IndexPhaseInfo::Chunking
        } else if desc_lower.contains("embed") {
            IndexPhaseInfo::Embedding
        } else if desc_lower.contains("bm25") || desc_lower.contains("full-text") {
            IndexPhaseInfo::Bm25Indexing
        } else if desc_lower.contains("stor") || desc_lower.contains("commit") {
            IndexPhaseInfo::Storing
        } else {
            IndexPhaseInfo::Finalizing
        }
    }
}

impl fmt::Display for IndexPhaseInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexPhaseInfo::Scanning => write!(f, "Scanning"),
            IndexPhaseInfo::Hashing => write!(f, "Hashing"),
            IndexPhaseInfo::Detecting => write!(f, "Detecting"),
            IndexPhaseInfo::Chunking => write!(f, "Chunking"),
            IndexPhaseInfo::Embedding => write!(f, "Embedding"),
            IndexPhaseInfo::Bm25Indexing => write!(f, "BM25 Indexing"),
            IndexPhaseInfo::Storing => write!(f, "Storing"),
            IndexPhaseInfo::Finalizing => write!(f, "Finalizing"),
        }
    }
}

/// File processing status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileProcessStatus {
    /// File indexed successfully.
    Success,
    /// File skipped (e.g., binary, too large).
    Skipped,
    /// File processing failed.
    Failed,
    /// File unchanged (incremental mode).
    Unchanged,
}

/// Index statistics summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatsSummary {
    pub file_count: i32,
    pub chunk_count: i32,
    pub symbol_count: i32,
    pub index_size_bytes: i64,
    pub languages: Vec<String>,
}

impl From<IndexStats> for IndexStatsSummary {
    fn from(stats: IndexStats) -> Self {
        Self {
            file_count: stats.file_count as i32,
            chunk_count: stats.chunk_count as i32,
            symbol_count: 0, // TODO: add to IndexStats
            index_size_bytes: 0,
            languages: Vec::new(),
        }
    }
}

/// File change kind.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
}

impl fmt::Display for FileChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileChangeKind::Created => write!(f, "created"),
            FileChangeKind::Modified => write!(f, "modified"),
            FileChangeKind::Deleted => write!(f, "deleted"),
        }
    }
}

/// Ranked file summary for RepoMap events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedFileSummary {
    pub filepath: String,
    pub rank: f64,
    pub symbol_count: i32,
}

/// Log level for diagnostic events.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

// ============================================================================
// Event Consumer Trait
// ============================================================================

/// Consumer trait for adapting events.
///
/// Implement this trait to receive and process retrieval events.
pub trait EventConsumer: Send + Sync {
    /// Called for each event.
    fn on_event(&mut self, event: &RetrievalEvent);

    /// Flush any buffered events.
    fn flush(&mut self) {}
}

/// JSON-lines consumer that writes events to a writer.
pub struct JsonLinesConsumer<W: std::io::Write + Send> {
    writer: W,
}

impl<W: std::io::Write + Send> JsonLinesConsumer<W> {
    /// Create a new JSON-lines consumer.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: std::io::Write + Send + Sync> EventConsumer for JsonLinesConsumer<W> {
    fn on_event(&mut self, event: &RetrievalEvent) {
        let line = event.to_json_line();
        let _ = writeln!(self.writer, "{}", line);
    }

    fn flush(&mut self) {
        let _ = self.writer.flush();
    }
}

/// Logging consumer that writes events to tracing.
pub struct LoggingConsumer {
    min_level: LogLevel,
}

impl LoggingConsumer {
    /// Create a new logging consumer.
    pub fn new(min_level: LogLevel) -> Self {
        Self { min_level }
    }
}

impl Default for LoggingConsumer {
    fn default() -> Self {
        Self::new(LogLevel::Debug)
    }
}

impl EventConsumer for LoggingConsumer {
    fn on_event(&mut self, event: &RetrievalEvent) {
        match event {
            RetrievalEvent::DiagnosticLog { level, .. } => {
                if self.should_log(*level) {
                    self.log_event(event);
                }
            }
            _ => {
                // Log all non-diagnostic events at debug level
                if self.should_log(LogLevel::Debug) {
                    self.log_event(event);
                }
            }
        }
    }
}

impl LoggingConsumer {
    fn should_log(&self, level: LogLevel) -> bool {
        let level_order = |l: LogLevel| match l {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
        };
        level_order(level) >= level_order(self.min_level)
    }

    fn log_event(&self, event: &RetrievalEvent) {
        let event_type = event.event_type();
        match event {
            RetrievalEvent::DiagnosticLog {
                level,
                module,
                message,
                ..
            } => match level {
                LogLevel::Trace => {
                    tracing::trace!(module = %module, "{}", message)
                }
                LogLevel::Debug => {
                    tracing::debug!(module = %module, "{}", message)
                }
                LogLevel::Info => {
                    tracing::info!(module = %module, "{}", message)
                }
                LogLevel::Warn => {
                    tracing::warn!(module = %module, "{}", message)
                }
                LogLevel::Error => {
                    tracing::error!(module = %module, "{}", message)
                }
            },
            _ => {
                tracing::debug!(event_type = event_type, "retrieval event");
            }
        }
    }
}

// ============================================================================
// Helper Functions for Creating Events
// ============================================================================

/// Generate a unique query ID.
pub fn generate_query_id() -> String {
    use std::sync::atomic::AtomicI64;
    use std::sync::atomic::Ordering;

    static COUNTER: AtomicI64 = AtomicI64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = RetrievalEvent::timestamp();
    format!("q-{}-{}", ts, count)
}

/// Generate a unique session ID.
pub fn generate_session_id() -> String {
    use std::sync::atomic::AtomicI64;
    use std::sync::atomic::Ordering;

    static COUNTER: AtomicI64 = AtomicI64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = RetrievalEvent::timestamp();
    format!("s-{}-{}", ts, count)
}

/// Generate a unique request ID.
pub fn generate_request_id() -> String {
    use std::sync::atomic::AtomicI64;
    use std::sync::atomic::Ordering;

    static COUNTER: AtomicI64 = AtomicI64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = RetrievalEvent::timestamp();
    format!("r-{}-{}", ts, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = RetrievalEvent::SearchStarted {
            query_id: "q-123".to_string(),
            query: "test query".to_string(),
            mode: SearchMode::Hybrid,
            limit: 10,
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("search_started"));
        assert!(json.contains("test query"));
    }

    #[test]
    fn test_event_to_json_line() {
        let event = RetrievalEvent::SearchCompleted {
            query_id: "q-123".to_string(),
            results: vec![],
            total_duration_ms: 100,
        };

        let line = event.to_json_line();
        assert!(line.contains("timestamp"));
        assert!(line.contains("search_completed"));
    }

    #[test]
    fn test_event_type() {
        let event = RetrievalEvent::IndexBuildStarted {
            workspace: "test".to_string(),
            mode: RebuildModeInfo::Clean,
            estimated_files: 100,
        };

        assert_eq!(event.event_type(), "index_build_started");
    }

    #[test]
    fn test_search_mode_display() {
        assert_eq!(format!("{}", SearchMode::Hybrid), "hybrid");
        assert_eq!(format!("{}", SearchMode::Bm25), "bm25");
        assert_eq!(format!("{}", SearchMode::Vector), "vector");
        assert_eq!(format!("{}", SearchMode::Snippet), "snippet");
    }

    #[test]
    fn test_generate_query_id() {
        let id1 = generate_query_id();
        let id2 = generate_query_id();
        assert_ne!(id1, id2);
        assert!(id1.starts_with("q-"));
    }

    #[test]
    fn test_json_lines_consumer() {
        let mut output = Vec::new();
        {
            let mut consumer = JsonLinesConsumer::new(&mut output);
            consumer.on_event(&RetrievalEvent::SessionStarted {
                session_id: "s-123".to_string(),
                config: ConfigSummary {
                    enabled: true,
                    data_dir: "/tmp".to_string(),
                    bm25_enabled: true,
                    vector_enabled: false,
                    query_rewrite_enabled: false,
                    reranker_backend: None,
                },
            });
            consumer.flush();
        }

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("session_started"));
        assert!(output_str.ends_with('\n'));
    }
}
