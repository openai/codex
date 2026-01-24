//! Grep Files Handler - Search for files containing a pattern
//!
//! This module provides the GrepFilesHandler which searches for files
//! whose contents match a pattern, returning file paths.

use std::fs;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::SearcherBuilder;
use grep_searcher::sinks::UTF8;
use ignore::WalkBuilder;
use serde::Deserialize;
use tokio::time::timeout;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct GrepFilesHandler;

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 2000;
/// Search timeout to prevent long-running searches
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Deserialize)]
struct GrepFilesArgs {
    pattern: String,
    #[serde(default)]
    include: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[async_trait]
impl ToolHandler for GrepFilesHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "grep_files handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: GrepFilesArgs = parse_arguments(&arguments)?;

        let pattern = args.pattern.trim();
        if pattern.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "pattern must not be empty".to_string(),
            ));
        }

        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let limit = args.limit.min(MAX_LIMIT);
        let search_path = turn.resolve_path(args.path.clone());

        verify_path_exists(&search_path).await?;

        let include = args.include.as_deref().map(str::trim).and_then(|val| {
            if val.is_empty() {
                None
            } else {
                Some(val.to_string())
            }
        });

        // Run search with timeout using spawn_blocking for sync grep operations
        let pattern_owned = pattern.to_string();
        let search_path_owned = search_path.clone();
        let include_owned = include.clone();

        let search_future = tokio::task::spawn_blocking(move || {
            run_grep_search(
                &pattern_owned,
                include_owned.as_deref(),
                &search_path_owned,
                limit,
            )
        });

        let search_results = timeout(COMMAND_TIMEOUT, search_future)
            .await
            .map_err(|_| {
                FunctionCallError::RespondToModel(
                    "grep search timed out after 30 seconds".to_string(),
                )
            })?
            .map_err(|e| {
                FunctionCallError::RespondToModel(format!("grep search task failed: {e}"))
            })??;

        if search_results.is_empty() {
            Ok(ToolOutput::Function {
                content: "No matches found.".to_string(),
                content_items: None,
                success: Some(false),
            })
        } else {
            Ok(ToolOutput::Function {
                content: search_results.join("\n"),
                content_items: None,
                success: Some(true),
            })
        }
    }
}

async fn verify_path_exists(path: &Path) -> Result<(), FunctionCallError> {
    tokio::fs::metadata(path).await.map_err(|err| {
        FunctionCallError::RespondToModel(format!("unable to access `{}`: {err}", path.display()))
    })?;
    Ok(())
}

/// Search for files containing pattern using grep crate
fn run_grep_search(
    pattern: &str,
    include: Option<&str>,
    search_path: &Path,
    limit: usize,
) -> Result<Vec<String>, FunctionCallError> {
    // Build regex matcher
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(pattern)
        .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid regex: {e}")))?;

    // Build file walker (respects .gitignore, .ignore)
    let mut walker_builder = WalkBuilder::new(search_path);
    walker_builder.hidden(false).git_ignore(true).ignore(true);

    // Apply glob filter if specified
    if let Some(glob_pattern) = include {
        let mut types_builder = ignore::types::TypesBuilder::new();
        types_builder.add("custom", glob_pattern).ok();
        types_builder.select("custom");
        if let Ok(types) = types_builder.build() {
            walker_builder.types(types);
        }
    }

    // Collect matching file paths with mtime for sorting
    let mut results: Vec<(String, Option<std::time::SystemTime>)> = Vec::new();

    for entry in walker_builder.build().flatten() {
        // Check limit
        if results.len() >= limit {
            break;
        }

        // Skip non-files
        let file_type = entry.file_type();
        if file_type.map(|t| !t.is_file()).unwrap_or(true) {
            continue;
        }

        let file_path = entry.path().to_path_buf();
        let mtime = fs::metadata(&file_path)
            .ok()
            .and_then(|m| m.modified().ok());
        let file_path_str = file_path.display().to_string();

        // Check if file contains pattern - collect first match directly
        // Note: UTF8 sink requires line numbers to be enabled
        let mut searcher = SearcherBuilder::new().line_number(true).build();
        let mut match_found = Vec::new();

        let search_result = searcher.search_path(
            &matcher,
            &file_path,
            UTF8(|_line_num, _line| {
                match_found.push(true);
                Ok(false) // Stop after first match
            }),
        );

        // Ignore search errors (binary files, permission issues, etc.)
        if let Err(e) = search_result {
            tracing::debug!("Search error in {}: {}", file_path.display(), e);
            continue;
        }

        if !match_found.is_empty() && results.len() < limit {
            results.push((file_path_str, mtime));
        }
    }

    results.sort_by(|a, b| match (&b.1, &a.1) {
        (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    Ok(results.into_iter().map(|(path, _)| path).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_grep_search_returns_results() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();
        std::fs::write(dir.join("match_one.txt"), "alpha beta gamma").unwrap();
        std::fs::write(dir.join("match_two.txt"), "alpha delta").unwrap();
        std::fs::write(dir.join("other.txt"), "omega").unwrap();

        let results = run_grep_search("alpha", None, dir, 10).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|path| path.ends_with("match_one.txt")));
        assert!(results.iter().any(|path| path.ends_with("match_two.txt")));
    }

    #[test]
    fn test_grep_search_with_glob_filter() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();
        std::fs::write(dir.join("match_one.rs"), "alpha beta gamma").unwrap();
        std::fs::write(dir.join("match_two.txt"), "alpha delta").unwrap();

        let results = run_grep_search("alpha", Some("*.rs"), dir, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results.iter().all(|path| path.ends_with("match_one.rs")));
    }

    #[test]
    fn test_grep_search_respects_limit() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();
        std::fs::write(dir.join("one.txt"), "alpha one").unwrap();
        std::fs::write(dir.join("two.txt"), "alpha two").unwrap();
        std::fs::write(dir.join("three.txt"), "alpha three").unwrap();

        let results = run_grep_search("alpha", None, dir, 2).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_grep_search_handles_no_matches() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();
        std::fs::write(dir.join("one.txt"), "omega").unwrap();

        let results = run_grep_search("alpha", None, dir, 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_grep_search_respects_ignore_file() {
        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();
        std::fs::write(dir.join("match.txt"), "alpha").unwrap();
        std::fs::write(dir.join("ignored.log"), "alpha").unwrap();
        std::fs::write(dir.join(".ignore"), "*.log").unwrap();

        let results = run_grep_search("alpha", None, dir, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results.iter().any(|path| path.ends_with("match.txt")));
        assert!(!results.iter().any(|path| path.ends_with(".log")));
    }
}
