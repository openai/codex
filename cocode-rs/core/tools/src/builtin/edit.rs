//! Edit tool for string replacement in files.
//!
//! Supports three matching strategies (tried in order):
//! 1. **Exact** — precise string matching (default)
//! 2. **Flexible** — whitespace-tolerant fallback when exact match fails
//! 3. **Regex** — token-based fuzzy matching (first occurrence only)
//!
//! Also supports file creation via `old_string == ""` and SHA256-based
//! concurrent modification detection.

use super::edit_strategies::MatchStrategy;
use super::edit_strategies::diff_stats;
use super::edit_strategies::find_closest_match;
use super::edit_strategies::pre_correct_escaping;
use super::edit_strategies::trim_pair_if_possible;
use super::edit_strategies::try_exact_replace;
use super::edit_strategies::try_flexible_replace;
use super::edit_strategies::try_regex_replace;
use super::prompts;
use crate::context::FileReadState;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_file_encoding::Encoding;
use cocode_file_encoding::LineEnding;
use cocode_file_encoding::detect_encoding;
use cocode_file_encoding::detect_line_ending;
use cocode_file_encoding::normalize_line_endings;
use cocode_file_encoding::preserve_trailing_newline;
use cocode_file_encoding::write_with_format_async;
use cocode_plan_mode::is_safe_file;
use cocode_protocol::ApprovalRequest;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::ContextModifier;
use cocode_protocol::PermissionResult;
use cocode_protocol::RiskSeverity;
use cocode_protocol::RiskType;
use cocode_protocol::SecurityRisk;
use cocode_protocol::ToolOutput;
use serde_json::Value;
use tokio::fs;

/// Tool for performing string replacements in files.
///
/// Requires the file to have been read first (tracked via FileTracker).
/// Supports file creation when `old_string` is empty.
pub struct EditTool;

impl EditTool {
    /// Create a new Edit tool.
    pub fn new() -> Self {
        Self
    }

    /// Create a new file (when `old_string == ""`).
    async fn create_new_file(
        &self,
        path: &std::path::Path,
        new_string: &str,
        ctx: &mut ToolContext,
    ) -> Result<ToolOutput> {
        // Reject if file already exists
        if path.exists() {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "Cannot create file: {} already exists. Use non-empty old_string to edit existing files.",
                    path.display()
                ),
            }
            .build());
        }

        // Plan mode check
        if ctx.is_plan_mode && !is_safe_file(path, ctx.plan_file_path.as_deref()) {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "Plan mode: cannot create '{}'. Only the plan file can be modified during plan mode.",
                    path.display()
                ),
            }
            .build());
        }

        // Create parent directories
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    crate::error::tool_error::ExecutionFailedSnafu {
                        message: format!("Failed to create directory: {e}"),
                    }
                    .build()
                })?;
            }
        }

        // Write file with UTF-8 / LF defaults (same as Write tool for new files)
        write_with_format_async(path, new_string, Encoding::Utf8, LineEnding::Lf)
            .await
            .map_err(|e| {
                crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!("Failed to write file: {e}"),
                }
                .build()
            })?;

        // Track modification and update read state
        let normalized = normalize_line_endings(new_string, LineEnding::Lf);
        ctx.record_file_modified(path).await;
        let new_mtime = fs::metadata(path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        ctx.record_file_read_with_state(
            path,
            FileReadState::complete(normalized.clone(), new_mtime),
        )
        .await;

        let mut result = ToolOutput::text(format!("Created new file: {}", path.display()));
        result.modifiers.push(ContextModifier::FileRead {
            path: path.to_path_buf(),
            content: normalized,
        });
        Ok(result)
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        prompts::EDIT_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to replace. Use an empty string to create a new file."
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with (must be different from old_string)"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default false)",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn check_permission(&self, input: &Value, ctx: &ToolContext) -> PermissionResult {
        if let Some(path_str) = input.get("file_path").and_then(|v| v.as_str()) {
            let path = ctx.resolve_path(path_str);

            // Locked directory → Deny
            if crate::sensitive_files::is_locked_directory(&path) {
                return PermissionResult::Denied {
                    reason: format!(
                        "Editing files in locked directory is not allowed: {}",
                        path.display()
                    ),
                };
            }

            // Plan mode: only plan file allowed
            if ctx.is_plan_mode {
                if let Some(ref plan_file) = ctx.plan_file_path {
                    if path != *plan_file {
                        return PermissionResult::Denied {
                            reason: format!(
                                "Plan mode: cannot edit '{}'. Only the plan file can be modified.",
                                path.display()
                            ),
                        };
                    }
                }
            }

            // Sensitive file → NeedsApproval (high severity)
            if crate::sensitive_files::is_sensitive_file(&path) {
                return PermissionResult::NeedsApproval {
                    request: ApprovalRequest {
                        request_id: format!("sensitive-edit-{}", path.display()),
                        tool_name: self.name().to_string(),
                        description: format!("Modifying sensitive file: {}", path.display()),
                        risks: vec![SecurityRisk {
                            risk_type: RiskType::SensitiveFile,
                            severity: RiskSeverity::High,
                            message: format!(
                                "File '{}' may contain credentials or sensitive configuration",
                                path.display()
                            ),
                        }],
                        allow_remember: true,
                        proposed_prefix_pattern: None,
                    },
                };
            }

            // Sensitive directory (.git/, .vscode/, .idea/) → NeedsApproval
            if crate::sensitive_files::is_sensitive_directory(&path) {
                return PermissionResult::NeedsApproval {
                    request: ApprovalRequest {
                        request_id: format!("sensitive-dir-edit-{}", path.display()),
                        tool_name: self.name().to_string(),
                        description: format!(
                            "Editing file in sensitive directory: {}",
                            path.display()
                        ),
                        risks: vec![SecurityRisk {
                            risk_type: RiskType::SystemConfig,
                            severity: RiskSeverity::Medium,
                            message: format!(
                                "Directory '{}' contains project configuration",
                                path.display()
                            ),
                        }],
                        allow_remember: true,
                        proposed_prefix_pattern: None,
                    },
                };
            }
        }

        // All edits default to NeedsApproval
        PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                request_id: format!(
                    "edit-{}",
                    input
                        .get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                ),
                tool_name: self.name().to_string(),
                description: format!(
                    "Edit: {}",
                    input
                        .get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                ),
                risks: vec![],
                allow_remember: true,
                proposed_prefix_pattern: None,
            },
        }
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        // ── Parse inputs ────────────────────────────────────────────
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "file_path must be a string",
            }
            .build()
        })?;
        let old_string = input["old_string"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "old_string must be a string",
            }
            .build()
        })?;
        let new_string = input["new_string"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "new_string must be a string",
            }
            .build()
        })?;
        let replace_all = input["replace_all"].as_bool().unwrap_or(false);

        let path = ctx.resolve_path(file_path);

        // ── File creation (old_string == "") ────────────────────────
        if old_string.is_empty() {
            return self.create_new_file(&path, new_string, ctx).await;
        }

        // ── Validation ──────────────────────────────────────────────
        if old_string == new_string {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "old_string and new_string must be different",
            }
            .build());
        }

        // Check for .ipynb files - redirect to NotebookEdit
        if path.extension().is_some_and(|ext| ext == "ipynb") {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "Cannot use Edit tool on Jupyter notebook files. \
                     Use the NotebookEdit tool instead to modify cells in '{}'.",
                    path.display()
                ),
            }
            .build());
        }

        // Plan mode check
        if ctx.is_plan_mode && !is_safe_file(&path, ctx.plan_file_path.as_deref()) {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "Plan mode: cannot edit '{}'. Only the plan file can be modified during plan mode.",
                    path.display()
                ),
            }
            .build());
        }

        // Verify file was read first
        if !ctx.was_file_read(&path).await {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "File must be read before editing: {}. Use the Read tool first.",
                    path.display()
                ),
            }
            .build());
        }

        // ── Read file once for both staleness check and editing ─────
        let bytes = fs::read(&path).await.map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to read file: {e}"),
            }
            .build()
        })?;
        let encoding = detect_encoding(&bytes);
        let content = encoding.decode(&bytes).map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to decode file: {e}"),
            }
            .build()
        })?;
        let line_ending = detect_line_ending(&content);

        // ── SHA256 staleness check ──────────────────────────────────
        if let Some(read_state) = ctx.file_read_state(&path).await {
            if let Some(ref stored_hash) = read_state.content_hash {
                let normalized = normalize_line_endings(&content, LineEnding::Lf);
                let current_hash = FileReadState::compute_hash(&normalized);
                if *stored_hash != current_hash {
                    return Err(crate::error::tool_error::ExecutionFailedSnafu {
                        message: format!(
                            "File has been modified externally since last read: {}. Read the file again before editing.",
                            path.display()
                        ),
                    }
                    .build());
                }
            }
        }

        // ── Pre-correction (unescape LLM bugs) ─────────────────────
        let (working_old, working_new) = pre_correct_escaping(old_string, new_string, &content);

        // ── Three-tier matching ─────────────────────────────────────
        let match_result = try_match(&content, &working_old, &working_new, replace_all);

        // ── Trim fallback ───────────────────────────────────────────
        let (replaced_content, match_strategy) = match match_result {
            Ok(ok) => ok,
            Err(_) => {
                // Try trimmed pair → re-run three-tier
                if let Some((trimmed_old, trimmed_new)) =
                    trim_pair_if_possible(&working_old, &working_new, &content)
                {
                    match try_match(&content, &trimmed_old, &trimmed_new, replace_all) {
                        Ok(ok) => ok,
                        Err(e) => return Err(e),
                    }
                } else {
                    // All strategies failed — return enhanced error
                    let hint = find_closest_match(&content, &working_old);
                    return Err(crate::error::tool_error::ExecutionFailedSnafu {
                        message: format!(
                            "old_string not found in file (tried exact, flexible, and regex matching): {}\n\
                             Hint: {hint}\n\
                             The file may have changed. Use the Read tool to re-read the file and verify the exact content before retrying.",
                            path.display()
                        ),
                    }
                    .build());
                }
            }
        };

        // ── Write back preserving encoding / line ending ────────────
        let new_content = preserve_trailing_newline(&content, &replaced_content);
        write_with_format_async(&path, &new_content, encoding, line_ending)
            .await
            .map_err(|e| {
                crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!("Failed to write file: {e}"),
                }
                .build()
            })?;

        // ── Track modification and update read state ────────────────
        let normalized_content = normalize_line_endings(&new_content, LineEnding::Lf);
        ctx.record_file_modified(&path).await;
        let new_mtime = fs::metadata(&path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        ctx.record_file_read_with_state(
            &path,
            FileReadState::complete(normalized_content.clone(), new_mtime),
        )
        .await;

        let stats = diff_stats(&content, &new_content);
        let strategy_note = match match_strategy {
            MatchStrategy::Exact => String::new(),
            other => format!(" (matched via {other} strategy)"),
        };
        let mut result = ToolOutput::text(format!(
            "Successfully edited {}{stats}{strategy_note}",
            path.display()
        ));
        result.modifiers.push(ContextModifier::FileRead {
            path: path.clone(),
            content: normalized_content,
        });

        Ok(result)
    }
}

/// Try three-tier matching: Exact → Flexible → Regex.
///
/// Returns `Ok((replaced_content, strategy))` on success, or `Err` with
/// a uniqueness error when `!replace_all && count > 1`.
fn try_match(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<(String, MatchStrategy)> {
    // Tier 1: Exact
    if let Some((replaced, count)) = try_exact_replace(content, old_string, new_string, replace_all)
    {
        if !replace_all && count > 1 {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "old_string is not unique in the file ({count} occurrences). \
                     Provide more context to make it unique, or use replace_all."
                ),
            }
            .build());
        }
        return Ok((replaced, MatchStrategy::Exact));
    }

    // Tier 2: Flexible
    if let Some((replaced, count)) =
        try_flexible_replace(content, old_string, new_string, replace_all)
    {
        if !replace_all && count > 1 {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "old_string is not unique in the file ({count} occurrences, flexible match). \
                     Provide more context to make it unique, or use replace_all."
                ),
            }
            .build());
        }
        tracing::info!(
            strategy = "flexible",
            "Edit matched via whitespace-flexible strategy"
        );
        return Ok((replaced, MatchStrategy::Flexible));
    }

    // Tier 3: Regex (always first match only, no uniqueness issue)
    if let Some((replaced, _count)) = try_regex_replace(content, old_string, new_string) {
        tracing::info!(strategy = "regex", "Edit matched via regex strategy");
        return Ok((replaced, MatchStrategy::Regex));
    }

    // All failed — signal caller to try trim fallback or error
    Err(crate::error::tool_error::ExecutionFailedSnafu {
        message: "no strategy matched".to_string(),
    }
    .build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_edit_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello World").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(file.path()).unwrap();
        assert_eq!(content, "Hello Rust");
    }

    #[tokio::test]
    async fn test_edit_requires_read_first() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello World").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        // Don't read the file first

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edit_non_unique_string() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "foo bar foo").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "foo",
            "new_string": "baz"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_edit_replace_all() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "foo bar foo").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "foo",
            "new_string": "baz",
            "replace_all": true
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(file.path()).unwrap();
        assert_eq!(content, "baz bar baz");
    }

    #[test]
    fn test_tool_properties() {
        let tool = EditTool::new();
        assert_eq!(tool.name(), "Edit");
        assert!(!tool.is_concurrent_safe());
    }

    #[tokio::test]
    async fn test_plan_mode_blocks_non_plan_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello World").unwrap();
        let path = file.path().to_str().unwrap().to_string();
        let plan_file = PathBuf::from("/tmp/plan.md");

        let tool = EditTool::new();
        let mut ctx = make_context().with_plan_mode(true, Some(plan_file));
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Plan mode"));
    }

    #[tokio::test]
    async fn test_plan_mode_allows_plan_file() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let plan_file = dir.path().join("plan.md");
        std::fs::write(&plan_file, "# Plan\n\nold content").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context().with_plan_mode(true, Some(plan_file.clone()));
        ctx.record_file_read(&plan_file).await;

        let input = serde_json::json!({
            "file_path": plan_file.to_str().unwrap(),
            "old_string": "old content",
            "new_string": "new content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&plan_file).unwrap();
        assert!(content.contains("new content"));
    }

    #[tokio::test]
    async fn test_non_plan_mode_allows_any_file() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "Hello World").unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(file.path()).await;

        let input = serde_json::json!({
            "file_path": path,
            "old_string": "World",
            "new_string": "Rust"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_edit_preserves_crlf_line_endings() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("crlf.txt");

        std::fs::write(&file_path, "line1\r\nline2\r\nline3\r\n").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "line2",
            "new_string": "modified"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let bytes = std::fs::read(&file_path).unwrap();
        let content = String::from_utf8(bytes.clone()).unwrap();
        assert!(content.contains("\r\n"), "CRLF should be preserved");
        assert!(content.contains("modified"), "Edit should be applied");
    }

    #[tokio::test]
    async fn test_edit_preserves_lf_line_endings() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("lf.txt");

        std::fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "line2",
            "new_string": "modified"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let bytes = std::fs::read(&file_path).unwrap();
        let content = String::from_utf8(bytes).unwrap();
        assert!(!content.contains("\r\n"), "LF should be preserved, no CRLF");
        assert!(content.contains("modified"), "Edit should be applied");
    }

    #[tokio::test]
    async fn test_edit_rejects_ipynb_files() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.ipynb");

        std::fs::write(
            &file_path,
            r#"{"cells": [], "metadata": {}, "nbformat": 4}"#,
        )
        .unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "cells",
            "new_string": "modified"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("NotebookEdit"));
    }

    #[tokio::test]
    async fn test_edit_flexible_match_indentation() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("indent.rs");

        std::fs::write(
            &file_path,
            "fn main() {\n    let x = 1;\n    let y = 2;\n}\n",
        )
        .unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "  let x = 1;\n  let y = 2;",
            "new_string": "  let x = 10;\n  let y = 20;"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("    let x = 10;"));
        assert!(content.contains("    let y = 20;"));
    }

    #[tokio::test]
    async fn test_edit_flexible_match_trailing_spaces() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("trailing.txt");

        std::fs::write(&file_path, "hello world\ngoodbye world\n").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "hello world  ",
            "new_string": "hello rust"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("hello rust"));
    }

    #[tokio::test]
    async fn test_edit_flexible_respects_replace_all() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("replace_all.txt");

        std::fs::write(&file_path, "    foo bar\n    baz\n    foo bar\n    baz\n").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "foo bar\nbaz",
            "new_string": "replaced\nline",
            "replace_all": true
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(
            content.matches("replaced").count(),
            2,
            "Should replace both occurrences"
        );
    }

    #[tokio::test]
    async fn test_edit_flexible_preserves_crlf() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("crlf_flex.txt");

        std::fs::write(&file_path, "    line1\r\n    line2\r\n    line3\r\n").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "line2",
            "new_string": "modified"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let bytes = std::fs::read(&file_path).unwrap();
        let content = String::from_utf8(bytes).unwrap();
        assert!(content.contains("\r\n"), "CRLF should be preserved");
        assert!(content.contains("modified"), "Edit should be applied");
    }

    #[tokio::test]
    async fn test_edit_diff_stats_in_output() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("stats.txt");

        std::fs::write(&file_path, "line1\nline2\nline3\n").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "line2",
            "new_string": "modified\nextra"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text"),
        };
        assert!(text.contains("Successfully edited"));
        assert!(text.contains("(+"), "Should contain diff stats");
    }

    // ── File creation tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_edit_create_new_file() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("new_file.txt");

        let tool = EditTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "",
            "new_string": "hello world"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text"),
        };
        assert!(text.contains("Created new file"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_edit_create_existing_file_error() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "already here").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "",
            "new_string": "overwrite attempt"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn test_edit_create_with_parent_dirs() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("deep").join("nested").join("file.txt");

        let tool = EditTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "",
            "new_string": "nested content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "nested content");
    }

    // ── SHA256 staleness test ───────────────────────────────────────

    #[tokio::test]
    async fn test_edit_sha256_detects_external_modification() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("sha_test.txt");
        std::fs::write(&file_path, "original content").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();

        // Record read state with hash
        let content = "original content".to_string();
        let mtime = std::fs::metadata(&file_path)
            .ok()
            .and_then(|m| m.modified().ok());
        ctx.record_file_read_with_state(&file_path, FileReadState::complete(content, mtime))
            .await;

        // Externally modify the file
        std::fs::write(&file_path, "externally modified").unwrap();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "original",
            "new_string": "updated"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("modified externally"));
    }

    // ── Regex fallback test ─────────────────────────────────────────

    #[tokio::test]
    async fn test_edit_regex_fallback() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("regex_test.rs");

        // File has collapsed whitespace
        std::fs::write(&file_path, "function test(){body}\n").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        // Model provides with spaces around delimiters
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "function test ( ) { body }",
            "new_string": "function test(){updated}"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("updated"));
        let text = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text"),
        };
        assert!(text.contains("regex"));
    }

    // ── Pre-correction unescape test ────────────────────────────────

    #[tokio::test]
    async fn test_edit_pre_correction_unescape() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("unescape_test.txt");

        std::fs::write(&file_path, "line1\nline2\n").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        // Model over-escapes: \\n instead of real newline
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "line1\\nline2",
            "new_string": "line1\\nupdated"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("updated"));
    }

    // ── SHA256 edge case: no hash (legacy read) skips check ───────

    #[tokio::test]
    async fn test_edit_sha256_no_hash_skips_check() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("no_hash.txt");
        std::fs::write(&file_path, "original content").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();

        // record_file_read (simple) does NOT store a hash
        ctx.record_file_read(&file_path).await;

        // Externally modify the file — staleness check should be skipped
        std::fs::write(&file_path, "externally modified original content").unwrap();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "externally modified ",
            "new_string": ""
        });

        // Should succeed because no hash → no staleness check
        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    // ── Sequential edits update hash correctly ────────────────────

    #[tokio::test]
    async fn test_edit_sequential_edits_update_hash() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("seq_edit.txt");
        std::fs::write(&file_path, "aaa bbb ccc").unwrap();

        let tool = EditTool::new();
        let mut ctx = make_context();

        // First read with full state
        let content = "aaa bbb ccc".to_string();
        let mtime = std::fs::metadata(&file_path)
            .ok()
            .and_then(|m| m.modified().ok());
        ctx.record_file_read_with_state(&file_path, FileReadState::complete(content, mtime))
            .await;

        // First edit
        let input1 = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "aaa",
            "new_string": "xxx"
        });
        let result1 = tool.execute(input1, &mut ctx).await.unwrap();
        assert!(!result1.is_error);
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "xxx bbb ccc");

        // Second edit — should use the updated hash from first edit
        let input2 = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "bbb",
            "new_string": "yyy"
        });
        let result2 = tool.execute(input2, &mut ctx).await.unwrap();
        assert!(!result2.is_error);
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), "xxx yyy ccc");
    }
}
