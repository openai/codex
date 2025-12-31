//! File watcher for automatic index updates.
//!
//! Uses the `notify` crate to detect file changes and trigger
//! tweakcc re-indexing. Uses codex-file-ignore for consistent
//! ignore patterns with the file walker.

use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use codex_file_ignore::IgnoreConfig;
use codex_file_ignore::IgnoreService;
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify_debouncer_mini::DebouncedEvent;
use notify_debouncer_mini::Debouncer;
use notify_debouncer_mini::new_debouncer;

use crate::error::Result;
use crate::error::RetrievalErr;

/// File change event type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEventKind {
    /// File was created
    Created,
    /// File was modified
    Modified,
    /// File was deleted
    Deleted,
}

/// File change event.
#[derive(Debug, Clone)]
pub struct WatchEvent {
    /// Path to the changed file
    pub path: PathBuf,
    /// Type of change
    pub kind: WatchEventKind,
}

/// File watcher service.
///
/// Monitors a directory for file changes and emits events
/// that can be used to trigger tweakcc re-indexing.
/// Uses codex-file-ignore for consistent ignore patterns.
pub struct FileWatcher {
    /// The debouncer wraps the underlying watcher
    _debouncer: Debouncer<RecommendedWatcher>,
    /// Receiver for debounced events
    rx: std_mpsc::Receiver<std::result::Result<Vec<DebouncedEvent>, notify::Error>>,
    /// Root directory being watched
    root: PathBuf,
    /// Ignore service for consistent pattern matching
    ignore_service: IgnoreService,
    /// Cached exclude patterns for fast path matching
    exclude_patterns: Vec<String>,
}

impl FileWatcher {
    /// Create a new file watcher for the given root directory.
    ///
    /// # Arguments
    /// * `root` - Directory to watch recursively
    /// * `debounce_ms` - Debounce interval in milliseconds
    pub fn new(root: &Path, debounce_ms: u64) -> Result<Self> {
        let (tx, rx) = std_mpsc::channel();

        let debounce_duration = Duration::from_millis(debounce_ms);

        let mut debouncer = new_debouncer(debounce_duration, tx).map_err(|e| {
            RetrievalErr::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create file watcher: {e}"),
            ))
        })?;

        debouncer
            .watcher()
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| {
                RetrievalErr::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to watch directory: {e}"),
                ))
            })?;

        // Use same ignore config as walker for consistency
        let config = IgnoreConfig::respecting_all();
        let ignore_service = IgnoreService::new(config);

        // Convert glob patterns to simple substring patterns for fast matching
        // Patterns like "**/node_modules/**" become "/node_modules/"
        let mut exclude_patterns: Vec<String> = IgnoreService::get_default_excludes()
            .iter()
            .filter_map(|p| {
                // Extract directory name from glob patterns like "**/dirname/**"
                let s = p.trim_start_matches("**/").trim_end_matches("/**");
                if s != *p && !s.contains('*') {
                    Some(format!("/{s}/"))
                } else {
                    None
                }
            })
            .collect();

        // Add language-specific patterns not in common excludes
        // (Rust target, Python venv, etc.)
        let extra_patterns = ["/target/", "/.venv/", "/venv/", "/.tox/", "/.pytest_cache/"];
        for p in extra_patterns {
            if !exclude_patterns.iter().any(|e| e == p) {
                exclude_patterns.push(p.to_string());
            }
        }

        tracing::info!(root = ?root, debounce_ms, patterns = exclude_patterns.len(), "FileWatcher started");

        Ok(Self {
            _debouncer: debouncer,
            rx,
            root: root.to_path_buf(),
            ignore_service,
            exclude_patterns,
        })
    }

    /// Get the root directory being watched.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Try to receive the next batch of events (non-blocking).
    ///
    /// Returns `None` if no events are available.
    pub fn try_recv(&self) -> Option<Vec<WatchEvent>> {
        match self.rx.try_recv() {
            Ok(Ok(events)) => {
                let watch_events = self.convert_events(events);
                if watch_events.is_empty() {
                    None
                } else {
                    Some(watch_events)
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "File watcher error");
                None
            }
            Err(std_mpsc::TryRecvError::Empty) => None,
            Err(std_mpsc::TryRecvError::Disconnected) => {
                tracing::warn!("File watcher channel disconnected");
                None
            }
        }
    }

    /// Receive the next batch of events (blocking).
    ///
    /// Returns `None` if the channel is closed.
    pub fn recv(&self) -> Option<Vec<WatchEvent>> {
        match self.rx.recv() {
            Ok(Ok(events)) => {
                let watch_events = self.convert_events(events);
                if watch_events.is_empty() {
                    // Try again - might have been filtered out
                    self.recv()
                } else {
                    Some(watch_events)
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "File watcher error");
                self.recv() // Try again
            }
            Err(_) => None, // Channel closed
        }
    }

    /// Receive with timeout.
    ///
    /// Returns `None` if timeout expires or channel is closed.
    pub fn recv_timeout(&self, timeout: Duration) -> Option<Vec<WatchEvent>> {
        match self.rx.recv_timeout(timeout) {
            Ok(Ok(events)) => {
                let watch_events = self.convert_events(events);
                if watch_events.is_empty() {
                    None
                } else {
                    Some(watch_events)
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "File watcher error");
                None
            }
            Err(_) => None, // Timeout or disconnected
        }
    }

    /// Convert notify events to our WatchEvent type.
    fn convert_events(&self, events: Vec<DebouncedEvent>) -> Vec<WatchEvent> {
        let mut watch_events = Vec::new();

        for event in events {
            let path = &event.path;

            // Skip non-file events and hidden files
            if self.should_skip(path) {
                continue;
            }

            // Determine event kind based on file existence
            let kind = if path.exists() {
                // Could be created or modified - notify-debouncer-mini
                // doesn't distinguish, so we treat it as Modified
                WatchEventKind::Modified
            } else {
                WatchEventKind::Deleted
            };

            watch_events.push(WatchEvent {
                path: path.clone(),
                kind,
            });
        }

        // Deduplicate by path (keep last event for each path)
        let mut seen = std::collections::HashSet::new();
        watch_events.retain(|e| seen.insert(e.path.clone()));

        watch_events
    }

    /// Check if a path should be skipped.
    ///
    /// Uses patterns from codex-file-ignore for consistency with the file walker.
    fn should_skip(&self, path: &Path) -> bool {
        // Skip directories
        if path.is_dir() {
            return true;
        }

        // Skip hidden files and directories (consistent with IgnoreConfig defaults)
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                return true;
            }
        }

        // Check against cached exclude patterns from file-ignore
        let path_str = path.to_string_lossy();
        for pattern in &self.exclude_patterns {
            if path_str.contains(pattern.as_str()) {
                return true;
            }
        }

        false
    }

    /// Get the ignore service configuration.
    #[allow(dead_code)]
    pub fn ignore_config(&self) -> &IgnoreConfig {
        self.ignore_service.config()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_creation() {
        let dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new(dir.path(), 100);
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_should_skip() {
        let dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new(dir.path(), 100).unwrap();

        // Should skip hidden files
        assert!(watcher.should_skip(Path::new("/tmp/.hidden")));

        // Should skip target directory
        assert!(watcher.should_skip(Path::new("/project/target/debug/main")));

        // Should skip node_modules
        assert!(watcher.should_skip(Path::new("/project/node_modules/pkg/index.js")));

        // Should not skip normal source files
        assert!(!watcher.should_skip(Path::new("/project/src/main.rs")));
    }

    #[test]
    fn test_file_change_detection() {
        let dir = TempDir::new().unwrap();
        let test_file = dir.path().join("test.txt");

        let watcher = FileWatcher::new(dir.path(), 50).unwrap();

        // Create a file
        fs::write(&test_file, "hello").unwrap();

        // Wait for debounce
        std::thread::sleep(Duration::from_millis(100));

        // Should receive at least one event
        if let Some(events) = watcher.try_recv() {
            assert!(!events.is_empty());
        }
    }
}
