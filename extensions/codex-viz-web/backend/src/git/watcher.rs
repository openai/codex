use crate::types::{RealtimeEvent, ChangeType};
use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, Debouncer, DebouncedEvent, FileIdMap};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, error, info};

/// Git file system watcher for real-time updates
pub struct GitWatcher {
    _debouncer: Debouncer<RecommendedWatcher, FileIdMap>,
    _event_tx: broadcast::Sender<RealtimeEvent>,
}

impl GitWatcher {
    /// Create a new GitWatcher for the given repository path
    pub fn new(repo_path: impl AsRef<Path>) -> Result<(Self, broadcast::Receiver<RealtimeEvent>)> {
        let repo_path = repo_path.as_ref().to_path_buf();
        let (event_tx, event_rx) = broadcast::channel(100);
        let event_tx_clone = event_tx.clone();

        // Create debouncer to avoid duplicate events
        let debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |result: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
                match result {
                    Ok(events) => {
                        for debounced_event in events {
                            if let Some(realtime_event) = Self::convert_event(&debounced_event.event) {
                                let _ = event_tx_clone.send(realtime_event);
                            }
                        }
                    }
                    Err(errors) => {
                        for error in errors {
                            error!("Watch error: {:?}", error);
                        }
                    }
                }
            },
        )?;

        // Watch .git directory for ref changes and object updates
        let git_dir = repo_path.join(".git");
        let mut debouncer_guard = debouncer;
        debouncer_guard
            .watcher()
            .watch(&git_dir, RecursiveMode::Recursive)?;

        info!("🔍 Watching git repository at: {:?}", repo_path);

        Ok((
            Self {
                _debouncer: debouncer_guard,
                _event_tx: event_tx,
            },
            event_rx,
        ))
    }

    /// Convert notify event to RealtimeEvent
    fn convert_event(event: &notify::Event) -> Option<RealtimeEvent> {
        match &event.kind {
            notify::EventKind::Create(_) => {
                if let Some(path) = event.paths.first() {
                    Self::classify_git_change(path, ChangeType::Added)
                } else {
                    None
                }
            }
            notify::EventKind::Modify(_) => {
                if let Some(path) = event.paths.first() {
                    Self::classify_git_change(path, ChangeType::Modified)
                } else {
                    None
                }
            }
            notify::EventKind::Remove(_) => {
                if let Some(path) = event.paths.first() {
                    Self::classify_git_change(path, ChangeType::Deleted)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Classify what type of git change occurred
    fn classify_git_change(path: &PathBuf, change_type: ChangeType) -> Option<RealtimeEvent> {
        let path_str = path.to_string_lossy();

        // Check if it's a ref change (new commit, branch, etc.)
        if path_str.contains(".git/refs/") {
            debug!("Detected ref change: {:?}", path);
            // Would need to parse the actual change
            // For now, just return a file changed event
            return Some(RealtimeEvent::FileChanged {
                path: path_str.to_string(),
                change_type,
            });
        }

        // Check if it's an object change
        if path_str.contains(".git/objects/") {
            debug!("Detected object change: {:?}", path);
            return Some(RealtimeEvent::FileChanged {
                path: path_str.to_string(),
                change_type,
            });
        }

        None
    }

}

