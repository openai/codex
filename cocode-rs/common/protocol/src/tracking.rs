//! Tracking types for conversation and compaction state.
//!
//! These types track the state of queries and auto-compaction.

use std::path::PathBuf;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

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

/// Tracking information for auto-compaction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
}

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

        let compact = AutoCompactTracking {
            compacted: true,
            turn_id: Some("turn-1".to_string()),
            turn_counter: 5,
        };
        let json = serde_json::to_string(&compact).unwrap();
        let parsed: AutoCompactTracking = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, compact);
    }
}
