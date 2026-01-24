//! File tracking for change detection.
//!
//! Tracks file read state to detect modifications since last read.
//! Matches readFileState in Claude Code.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::SystemTime;

// ============================================
// Read File State
// ============================================

/// State of a read file.
#[derive(Debug, Clone)]
pub struct ReadFileState {
    /// File content at time of read.
    pub content: String,
    /// Timestamp of last modification at read time.
    pub last_modified: SystemTime,
    /// Turn number when read.
    pub read_turn: i32,
    /// Offset if partial read.
    pub offset: Option<i32>,
    /// Limit if partial read.
    pub limit: Option<i32>,
}

// ============================================
// File Tracker
// ============================================

/// Tracks file read state for change detection.
///
/// Matches readFileState in Claude Code.
pub struct FileTracker {
    /// Map of file path -> read state.
    files: RwLock<HashMap<PathBuf, ReadFileState>>,
    /// Paths that trigger nested memory lookup.
    nested_memory_triggers: RwLock<HashSet<PathBuf>>,
}

impl FileTracker {
    /// Create a new file tracker.
    pub fn new() -> Self {
        Self {
            files: RwLock::new(HashMap::new()),
            nested_memory_triggers: RwLock::new(HashSet::new()),
        }
    }

    /// Record a file read.
    pub fn track_read(
        &self,
        path: PathBuf,
        content: String,
        turn: i32,
        offset: Option<i32>,
        limit: Option<i32>,
    ) {
        let last_modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        let state = ReadFileState {
            content,
            last_modified,
            read_turn: turn,
            offset,
            limit,
        };

        let mut files = self.files.write().expect("file tracker lock poisoned");
        files.insert(path.clone(), state);

        // Trigger nested memory lookup if full read (no offset/limit)
        if offset.is_none() && limit.is_none() {
            let mut triggers = self
                .nested_memory_triggers
                .write()
                .expect("triggers lock poisoned");
            triggers.insert(path);
        }
    }

    /// Get all tracked files.
    pub fn get_tracked_files(&self) -> Vec<(PathBuf, ReadFileState)> {
        let files = self.files.read().expect("file tracker lock poisoned");
        files.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Get and clear nested memory triggers.
    pub fn get_nested_memory_triggers(&self) -> Vec<PathBuf> {
        let mut triggers = self
            .nested_memory_triggers
            .write()
            .expect("triggers lock poisoned");
        triggers.drain().collect()
    }

    /// Update file modification time after confirming no change.
    pub fn update_modified_time(&self, path: &PathBuf, new_time: SystemTime) {
        let mut files = self.files.write().expect("file tracker lock poisoned");
        if let Some(state) = files.get_mut(path) {
            state.last_modified = new_time;
        }
    }

    /// Remove a file from tracking (e.g., when deleted).
    ///
    /// This prevents repeated notifications for deleted files.
    pub fn remove(&self, path: &PathBuf) {
        let mut files = self.files.write().expect("file tracker lock poisoned");
        files.remove(path);
    }

    /// Check if a file has been modified since last read.
    pub fn has_file_changed(&self, path: &PathBuf) -> bool {
        let files = self.files.read().expect("file tracker lock poisoned");
        if let Some(state) = files.get(path) {
            // Skip partial reads
            if state.offset.is_some() || state.limit.is_some() {
                return false;
            }

            if let Ok(metadata) = std::fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    return modified > state.last_modified;
                }
            }
        }
        false
    }

    /// Clear all tracked files (call at session end).
    pub fn clear(&self) {
        let mut files = self.files.write().expect("file tracker lock poisoned");
        files.clear();
        let mut triggers = self
            .nested_memory_triggers
            .write()
            .expect("triggers lock poisoned");
        triggers.clear();
    }

    /// Get number of tracked files.
    pub fn len(&self) -> usize {
        let files = self.files.read().expect("file tracker lock poisoned");
        files.len()
    }

    /// Check if tracker is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for FileTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FileTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let files = self.files.read().expect("file tracker lock poisoned");
        f.debug_struct("FileTracker")
            .field("tracked_files", &files.len())
            .finish()
    }
}

// ============================================
// Tests
// ============================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_tracker_new() {
        let tracker = FileTracker::new();
        assert!(tracker.is_empty());
    }

    #[test]
    fn test_track_read() {
        let tracker = FileTracker::new();
        let path = PathBuf::from("/test/file.txt");

        tracker.track_read(path.clone(), "content".to_string(), 1, None, None);

        let files = tracker.get_tracked_files();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, path);
        assert_eq!(files[0].1.content, "content");
        assert_eq!(files[0].1.read_turn, 1);
    }

    #[test]
    fn test_nested_memory_triggers_full_read() {
        let tracker = FileTracker::new();
        let path = PathBuf::from("/test/file.txt");

        // Full read triggers nested memory
        tracker.track_read(path.clone(), "content".to_string(), 1, None, None);
        let triggers = tracker.get_nested_memory_triggers();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0], path);

        // Triggers cleared after retrieval
        let triggers2 = tracker.get_nested_memory_triggers();
        assert!(triggers2.is_empty());
    }

    #[test]
    fn test_partial_read_no_trigger() {
        let tracker = FileTracker::new();
        let path = PathBuf::from("/test/file.txt");

        // Partial read doesn't trigger nested memory
        tracker.track_read(path, "content".to_string(), 1, Some(10), Some(20));
        let triggers = tracker.get_nested_memory_triggers();
        assert!(triggers.is_empty());
    }

    #[test]
    fn test_clear() {
        let tracker = FileTracker::new();
        tracker.track_read(
            PathBuf::from("/test/file.txt"),
            "content".to_string(),
            1,
            None,
            None,
        );

        tracker.clear();
        assert!(tracker.is_empty());
        assert!(tracker.get_nested_memory_triggers().is_empty());
    }

    #[test]
    fn test_len() {
        let tracker = FileTracker::new();
        assert_eq!(tracker.len(), 0);

        tracker.track_read(
            PathBuf::from("/test/file1.txt"),
            "content1".to_string(),
            1,
            None,
            None,
        );
        assert_eq!(tracker.len(), 1);

        tracker.track_read(
            PathBuf::from("/test/file2.txt"),
            "content2".to_string(),
            2,
            None,
            None,
        );
        assert_eq!(tracker.len(), 2);
    }
}
