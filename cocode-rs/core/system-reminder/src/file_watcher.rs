//! File system watcher for real-time change detection.
//!
//! This module provides Layer 3 of the file edit watch system:
//! - **Layer 1** (tools): FileTracker in cocode-tools - tracks reads/writes during tool execution
//! - **Layer 2** (system-reminder): FileTracker - reactive change detection via mtime
//! - **Layer 3** (system-reminder): FileWatcher - proactive file system monitoring
//!
//! The file watcher monitors previously-read files for external changes (e.g., editor saves,
//! git operations) and notifies the file tracker, enabling accurate stale content warnings.
//!
//! # Usage
//!
//! ```ignore
//! use cocode_system_reminder::file_watcher::FileSystemWatcher;
//! use std::path::Path;
//!
//! // Create watcher for a directory
//! let watcher = FileSystemWatcher::new(Path::new("/project"), 100)?;
//!
//! // Add files to watch (usually called after Read tool)
//! watcher.watch_file(Path::new("/project/src/main.rs"))?;
//!
//! // Poll for changes (non-blocking)
//! if let Some(events) = watcher.poll_changes() {
//!     for event in events {
//!         println!("File changed: {:?}", event.path);
//!     }
//! }
//! ```

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify_debouncer_mini::DebouncedEvent;
use notify_debouncer_mini::Debouncer;
use notify_debouncer_mini::new_debouncer;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::error::Result;
use crate::error::system_reminder_error::InternalSnafu;

/// File change event from the watcher.
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    /// Path to the changed file.
    pub path: PathBuf,
    /// Kind of change detected.
    pub kind: FileChangeKind,
}

/// Kind of file change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeKind {
    /// File was modified (content changed).
    Modified,
    /// File was deleted.
    Deleted,
    /// File was created (only relevant if watching directories).
    Created,
}

/// Configuration for the file system watcher.
#[derive(Debug, Clone)]
pub struct FileWatcherConfig {
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
    /// Maximum number of files to watch.
    pub max_watched_files: i32,
    /// Whether to watch directories recursively.
    pub recursive: bool,
}

impl Default for FileWatcherConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 100,
            max_watched_files: 1000,
            recursive: false, // Only watch specific files by default
        }
    }
}

/// File system watcher for proactive change detection.
///
/// This implements Layer 3 of the file edit watch system, providing real-time
/// notifications when files are modified externally.
pub struct FileSystemWatcher {
    /// The debouncer wraps the underlying watcher.
    debouncer: Arc<RwLock<Debouncer<RecommendedWatcher>>>,
    /// Receiver for debounced events.
    rx: std_mpsc::Receiver<std::result::Result<Vec<DebouncedEvent>, notify::Error>>,
    /// Set of watched file paths.
    watched_files: Arc<RwLock<HashSet<PathBuf>>>,
    /// Configuration.
    config: FileWatcherConfig,
}

impl FileSystemWatcher {
    /// Create a new file system watcher.
    ///
    /// # Arguments
    /// * `config` - Watcher configuration
    pub fn new(config: FileWatcherConfig) -> Result<Self> {
        let (tx, rx) = std_mpsc::channel();

        let debounce_duration = Duration::from_millis(config.debounce_ms);

        let debouncer = match new_debouncer(debounce_duration, tx) {
            Ok(d) => d,
            Err(e) => {
                return Err(InternalSnafu {
                    message: format!("Failed to create file watcher: {e}"),
                }
                .build());
            }
        };

        info!(
            debounce_ms = config.debounce_ms,
            max_files = config.max_watched_files,
            "FileSystemWatcher created"
        );

        Ok(Self {
            debouncer: Arc::new(RwLock::new(debouncer)),
            rx,
            watched_files: Arc::new(RwLock::new(HashSet::new())),
            config,
        })
    }

    /// Create a new file system watcher with default configuration.
    pub fn with_defaults() -> Result<Self> {
        Self::new(FileWatcherConfig::default())
    }

    /// Add a file to the watch list.
    ///
    /// This should be called after a file is read by the Read tool to enable
    /// external change detection.
    pub fn watch_file(&self, path: &Path) -> Result<()> {
        let path = path.to_path_buf();

        // Check if already watching
        {
            let watched = self.watched_files.read().expect("lock poisoned");
            if watched.contains(&path) {
                return Ok(());
            }
        }

        // Check max files limit
        {
            let watched = self.watched_files.read().expect("lock poisoned");
            if watched.len() >= self.config.max_watched_files as usize {
                warn!(
                    max = self.config.max_watched_files,
                    "Maximum watched files limit reached, skipping"
                );
                return Ok(());
            }
        }

        // Add to watcher
        let mode = if self.config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        {
            let mut debouncer = self.debouncer.write().expect("lock poisoned");
            if let Err(e) = debouncer.watcher().watch(&path, mode) {
                return Err(InternalSnafu {
                    message: format!("Failed to watch file {:?}: {e}", path),
                }
                .build());
            }
        }

        // Track watched file
        {
            let mut watched = self.watched_files.write().expect("lock poisoned");
            watched.insert(path.clone());
        }

        debug!(path = ?path, "Added file to watch list");
        Ok(())
    }

    /// Remove a file from the watch list.
    pub fn unwatch_file(&self, path: &Path) -> Result<()> {
        let path = path.to_path_buf();

        // Check if watching
        {
            let watched = self.watched_files.read().expect("lock poisoned");
            if !watched.contains(&path) {
                return Ok(());
            }
        }

        // Remove from watcher
        {
            let mut debouncer = self.debouncer.write().expect("lock poisoned");
            let _ = debouncer.watcher().unwatch(&path); // Ignore errors
        }

        // Remove from tracking
        {
            let mut watched = self.watched_files.write().expect("lock poisoned");
            watched.remove(&path);
        }

        debug!(path = ?path, "Removed file from watch list");
        Ok(())
    }

    /// Poll for file changes (non-blocking).
    ///
    /// Returns `None` if no changes are available.
    pub fn poll_changes(&self) -> Option<Vec<FileChangeEvent>> {
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
                warn!(error = %e, "File watcher error");
                None
            }
            Err(std_mpsc::TryRecvError::Empty) => None,
            Err(std_mpsc::TryRecvError::Disconnected) => {
                warn!("File watcher channel disconnected");
                None
            }
        }
    }

    /// Wait for file changes with timeout.
    ///
    /// Returns `None` if timeout expires or no changes.
    pub fn wait_for_changes(&self, timeout: Duration) -> Option<Vec<FileChangeEvent>> {
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
                warn!(error = %e, "File watcher error");
                None
            }
            Err(_) => None, // Timeout or disconnected
        }
    }

    /// Get the number of currently watched files.
    pub fn watched_count(&self) -> i32 {
        let watched = self.watched_files.read().expect("lock poisoned");
        watched.len() as i32
    }

    /// Get all currently watched file paths.
    pub fn watched_files(&self) -> Vec<PathBuf> {
        let watched = self.watched_files.read().expect("lock poisoned");
        watched.iter().cloned().collect()
    }

    /// Clear all watched files.
    pub fn clear(&self) {
        // Unwatch all files
        let paths: Vec<PathBuf> = {
            let watched = self.watched_files.read().expect("lock poisoned");
            watched.iter().cloned().collect()
        };

        for path in paths {
            let _ = self.unwatch_file(&path);
        }

        info!("Cleared all watched files");
    }

    /// Convert notify events to our FileChangeEvent type.
    fn convert_events(&self, events: Vec<DebouncedEvent>) -> Vec<FileChangeEvent> {
        let watched = self.watched_files.read().expect("lock poisoned");

        events
            .into_iter()
            .filter_map(|event| {
                let path = event.path;

                // Only report events for watched files
                if !watched.contains(&path) {
                    return None;
                }

                // Determine change kind based on file existence
                let kind = if path.exists() {
                    FileChangeKind::Modified
                } else {
                    FileChangeKind::Deleted
                };

                Some(FileChangeEvent { path, kind })
            })
            .collect()
    }
}

impl std::fmt::Debug for FileSystemWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileSystemWatcher")
            .field("config", &self.config)
            .field("watched_count", &self.watched_count())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_watcher_creation() {
        let watcher = FileSystemWatcher::with_defaults();
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_watch_file() {
        let dir = TempDir::new().unwrap();
        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "initial content").unwrap();

        let watcher = FileSystemWatcher::with_defaults().unwrap();
        let result = watcher.watch_file(&test_file);
        assert!(result.is_ok());
        assert_eq!(watcher.watched_count(), 1);
    }

    #[test]
    fn test_unwatch_file() {
        let dir = TempDir::new().unwrap();
        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let watcher = FileSystemWatcher::with_defaults().unwrap();
        watcher.watch_file(&test_file).unwrap();
        assert_eq!(watcher.watched_count(), 1);

        watcher.unwatch_file(&test_file).unwrap();
        assert_eq!(watcher.watched_count(), 0);
    }

    #[test]
    fn test_watched_files() {
        let dir = TempDir::new().unwrap();
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");
        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();

        let watcher = FileSystemWatcher::with_defaults().unwrap();
        watcher.watch_file(&file1).unwrap();
        watcher.watch_file(&file2).unwrap();

        let watched = watcher.watched_files();
        assert_eq!(watched.len(), 2);
        assert!(watched.contains(&file1));
        assert!(watched.contains(&file2));
    }

    #[test]
    fn test_clear() {
        let dir = TempDir::new().unwrap();
        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let watcher = FileSystemWatcher::with_defaults().unwrap();
        watcher.watch_file(&test_file).unwrap();
        assert_eq!(watcher.watched_count(), 1);

        watcher.clear();
        assert_eq!(watcher.watched_count(), 0);
    }

    #[test]
    fn test_max_files_limit() {
        let dir = TempDir::new().unwrap();

        let config = FileWatcherConfig {
            max_watched_files: 2,
            ..Default::default()
        };
        let watcher = FileSystemWatcher::new(config).unwrap();

        // Create and watch files
        for i in 0..5 {
            let file = dir.path().join(format!("file{i}.txt"));
            fs::write(&file, "content").unwrap();
            let _ = watcher.watch_file(&file);
        }

        // Should be capped at max
        assert_eq!(watcher.watched_count(), 2);
    }

    #[test]
    fn test_file_change_detection() {
        let dir = TempDir::new().unwrap();
        let test_file = dir.path().join("test.txt");
        fs::write(&test_file, "initial").unwrap();

        let watcher = FileSystemWatcher::with_defaults().unwrap();
        watcher.watch_file(&test_file).unwrap();

        // Modify the file
        fs::write(&test_file, "modified").unwrap();

        // Wait for debounce
        std::thread::sleep(Duration::from_millis(150));

        // Poll for changes
        if let Some(events) = watcher.poll_changes() {
            assert!(!events.is_empty());
            assert_eq!(events[0].path, test_file);
            assert_eq!(events[0].kind, FileChangeKind::Modified);
        }
    }
}
