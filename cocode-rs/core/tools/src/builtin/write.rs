//! Write tool for creating or overwriting files.

use super::prompts;
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

/// Tool for writing files to the local filesystem.
///
/// For existing files, requires the file to have been read first.
pub struct WriteTool;

impl WriteTool {
    /// Create a new Write tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "Write"
    }

    fn description(&self) -> &str {
        prompts::WRITE_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
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
                        "Writing to locked directory is not allowed: {}",
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
                                "Plan mode: cannot write to '{}'. Only the plan file can be modified.",
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
                        request_id: format!("sensitive-write-{}", path.display()),
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
                    },
                };
            }

            // Sensitive directory (.git/, .vscode/, .idea/) → NeedsApproval
            if crate::sensitive_files::is_sensitive_directory(&path) {
                return PermissionResult::NeedsApproval {
                    request: ApprovalRequest {
                        request_id: format!("sensitive-dir-write-{}", path.display()),
                        tool_name: self.name().to_string(),
                        description: format!("Writing to sensitive directory: {}", path.display()),
                        risks: vec![SecurityRisk {
                            risk_type: RiskType::SystemConfig,
                            severity: RiskSeverity::Medium,
                            message: format!(
                                "Directory '{}' contains project configuration",
                                path.display()
                            ),
                        }],
                        allow_remember: true,
                    },
                };
            }
        }

        // All writes default to NeedsApproval
        PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                request_id: format!(
                    "write-{}",
                    input
                        .get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                ),
                tool_name: self.name().to_string(),
                description: format!(
                    "Write: {}",
                    input
                        .get("file_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                ),
                risks: vec![],
                allow_remember: true,
            },
        }
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "file_path must be a string",
            }
            .build()
        })?;
        let content = input["content"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "content must be a string",
            }
            .build()
        })?;

        let path = ctx.resolve_path(file_path);

        // Plan mode check: only allow writes to the plan file
        if ctx.is_plan_mode {
            if !is_safe_file(&path, ctx.plan_file_path.as_deref()) {
                return Err(crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!(
                        "Plan mode: cannot write to '{}'. Only the plan file can be modified during plan mode.",
                        path.display()
                    ),
                }
                .build());
            }
        }

        // Detect original encoding, line ending, and trailing newline for existing files
        let (original_encoding, original_line_ending, original_content_for_trailing) = if path
            .exists()
        {
            if !ctx.was_file_read(&path).await {
                return Err(crate::error::tool_error::ExecutionFailedSnafu {
                        message: format!(
                            "Existing file must be read before overwriting: {}. Use the Read tool first.",
                            path.display()
                        ),
                    }
                    .build());
            }

            // Check file_mtime hasn't changed since last read (detect external modifications)
            let current_mtime = fs::metadata(&path)
                .await
                .ok()
                .and_then(|m| m.modified().ok());
            if let Some(read_state) = ctx.file_read_state(&path).await {
                if let (Some(read_mtime), Some(curr_mtime)) = (read_state.file_mtime, current_mtime)
                {
                    if curr_mtime > read_mtime {
                        return Err(crate::error::tool_error::ExecutionFailedSnafu {
                                message: format!(
                                    "File has been modified externally since last read: {}. Read the file again before writing.",
                                    path.display()
                                ),
                            }
                            .build());
                    }
                }
            }

            // Detect encoding and line ending from original file
            let bytes = fs::read(&path).await.map_err(|e| {
                crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!("Failed to read file for encoding detection: {e}"),
                }
                .build()
            })?;
            let encoding = detect_encoding(&bytes);
            let original_content = encoding.decode(&bytes).map_err(|e| {
                crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!("Failed to decode file: {e}"),
                }
                .build()
            })?;
            let line_ending = detect_line_ending(&original_content);
            (encoding, line_ending, Some(original_content))
        } else {
            // New file: use defaults (UTF-8, LF), no trailing newline preservation
            (Encoding::Utf8, LineEnding::Lf, None)
        };

        // Ensure parent directory exists
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

        // Preserve trailing newline state from original file
        let content_to_write = if let Some(ref orig) = original_content_for_trailing {
            preserve_trailing_newline(orig, content)
        } else {
            content.to_string()
        };

        // Write file preserving encoding and line ending
        write_with_format_async(
            &path,
            &content_to_write,
            original_encoding,
            original_line_ending,
        )
        .await
        .map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to write file: {e}"),
            }
            .build()
        })?;

        // Normalize content for the context (always use LF internally)
        let normalized_content = normalize_line_endings(&content_to_write, LineEnding::Lf);

        // Track modification and update read state with new content/mtime
        ctx.record_file_modified(&path).await;
        let new_mtime = fs::metadata(&path)
            .await
            .ok()
            .and_then(|m| m.modified().ok());
        use crate::context::FileReadState;
        ctx.record_file_read_with_state(
            &path,
            FileReadState::complete(normalized_content.clone(), new_mtime),
        )
        .await;

        let mut result = ToolOutput::text(format!("Successfully wrote to {}", path.display()));
        result.modifiers.push(ContextModifier::FileRead {
            path: path.clone(),
            content: normalized_content,
        });

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_write_new_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");

        let tool = WriteTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "Hello World"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello World");
    }

    #[tokio::test]
    async fn test_write_creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("sub").join("dir").join("test.txt");

        let tool = WriteTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "nested content"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "nested content");
    }

    #[tokio::test]
    async fn test_write_existing_requires_read() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "original").unwrap();

        let tool = WriteTool::new();
        let mut ctx = make_context();
        // Don't read the file first

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "overwritten"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_write_existing_after_read() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("existing.txt");
        std::fs::write(&file_path, "original").unwrap();

        let tool = WriteTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "overwritten"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "overwritten");
    }

    #[test]
    fn test_tool_properties() {
        let tool = WriteTool::new();
        assert_eq!(tool.name(), "Write");
        assert!(!tool.is_concurrent_safe());
    }

    #[tokio::test]
    async fn test_plan_mode_blocks_non_plan_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("code.rs");
        let plan_file = dir.path().join("plan.md");

        let tool = WriteTool::new();
        let mut ctx = make_context().with_plan_mode(true, Some(plan_file));

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "fn main() {}"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Plan mode"));
    }

    #[tokio::test]
    async fn test_plan_mode_allows_plan_file() {
        let dir = TempDir::new().unwrap();
        let plan_file = dir.path().join("plan.md");

        let tool = WriteTool::new();
        let mut ctx = make_context().with_plan_mode(true, Some(plan_file.clone()));

        let input = serde_json::json!({
            "file_path": plan_file.to_str().unwrap(),
            "content": "# My Plan\n\n- Step 1\n- Step 2"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        let content = std::fs::read_to_string(&plan_file).unwrap();
        assert!(content.contains("# My Plan"));
    }

    #[tokio::test]
    async fn test_non_plan_mode_allows_any_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("code.rs");

        let tool = WriteTool::new();
        // is_plan_mode = false (default)
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "fn main() {}"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_write_preserves_crlf_line_endings() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("crlf.txt");

        // Create file with CRLF line endings
        std::fs::write(&file_path, "line1\r\nline2\r\n").unwrap();

        let tool = WriteTool::new();
        let mut ctx = make_context();
        ctx.record_file_read(&file_path).await;

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "new line1\nnew line2\n"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        // Verify CRLF was preserved
        let bytes = std::fs::read(&file_path).unwrap();
        assert!(bytes.windows(2).any(|w| w == b"\r\n"));
        assert!(!bytes.contains(&b'\n') || bytes.windows(2).filter(|w| *w == b"\r\n").count() > 0);
    }

    #[tokio::test]
    async fn test_write_new_file_uses_lf() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("new.txt");

        let tool = WriteTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "line1\nline2\n"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(!result.is_error);

        // Verify LF is used for new files
        let bytes = std::fs::read(&file_path).unwrap();
        assert_eq!(bytes, b"line1\nline2\n");
    }
}
