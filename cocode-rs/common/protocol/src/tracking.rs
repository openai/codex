//! Tracking types for conversation and compaction state.
//!
//! These types track the state of queries and auto-compaction.

use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

use serde::Deserialize;
use serde::Serialize;

/// Tracking information for a query chain.
///
/// Used to track the lineage of sub-agent queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryTracking {
    /// Unique identifier for this query chain.
    pub chain_id: String,
    /// Depth in the query chain (0 = root query).
    pub depth: i32,
    /// Parent query identifier (None for root queries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_query_id: Option<String>,
}

impl QueryTracking {
    /// Create a new root query tracking.
    pub fn new_root(chain_id: impl Into<String>) -> Self {
        Self {
            chain_id: chain_id.into(),
            depth: 0,
            parent_query_id: None,
        }
    }

    /// Create a child query tracking.
    pub fn child(&self, parent_query_id: impl Into<String>) -> Self {
        Self {
            chain_id: self.chain_id.clone(),
            depth: self.depth + 1,
            parent_query_id: Some(parent_query_id.into()),
        }
    }

    /// Check if this is a root query.
    pub fn is_root(&self) -> bool {
        self.depth == 0
    }
}

/// Tracking information for auto-compaction and session memory extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCompactTracking {
    /// Whether compaction has been performed.
    #[serde(default)]
    pub compacted: bool,
    /// Turn ID when compaction occurred.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// Turn counter at time of tracking.
    #[serde(default)]
    pub turn_counter: i32,

    // ========================================================================
    // Session Memory Extraction Tracking
    // ========================================================================
    /// Number of extractions performed in this session.
    #[serde(default)]
    pub extraction_count: i32,
    /// Token count at the last extraction.
    #[serde(default)]
    pub last_extraction_tokens: i32,
    /// Tool call count at the last extraction.
    #[serde(default)]
    pub last_extraction_tool_calls: i32,
    /// Last summarized message ID (for incremental updates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_extraction_id: Option<String>,
    /// Whether an extraction is currently in progress.
    #[serde(default)]
    pub extraction_in_progress: bool,
    /// Total tool calls in the session.
    #[serde(default)]
    pub tool_call_count: i32,

    /// Timestamp of last extraction (not serialized, runtime-only).
    #[serde(skip)]
    last_extraction_time: Option<Instant>,
}

impl Default for AutoCompactTracking {
    fn default() -> Self {
        Self {
            compacted: false,
            turn_id: None,
            turn_counter: 0,
            extraction_count: 0,
            last_extraction_tokens: 0,
            last_extraction_tool_calls: 0,
            last_extraction_id: None,
            extraction_in_progress: false,
            tool_call_count: 0,
            last_extraction_time: None,
        }
    }
}

impl PartialEq for AutoCompactTracking {
    fn eq(&self, other: &Self) -> bool {
        // Compare all serializable fields
        self.compacted == other.compacted
            && self.turn_id == other.turn_id
            && self.turn_counter == other.turn_counter
            && self.extraction_count == other.extraction_count
            && self.last_extraction_tokens == other.last_extraction_tokens
            && self.last_extraction_tool_calls == other.last_extraction_tool_calls
            && self.last_extraction_id == other.last_extraction_id
            && self.extraction_in_progress == other.extraction_in_progress
            && self.tool_call_count == other.tool_call_count
    }
}

impl Eq for AutoCompactTracking {}

impl AutoCompactTracking {
    /// Create a new tracking instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that compaction was performed.
    pub fn mark_compacted(&mut self, turn_id: impl Into<String>, turn_counter: i32) {
        self.compacted = true;
        self.turn_id = Some(turn_id.into());
        self.turn_counter = turn_counter;
    }

    /// Reset tracking state.
    pub fn reset(&mut self) {
        self.compacted = false;
        self.turn_id = None;
        self.turn_counter = 0;
        self.extraction_count = 0;
        self.last_extraction_tokens = 0;
        self.last_extraction_tool_calls = 0;
        self.last_extraction_id = None;
        self.extraction_in_progress = false;
        self.last_extraction_time = None;
        self.tool_call_count = 0;
    }

    /// Record that a tool call was made.
    pub fn record_tool_call(&mut self) {
        self.tool_call_count += 1;
    }

    /// Mark that extraction has started.
    pub fn mark_extraction_started(&mut self) {
        self.extraction_in_progress = true;
    }

    /// Mark that extraction has completed successfully.
    pub fn mark_extraction_completed(
        &mut self,
        current_tokens: i32,
        last_summarized_id: impl Into<String>,
    ) {
        self.extraction_in_progress = false;
        self.extraction_count += 1;
        self.last_extraction_tokens = current_tokens;
        self.last_extraction_tool_calls = self.tool_call_count;
        self.last_extraction_id = Some(last_summarized_id.into());
        self.last_extraction_time = Some(Instant::now());
    }

    /// Mark that extraction failed.
    pub fn mark_extraction_failed(&mut self) {
        self.extraction_in_progress = false;
    }

    /// Get the time since the last extraction.
    ///
    /// Returns `Duration::MAX` if no extraction has occurred yet.
    pub fn time_since_extraction(&self) -> Duration {
        self.last_extraction_time
            .map(|t| t.elapsed())
            .unwrap_or(Duration::MAX)
    }

    /// Get the number of tokens accumulated since last extraction.
    pub fn tokens_since_extraction(&self, current_tokens: i32) -> i32 {
        current_tokens - self.last_extraction_tokens
    }

    /// Get the number of tool calls since last extraction.
    pub fn tool_calls_since_extraction(&self) -> i32 {
        self.tool_call_count - self.last_extraction_tool_calls
    }
}

/// Information about a file that was read.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadInfo {
    /// Content of the file.
    pub content: String,
    /// When the file was read.
    #[serde(with = "humantime_serde")]
    pub timestamp: SystemTime,
    /// Offset from which reading started (if partial read).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i32>,
    /// Number of lines read (if limited).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<i32>,
    /// File modification time at read time.
    #[serde(with = "humantime_serde")]
    pub file_mtime: SystemTime,
    /// Number of times this file has been accessed.
    #[serde(default)]
    pub access_count: i32,
    /// Whether the entire file was read.
    #[serde(default)]
    pub is_complete_read: bool,
}

impl FileReadInfo {
    /// Create a new file read info.
    pub fn new(content: impl Into<String>, file_mtime: SystemTime) -> Self {
        Self {
            content: content.into(),
            timestamp: SystemTime::now(),
            offset: None,
            limit: None,
            file_mtime,
            access_count: 1,
            is_complete_read: true,
        }
    }

    /// Create a partial file read info.
    pub fn partial(
        content: impl Into<String>,
        file_mtime: SystemTime,
        offset: i32,
        limit: i32,
    ) -> Self {
        Self {
            content: content.into(),
            timestamp: SystemTime::now(),
            offset: Some(offset),
            limit: Some(limit),
            file_mtime,
            access_count: 1,
            is_complete_read: false,
        }
    }

    /// Record another access to this file.
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.timestamp = SystemTime::now();
    }

    /// Check if the file has been modified since this read.
    pub fn is_stale(&self, current_mtime: SystemTime) -> bool {
        current_mtime > self.file_mtime
    }
}

/// A change detected in a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// Path to the changed file.
    pub path: PathBuf,
    /// Type of change.
    pub change_type: FileChangeType,
    /// When the change was detected.
    #[serde(with = "humantime_serde")]
    pub detected_at: SystemTime,
}

impl FileChange {
    /// Create a new file change.
    pub fn new(path: impl Into<PathBuf>, change_type: FileChangeType) -> Self {
        Self {
            path: path.into(),
            change_type,
            detected_at: SystemTime::now(),
        }
    }

    /// Create a modification change.
    pub fn modified(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FileChangeType::Modified)
    }

    /// Create a deletion change.
    pub fn deleted(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FileChangeType::Deleted)
    }

    /// Create a creation change.
    pub fn created(path: impl Into<PathBuf>) -> Self {
        Self::new(path, FileChangeType::Created)
    }
}

/// Type of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileChangeType {
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
    /// File was created.
    Created,
}

impl FileChangeType {
    /// Get the change type as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            FileChangeType::Modified => "modified",
            FileChangeType::Deleted => "deleted",
            FileChangeType::Created => "created",
        }
    }
}

impl std::fmt::Display for FileChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_tracking_root() {
        let tracking = QueryTracking::new_root("chain-1");
        assert_eq!(tracking.chain_id, "chain-1");
        assert_eq!(tracking.depth, 0);
        assert!(tracking.is_root());
        assert!(tracking.parent_query_id.is_none());
    }

    #[test]
    fn test_query_tracking_child() {
        let root = QueryTracking::new_root("chain-1");
        let child = root.child("query-1");

        assert_eq!(child.chain_id, "chain-1");
        assert_eq!(child.depth, 1);
        assert!(!child.is_root());
        assert_eq!(child.parent_query_id.as_deref(), Some("query-1"));

        let grandchild = child.child("query-2");
        assert_eq!(grandchild.depth, 2);
    }

    #[test]
    fn test_auto_compact_tracking() {
        let mut tracking = AutoCompactTracking::new();
        assert!(!tracking.compacted);
        assert!(tracking.turn_id.is_none());

        tracking.mark_compacted("turn-1", 5);
        assert!(tracking.compacted);
        assert_eq!(tracking.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(tracking.turn_counter, 5);

        tracking.reset();
        assert!(!tracking.compacted);
        assert!(tracking.turn_id.is_none());
        assert_eq!(tracking.turn_counter, 0);
    }

    #[test]
    fn test_file_read_info() {
        let mtime = SystemTime::now();
        let info = FileReadInfo::new("content", mtime);

        assert_eq!(info.content, "content");
        assert_eq!(info.access_count, 1);
        assert!(info.is_complete_read);
        assert!(info.offset.is_none());
        assert!(info.limit.is_none());
    }

    #[test]
    fn test_file_read_info_partial() {
        let mtime = SystemTime::now();
        let info = FileReadInfo::partial("partial", mtime, 10, 100);

        assert_eq!(info.offset, Some(10));
        assert_eq!(info.limit, Some(100));
        assert!(!info.is_complete_read);
    }

    #[test]
    fn test_file_read_info_access() {
        let mtime = SystemTime::now();
        let mut info = FileReadInfo::new("content", mtime);
        assert_eq!(info.access_count, 1);

        info.record_access();
        assert_eq!(info.access_count, 2);
    }

    #[test]
    fn test_file_change() {
        let change = FileChange::modified("/tmp/test.txt");
        assert_eq!(change.change_type, FileChangeType::Modified);

        let change = FileChange::deleted("/tmp/test.txt");
        assert_eq!(change.change_type, FileChangeType::Deleted);

        let change = FileChange::created("/tmp/test.txt");
        assert_eq!(change.change_type, FileChangeType::Created);
    }

    #[test]
    fn test_file_change_type_display() {
        assert_eq!(FileChangeType::Modified.as_str(), "modified");
        assert_eq!(FileChangeType::Deleted.as_str(), "deleted");
        assert_eq!(FileChangeType::Created.as_str(), "created");
    }

    #[test]
    fn test_serde_roundtrip() {
        let tracking = QueryTracking::new_root("chain-1");
        let json = serde_json::to_string(&tracking).unwrap();
        let parsed: QueryTracking = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, tracking);

        let mut compact = AutoCompactTracking::new();
        compact.mark_compacted("turn-1", 5);
        let json = serde_json::to_string(&compact).unwrap();
        let parsed: AutoCompactTracking = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, compact);
    }

    #[test]
    fn test_extraction_tracking() {
        let mut tracking = AutoCompactTracking::new();
        assert_eq!(tracking.extraction_count, 0);
        assert!(!tracking.extraction_in_progress);

        // Record some tool calls
        tracking.record_tool_call();
        tracking.record_tool_call();
        assert_eq!(tracking.tool_call_count, 2);

        // Start extraction
        tracking.mark_extraction_started();
        assert!(tracking.extraction_in_progress);

        // Complete extraction
        tracking.mark_extraction_completed(10000, "msg-123");
        assert!(!tracking.extraction_in_progress);
        assert_eq!(tracking.extraction_count, 1);
        assert_eq!(tracking.last_extraction_tokens, 10000);
        assert_eq!(tracking.last_extraction_tool_calls, 2);
        assert_eq!(tracking.last_extraction_id.as_deref(), Some("msg-123"));

        // Check tokens/calls since extraction
        assert_eq!(tracking.tokens_since_extraction(15000), 5000);
        assert_eq!(tracking.tool_calls_since_extraction(), 0);

        // Record more tool calls
        tracking.record_tool_call();
        assert_eq!(tracking.tool_calls_since_extraction(), 1);
    }

    #[test]
    fn test_extraction_failure() {
        let mut tracking = AutoCompactTracking::new();
        tracking.mark_extraction_started();
        assert!(tracking.extraction_in_progress);

        tracking.mark_extraction_failed();
        assert!(!tracking.extraction_in_progress);
        assert_eq!(tracking.extraction_count, 0); // Count should not increase on failure
    }
}
