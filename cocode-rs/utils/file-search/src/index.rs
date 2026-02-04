//! File index with caching for autocomplete suggestions.
//!
//! This module provides a cached file index for fast file suggestions,
//! aligned with Claude Code's FileIndex system.

use std::collections::HashSet;
use std::num::NonZero;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use tokio::process::Command;
use tokio::sync::RwLock;

use crate::FileSearchResults;
use crate::run;

/// Maximum number of suggestions to return.
pub const MAX_SUGGESTIONS: i32 = 15;

/// Cache time-to-live in seconds.
pub const CACHE_TTL_SECS: u64 = 60;

/// A single file suggestion with relevance score.
#[derive(Debug, Clone)]
pub struct FileSuggestion {
    /// Relative path to the file.
    pub path: String,
    /// Display text (may include icons or formatting).
    pub display_text: String,
    /// Relevance score from fuzzy matching (higher = better).
    pub score: u32,
    /// Character indices that matched the query (for highlighting).
    pub match_indices: Vec<i32>,
    /// Whether this is a directory (for @src/ style navigation).
    pub is_directory: bool,
}

impl FileSuggestion {
    /// Create a new file suggestion.
    pub fn new(path: String, score: u32, indices: Vec<u32>) -> Self {
        let display_text = path.clone();
        let match_indices = indices.into_iter().map(|i| i as i32).collect();
        Self {
            path,
            display_text,
            score,
            match_indices,
            is_directory: false,
        }
    }

    /// Create a directory suggestion.
    pub fn directory(path: String) -> Self {
        let display_text = format!("{path}/");
        Self {
            path,
            display_text,
            score: 0, // Directories appear when query ends with /
            match_indices: vec![],
            is_directory: true,
        }
    }
}

/// Result of file discovery operation.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryResult {
    /// List of tracked files (relative paths).
    pub files: Vec<String>,
    /// Extracted directory prefixes.
    pub directories: Vec<String>,
}

/// Cached file index with background refresh support.
pub struct FileIndex {
    /// Cached files (relative paths).
    files: Vec<String>,
    /// Extracted directory prefixes for navigation.
    directories: Vec<String>,
    /// Last refresh timestamp.
    last_refresh: Option<Instant>,
    /// Whether a refresh is in progress.
    refresh_in_progress: Arc<AtomicBool>,
    /// Working directory.
    cwd: std::path::PathBuf,
}

impl FileIndex {
    /// Create a new file index for the given directory.
    pub fn new(cwd: impl Into<std::path::PathBuf>) -> Self {
        Self {
            files: Vec::new(),
            directories: Vec::new(),
            last_refresh: None,
            refresh_in_progress: Arc::new(AtomicBool::new(false)),
            cwd: cwd.into(),
        }
    }

    /// Check if the cache is still valid.
    pub fn is_cache_valid(&self) -> bool {
        self.last_refresh
            .map(|t| t.elapsed() < Duration::from_secs(CACHE_TTL_SECS))
            .unwrap_or(false)
    }

    /// Get file suggestions for a query.
    ///
    /// Uses cache-first strategy with 60s TTL.
    /// If query ends with `/`, shows directory suggestions.
    pub async fn get_suggestions(&mut self, query: &str, max_results: i32) -> Vec<FileSuggestion> {
        // Refresh cache if needed
        if !self.is_cache_valid() && !self.refresh_in_progress.load(Ordering::Relaxed) {
            self.refresh().await;
        }

        // Handle directory navigation (query ends with /)
        if query.ends_with('/') {
            return self.get_directory_suggestions(query, max_results);
        }

        // Fuzzy search files
        self.search_files(query, max_results)
    }

    /// Get directory suggestions for prefix navigation.
    fn get_directory_suggestions(&self, prefix: &str, max_results: i32) -> Vec<FileSuggestion> {
        let prefix_lower = prefix.to_lowercase();

        self.directories
            .iter()
            .filter(|d| d.to_lowercase().starts_with(&prefix_lower))
            .take(max_results as usize)
            .map(|d| FileSuggestion::directory(d.clone()))
            .collect()
    }

    /// Search files using fuzzy matching.
    fn search_files(&self, query: &str, max_results: i32) -> Vec<FileSuggestion> {
        if query.is_empty() || self.files.is_empty() {
            return Vec::new();
        }

        // Use the existing run() function with a temporary directory
        // containing symlinks to our cached files
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let limit = NonZero::new(max_results as usize).unwrap_or(NonZero::new(15).expect("15 > 0"));

        match run(
            query,
            limit,
            &self.cwd,
            vec![],
            NonZero::new(2).expect("2 > 0"),
            cancel_flag,
            true, // compute_indices for highlighting
            true, // respect_gitignore
        ) {
            Ok(FileSearchResults { matches, .. }) => matches
                .into_iter()
                .map(|m| FileSuggestion::new(m.path, m.score, m.indices.unwrap_or_default()))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Refresh the file index.
    pub async fn refresh(&mut self) {
        if self.refresh_in_progress.swap(true, Ordering::SeqCst) {
            // Another refresh is in progress
            return;
        }

        let result = discover_files(&self.cwd).await;
        self.files = result.files;
        self.directories = result.directories;
        self.last_refresh = Some(Instant::now());
        self.refresh_in_progress.store(false, Ordering::SeqCst);
    }

    /// Force a background refresh.
    pub fn refresh_background(index: Arc<RwLock<Self>>) {
        tokio::spawn(async move {
            let mut guard = index.write().await;
            guard.refresh().await;
        });
    }

    /// Get the current file count.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Get the current directory count.
    pub fn directory_count(&self) -> usize {
        self.directories.len()
    }
}

/// Discover project files using git or ripgrep fallback.
///
/// Strategy (aligned with Claude Code):
/// 1. Try: `git ls-files --recurse-submodules`
/// 2. Fallback: `rg --files --follow --hidden --glob '!.git/'`
pub async fn discover_files(cwd: &Path) -> DiscoveryResult {
    // Try git ls-files first
    if let Some(result) = try_git_ls_files(cwd).await {
        return result;
    }

    // Fallback to ripgrep
    if let Some(result) = try_ripgrep_files(cwd).await {
        return result;
    }

    // Empty result if both fail
    DiscoveryResult::default()
}

/// Try to discover files using git ls-files.
async fn try_git_ls_files(cwd: &Path) -> Option<DiscoveryResult> {
    let output = Command::new("git")
        .arg("ls-files")
        .arg("--recurse-submodules")
        .current_dir(cwd)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = stdout
        .lines()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let directories = extract_directories(&files);

    Some(DiscoveryResult { files, directories })
}

/// Try to discover files using ripgrep.
async fn try_ripgrep_files(cwd: &Path) -> Option<DiscoveryResult> {
    let output = Command::new("rg")
        .args(["--files", "--follow", "--hidden", "--glob", "!.git/"])
        .current_dir(cwd)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = stdout
        .lines()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let directories = extract_directories(&files);

    Some(DiscoveryResult { files, directories })
}

/// Extract directory prefixes from file paths.
///
/// Example: `src/components/Button.tsx` → `["src/", "src/components/"]`
pub fn extract_directories(files: &[String]) -> Vec<String> {
    let mut dirs: HashSet<String> = HashSet::new();

    for path in files {
        let mut prefix = String::new();
        for component in path.split('/') {
            if prefix.is_empty() {
                // First component
                if !component.contains('.') {
                    // Likely a directory, not a file
                    dirs.insert(format!("{component}/"));
                    prefix = format!("{component}/");
                }
            } else {
                // Check if this is a directory (not the file name)
                let next_prefix = format!("{prefix}{component}/");
                // Only add if there are more components after this
                if path.starts_with(&next_prefix) {
                    dirs.insert(next_prefix.clone());
                    prefix = next_prefix;
                } else {
                    break;
                }
            }
        }
    }

    let mut result: Vec<String> = dirs.into_iter().collect();
    result.sort();
    result
}

/// Shared file index for use across the application.
pub type SharedFileIndex = Arc<RwLock<FileIndex>>;

/// Create a shared file index.
pub fn create_shared_index(cwd: impl Into<std::path::PathBuf>) -> SharedFileIndex {
    Arc::new(RwLock::new(FileIndex::new(cwd)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_directories_basic() {
        let files = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/utils/mod.rs".to_string(),
            "Cargo.toml".to_string(),
        ];

        let dirs = extract_directories(&files);

        assert!(dirs.contains(&"src/".to_string()));
        assert!(dirs.contains(&"src/utils/".to_string()));
    }

    #[test]
    fn test_extract_directories_nested() {
        let files = vec![
            "src/components/Button.tsx".to_string(),
            "src/components/Input.tsx".to_string(),
            "src/utils/helpers.ts".to_string(),
        ];

        let dirs = extract_directories(&files);

        assert!(dirs.contains(&"src/".to_string()));
        assert!(dirs.contains(&"src/components/".to_string()));
        assert!(dirs.contains(&"src/utils/".to_string()));
    }

    #[test]
    fn test_file_suggestion_new() {
        let suggestion = FileSuggestion::new("src/main.rs".to_string(), 100, vec![0, 4, 5]);

        assert_eq!(suggestion.path, "src/main.rs");
        assert_eq!(suggestion.score, 100);
        assert_eq!(suggestion.match_indices, vec![0, 4, 5]);
        assert!(!suggestion.is_directory);
    }

    #[test]
    fn test_file_suggestion_directory() {
        let suggestion = FileSuggestion::directory("src".to_string());

        assert_eq!(suggestion.path, "src");
        assert_eq!(suggestion.display_text, "src/");
        assert!(suggestion.is_directory);
    }

    #[test]
    fn test_file_index_cache_validity() {
        let index = FileIndex::new("/tmp");
        assert!(!index.is_cache_valid());
    }

    #[tokio::test]
    async fn test_discovery_result_default() {
        let result = DiscoveryResult::default();
        assert!(result.files.is_empty());
        assert!(result.directories.is_empty());
    }
}
