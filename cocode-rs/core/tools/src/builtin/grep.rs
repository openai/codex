//! Grep tool for content search with regex.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ApprovalRequest;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::PermissionResult;
use cocode_protocol::ToolOutput;
use globset::Glob;
use regex::Regex;
use serde_json::Value;
use std::path::PathBuf;
use walkdir::WalkDir;

/// Output mode for grep results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Show matching lines with content.
    Content,
    /// Show only file paths.
    FilesWithMatches,
    /// Show match counts per file.
    Count,
}

impl Default for OutputMode {
    fn default() -> Self {
        OutputMode::FilesWithMatches
    }
}

/// Tool for searching file contents using regex.
///
/// This is a safe tool that can run concurrently with other tools.
pub struct GrepTool {
    /// Maximum files to search.
    max_files: i32,
    /// Maximum results to return.
    max_results: i32,
    /// Maximum depth to traverse.
    max_depth: i32,
}

impl GrepTool {
    /// Create a new Grep tool with default settings.
    pub fn new() -> Self {
        Self {
            max_files: 5000,
            max_results: 500,
            max_depth: 20,
        }
    }

    /// Set the maximum files to search.
    pub fn with_max_files(mut self, max: i32) -> Self {
        self.max_files = max;
        self
    }

    /// Set the maximum results.
    pub fn with_max_results(mut self, max: i32) -> Self {
        self.max_results = max;
        self
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        prompts::GREP_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in (defaults to current directory)"
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., \"*.rs\", \"*.{ts,tsx}\")"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode: content (show lines), files_with_matches (file paths only), count (match counts)"
                },
                "-i": {
                    "type": "boolean",
                    "description": "Case insensitive search"
                },
                "-n": {
                    "type": "boolean",
                    "description": "Show line numbers (default: true)"
                },
                "-A": {
                    "type": "integer",
                    "description": "Lines to show after each match"
                },
                "-B": {
                    "type": "integer",
                    "description": "Lines to show before each match"
                },
                "-C": {
                    "type": "integer",
                    "description": "Lines to show before and after each match"
                },
                "head_limit": {
                    "type": "integer",
                    "description": "Limit output to first N lines/entries. Defaults to 0 (unlimited). Works across all output modes."
                },
                "offset": {
                    "type": "integer",
                    "description": "Skip first N lines/entries before applying head_limit. Defaults to 0."
                },
                "multiline": {
                    "type": "boolean",
                    "description": "Enable multiline mode where . matches newlines and patterns can span lines. Default: false."
                },
                "type": {
                    "type": "string",
                    "description": "File type to search (e.g., js, py, rust, go, java). More efficient than glob for standard file types."
                }
            },
            "required": ["pattern"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    fn max_result_size_chars(&self) -> i32 {
        20_000
    }

    async fn check_permission(&self, input: &Value, ctx: &ToolContext) -> PermissionResult {
        let search_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| ctx.resolve_path(p))
            .unwrap_or_else(|| ctx.cwd.clone());

        // Sensitive directory targets → NeedsApproval
        if crate::sensitive_files::is_sensitive_directory(&search_path) {
            return PermissionResult::NeedsApproval {
                request: ApprovalRequest {
                    request_id: format!("grep-sensitive-{}", search_path.display()),
                    tool_name: self.name().to_string(),
                    description: format!(
                        "Searching sensitive directory: {}",
                        search_path.display()
                    ),
                    risks: vec![],
                    allow_remember: true,
                },
            };
        }

        // Outside working directory → NeedsApproval
        if crate::sensitive_files::is_outside_cwd(&search_path, &ctx.cwd) {
            return PermissionResult::NeedsApproval {
                request: ApprovalRequest {
                    request_id: format!("grep-outside-cwd-{}", search_path.display()),
                    tool_name: self.name().to_string(),
                    description: format!(
                        "Searching outside working directory: {}",
                        search_path.display()
                    ),
                    risks: vec![],
                    allow_remember: true,
                },
            };
        }

        // In working directory → Allowed
        PermissionResult::Allowed
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let pattern_str = input["pattern"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "pattern must be a string",
            }
            .build()
        })?;

        let case_insensitive = input["-i"].as_bool().unwrap_or(false);
        let show_line_numbers = input["-n"].as_bool().unwrap_or(true);
        let multiline = input["multiline"].as_bool().unwrap_or(false);

        let context_after = input["-A"].as_i64().unwrap_or(0) as i32;
        let context_before = input["-B"].as_i64().unwrap_or(0) as i32;
        let context_both = input["-C"].as_i64().unwrap_or(0) as i32;

        let after_lines = context_after.max(context_both);
        let before_lines = context_before.max(context_both);

        let head_limit = input["head_limit"]
            .as_i64()
            .map(|n| n as i32)
            .unwrap_or(self.max_results);
        let offset = input["offset"].as_i64().unwrap_or(0) as i32;

        let output_mode = match input["output_mode"].as_str() {
            Some("content") => OutputMode::Content,
            Some("count") => OutputMode::Count,
            _ => OutputMode::FilesWithMatches,
        };

        let search_path = input["path"]
            .as_str()
            .map(|p| ctx.resolve_path(p))
            .unwrap_or_else(|| ctx.cwd.clone());

        let file_glob = input["glob"].as_str();
        let file_type = input["type"].as_str();

        // Build regex
        let mut regex_pattern = String::new();
        if multiline {
            regex_pattern.push_str("(?s)");
        }
        if case_insensitive {
            regex_pattern.push_str("(?i)");
        }
        regex_pattern.push_str(pattern_str);

        let regex = Regex::new(&regex_pattern).map_err(|e| {
            crate::error::tool_error::InvalidInputSnafu {
                message: format!("Invalid regex pattern: {e}"),
            }
            .build()
        })?;

        // Build file glob: use explicit glob, or derive from type parameter
        let effective_glob = if file_glob.is_some() {
            file_glob.map(String::from)
        } else {
            file_type.map(|t| format!("*.{}", type_to_extension(t)))
        };

        let file_matcher = effective_glob
            .as_deref()
            .map(|g| {
                Glob::new(g)
                    .map_err(|e| {
                        crate::error::tool_error::InvalidInputSnafu {
                            message: format!("Invalid glob pattern: {e}"),
                        }
                        .build()
                    })
                    .map(|glob| glob.compile_matcher())
            })
            .transpose()?;

        // Collect files to search
        let files: Vec<PathBuf> = if search_path.is_file() {
            vec![search_path.clone()]
        } else {
            WalkDir::new(&search_path)
                .max_depth(self.max_depth as usize)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| {
                    // Apply glob filter if provided
                    if let Some(ref matcher) = file_matcher {
                        let relative = e.path().strip_prefix(&search_path).unwrap_or(e.path());
                        matcher.is_match(relative) || matcher.is_match(e.path())
                    } else {
                        true
                    }
                })
                .take(self.max_files as usize)
                .map(|e| e.path().to_path_buf())
                .collect()
        };

        // Search files
        let mut results = Vec::new();
        let mut total_matches = 0;
        let mut skipped = 0_i32;

        for file_path in &files {
            if ctx.is_cancelled() {
                break;
            }

            // Read file (skip binary files)
            let content = match tokio::fs::read_to_string(file_path).await {
                Ok(c) => c,
                Err(_) => continue, // Skip files that can't be read as text
            };

            let lines: Vec<&str> = content.lines().collect();
            let mut file_matches = Vec::new();

            for (line_idx, line) in lines.iter().enumerate() {
                if regex.is_match(line) {
                    file_matches.push((line_idx, *line));
                    total_matches += 1;

                    if total_matches >= head_limit {
                        break;
                    }
                }
            }

            if !file_matches.is_empty() {
                // Apply offset: skip first N entries
                if offset > 0 && skipped < offset {
                    skipped += 1;
                    continue;
                }

                match output_mode {
                    OutputMode::FilesWithMatches => {
                        results.push(file_path.display().to_string());
                    }
                    OutputMode::Count => {
                        results.push(format!("{}:{}", file_path.display(), file_matches.len()));
                    }
                    OutputMode::Content => {
                        for (line_idx, line) in file_matches {
                            let line_num = line_idx + 1;

                            // Get context lines
                            let start = line_idx.saturating_sub(before_lines as usize);
                            let end = (line_idx + 1 + after_lines as usize).min(lines.len());

                            if before_lines > 0 || after_lines > 0 {
                                // Show context
                                for (ctx_idx, ctx_line) in lines[start..end].iter().enumerate() {
                                    let actual_line_num = start + ctx_idx + 1;
                                    let prefix = if actual_line_num == line_num {
                                        ":"
                                    } else {
                                        "-"
                                    };

                                    if show_line_numbers {
                                        results.push(format!(
                                            "{}{}{}{}",
                                            file_path.display(),
                                            prefix,
                                            actual_line_num,
                                            prefix
                                        ));
                                        results.push(ctx_line.to_string());
                                    } else {
                                        results.push(format!(
                                            "{}{}{}",
                                            file_path.display(),
                                            prefix,
                                            ctx_line
                                        ));
                                    }
                                }
                                results.push("--".to_string());
                            } else {
                                if show_line_numbers {
                                    results.push(format!(
                                        "{}:{}:{}",
                                        file_path.display(),
                                        line_num,
                                        line
                                    ));
                                } else {
                                    results.push(format!("{}:{}", file_path.display(), line));
                                }
                            }
                        }
                    }
                }

                if results.len() >= head_limit as usize {
                    break;
                }
            }
        }

        // Format output
        if results.is_empty() {
            Ok(ToolOutput::text(format!(
                "No matches found for pattern '{}' in {}",
                pattern_str,
                search_path.display()
            )))
        } else {
            let truncated = results.len() >= head_limit as usize;
            let output = results.join("\n");

            if truncated {
                Ok(ToolOutput::text(format!(
                    "{}\n\n... (truncated at {} results)",
                    output, head_limit
                )))
            } else {
                Ok(ToolOutput::text(output))
            }
        }
    }
}

/// Map a type name to a file extension for glob filtering.
fn type_to_extension(type_name: &str) -> &str {
    match type_name {
        "js" | "javascript" => "js",
        "ts" | "typescript" => "ts",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "py" | "python" => "py",
        "rs" | "rust" => "rs",
        "go" | "golang" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" | "c++" => "cpp",
        "h" => "h",
        "hpp" => "hpp",
        "cs" | "csharp" => "cs",
        "rb" | "ruby" => "rb",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kotlin" => "kt",
        "scala" => "scala",
        "sh" | "bash" | "shell" => "sh",
        "yaml" | "yml" => "yml",
        "json" => "json",
        "toml" => "toml",
        "xml" => "xml",
        "html" => "html",
        "css" => "css",
        "sql" => "sql",
        "md" | "markdown" => "md",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_context(cwd: PathBuf) -> ToolContext {
        ToolContext::new("call-1", "session-1", cwd)
    }

    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create test files
        let mut file1 = File::create(dir.path().join("file1.rs")).unwrap();
        writeln!(file1, "fn main() {{").unwrap();
        writeln!(file1, "    println!(\"Hello, world!\");").unwrap();
        writeln!(file1, "}}").unwrap();

        let mut file2 = File::create(dir.path().join("file2.rs")).unwrap();
        writeln!(file2, "fn test_something() {{").unwrap();
        writeln!(file2, "    assert!(true);").unwrap();
        writeln!(file2, "}}").unwrap();

        let mut file3 = File::create(dir.path().join("other.txt")).unwrap();
        writeln!(file3, "This is a text file.").unwrap();
        writeln!(file3, "It has some content.").unwrap();

        dir
    }

    #[tokio::test]
    async fn test_grep_basic() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "fn "
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("file1.rs"));
        assert!(content.contains("file2.rs"));
    }

    #[tokio::test]
    async fn test_grep_with_glob() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "fn ",
            "glob": "*.rs"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("file1.rs"));
        assert!(!content.contains("other.txt"));
    }

    #[tokio::test]
    async fn test_grep_content_mode() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "println",
            "output_mode": "content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("println"));
        assert!(content.contains("Hello, world!"));
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "HELLO",
            "-i": true,
            "output_mode": "content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("Hello"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "nonexistent_pattern_xyz"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("No matches found"));
    }

    #[test]
    fn test_tool_properties() {
        let tool = GrepTool::new();
        assert_eq!(tool.name(), "Grep");
        assert!(tool.is_concurrent_safe());
    }
}
