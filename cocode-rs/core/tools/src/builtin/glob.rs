//! Glob tool for pattern-based file search.

use super::prompts;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ToolOutput};
use globset::{Glob, GlobSetBuilder};
use serde_json::Value;
use walkdir::WalkDir;

/// Tool for finding files using glob patterns.
///
/// This is a safe tool that can run concurrently with other tools.
pub struct GlobTool {
    /// Maximum results to return.
    max_results: i32,
    /// Maximum depth to traverse.
    max_depth: i32,
}

impl GlobTool {
    /// Create a new Glob tool with default settings.
    pub fn new() -> Self {
        Self {
            max_results: 1000,
            max_depth: 20,
        }
    }

    /// Set the maximum results.
    pub fn with_max_results(mut self, max: i32) -> Self {
        self.max_results = max;
        self
    }

    /// Set the maximum depth.
    pub fn with_max_depth(mut self, depth: i32) -> Self {
        self.max_depth = depth;
        self
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "Glob"
    }

    fn description(&self) -> &str {
        prompts::GLOB_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g., \"**/*.rs\", \"src/**/*.ts\")"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (defaults to current directory)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("pattern must be a string"))?;

        let search_path = input["path"]
            .as_str()
            .map(|p| ctx.resolve_path(p))
            .unwrap_or_else(|| ctx.cwd.clone());

        // Validate search path
        if !search_path.exists() {
            return Err(ToolError::execution_failed(format!(
                "Directory not found: {}",
                search_path.display()
            )));
        }

        // Build glob matcher
        let glob = Glob::new(pattern)
            .map_err(|e| ToolError::invalid_input(format!("Invalid glob pattern: {e}")))?;

        let mut glob_builder = GlobSetBuilder::new();
        glob_builder.add(glob);
        let glob_set = glob_builder
            .build()
            .map_err(|e| ToolError::invalid_input(format!("Failed to build glob set: {e}")))?;

        // Walk directory and collect matches
        let mut matches = Vec::new();
        let walker = WalkDir::new(&search_path)
            .max_depth(self.max_depth as usize)
            .follow_links(false);

        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if ctx.is_cancelled() {
                break;
            }

            let path = entry.path();

            // Get path relative to search directory for matching
            let relative = path.strip_prefix(&search_path).unwrap_or(path);

            if glob_set.is_match(relative) {
                matches.push(path.to_path_buf());

                if matches.len() >= self.max_results as usize {
                    break;
                }
            }
        }

        // Sort by modification time (most recent first)
        matches.sort_by(|a, b| {
            let a_time = a.metadata().and_then(|m| m.modified()).ok();
            let b_time = b.metadata().and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        // Format output
        if matches.is_empty() {
            Ok(ToolOutput::text(format!(
                "No files found matching pattern '{}' in {}",
                pattern,
                search_path.display()
            )))
        } else {
            let output = matches
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");

            let count = matches.len();
            let truncated = count >= self.max_results as usize;

            let header = if truncated {
                format!(
                    "Found {} files (truncated at {}):\n",
                    count, self.max_results
                )
            } else {
                format!("Found {} files:\n", count)
            };

            Ok(ToolOutput::text(format!("{header}{output}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_context(cwd: PathBuf) -> ToolContext {
        ToolContext::new("call-1", "session-1", cwd)
    }

    fn setup_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create test files
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::create_dir_all(dir.path().join("tests")).unwrap();

        File::create(dir.path().join("src/main.rs")).unwrap();
        File::create(dir.path().join("src/lib.rs")).unwrap();
        File::create(dir.path().join("tests/test.rs")).unwrap();
        File::create(dir.path().join("README.md")).unwrap();

        dir
    }

    #[tokio::test]
    async fn test_glob_rust_files() {
        let dir = setup_test_dir();
        let tool = GlobTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "**/*.rs"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("main.rs"));
        assert!(content.contains("lib.rs"));
        assert!(content.contains("test.rs"));
        assert!(!content.contains("README.md"));
    }

    #[tokio::test]
    async fn test_glob_specific_dir() {
        let dir = setup_test_dir();
        let tool = GlobTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "*.rs",
            "path": "src"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("main.rs"));
        assert!(content.contains("lib.rs"));
        assert!(!content.contains("test.rs")); // Not in src/
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let dir = setup_test_dir();
        let tool = GlobTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "**/*.xyz"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("No files found"));
    }

    #[tokio::test]
    async fn test_glob_invalid_pattern() {
        let dir = setup_test_dir();
        let tool = GlobTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "pattern": "[invalid"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_properties() {
        let tool = GlobTool::new();
        assert_eq!(tool.name(), "Glob");
        assert!(tool.is_concurrent_safe());
    }
}
