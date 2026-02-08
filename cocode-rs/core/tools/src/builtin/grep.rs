//! Grep tool for content search using the ripgrep library ecosystem.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_file_ignore::IgnoreConfig;
use cocode_file_ignore::IgnoreService;
use cocode_protocol::ApprovalRequest;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::PermissionResult;
use cocode_protocol::ToolOutput;
use grep_regex::RegexMatcherBuilder;
use grep_searcher::BinaryDetection;
use grep_searcher::Searcher;
use grep_searcher::SearcherBuilder;
use grep_searcher::Sink;
use grep_searcher::SinkContext;
use grep_searcher::SinkMatch;
use serde_json::Value;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// Search timeout to prevent long-running searches.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Output mode for grep results.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputMode {
    /// Show matching lines with content.
    Content,
    /// Show only file paths.
    #[default]
    FilesWithMatches,
    /// Show match counts per file.
    Count,
}

/// A single entry from grep search: a match line, context line, or group break.
#[derive(Debug, Clone)]
struct GrepMatchLine {
    file_path: String,
    line_number: u64,
    line_content: String,
    is_context: bool,
    /// Sentinel: true means this is a `--` separator between non-contiguous
    /// context groups within the same file.
    is_break: bool,
}

/// Custom Sink that distinguishes between match lines and context lines.
struct ContextAwareSink<'a> {
    matches: &'a mut Vec<GrepMatchLine>,
    file_path: String,
    limit: usize,
}

impl Sink for ContextAwareSink<'_> {
    type Error = io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> std::result::Result<bool, io::Error> {
        if self.matches.len() >= self.limit {
            return Ok(false);
        }
        self.matches.push(GrepMatchLine {
            file_path: self.file_path.clone(),
            line_number: mat.line_number().unwrap_or(0),
            line_content: String::from_utf8_lossy(mat.bytes()).trim_end().to_string(),
            is_context: false,
            is_break: false,
        });
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        ctx: &SinkContext<'_>,
    ) -> std::result::Result<bool, io::Error> {
        if self.matches.len() >= self.limit {
            return Ok(false);
        }
        self.matches.push(GrepMatchLine {
            file_path: self.file_path.clone(),
            line_number: ctx.line_number().unwrap_or(0),
            line_content: String::from_utf8_lossy(ctx.bytes()).trim_end().to_string(),
            is_context: true,
            is_break: false,
        });
        Ok(true)
    }

    fn context_break(&mut self, _searcher: &Searcher) -> std::result::Result<bool, io::Error> {
        if self.matches.len() >= self.limit {
            return Ok(false);
        }
        self.matches.push(GrepMatchLine {
            file_path: self.file_path.clone(),
            line_number: 0,
            line_content: String::new(),
            is_context: false,
            is_break: true,
        });
        Ok(true)
    }
}

/// Parameters for the synchronous grep search (all owned, Send-safe).
struct GrepSearchParams {
    pattern: String,
    case_insensitive: bool,
    multiline: bool,
    before_context: usize,
    after_context: usize,
    search_path: PathBuf,
    effective_glob: Option<String>,
    max_depth: usize,
    max_results: usize,
}

/// Tool for searching file contents using the grep crate (ripgrep's core library).
///
/// This is a safe tool that can run concurrently with other tools.
pub struct GrepTool {
    /// Maximum results to return.
    max_results: i32,
    /// Maximum depth to traverse.
    max_depth: i32,
}

impl GrepTool {
    /// Create a new Grep tool with default settings.
    pub fn new() -> Self {
        Self {
            max_results: 500,
            max_depth: 20,
        }
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
                    proposed_prefix_pattern: None,
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
                    proposed_prefix_pattern: None,
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

        let context_after = input["-A"].as_i64().unwrap_or(0) as usize;
        let context_before = input["-B"].as_i64().unwrap_or(0) as usize;
        let context_both = input["-C"].as_i64().unwrap_or(0) as usize;

        let after_lines = context_after.max(context_both);
        let before_lines = context_before.max(context_both);

        let head_limit = input["head_limit"]
            .as_i64()
            .map(|n| n as i32)
            .unwrap_or(self.max_results);
        let offset = input["offset"].as_i64().unwrap_or(0) as usize;

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

        // Build effective glob from explicit glob or type parameter
        let effective_glob = if file_glob.is_some() {
            file_glob.map(String::from)
        } else {
            file_type.map(|t| format!("*.{}", type_to_extension(t)))
        };

        let params = GrepSearchParams {
            pattern: pattern_str.to_string(),
            case_insensitive,
            multiline,
            before_context: before_lines,
            after_context: after_lines,
            search_path: search_path.clone(),
            effective_glob,
            max_depth: self.max_depth as usize,
            max_results: self.max_results as usize,
        };

        // Run search in spawn_blocking with timeout
        let search_future = tokio::task::spawn_blocking(move || run_grep_search(&params));

        let matches = timeout(COMMAND_TIMEOUT, search_future)
            .await
            .map_err(|_| {
                crate::error::tool_error::ExecutionFailedSnafu {
                    message: "grep search timed out after 30 seconds",
                }
                .build()
            })?
            .map_err(|e| {
                crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!("grep search task failed: {e}"),
                }
                .build()
            })??;

        // Format output
        format_grep_output(
            &matches,
            pattern_str,
            &search_path,
            output_mode,
            show_line_numbers,
            offset,
            head_limit as usize,
        )
    }
}

/// Execute the grep search synchronously (called from spawn_blocking).
fn run_grep_search(params: &GrepSearchParams) -> Result<Vec<GrepMatchLine>> {
    // Build regex matcher
    let mut builder = RegexMatcherBuilder::new();
    builder.case_insensitive(params.case_insensitive);
    if params.multiline {
        builder.multi_line(true).dot_matches_new_line(true);
    }
    let matcher = builder.build(&params.pattern).map_err(|e| {
        crate::error::tool_error::InvalidInputSnafu {
            message: format!("Invalid regex pattern: {e}"),
        }
        .build()
    })?;

    // Build searcher with context support and binary detection
    let mut searcher_builder = SearcherBuilder::new();
    searcher_builder
        .line_number(true)
        .binary_detection(BinaryDetection::quit(0));
    if params.before_context > 0 {
        searcher_builder.before_context(params.before_context);
    }
    if params.after_context > 0 {
        searcher_builder.after_context(params.after_context);
    }
    if params.multiline {
        searcher_builder.multi_line(true);
    }

    let mut matches: Vec<GrepMatchLine> = Vec::new();

    if params.search_path.is_file() {
        // Search a single file directly
        let file_path_str = params.search_path.display().to_string();
        let mut searcher = searcher_builder.build();
        let mut sink = ContextAwareSink {
            matches: &mut matches,
            file_path: file_path_str,
            limit: params.max_results,
        };
        if let Err(e) = searcher.search_path(&matcher, &params.search_path, &mut sink) {
            tracing::debug!("Search error in {}: {e}", params.search_path.display());
        }
    } else {
        // Build walker via IgnoreService (respects .gitignore, .ignore)
        let ignore_config = IgnoreConfig::default().with_hidden(true);
        let ignore_service = IgnoreService::new(ignore_config);
        let mut walker_builder = ignore_service.create_walk_builder(&params.search_path);
        walker_builder.max_depth(Some(params.max_depth));

        // Apply glob/type filter via ignore::types::TypesBuilder
        if let Some(ref glob_pattern) = params.effective_glob {
            let mut types_builder = ignore::types::TypesBuilder::new();
            if let Err(e) = types_builder.add("custom", glob_pattern) {
                tracing::warn!("Invalid glob filter pattern '{glob_pattern}': {e}");
            }
            types_builder.select("custom");
            if let Ok(types) = types_builder.build() {
                walker_builder.types(types);
            }
        }

        for entry in walker_builder.build().flatten() {
            if matches.len() >= params.max_results {
                break;
            }

            let file_type = entry.file_type();
            if file_type.map(|t| !t.is_file()).unwrap_or(true) {
                continue;
            }

            let file_path = entry.path().to_path_buf();
            let file_path_str = file_path.display().to_string();

            let mut searcher = searcher_builder.build();
            let mut sink = ContextAwareSink {
                matches: &mut matches,
                file_path: file_path_str,
                limit: params.max_results,
            };

            if let Err(e) = searcher.search_path(&matcher, &file_path, &mut sink) {
                tracing::debug!("Search error in {}: {e}", file_path.display());
            }
        }
    }

    Ok(matches)
}

/// Format grep output based on output mode with offset/limit support.
fn format_grep_output(
    matches: &[GrepMatchLine],
    pattern: &str,
    search_path: &Path,
    output_mode: OutputMode,
    show_line_numbers: bool,
    offset: usize,
    head_limit: usize,
) -> Result<ToolOutput> {
    if matches.is_empty() {
        return Ok(ToolOutput::text(format!(
            "No matches found for pattern '{pattern}' in {}",
            search_path.display()
        )));
    }

    let mut results: Vec<String> = Vec::new();

    match output_mode {
        OutputMode::FilesWithMatches => {
            // Collect unique file paths from actual match lines
            let mut seen = std::collections::HashSet::new();
            for m in matches.iter().filter(|m| !m.is_context && !m.is_break) {
                if seen.insert(m.file_path.clone()) {
                    results.push(m.file_path.clone());
                }
            }
        }
        OutputMode::Count => {
            // Count actual matches grouped by file path
            let mut counts: std::collections::HashMap<&str, usize> =
                std::collections::HashMap::new();
            // Preserve insertion order with a separate vec
            let mut order: Vec<&str> = Vec::new();
            for m in matches.iter().filter(|m| !m.is_context && !m.is_break) {
                let count = counts.entry(&m.file_path).or_insert(0);
                if *count == 0 {
                    order.push(&m.file_path);
                }
                *count += 1;
            }
            for file in &order {
                if let Some(&count) = counts.get(file) {
                    results.push(format!("{file}:{count}"));
                }
            }
        }
        OutputMode::Content => {
            let mut prev_file: Option<&str> = None;
            for m in matches {
                // File header when switching to a new file
                if prev_file != Some(&m.file_path) {
                    if prev_file.is_some() {
                        results.push(String::new()); // blank line between files
                    }
                    results.push(format!("{}:", m.file_path));
                    prev_file = Some(&m.file_path);
                }

                // Context break between non-contiguous groups within a file
                if m.is_break {
                    results.push("  --".to_string());
                    continue;
                }

                let separator = if m.is_context { "-" } else { ":" };
                if show_line_numbers {
                    results.push(format!("  {}{separator}{}", m.line_number, m.line_content));
                } else {
                    results.push(format!("  {}", m.line_content));
                }
            }
        }
    }

    // Apply offset and head_limit
    let total = results.len();
    let results: Vec<String> = results.into_iter().skip(offset).collect();
    let truncated = head_limit > 0 && results.len() > head_limit;
    let results: Vec<String> = if head_limit > 0 {
        results.into_iter().take(head_limit).collect()
    } else {
        results
    };

    let output = results.join("\n");

    if truncated {
        Ok(ToolOutput::text(format!(
            "{output}\n\n... (truncated at {head_limit} results, {total} total)"
        )))
    } else {
        Ok(ToolOutput::text(output))
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

    #[tokio::test]
    async fn test_grep_respects_gitignore() {
        let dir = TempDir::new().unwrap();

        // Create .gitignore that excludes *.log
        std::fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();

        // Create files
        let mut rs_file = File::create(dir.path().join("main.rs")).unwrap();
        writeln!(rs_file, "fn hello() {{}}").unwrap();

        let mut log_file = File::create(dir.path().join("debug.log")).unwrap();
        writeln!(log_file, "fn should_be_ignored() {{}}").unwrap();

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

        assert!(content.contains("main.rs"));
        assert!(!content.contains("debug.log"));
    }

    #[tokio::test]
    async fn test_grep_skips_binary_files() {
        let dir = TempDir::new().unwrap();

        // Create a text file with a match
        let mut text_file = File::create(dir.path().join("text.rs")).unwrap();
        writeln!(text_file, "fn search_me() {{}}").unwrap();

        // Create a binary file with null bytes
        let mut binary_file = File::create(dir.path().join("binary.bin")).unwrap();
        binary_file
            .write_all(b"fn search_me() {}\x00\x00\x00binary data")
            .unwrap();

        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "search_me"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("text.rs"));
        // Binary file should be skipped by grep-searcher
        assert!(!content.contains("binary.bin"));
    }

    #[tokio::test]
    async fn test_grep_context_lines_with_sink() {
        let dir = TempDir::new().unwrap();

        let mut file = File::create(dir.path().join("ctx.txt")).unwrap();
        writeln!(file, "line 1").unwrap();
        writeln!(file, "line 2 match").unwrap();
        writeln!(file, "line 3").unwrap();
        writeln!(file, "line 4").unwrap();
        writeln!(file, "line 5 match").unwrap();
        writeln!(file, "line 6").unwrap();

        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "match",
            "output_mode": "content",
            "-B": 1,
            "-A": 1
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        // Should contain match lines (with :)
        assert!(content.contains("line 2 match"));
        assert!(content.contains("line 5 match"));
        // Should contain context lines (with -)
        assert!(content.contains("line 1"));
        assert!(content.contains("line 3"));
        assert!(content.contains("line 4"));
        assert!(content.contains("line 6"));
    }

    #[tokio::test]
    async fn test_grep_count_mode() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "fn ",
            "output_mode": "count"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        // Each .rs file has one "fn " match
        assert!(content.contains(":1"));
    }

    #[tokio::test]
    async fn test_grep_multiline_cross_line() {
        let dir = TempDir::new().unwrap();

        let mut file = File::create(dir.path().join("multi.txt")).unwrap();
        writeln!(file, "fn hello() {{").unwrap();
        writeln!(file, "    world").unwrap();
        writeln!(file, "}}").unwrap();

        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "hello.*world",
            "multiline": true,
            "output_mode": "content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        // Should match across lines when multiline is enabled
        assert!(content.contains("hello"));
        assert!(content.contains("world"));
    }

    #[tokio::test]
    async fn test_grep_context_break_separators() {
        let dir = TempDir::new().unwrap();

        // Create a file with two matches far apart so context groups are disjoint
        let mut file = File::create(dir.path().join("breaks.txt")).unwrap();
        writeln!(file, "line 1 match").unwrap();
        writeln!(file, "line 2").unwrap();
        writeln!(file, "line 3").unwrap();
        writeln!(file, "line 4").unwrap();
        writeln!(file, "line 5").unwrap();
        writeln!(file, "line 6").unwrap();
        writeln!(file, "line 7").unwrap();
        writeln!(file, "line 8 match").unwrap();

        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "match",
            "output_mode": "content",
            "-A": 1
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        // Should have a -- separator between disjoint context groups
        assert!(content.contains("line 1 match"));
        assert!(content.contains("line 8 match"));
        assert!(content.contains("  --"));
    }

    #[tokio::test]
    async fn test_grep_content_grouped_by_file() {
        let dir = setup_test_dir();
        let tool = GrepTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "fn ",
            "output_mode": "content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        // Output should have file headers (ending with :) and indented match lines
        assert!(content.contains("file1.rs:") || content.contains("file2.rs:"));
        // Lines should be indented with 2 spaces
        assert!(content.contains("  "));
    }
}
