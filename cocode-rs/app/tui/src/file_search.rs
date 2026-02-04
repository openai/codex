//! File search manager with debouncing for autocomplete.
//!
//! This module provides debounced file search for the @mention autocomplete
//! feature, aligned with Claude Code's FileIndex system.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::state::FileSuggestionItem;

/// Debounce delay in milliseconds (aligned with codex-rs pattern).
const DEBOUNCE_MS: u64 = 100;

/// Maximum number of suggestions to return.
const MAX_SUGGESTIONS: i32 = 15;

/// Events sent from the file search manager to the TUI.
#[derive(Debug, Clone)]
pub enum FileSearchEvent {
    /// Search results are ready.
    SearchResult {
        /// The query that was searched.
        query: String,
        /// The start position of the @mention.
        start_pos: i32,
        /// The matching suggestions.
        suggestions: Vec<FileSuggestionItem>,
    },
}

/// Manages file search with debouncing.
///
/// This struct handles:
/// - Debounced search scheduling (100ms delay)
/// - Cancellation of in-flight searches when query changes
/// - Background index refresh
pub struct FileSearchManager {
    /// Working directory for file discovery.
    cwd: PathBuf,
    /// Cached file index.
    file_index: Arc<RwLock<FileIndexCache>>,
    /// Currently scheduled search (debounce timer).
    pending_search: Option<PendingSearch>,
    /// Event sender to notify TUI of results.
    event_tx: mpsc::Sender<FileSearchEvent>,
}

/// A pending search waiting for debounce timeout.
struct PendingSearch {
    /// The query to search for.
    query: String,
    /// Start position of the @mention.
    start_pos: i32,
    /// When the search was scheduled.
    scheduled_at: Instant,
    /// Handle to cancel the search task.
    handle: JoinHandle<()>,
}

/// Cached file index with TTL.
struct FileIndexCache {
    /// List of files (relative paths).
    files: Vec<String>,
    /// Directory prefixes for navigation.
    directories: Vec<String>,
    /// Last refresh time.
    last_refresh: Option<Instant>,
    /// Whether a refresh is in progress.
    refreshing: bool,
}

impl Default for FileIndexCache {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            directories: Vec::new(),
            last_refresh: None,
            refreshing: false,
        }
    }
}

impl FileIndexCache {
    /// Cache TTL in seconds.
    const CACHE_TTL_SECS: u64 = 60;

    /// Check if cache is still valid.
    fn is_valid(&self) -> bool {
        self.last_refresh
            .map(|t| t.elapsed() < Duration::from_secs(Self::CACHE_TTL_SECS))
            .unwrap_or(false)
    }
}

impl FileSearchManager {
    /// Create a new file search manager.
    pub fn new(cwd: PathBuf, event_tx: mpsc::Sender<FileSearchEvent>) -> Self {
        Self {
            cwd,
            file_index: Arc::new(RwLock::new(FileIndexCache::default())),
            pending_search: None,
            event_tx,
        }
    }

    /// Handle a query change from user input.
    ///
    /// This method debounces the search - if called multiple times in quick
    /// succession, only the last query will be searched after the debounce
    /// delay.
    pub fn on_query(&mut self, query: String, start_pos: i32) {
        // Cancel any pending search
        if let Some(pending) = self.pending_search.take() {
            pending.handle.abort();
        }

        // Don't search empty queries
        if query.is_empty() {
            return;
        }

        // Schedule a new debounced search
        let file_index = self.file_index.clone();
        let cwd = self.cwd.clone();
        let event_tx = self.event_tx.clone();
        let query_clone = query.clone();

        let handle = tokio::spawn(async move {
            // Wait for debounce delay
            tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;

            // Perform the search
            let suggestions = search_files(&file_index, &cwd, &query_clone).await;

            // Send results
            let _ = event_tx
                .send(FileSearchEvent::SearchResult {
                    query: query_clone,
                    start_pos,
                    suggestions,
                })
                .await;
        });

        self.pending_search = Some(PendingSearch {
            query,
            start_pos,
            scheduled_at: Instant::now(),
            handle,
        });
    }

    /// Cancel any pending search.
    pub fn cancel(&mut self) {
        if let Some(pending) = self.pending_search.take() {
            pending.handle.abort();
        }
    }

    /// Force refresh the file index.
    pub fn refresh_index(&self) {
        let file_index = self.file_index.clone();
        let cwd = self.cwd.clone();

        tokio::spawn(async move {
            refresh_index(&file_index, &cwd).await;
        });
    }

    /// Get the current working directory.
    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }
}

/// Search files using the cached index.
async fn search_files(
    file_index: &Arc<RwLock<FileIndexCache>>,
    cwd: &PathBuf,
    query: &str,
) -> Vec<FileSuggestionItem> {
    // Check if cache needs refresh
    {
        let cache = file_index.read().await;
        if !cache.is_valid() && !cache.refreshing {
            drop(cache);
            refresh_index(file_index, cwd).await;
        }
    }

    // Handle directory navigation (query ends with /)
    if query.ends_with('/') {
        return search_directories(file_index, query).await;
    }

    // Fuzzy search files
    search_files_fuzzy(file_index, cwd, query).await
}

/// Search for directory suggestions.
async fn search_directories(
    file_index: &Arc<RwLock<FileIndexCache>>,
    prefix: &str,
) -> Vec<FileSuggestionItem> {
    let cache = file_index.read().await;
    let prefix_lower = prefix.to_lowercase();

    cache
        .directories
        .iter()
        .filter(|d| d.to_lowercase().starts_with(&prefix_lower))
        .take(MAX_SUGGESTIONS as usize)
        .map(|d| FileSuggestionItem {
            path: d.clone(),
            display_text: format!("{d}/"),
            score: 0,
            match_indices: vec![],
            is_directory: true,
        })
        .collect()
}

/// Fuzzy search files using nucleo matcher.
async fn search_files_fuzzy(
    _file_index: &Arc<RwLock<FileIndexCache>>,
    cwd: &PathBuf,
    query: &str,
) -> Vec<FileSuggestionItem> {
    use std::num::NonZero;
    use std::sync::atomic::AtomicBool;

    // Use the file-search crate's run function
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let limit = NonZero::new(MAX_SUGGESTIONS as usize).expect("MAX_SUGGESTIONS > 0");

    match cocode_file_search::run(
        query,
        limit,
        cwd,
        vec![],
        NonZero::new(2).expect("2 > 0"),
        cancel_flag,
        true, // compute_indices for highlighting
        true, // respect_gitignore
    ) {
        Ok(results) => results
            .matches
            .into_iter()
            .map(|m| FileSuggestionItem {
                path: m.path.clone(),
                display_text: m.path,
                score: m.score,
                match_indices: m
                    .indices
                    .unwrap_or_default()
                    .into_iter()
                    .map(|i| i as i32)
                    .collect(),
                is_directory: false,
            })
            .collect(),
        Err(e) => {
            tracing::warn!("File search failed: {e}");
            Vec::new()
        }
    }
}

/// Refresh the file index from disk.
async fn refresh_index(file_index: &Arc<RwLock<FileIndexCache>>, cwd: &PathBuf) {
    // Mark as refreshing
    {
        let mut cache = file_index.write().await;
        if cache.refreshing {
            return;
        }
        cache.refreshing = true;
    }

    // Discover files
    let result = cocode_file_search::discover_files(cwd).await;

    // Update cache
    {
        let mut cache = file_index.write().await;
        cache.files = result.files;
        cache.directories = result.directories;
        cache.last_refresh = Some(Instant::now());
        cache.refreshing = false;
    }
}

/// Create a channel for file search events.
pub fn create_file_search_channel() -> (
    mpsc::Sender<FileSearchEvent>,
    mpsc::Receiver<FileSearchEvent>,
) {
    mpsc::channel(16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_file_index_cache_validity() {
        let cache = FileIndexCache::default();
        assert!(!cache.is_valid());
    }

    #[tokio::test]
    async fn test_create_file_search_manager() {
        let (tx, _rx) = create_file_search_channel();
        let manager = FileSearchManager::new(PathBuf::from("/tmp"), tx);
        assert_eq!(manager.cwd(), &PathBuf::from("/tmp"));
    }

    #[tokio::test]
    async fn test_cancel_pending_search() {
        let (tx, _rx) = create_file_search_channel();
        let mut manager = FileSearchManager::new(PathBuf::from("/tmp"), tx);

        // Schedule a search
        manager.on_query("test".to_string(), 0);
        assert!(manager.pending_search.is_some());

        // Cancel it
        manager.cancel();
        assert!(manager.pending_search.is_none());
    }
}
