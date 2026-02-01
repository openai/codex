//! File tracker for change detection.
//!
//! This module tracks file reads and detects modifications
//! since the file was last read.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::SystemTime;

/// State of a read file.
#[derive(Debug, Clone)]
pub struct ReadFileState {
    /// File content at read time.
    pub content: String,
    /// Last modified time when read.
    pub last_modified: Option<SystemTime>,
    /// Turn number when the file was read.
    pub read_turn: i32,
    /// Byte offset if partial read (None = full read).
    pub offset: Option<i64>,
    /// Line limit if partial read (None = full read).
    pub limit: Option<i64>,
}

impl ReadFileState {
    /// Create a new read file state.
    pub fn new(content: String, last_modified: Option<SystemTime>, read_turn: i32) -> Self {
        Self {
            content,
            last_modified,
            read_turn,
            offset: None,
            limit: None,
        }
    }

    /// Create a partial read state.
    pub fn partial(
        content: String,
        last_modified: Option<SystemTime>,
        read_turn: i32,
        offset: i64,
        limit: i64,
    ) -> Self {
        Self {
            content,
            last_modified,
            read_turn,
            offset: Some(offset),
            limit: Some(limit),
        }
    }

    /// Check if this was a partial read.
    pub fn is_partial(&self) -> bool {
        self.offset.is_some() || self.limit.is_some()
    }
}

/// Tracks file reads and detects changes.
#[derive(Debug, Default)]
pub struct FileTracker {
    files: RwLock<HashMap<PathBuf, ReadFileState>>,
    nested_memory_triggers: RwLock<HashSet<PathBuf>>,
}

impl FileTracker {
    /// Create a new file tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sync file read state from an external source (e.g., tool executor's tracker).
    ///
    /// This method allows the system-reminder FileTracker to receive file read
    /// information from the tool layer's FileTracker, enabling accurate change
    /// detection across the system.
    ///
    /// # Arguments
    ///
    /// * `path` - The file path that was read
    /// * `content` - The content that was read (used for fallback change detection)
    /// * `last_modified` - The file's modification time when read
    /// * `read_turn` - The turn number when the file was read
    pub fn sync_read(
        &self,
        path: impl AsRef<Path>,
        content: String,
        last_modified: Option<SystemTime>,
        read_turn: i32,
    ) {
        let state = ReadFileState::new(content, last_modified, read_turn);
        let _ = self.track_read(path, state);
    }

    /// Sync a partial file read from an external source.
    ///
    /// Similar to `sync_read` but for partial reads with offset/limit.
    pub fn sync_partial_read(
        &self,
        path: impl AsRef<Path>,
        content: String,
        last_modified: Option<SystemTime>,
        read_turn: i32,
        offset: i64,
        limit: i64,
    ) {
        let state = ReadFileState::partial(content, last_modified, read_turn, offset, limit);
        let _ = self.track_read(path, state);
    }

    /// Track a file read.
    ///
    /// Returns `true` if this file triggers nested memory lookup
    /// (e.g., CLAUDE.md, AGENTS.md files).
    pub fn track_read(&self, path: impl AsRef<Path>, state: ReadFileState) -> bool {
        let path = path.as_ref().to_path_buf();
        let is_memory_trigger = Self::is_nested_memory_trigger(&path);

        {
            let mut files = self.files.write().expect("lock poisoned");
            files.insert(path.clone(), state);
        }

        if is_memory_trigger {
            let mut triggers = self.nested_memory_triggers.write().expect("lock poisoned");
            triggers.insert(path);
            true
        } else {
            false
        }
    }

    /// Check if a file has changed since it was last read.
    ///
    /// Returns `None` if the file isn't tracked.
    /// Skips change detection for partial reads.
    pub fn has_file_changed(&self, path: impl AsRef<Path>) -> Option<bool> {
        let path = path.as_ref();
        let files = self.files.read().expect("lock poisoned");

        let state = files.get(path)?;

        // Skip partial reads - can't reliably detect changes
        if state.is_partial() {
            return Some(false);
        }

        // Check modification time
        let current_mtime = std::fs::metadata(path).ok()?.modified().ok();

        match (state.last_modified, current_mtime) {
            (Some(old), Some(new)) => Some(new > old),
            (None, Some(_)) => Some(true), // File now has mtime
            (Some(_), None) => Some(true), // File lost mtime (weird but changed)
            (None, None) => {
                // Fall back to content comparison
                let current_content = std::fs::read_to_string(path).ok()?;
                Some(current_content != state.content)
            }
        }
    }

    /// Get the tracked state for a file.
    pub fn get_state(&self, path: impl AsRef<Path>) -> Option<ReadFileState> {
        let files = self.files.read().expect("lock poisoned");
        files.get(path.as_ref()).cloned()
    }

    /// Get all tracked files.
    pub fn tracked_files(&self) -> Vec<PathBuf> {
        let files = self.files.read().expect("lock poisoned");
        files.keys().cloned().collect()
    }

    /// Get files that have changed since last read.
    pub fn changed_files(&self) -> Vec<PathBuf> {
        self.tracked_files()
            .into_iter()
            .filter(|p| self.has_file_changed(p) == Some(true))
            .collect()
    }

    /// Update the modification time for a file.
    pub fn update_modified_time(&self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        let mut files = self.files.write().expect("lock poisoned");

        if let Some(state) = files.get_mut(path) {
            if let Ok(meta) = std::fs::metadata(path) {
                state.last_modified = meta.modified().ok();
            }
        }
    }

    /// Remove tracking for a file.
    pub fn remove(&self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        let mut files = self.files.write().expect("lock poisoned");
        files.remove(path);

        let mut triggers = self.nested_memory_triggers.write().expect("lock poisoned");
        triggers.remove(path);
    }

    /// Clear all tracked files.
    pub fn clear(&self) {
        let mut files = self.files.write().expect("lock poisoned");
        files.clear();

        let mut triggers = self.nested_memory_triggers.write().expect("lock poisoned");
        triggers.clear();
    }

    /// Get and clear nested memory trigger paths.
    ///
    /// Returns paths that need nested memory lookup, then clears them.
    pub fn drain_nested_memory_triggers(&self) -> HashSet<PathBuf> {
        let mut triggers = self.nested_memory_triggers.write().expect("lock poisoned");
        std::mem::take(&mut *triggers)
    }

    /// Check if there are pending nested memory triggers.
    pub fn has_nested_memory_triggers(&self) -> bool {
        let triggers = self.nested_memory_triggers.read().expect("lock poisoned");
        !triggers.is_empty()
    }

    /// Check if a path triggers nested memory lookup.
    fn is_nested_memory_trigger(path: &Path) -> bool {
        let filename = path.file_name().and_then(|n| n.to_str());
        matches!(
            filename,
            Some("CLAUDE.md" | "AGENTS.md" | "settings.json" | ".cursorrules" | ".aider.conf.yml")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_track_read() {
        let tracker = FileTracker::new();
        let state = ReadFileState::new("content".to_string(), None, 1);

        let is_trigger = tracker.track_read("/tmp/test.rs", state);
        assert!(!is_trigger);

        assert!(tracker.get_state("/tmp/test.rs").is_some());
    }

    #[test]
    fn test_nested_memory_trigger() {
        let tracker = FileTracker::new();

        // Regular file - not a trigger
        let state = ReadFileState::new("content".to_string(), None, 1);
        assert!(!tracker.track_read("/project/src/main.rs", state));

        // CLAUDE.md - is a trigger
        let state = ReadFileState::new("instructions".to_string(), None, 1);
        assert!(tracker.track_read("/project/CLAUDE.md", state));

        // AGENTS.md - is a trigger
        let state = ReadFileState::new("agents".to_string(), None, 1);
        assert!(tracker.track_read("/project/AGENTS.md", state));

        assert!(tracker.has_nested_memory_triggers());

        let triggers = tracker.drain_nested_memory_triggers();
        assert_eq!(triggers.len(), 2);
        assert!(!tracker.has_nested_memory_triggers());
    }

    #[test]
    fn test_partial_read() {
        let state = ReadFileState::partial("partial content".to_string(), None, 1, 100, 50);

        assert!(state.is_partial());

        let full = ReadFileState::new("full".to_string(), None, 1);
        assert!(!full.is_partial());
    }

    #[test]
    fn test_tracked_files() {
        let tracker = FileTracker::new();

        tracker.track_read("/a.rs", ReadFileState::new("a".to_string(), None, 1));
        tracker.track_read("/b.rs", ReadFileState::new("b".to_string(), None, 1));
        tracker.track_read("/c.rs", ReadFileState::new("c".to_string(), None, 1));

        let files = tracker.tracked_files();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_remove_tracking() {
        let tracker = FileTracker::new();

        tracker.track_read("/test.rs", ReadFileState::new("test".to_string(), None, 1));
        assert!(tracker.get_state("/test.rs").is_some());

        tracker.remove("/test.rs");
        assert!(tracker.get_state("/test.rs").is_none());
    }

    #[test]
    fn test_clear() {
        let tracker = FileTracker::new();

        tracker.track_read("/a.rs", ReadFileState::new("a".to_string(), None, 1));
        tracker.track_read("/CLAUDE.md", ReadFileState::new("md".to_string(), None, 1));

        tracker.clear();

        assert!(tracker.tracked_files().is_empty());
        assert!(!tracker.has_nested_memory_triggers());
    }

    #[test]
    fn test_mtime_comparison() {
        // Create a simple mtime
        let old_time = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let new_time = SystemTime::UNIX_EPOCH + Duration::from_secs(2000);

        let state = ReadFileState::new("content".to_string(), Some(old_time), 1);
        assert_eq!(state.last_modified, Some(old_time));

        // Newer time should indicate change
        assert!(new_time > old_time);
    }
}
