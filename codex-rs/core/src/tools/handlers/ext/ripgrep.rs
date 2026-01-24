//! Rich Grep Handler - Search file contents with grep crate
//!
//! This module provides the RipGrepHandler which searches file contents
//! using the grep crate (ripgrep's core library), returning matching lines
//! with file paths and line numbers.

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::Searcher;
use grep_searcher::SearcherBuilder;
use grep_searcher::Sink;
use grep_searcher::SinkContext;
use grep_searcher::SinkMatch;
use ignore::WalkBuilder;
use indexmap::IndexMap;
use serde::Deserialize;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;

/// Internal safety limit (not exposed to LLM)
const INTERNAL_LIMIT: usize = 2000;
/// Search timeout to prevent long-running searches
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Rich Grep tool arguments
#[derive(Debug, Clone, Deserialize)]
struct RipGrepArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    include: Option<String>,
    #[serde(default)]
    case_sensitive: Option<bool>,
    #[serde(default)]
    fixed_strings: Option<bool>,
    #[serde(default)]
    context: Option<i32>,
    #[serde(default)]
    after: Option<i32>,
    #[serde(default)]
    before: Option<i32>,
}

/// Match result from grep search
#[derive(Debug, Clone)]
struct GrepMatch {
    file_path: String,
    line_number: i32,
    line_content: String,
    is_context: bool,
    /// File modification time for sorting (newest first)
    mtime: Option<std::time::SystemTime>,
}

/// Rich Grep Handler using grep crate (ripgrep's core library)
pub struct RipGrepHandler;

#[async_trait]
impl ToolHandler for RipGrepHandler {
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
                    "Invalid payload type for grep_files".to_string(),
                ));
            }
        };

        let args: RipGrepArgs = serde_json::from_str(arguments)
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

        // Run search with timeout using spawn_blocking for sync grep operations
        let args_clone = args.clone();
        let search_path_clone = search_path.clone();

        let search_future = tokio::task::spawn_blocking(move || {
            run_ripgrep_search(&args_clone, &search_path_clone)
        });

        let matches = timeout(COMMAND_TIMEOUT, search_future)
            .await
            .map_err(|_| {
                FunctionCallError::RespondToModel(
                    "grep search timed out after 30 seconds".to_string(),
                )
            })?
            .map_err(|e| {
                FunctionCallError::RespondToModel(format!("grep search task failed: {e}"))
            })??;

        // Format output
        let content = format_output(&matches, &args.pattern, &search_path);

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(!matches.is_empty()),
        })
    }
}

/// Custom Sink that distinguishes between match lines and context lines
struct ContextAwareSink<'a> {
    matches: &'a mut Vec<GrepMatch>,
    file_path: String,
    mtime: Option<std::time::SystemTime>,
    limit: usize,
}

impl Sink for ContextAwareSink<'_> {
    type Error = io::Error;

    fn matched(&mut self, _searcher: &Searcher, mat: &SinkMatch<'_>) -> Result<bool, Self::Error> {
        if self.matches.len() >= self.limit {
            return Ok(false);
        }
        self.matches.push(GrepMatch {
            file_path: self.file_path.clone(),
            line_number: mat.line_number().unwrap_or(0) as i32,
            line_content: String::from_utf8_lossy(mat.bytes()).trim_end().to_string(),
            is_context: false,
            mtime: self.mtime,
        });
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, Self::Error> {
        if self.matches.len() >= self.limit {
            return Ok(false);
        }
        self.matches.push(GrepMatch {
            file_path: self.file_path.clone(),
            line_number: ctx.line_number().unwrap_or(0) as i32,
            line_content: String::from_utf8_lossy(ctx.bytes()).trim_end().to_string(),
            is_context: true,
            mtime: self.mtime,
        });
        Ok(true)
    }
}

/// Execute the grep search synchronously (called from spawn_blocking)
fn run_ripgrep_search(
    args: &RipGrepArgs,
    search_path: &Path,
) -> Result<Vec<GrepMatch>, FunctionCallError> {
    // Build regex matcher
    let pattern = if args.fixed_strings.unwrap_or(false) {
        regex::escape(&args.pattern)
    } else {
        args.pattern.clone()
    };

    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!args.case_sensitive.unwrap_or(false))
        .build(&pattern)
        .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid regex: {e}")))?;

    // Build searcher with context support
    let mut searcher_builder = SearcherBuilder::new();
    searcher_builder.line_number(true);

    if let Some(c) = args.context {
        if c > 0 {
            let ctx = c as usize;
            searcher_builder.before_context(ctx);
            searcher_builder.after_context(ctx);
        }
    }
    if let Some(b) = args.before {
        if b > 0 {
            searcher_builder.before_context(b as usize);
        }
    }
    if let Some(a) = args.after {
        if a > 0 {
            searcher_builder.after_context(a as usize);
        }
    }

    // Build file walker (respects .gitignore, .ignore)
    let mut walker_builder = WalkBuilder::new(search_path);
    walker_builder.hidden(false).git_ignore(true).ignore(true);

    // Apply glob filter if specified
    if let Some(ref glob_pattern) = args.include {
        let mut types_builder = ignore::types::TypesBuilder::new();
        types_builder.add("custom", glob_pattern).ok();
        types_builder.select("custom");
        if let Ok(types) = types_builder.build() {
            walker_builder.types(types);
        }
    }

    // Execute search with context-aware sink
    let mut matches: Vec<GrepMatch> = Vec::new();

    for entry in walker_builder.build().flatten() {
        if matches.len() >= INTERNAL_LIMIT {
            break;
        }

        let file_type = entry.file_type();
        if file_type.map(|t| !t.is_file()).unwrap_or(true) {
            continue;
        }

        let file_path = entry.path().to_path_buf();
        let mtime = fs::metadata(&file_path)
            .ok()
            .and_then(|m| m.modified().ok());
        let file_path_str = file_path.display().to_string();

        let mut file_searcher = searcher_builder.build();

        // Use custom ContextAwareSink to distinguish match vs context lines
        let mut sink = ContextAwareSink {
            matches: &mut matches,
            file_path: file_path_str,
            mtime,
            limit: INTERNAL_LIMIT,
        };

        let search_result = file_searcher.search_path(&matcher, &file_path, &mut sink);

        if let Err(e) = search_result {
            tracing::debug!("Search error in {}: {}", file_path.display(), e);
        }
    }

    matches.sort_by(|a, b| match (&b.mtime, &a.mtime) {
        (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    Ok(matches)
}

/// Format matches into readable output
fn format_output(matches: &[GrepMatch], pattern: &str, path: &Path) -> String {
    if matches.is_empty() {
        return format!("No matches found for pattern \"{}\"", pattern);
    }

    // Count actual matches (not context lines)
    let match_count = matches.iter().filter(|m| !m.is_context).count();

    // Group by file - use IndexMap to preserve mtime order
    let mut by_file: IndexMap<&str, Vec<&GrepMatch>> = IndexMap::new();
    for m in matches {
        by_file.entry(&m.file_path).or_default().push(m);
    }

    let match_word = if match_count == 1 { "match" } else { "matches" };
    let mut output = format!(
        "Found {} {} for pattern \"{}\" in path \"{}\":\n---\n",
        match_count,
        match_word,
        pattern,
        path.display()
    );

    for (file, file_matches) in by_file {
        output.push_str(&format!("File: {}\n", file));
        for m in file_matches {
            let prefix = if m.is_context { " " } else { ">" };
            output.push_str(&format!(
                "{}L{}: {}\n",
                prefix, m.line_number, m.line_content
            ));
        }
        output.push_str("---\n");
    }

    // Add truncation notice if limit was hit
    if matches.len() >= INTERNAL_LIMIT {
        output.push_str(&format!(
            "\n(Output truncated at {} lines. Refine your search pattern for more specific results.)\n",
            INTERNAL_LIMIT
        ));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_output_empty() {
        let output = format_output(&[], "test", Path::new("."));
        assert!(output.contains("No matches found"));
    }

    #[test]
    fn test_format_output_with_matches() {
        let matches = vec![
            GrepMatch {
                file_path: "src/main.rs".to_string(),
                line_number: 10,
                line_content: "fn main() {".to_string(),
                is_context: false,
                mtime: None,
            },
            GrepMatch {
                file_path: "src/main.rs".to_string(),
                line_number: 11,
                line_content: "    println!(\"hello\");".to_string(),
                is_context: true,
                mtime: None,
            },
        ];
        let output = format_output(&matches, "main", Path::new("."));
        assert!(output.contains("Found 1 match"));
        assert!(output.contains("File: src/main.rs"));
        assert!(output.contains(">L10: fn main()"));
        assert!(output.contains(" L11:"));
    }

    #[test]
    fn test_ripgrep_args_defaults() {
        let json = r#"{"pattern": "test"}"#;
        let args: RipGrepArgs = serde_json::from_str(json).expect("parse args");
        assert_eq!(args.pattern, "test");
        assert!(args.path.is_none());
        assert!(args.include.is_none());
        assert!(args.case_sensitive.is_none());
        assert!(args.fixed_strings.is_none());
        assert!(args.context.is_none());
        assert!(args.after.is_none());
        assert!(args.before.is_none());
    }

    #[test]
    fn test_ripgrep_args_full() {
        let json = r#"{
            "pattern": "test",
            "path": "/src",
            "include": "*.rs",
            "case_sensitive": true,
            "fixed_strings": true,
            "context": 2,
            "after": 3,
            "before": 1
        }"#;
        let args: RipGrepArgs = serde_json::from_str(json).expect("parse args");
        assert_eq!(args.pattern, "test");
        assert_eq!(args.path.as_deref(), Some("/src"));
        assert_eq!(args.include.as_deref(), Some("*.rs"));
        assert_eq!(args.case_sensitive, Some(true));
        assert_eq!(args.fixed_strings, Some(true));
        assert_eq!(args.context, Some(2));
        assert_eq!(args.after, Some(3));
        assert_eq!(args.before, Some(1));
    }

    #[test]
    fn test_grep_crate_integration() {
        use std::fs;
        use tempfile::tempdir;

        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create test files with searchable content
        fs::write(
            dir.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}",
        )
        .expect("write");
        fs::write(
            dir.join("lib.rs"),
            "pub fn helper() {\n    // helper function\n}",
        )
        .expect("write");
        fs::write(dir.join("ignored.log"), "fn should_be_ignored() {}").expect("write");

        // Create .ignore file
        fs::write(dir.join(".ignore"), "*.log").expect("write ignore");

        // Build matcher and searcher
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(true)
            .build("fn")
            .expect("build matcher");

        let mut searcher = SearcherBuilder::new().line_number(true).build();

        // Build walker
        let walker = WalkBuilder::new(dir)
            .hidden(false)
            .git_ignore(true)
            .ignore(true)
            .build();

        let mut matches = Vec::new();

        for entry in walker.flatten() {
            if entry.file_type().map(|t| !t.is_file()).unwrap_or(true) {
                continue;
            }

            let file_path = entry.path().to_path_buf();
            let file_path_str = file_path.display().to_string();

            // Use ContextAwareSink for searching
            let mut sink = ContextAwareSink {
                matches: &mut matches,
                file_path: file_path_str,
                mtime: None,
                limit: 100,
            };

            let _ = searcher.search_path(&matcher, &file_path, &mut sink);
        }

        // Should find matches in .rs files but not in .log (filtered by .ignore)
        assert!(!matches.is_empty());
        assert!(matches.iter().any(|m| m.file_path.ends_with("main.rs")));
        assert!(matches.iter().any(|m| m.file_path.ends_with("lib.rs")));
        assert!(!matches.iter().any(|m| m.file_path.ends_with(".log")));
    }

    #[test]
    fn test_context_aware_sink_distinguishes_context() {
        use std::fs;
        use tempfile::tempdir;

        let temp = tempdir().expect("create temp dir");
        let dir = temp.path();

        // Create a file with multiple lines
        fs::write(
            dir.join("test.txt"),
            "line 1\nline 2 match\nline 3\nline 4 match\nline 5",
        )
        .expect("write");

        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(true)
            .build("match")
            .expect("build matcher");

        // Enable context lines
        let mut searcher = SearcherBuilder::new()
            .line_number(true)
            .before_context(1)
            .after_context(1)
            .build();

        let mut matches = Vec::new();
        let file_path = dir.join("test.txt");
        let file_path_str = file_path.display().to_string();

        let mut sink = ContextAwareSink {
            matches: &mut matches,
            file_path: file_path_str,
            mtime: None,
            limit: 100,
        };

        let _ = searcher.search_path(&matcher, &file_path, &mut sink);

        // Should have matches and context lines
        assert!(!matches.is_empty());

        // Check that we have both match lines (is_context=false) and context lines (is_context=true)
        let match_lines: Vec<_> = matches.iter().filter(|m| !m.is_context).collect();
        let context_lines: Vec<_> = matches.iter().filter(|m| m.is_context).collect();

        assert_eq!(match_lines.len(), 2, "Should have 2 match lines");
        assert!(!context_lines.is_empty(), "Should have context lines");

        // Verify match content
        assert!(
            match_lines
                .iter()
                .any(|m| m.line_content.contains("line 2 match"))
        );
        assert!(
            match_lines
                .iter()
                .any(|m| m.line_content.contains("line 4 match"))
        );
    }
}
