//! Shell tool for executing commands via array format (direct exec, no shell).

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ApprovalRequest;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::PermissionResult;
use cocode_protocol::ToolOutput;
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;

/// Default timeout in seconds.
const DEFAULT_TIMEOUT_SECS: i64 = 120;
/// Maximum timeout in seconds.
const MAX_TIMEOUT_SECS: i64 = 600;

/// Tool for executing commands via array format (direct exec, no shell).
///
/// Unlike [`BashTool`], this tool takes a `Vec<String>` command array
/// and executes it directly without a shell interpreter. This is useful
/// for models that prefer structured command invocation.
pub struct ShellTool;

impl ShellTool {
    /// Create a new Shell tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        prompts::SHELL_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Command as array: [program, arg1, arg2, ...]"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Optional timeout in seconds (max 600)"
                }
            },
            "required": ["command"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        ConcurrencySafety::Unsafe
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn max_result_size_chars(&self) -> i32 {
        30_000
    }

    async fn check_permission(&self, input: &Value, _ctx: &ToolContext) -> PermissionResult {
        let args = match input.get("command").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return PermissionResult::Passthrough,
        };

        let command_str: String = args
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        if command_str.is_empty() {
            return PermissionResult::Passthrough;
        }

        // Run security analysis on the joined command string
        let (_, analysis) = cocode_shell_parser::parse_and_analyze(&command_str);

        if analysis.has_risks() {
            let allow_phase_risks =
                analysis.risks_by_phase(cocode_shell_parser::security::RiskPhase::Allow);
            if !allow_phase_risks.is_empty() {
                let risk_msgs: Vec<String> = allow_phase_risks
                    .iter()
                    .map(|r| format!("{}: {}", r.kind, r.message))
                    .collect();
                return PermissionResult::Denied {
                    reason: format!(
                        "Command blocked due to security risks: {}",
                        risk_msgs.join("; ")
                    ),
                };
            }
        }

        // Non-trivial command → needs approval
        PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                request_id: format!(
                    "shell-cmd-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0)
                ),
                tool_name: "shell".to_string(),
                description: if command_str.len() > 120 {
                    format!("{}...", &command_str[..120])
                } else {
                    command_str
                },
                risks: vec![],
                allow_remember: true,
                proposed_prefix_pattern: None,
            },
        }
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let args: Vec<String> = input["command"]
            .as_array()
            .ok_or_else(|| {
                crate::error::tool_error::InvalidInputSnafu {
                    message: "command must be an array of strings",
                }
                .build()
            })?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if args.is_empty() {
            return Err(crate::error::tool_error::InvalidInputSnafu {
                message: "command array must not be empty",
            }
            .build());
        }

        let timeout_secs = input["timeout"]
            .as_i64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .min(MAX_TIMEOUT_SECS);

        ctx.emit_progress(format!("Executing: {}", args.join(" ")))
            .await;

        // Direct exec — no shell interpreter
        let timeout_duration = std::time::Duration::from_secs(timeout_secs as u64);

        let result = tokio::time::timeout(timeout_duration, async {
            Command::new(&args[0])
                .args(&args[1..])
                .current_dir(&ctx.cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
        })
        .await;

        let output = match result {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                return Err(crate::error::tool_error::ExecutionFailedSnafu {
                    message: format!("Failed to execute command: {e}"),
                }
                .build());
            }
            Err(_) => {
                return Err(crate::error::tool_error::TimeoutSnafu { timeout_secs }.build());
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut text = String::new();
        if !stdout.is_empty() {
            text.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str("STDERR:\n");
            text.push_str(&stderr);
        }

        if exit_code != 0 {
            if text.is_empty() {
                text = format!("Command failed with exit code {exit_code}");
            } else {
                text.push_str(&format!("\n\nExit code: {exit_code}"));
            }
            return Ok(ToolOutput::error(text));
        }

        if text.is_empty() {
            text = "(no output)".to_string();
        }
        Ok(ToolOutput::text(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_context() -> ToolContext {
        ToolContext::new("call-1", "session-1", PathBuf::from("/tmp"))
    }

    #[tokio::test]
    async fn test_shell_echo() {
        let tool = ShellTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "command": ["echo", "hello"]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        let content = match &result.content {
            cocode_protocol::ToolResultContent::Text(t) => t,
            _ => panic!("Expected text content"),
        };
        assert!(content.contains("hello"));
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_shell_failure() {
        let tool = ShellTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "command": ["false"]
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_shell_empty_command() {
        let tool = ShellTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "command": []
        });

        let result = tool.execute(input, &mut ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_tool_properties() {
        let tool = ShellTool::new();
        assert_eq!(tool.name(), "shell");
        assert!(!tool.is_concurrent_safe());
        assert_eq!(tool.max_result_size_chars(), 30_000);
    }
}
