//! ReadManyFiles tool for batch reading multiple files in a single call.

use super::prompts;
use crate::context::FileReadState;
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

/// Maximum total output characters.
const MAX_TOTAL_CHARS: usize = 200_000;
/// Maximum number of files per call.
const MAX_FILES: usize = 50;
/// Maximum lines per file.
const MAX_LINES_PER_FILE: usize = 500;
/// Maximum characters per line before truncation.
const MAX_LINE_CHARS: usize = 2000;

/// Tool for reading multiple files in a single tool call.
///
/// Useful when exploring a codebase and needing to read several files at once,
/// reducing the number of round-trips.
pub struct ReadManyFilesTool;

impl ReadManyFilesTool {
    /// Create a new ReadManyFiles tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadManyFilesTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadManyFilesTool {
    fn name(&self) -> &str {
        "ReadManyFiles"
    }

    fn description(&self) -> &str {
        prompts::READ_MANY_FILES_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Array of absolute file paths to read"
                }
            },
            "required": ["paths"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Safe
    }

    fn max_result_size_chars(&self) -> i32 {
        MAX_TOTAL_CHARS as i32
    }

    async fn check_permission(&self, input: &Value, ctx: &ToolContext) -> PermissionResult {
        let paths = match input.get("paths").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return PermissionResult::Passthrough,
        };

        for path_val in paths {
            let Some(path_str) = path_val.as_str() else {
                continue;
            };
            let path = ctx.resolve_path(path_str);

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
                        request_id: format!("sensitive-readmany-{}", path.display()),
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
                        proposed_prefix_pattern: None,
                    },
                };
            }

            // Outside working directory → NeedsApproval
            if crate::sensitive_files::is_outside_cwd(&path, &ctx.cwd) {
                return PermissionResult::NeedsApproval {
                    request: ApprovalRequest {
                        request_id: format!("outside-cwd-readmany-{}", path.display()),
                        tool_name: self.name().to_string(),
                        description: format!(
                            "Reading file outside working directory: {}",
                            path.display()
                        ),
                        risks: vec![],
                        allow_remember: true,
                        proposed_prefix_pattern: None,
                    },
                };
            }
        }

        PermissionResult::Allowed
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let paths: Vec<String> = input["paths"]
            .as_array()
            .ok_or_else(|| {
                crate::error::tool_error::InvalidInputSnafu {
                    message: "paths must be an array of strings",
                }
                .build()
            })?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if paths.is_empty() {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "paths array must not be empty",
            }
            .build());
        }

        if paths.len() > MAX_FILES {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: format!("Too many files: {} (max: {MAX_FILES})", paths.len()),
            }
            .build());
        }

        let mut output = String::new();
        let mut modifiers = Vec::new();
        let mut truncated_total = false;

        for path_str in &paths {
            let path = ctx.resolve_path(path_str);

            if !path.exists() {
                output.push_str(&format!("--- {path_str} --- [NOT FOUND]\n\n"));
                if output.len() > MAX_TOTAL_CHARS {
                    truncated_total = true;
                    break;
                }
                continue;
            }

            // Try reading as UTF-8
            let content = match fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => {
                    output.push_str(&format!(
                        "--- {} --- [BINARY/ENCODING ERROR]\n\n",
                        path.display()
                    ));
                    if output.len() > MAX_TOTAL_CHARS {
                        truncated_total = true;
                        break;
                    }
                    continue;
                }
            };

            let lines: Vec<&str> = content.lines().collect();
            let limit = MAX_LINES_PER_FILE.min(lines.len());
            let truncated = lines.len() > limit;

            output.push_str(&format!("--- {} ---\n", path.display()));
            for (i, line) in lines[..limit].iter().enumerate() {
                let line_str = if line.len() > MAX_LINE_CHARS {
                    &line[..line.floor_char_boundary(MAX_LINE_CHARS)]
                } else {
                    line
                };
                output.push_str(&format!("{:>6}\t{}\n", i + 1, line_str));
            }
            if truncated {
                output.push_str(&format!("  ... ({} more lines)\n", lines.len() - limit));
            }
            output.push('\n');

            // Record read state
            let file_mtime = fs::metadata(&path)
                .await
                .ok()
                .and_then(|m| m.modified().ok());
            let read_state = if !truncated {
                FileReadState::complete(content.clone(), file_mtime)
            } else {
                FileReadState::partial(0, MAX_LINES_PER_FILE as i32, file_mtime)
            };
            ctx.record_file_read_with_state(&path, read_state).await;

            modifiers.push(ContextModifier::FileRead {
                path: path.clone(),
                content,
            });

            if output.len() > MAX_TOTAL_CHARS {
                truncated_total = true;
                break;
            }
        }

        if truncated_total {
            output.push_str("[output truncated: total size limit reached]\n");
        }

        let mut result = ToolOutput::text(output);
        result.modifiers = modifiers;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_context(cwd: PathBuf) -> ToolContext {
        ToolContext::new("call-1", "session-1", cwd)
    }

    #[tokio::test]
    async fn test_read_many_basic() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello\n").unwrap();
        std::fs::write(dir.path().join("b.txt"), "world\n").unwrap();

        let tool = ReadManyFilesTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "paths": [
                dir.path().join("a.txt").to_str().unwrap(),
                dir.path().join("b.txt").to_str().unwrap()
            ]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text"),
        };

        assert!(content.contains("a.txt"));
        assert!(content.contains("hello"));
        assert!(content.contains("b.txt"));
        assert!(content.contains("world"));
        assert_eq!(result.modifiers.len(), 2);
    }

    #[tokio::test]
    async fn test_read_many_missing_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("exists.txt"), "content\n").unwrap();

        let tool = ReadManyFilesTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "paths": [
                dir.path().join("exists.txt").to_str().unwrap(),
                dir.path().join("missing.txt").to_str().unwrap()
            ]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text"),
        };

        assert!(content.contains("content"));
        assert!(content.contains("[NOT FOUND]"));
    }

    #[tokio::test]
    async fn test_read_many_empty_paths() {
        let dir = TempDir::new().unwrap();
        let tool = ReadManyFilesTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "paths": []
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_read_many_tracks_file_reads() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("tracked.txt");
        std::fs::write(&file_path, "tracked content\n").unwrap();

        let tool = ReadManyFilesTool::new();
        let mut ctx = make_context(dir.path().to_path_buf());

        let input = serde_json::json!({
            "paths": [file_path.to_str().unwrap()]
        });

        tool.execute(input, &mut ctx).await.unwrap();

        // File should be recorded as read
        assert!(ctx.was_file_read(&file_path).await);
    }

    #[test]
    fn test_tool_properties() {
        let tool = ReadManyFilesTool::new();
        assert_eq!(tool.name(), "ReadManyFiles");
        assert!(tool.is_concurrent_safe());
        assert!(tool.is_read_only()); // default is true
    }
}
