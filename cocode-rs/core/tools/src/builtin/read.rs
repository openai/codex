//! Read tool for reading file contents.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
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

/// Tool for reading file contents.
///
/// This is a safe tool that can run concurrently with other tools.
pub struct ReadTool {
    /// Maximum file size to read (bytes).
    max_file_size: i64,
    /// Maximum lines to read.
    max_lines: i32,
}

impl ReadTool {
    /// Create a new Read tool with default settings.
    pub fn new() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10 MB
            max_lines: 2000,
        }
    }

    /// Set the maximum file size.
    pub fn with_max_file_size(mut self, size: i64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Set the maximum lines.
    pub fn with_max_lines(mut self, lines: i32) -> Self {
        self.max_lines = lines;
        self
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        prompts::READ_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The absolute path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (1-indexed)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read"
                }
            },
            "required": ["file_path"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    fn max_result_size_chars(&self) -> i32 {
        100_000
    }

    async fn check_permission(&self, input: &Value, ctx: &ToolContext) -> PermissionResult {
        let file_path = match input.get("file_path").and_then(|v| v.as_str()) {
            Some(fp) => fp,
            None => return PermissionResult::Passthrough,
        };

        let path = ctx.resolve_path(file_path);

        // Locked directory → Deny
        if crate::sensitive_files::is_locked_directory(&path) {
            return PermissionResult::Denied {
                reason: format!(
                    "Reading locked directory is not allowed: {}",
                    path.display()
                ),
            };
        }

        // Sensitive file → NeedsApproval
        if crate::sensitive_files::is_sensitive_file(&path) {
            return PermissionResult::NeedsApproval {
                request: ApprovalRequest {
                    request_id: format!("sensitive-read-{}", path.display()),
                    tool_name: self.name().to_string(),
                    description: format!("Reading sensitive file: {}", path.display()),
                    risks: vec![SecurityRisk {
                        risk_type: RiskType::SensitiveFile,
                        severity: RiskSeverity::Medium,
                        message: format!(
                            "File '{}' may contain credentials or sensitive configuration",
                            path.display()
                        ),
                    }],
                    allow_remember: true,
                },
            };
        }

        // Outside working directory → NeedsApproval
        if crate::sensitive_files::is_outside_cwd(&path, &ctx.cwd) {
            return PermissionResult::NeedsApproval {
                request: ApprovalRequest {
                    request_id: format!("outside-cwd-read-{}", path.display()),
                    tool_name: self.name().to_string(),
                    description: format!(
                        "Reading file outside working directory: {}",
                        path.display()
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
        let file_path = input["file_path"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "file_path must be a string",
            }
            .build()
        })?;

        let offset = input["offset"].as_i64().map(|n| n as i32).unwrap_or(0);
        let limit = input["limit"]
            .as_i64()
            .map(|n| n as i32)
            .unwrap_or(self.max_lines);

        // Resolve path
        let path = ctx.resolve_path(file_path);

        // Check if file exists
        if !path.exists() {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("File not found: {}", path.display()),
            }
            .build());
        }

        // Check file size
        let metadata = fs::metadata(&path).await?;
        if metadata.len() as i64 > self.max_file_size {
            return Err(crate::error::tool_error::ExecutionFailedSnafu {
                message: format!(
                    "File too large: {} bytes (max: {} bytes)",
                    metadata.len(),
                    self.max_file_size
                ),
            }
            .build());
        }

        // Get file modification time for tracking
        let file_mtime = metadata.modified().ok();

        // Read file
        let content = fs::read_to_string(&path).await.map_err(|e| {
            crate::error::tool_error::ExecutionFailedSnafu {
                message: format!("Failed to read file: {e}"),
            }
            .build()
        })?;

        // Apply offset and limit
        let lines: Vec<&str> = content.lines().collect();
        let start = offset.max(0) as usize;
        let end = (start + limit as usize).min(lines.len());
        let is_complete = start == 0 && end >= lines.len();

        // Format with line numbers (cat -n format)
        let mut output = String::new();
        for (idx, line) in lines[start..end].iter().enumerate() {
            let line_num = start + idx + 1;
            // Truncate lines > 2000 characters
            let truncated = if line.len() > 2000 {
                format!("{}...", &line[..2000])
            } else {
                line.to_string()
            };
            output.push_str(&format!("{:>6}\t{}\n", line_num, truncated));
        }

        // Record file read with full state tracking
        use crate::context::FileReadState;
        let read_state = if is_complete {
            FileReadState::complete(content.clone(), file_mtime)
        } else {
            FileReadState::partial(offset, limit, file_mtime)
        };
        ctx.record_file_read_with_state(&path, read_state).await;

        // Create output with file read modifier
        let mut result = ToolOutput::text(output);
        result.modifiers.push(ContextModifier::FileRead {
            path: path.clone(),
            content: content.clone(),
        });

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_read_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        let tool = ReadTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file.path().to_str().unwrap()
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 2"));
        assert!(content.contains("Line 3"));
    }

    #[tokio::test]
    async fn test_read_with_offset_and_limit() {
        let mut file = NamedTempFile::new().unwrap();
        for i in 1..=10 {
            writeln!(file, "Line {i}").unwrap();
        }

        let tool = ReadTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": file.path().to_str().unwrap(),
            "offset": 3,
            "limit": 2
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };

        assert!(content.contains("Line 4"));
        assert!(content.contains("Line 5"));
        assert!(!content.contains("Line 3"));
        assert!(!content.contains("Line 6"));
    }

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let tool = ReadTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "file_path": "/nonexistent/file.txt"
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_properties() {
        let tool = ReadTool::new();
        assert_eq!(tool.name(), "Read");
        assert!(tool.is_concurrent_safe());
    }
}
