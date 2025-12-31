//! Glob Files Handler - Find files by pattern matching
//!
//! This module provides the GlobFilesHandler which finds files matching
//! a glob pattern, respecting .gitignore and .ignore files.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_file_ignore::IgnoreConfig;
use codex_file_ignore::IgnoreService;
use glob::Pattern;
use serde::Deserialize;
use std::cmp::Ordering;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;

/// Internal safety limit (not exposed to LLM)
const INTERNAL_LIMIT: usize = 200;

/// Threshold for "recent" files (24 hours)
const RECENT_THRESHOLD_SECS: u64 = 24 * 60 * 60;

/// Glob Files tool arguments
#[derive(Debug, Clone, Deserialize)]
struct GlobFilesArgs {
    pattern: String,
    path: Option<String>,
    #[serde(default)]
    case_sensitive: bool,
}

/// File entry with metadata for sorting
struct FileEntry {
    path: PathBuf,
    mtime: Option<SystemTime>,
}

/// Glob Files Handler
pub struct GlobFilesHandler;

#[async_trait]
impl ToolHandler for GlobFilesHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for glob_files".to_string(),
                ));
            }
        };

        let args: GlobFilesArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // Validate pattern
        if args.pattern.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "Pattern must not be empty".to_string(),
            ));
        }

        // 2. Resolve search path
        let search_path = invocation.turn.resolve_path(args.path.clone());

        // Verify path exists
        if !search_path.exists() {
            return Err(FunctionCallError::RespondToModel(format!(
                "Path does not exist: {}",
                search_path.display()
            )));
        }

        if !search_path.is_dir() {
            return Err(FunctionCallError::RespondToModel(format!(
                "Path is not a directory: {}",
                search_path.display()
            )));
        }

        // 3. Create ignore service with fixed config (always respect ignores)
        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: true, // glob should find hidden files (dot files)
            follow_links: false,
            custom_excludes: Vec::new(),
        };
        let ignore_service = IgnoreService::new(ignore_config);

        // 4. Build walker with ignore rules
        let walker = ignore_service.create_walk_builder(&search_path);

        // 5. Compile glob pattern
        // Note: case_sensitive is handled at match time in matches_with(), not at pattern compile
        let glob_pattern = Pattern::new(&args.pattern)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid glob pattern: {e}")))?;

        // 6. Collect matching files
        let mut entries: Vec<FileEntry> = Vec::with_capacity(INTERNAL_LIMIT);

        for entry_result in walker.build() {
            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Skip directories
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }

            // Get relative path for pattern matching
            let rel_path = match entry.path().strip_prefix(&search_path) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let path_str = rel_path.to_string_lossy();

            // Match pattern
            let matches = if args.case_sensitive {
                glob_pattern.matches(&path_str)
            } else {
                glob_pattern.matches_with(
                    &path_str,
                    glob::MatchOptions {
                        case_sensitive: false,
                        require_literal_separator: false,
                        require_literal_leading_dot: false,
                    },
                )
            };

            if matches {
                let mtime = entry.metadata().ok().and_then(|m| m.modified().ok());

                entries.push(FileEntry {
                    path: entry.path().to_path_buf(),
                    mtime,
                });
            }
        }

        // 7. Sort by mtime (recent files first within 24h, then alphabetical)
        sort_by_mtime(&mut entries);

        // 8. Format output
        let total = entries.len();
        let results: Vec<String> = entries
            .iter()
            .take(INTERNAL_LIMIT)
            .map(|e| {
                // Return relative path from search_path
                e.path
                    .strip_prefix(&search_path)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| e.path.display().to_string())
            })
            .collect();

        let content = if results.is_empty() {
            format!("No files found matching pattern \"{}\"", args.pattern)
        } else {
            let mut output = format!(
                "Found {} file(s) matching \"{}\", sorted by modification time (newest first):\n",
                total, args.pattern
            );
            output.push_str(&results.join("\n"));
            if total > INTERNAL_LIMIT {
                output.push_str(&format!("\n... and {} more files", total - INTERNAL_LIMIT));
            }
            output
        };

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(!results.is_empty()),
        })
    }
}

/// Sort files by modification time.
/// Recent files (within 24 hours) come first, sorted by mtime (newest first).
/// Older files are sorted alphabetically.
fn sort_by_mtime(entries: &mut [FileEntry]) {
    let now = SystemTime::now();
    let threshold = Duration::from_secs(RECENT_THRESHOLD_SECS);

    entries.sort_by(|a, b| {
        let is_recent_a = is_recent(&a.mtime, now, threshold);
        let is_recent_b = is_recent(&b.mtime, now, threshold);

        match (is_recent_a, is_recent_b) {
            // Both recent: newest first
            (true, true) => compare_mtime_desc(&a.mtime, &b.mtime),
            // a recent, b not: a comes first
            (true, false) => Ordering::Less,
            // b recent, a not: b comes first
            (false, true) => Ordering::Greater,
            // Both old: alphabetical
            (false, false) => a.path.cmp(&b.path),
        }
    });
}

/// Check if a file is recent (within threshold of now)
fn is_recent(mtime: &Option<SystemTime>, now: SystemTime, threshold: Duration) -> bool {
    mtime
        .and_then(|t| now.duration_since(t).ok())
        .map(|d| d < threshold)
        .unwrap_or(false)
}

/// Compare modification times in descending order (newest first)
fn compare_mtime_desc(a: &Option<SystemTime>, b: &Option<SystemTime>) -> Ordering {
    match (a, b) {
        (Some(ta), Some(tb)) => tb.cmp(ta), // Reverse order for descending
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_file_ignore::IgnoreConfig;
    use codex_file_ignore::IgnoreService;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_glob_pattern_valid() {
        let pattern = Pattern::new("**/*.rs").expect("valid pattern");
        assert!(pattern.matches("src/main.rs"));
        assert!(pattern.matches("deep/nested/file.rs"));
        assert!(!pattern.matches("file.txt"));
    }

    #[test]
    fn test_glob_pattern_case_insensitive() {
        let pattern = Pattern::new("*.RS").expect("valid pattern");
        let matches = pattern.matches_with(
            "file.rs",
            glob::MatchOptions {
                case_sensitive: false,
                ..Default::default()
            },
        );
        assert!(matches);
    }

    #[test]
    fn test_is_recent() {
        let now = SystemTime::now();
        let threshold = Duration::from_secs(60); // 1 minute

        // Recent file
        let recent = Some(now - Duration::from_secs(30));
        assert!(is_recent(&recent, now, threshold));

        // Old file
        let old = Some(now - Duration::from_secs(120));
        assert!(!is_recent(&old, now, threshold));

        // No mtime
        assert!(!is_recent(&None, now, threshold));
    }

    #[test]
    fn test_sort_by_mtime() {
        let now = SystemTime::now();
        let one_hour_ago = now - Duration::from_secs(3600);
        let two_days_ago = now - Duration::from_secs(2 * 24 * 3600);

        let mut entries = vec![
            FileEntry {
                path: PathBuf::from("old_a.txt"),
                mtime: Some(two_days_ago),
            },
            FileEntry {
                path: PathBuf::from("recent.txt"),
                mtime: Some(one_hour_ago),
            },
            FileEntry {
                path: PathBuf::from("old_b.txt"),
                mtime: Some(two_days_ago),
            },
        ];

        sort_by_mtime(&mut entries);

        // Recent file should be first
        assert_eq!(entries[0].path, PathBuf::from("recent.txt"));
        // Old files sorted alphabetically
        assert_eq!(entries[1].path, PathBuf::from("old_a.txt"));
        assert_eq!(entries[2].path, PathBuf::from("old_b.txt"));
    }

    #[tokio::test]
    async fn test_glob_files_integration() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir = temp.path();

        // Create test files
        fs::write(dir.join("file1.rs"), "rust code")?;
        fs::write(dir.join("file2.rs"), "more rust")?;
        fs::write(dir.join("file.txt"), "text file")?;

        // Create subdirectory with more files
        fs::create_dir(dir.join("src"))?;
        fs::write(dir.join("src/main.rs"), "main")?;

        // Create ignore to test filtering
        fs::write(dir.join(".ignore"), "*.txt")?;

        // Test pattern matching
        let pattern = Pattern::new("**/*.rs")?;
        assert!(pattern.matches("file1.rs"));
        assert!(pattern.matches("src/main.rs"));
        assert!(!pattern.matches("file.txt"));

        Ok(())
    }

    #[tokio::test]
    async fn test_ignore_filters_files() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir = temp.path();

        // Create files
        fs::write(dir.join("keep.rs"), "rust code")?;
        fs::write(dir.join("ignore.log"), "log file")?;

        // Create .ignore to filter .log files
        fs::write(dir.join(".ignore"), "*.log")?;

        // Walk and verify
        let ignore_config = IgnoreConfig {
            respect_gitignore: true,
            respect_ignore: true,
            include_hidden: false,
            ..Default::default()
        };
        let ignore_service = IgnoreService::new(ignore_config);
        let walker = ignore_service.create_walk_builder(dir);

        let files: Vec<String> = walker
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .filter_map(|e| {
                e.path()
                    .strip_prefix(dir)
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .collect();

        assert!(files.iter().any(|f| f.ends_with("keep.rs")));
        assert!(!files.iter().any(|f| f.ends_with("ignore.log")));

        Ok(())
    }

    #[tokio::test]
    async fn test_nested_ignore_override() -> anyhow::Result<()> {
        let temp = tempdir()?;
        let dir = temp.path();

        // Create nested structure
        fs::create_dir_all(dir.join("src"))?;
        fs::write(dir.join("root.log"), "root log")?;
        fs::write(dir.join("src/keep.log"), "src log to keep")?;
        fs::write(dir.join("src/main.rs"), "main")?;

        // Root: ignore all .log
        fs::write(dir.join(".ignore"), "*.log")?;
        // Nested: un-ignore .log in src/
        fs::write(dir.join("src/.ignore"), "!*.log")?;

        let ignore_service = IgnoreService::new(IgnoreConfig::default());
        let walker = ignore_service.create_walk_builder(dir);

        let files: Vec<String> = walker
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .filter_map(|e| {
                e.path()
                    .strip_prefix(dir)
                    .ok()
                    .map(|p| p.to_string_lossy().to_string())
            })
            .collect();

        // root.log should be ignored
        assert!(!files.iter().any(|f| f == "root.log"));
        // src/keep.log should be kept (nested .ignore overrides)
        assert!(files.iter().any(|f| f.contains("keep.log")));
        // src/main.rs should be kept
        assert!(files.iter().any(|f| f.contains("main.rs")));

        Ok(())
    }
}
