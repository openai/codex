//! File watcher for continuous code review

use anyhow::{Context, Result};
use async_channel::{Receiver, Sender};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Watch event types
#[derive(Debug, Clone)]
pub enum WatchEvent {
    /// File was created
    Created(PathBuf),
    /// File was modified
    Modified(PathBuf),
    /// File was deleted
    Deleted(PathBuf),
    /// Multiple files changed (batch)
    Batch(Vec<PathBuf>),
}

impl WatchEvent {
    /// Get all file paths from this event
    pub fn paths(&self) -> Vec<&Path> {
        match self {
            WatchEvent::Created(p) | WatchEvent::Modified(p) | WatchEvent::Deleted(p) => {
                vec![p.as_path()]
            }
            WatchEvent::Batch(paths) => paths.iter().map(|p| p.as_path()).collect(),
        }
    }

    /// Check if event should trigger review
    pub fn should_trigger_review(&self) -> bool {
        match self {
            WatchEvent::Modified(_) | WatchEvent::Created(_) | WatchEvent::Batch(_) => true,
            WatchEvent::Deleted(_) => false,
        }
    }
}

/// File watcher for monitoring code changes
pub struct FileWatcher {
    watch_dir: PathBuf,
    gitignore: Arc<RwLock<Gitignore>>,
    watch_patterns: Vec<String>,
    event_sender: Sender<WatchEvent>,
    event_receiver: Receiver<WatchEvent>,
    watcher: Option<RecommendedWatcher>,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(
        watch_dir: impl Into<PathBuf>,
        watch_patterns: Vec<String>,
        ignore_patterns: Vec<String>,
    ) -> Result<Self> {
        let watch_dir = watch_dir.into();

        // Build gitignore
        let mut builder = GitignoreBuilder::new(&watch_dir);
        for pattern in &ignore_patterns {
            builder.add_line(None, pattern)?;
        }
        let gitignore = builder.build()?;

        let (event_sender, event_receiver) = async_channel::unbounded();

        Ok(Self {
            watch_dir,
            gitignore: Arc::new(RwLock::new(gitignore)),
            watch_patterns,
            event_sender,
            event_receiver,
            watcher: None,
        })
    }

    /// Start watching for file changes
    pub fn start(&mut self) -> Result<()> {
        info!("Starting file watcher for {:?}", self.watch_dir);

        let sender = self.event_sender.clone();
        let gitignore = Arc::clone(&self.gitignore);
        let watch_patterns = self.watch_patterns.clone();

        let mut watcher = notify::recommended_watcher(move |result: Result<Event, _>| {
            match result {
                Ok(event) => {
                    if let Err(e) = Self::handle_notify_event(
                        event,
                        &sender,
                        &gitignore,
                        &watch_patterns,
                    ) {
                        error!("Error handling watch event: {}", e);
                    }
                }
                Err(e) => {
                    error!("Watch error: {}", e);
                }
            }
        })?;

        watcher.watch(&self.watch_dir, RecursiveMode::Recursive)?;
        self.watcher = Some(watcher);

        info!("File watcher started successfully");
        Ok(())
    }

    /// Stop watching
    pub fn stop(&mut self) -> Result<()> {
        if let Some(mut watcher) = self.watcher.take() {
            watcher.unwatch(&self.watch_dir)?;
            info!("File watcher stopped");
        }
        Ok(())
    }

    /// Get the next watch event
    pub async fn next_event(&self) -> Option<WatchEvent> {
        self.event_receiver.recv().await.ok()
    }

    /// Handle notify event and convert to WatchEvent
    fn handle_notify_event(
        event: Event,
        sender: &Sender<WatchEvent>,
        gitignore: &Arc<RwLock<Gitignore>>,
        watch_patterns: &[String],
    ) -> Result<()> {
        let rt = tokio::runtime::Handle::try_current()
            .context("No tokio runtime available")?;

        rt.block_on(async {
            let gitignore = gitignore.read().await;

            let paths: Vec<PathBuf> = event
                .paths
                .into_iter()
                .filter(|path| {
                    // Check if path matches gitignore
                    let is_dir = path.is_dir();
                    let matched = gitignore.matched(path, is_dir);
                    if matched.is_ignore() {
                        debug!("Ignoring path: {:?}", path);
                        return false;
                    }

                    // Check if path matches watch patterns
                    if !watch_patterns.is_empty() {
                        let path_str = path.to_string_lossy();
                        let matches = watch_patterns.iter().any(|pattern| {
                            // Simple glob matching - could be improved with proper glob crate
                            if pattern.contains("**") {
                                let suffix = pattern.trim_start_matches("**");
                                path_str.ends_with(suffix.trim_start_matches('/'))
                            } else {
                                path_str.contains(pattern)
                            }
                        });
                        if !matches {
                            debug!("Path doesn't match watch patterns: {:?}", path);
                            return false;
                        }
                    }

                    true
                })
                .collect();

            if paths.is_empty() {
                return Ok(());
            }

            let watch_event = match event.kind {
                EventKind::Create(_) => {
                    if paths.len() == 1 {
                        WatchEvent::Created(paths[0].clone())
                    } else {
                        WatchEvent::Batch(paths)
                    }
                }
                EventKind::Modify(_) => {
                    if paths.len() == 1 {
                        WatchEvent::Modified(paths[0].clone())
                    } else {
                        WatchEvent::Batch(paths)
                    }
                }
                EventKind::Remove(_) => {
                    if paths.len() == 1 {
                        WatchEvent::Deleted(paths[0].clone())
                    } else {
                        WatchEvent::Batch(paths)
                    }
                }
                _ => return Ok(()),
            };

            sender.send(watch_event).await?;
            Ok(())
        })
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_watch_event_paths() {
        let event = WatchEvent::Modified(PathBuf::from("/test/file.rs"));
        let paths = event.paths();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], Path::new("/test/file.rs"));
    }

    #[test]
    fn test_should_trigger_review() {
        assert!(WatchEvent::Modified(PathBuf::from("test.rs")).should_trigger_review());
        assert!(WatchEvent::Created(PathBuf::from("test.rs")).should_trigger_review());
        assert!(!WatchEvent::Deleted(PathBuf::from("test.rs")).should_trigger_review());
    }

    #[tokio::test]
    async fn test_file_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new(
            temp_dir.path(),
            vec!["**/*.rs".to_string()],
            vec!["**/target/**".to_string()],
        );
        assert!(watcher.is_ok());
    }
}
