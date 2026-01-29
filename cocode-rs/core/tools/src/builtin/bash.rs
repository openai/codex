//! Bash tool for executing shell commands.

use super::prompts;
use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::{ConcurrencySafety, ToolOutput};
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;

/// Maximum output size (bytes) before truncation.
const MAX_OUTPUT_SIZE: usize = 30_000;
/// Default timeout in seconds.
const DEFAULT_TIMEOUT_SECS: i64 = 120;
/// Maximum timeout in seconds.
const MAX_TIMEOUT_SECS: i64 = 600;

/// Tool for executing shell commands.
pub struct BashTool;

impl BashTool {
    /// Create a new Bash tool.
    pub fn new() -> Self {
        Self
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a command is read-only (safe for concurrent execution).
pub fn is_read_only_command(command: &str) -> bool {
    let trimmed = command.trim();
    let first_word = trimmed.split_whitespace().next().unwrap_or("");

    matches!(
        first_word,
        "ls" | "cat"
            | "head"
            | "tail"
            | "wc"
            | "grep"
            | "rg"
            | "find"
            | "which"
            | "whoami"
            | "pwd"
            | "echo"
            | "date"
            | "env"
            | "printenv"
            | "uname"
            | "hostname"
            | "df"
            | "du"
            | "file"
            | "stat"
            | "type"
            | "git"
    ) && !trimmed.contains("&&")
        && !trimmed.contains("||")
        && !trimmed.contains(';')
        && !trimmed.contains('|')
        && !trimmed.contains('>')
        && !trimmed.contains('<')
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        prompts::BASH_DESCRIPTION
    }

    fn input_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "description": {
                    "type": "string",
                    "description": "Clear description of what this command does"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Optional timeout in milliseconds (max 600000)"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Run command in background",
                    "default": false
                }
            },
            "required": ["command"]
        })
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        // Bash is generally unsafe, but per-command check via is_concurrency_safe_for()
        ConcurrencySafety::Unsafe
    }

    fn is_concurrency_safe_for(&self, input: &Value) -> bool {
        input["command"]
            .as_str()
            .map(is_read_only_command)
            .unwrap_or(false)
    }

    fn is_read_only(&self) -> bool {
        false // Cannot determine without input
    }

    fn max_result_size_chars(&self) -> i32 {
        30_000
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| ToolError::invalid_input("command must be a string"))?;

        let timeout_ms = input["timeout"]
            .as_i64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS * 1000);
        let timeout_secs = (timeout_ms / 1000).min(MAX_TIMEOUT_SECS);
        let run_in_background = input["run_in_background"].as_bool().unwrap_or(false);

        // Emit progress
        let desc = input["description"].as_str().unwrap_or("Executing command");
        ctx.emit_progress(desc).await;

        // Background execution stub
        if run_in_background {
            // TODO: spawn as background task via BackgroundTaskRegistry (from exec/shell)
            // and return task ID immediately. For now, return a stub response.
            let task_id = format!("bg-{}", ctx.call_id);
            return Ok(ToolOutput::text(format!(
                "Background task started with ID: {task_id}\n\n\
                 [Background execution not yet connected — command will not run.\\n\
                 To enable, wire up BackgroundTaskRegistry from exec/shell.]"
            )));
        }

        // Execute command
        let timeout_duration = std::time::Duration::from_secs(timeout_secs as u64);

        let result = tokio::time::timeout(timeout_duration, async {
            Command::new("bash")
                .arg("-c")
                .arg(command)
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
                return Err(ToolError::execution_failed(format!(
                    "Failed to execute command: {e}"
                )));
            }
            Err(_) => {
                return Err(ToolError::timeout(timeout_secs));
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Build output
        let mut result_text = String::new();
        if !stdout.is_empty() {
            let truncated = truncate_output(&stdout, MAX_OUTPUT_SIZE);
            result_text.push_str(&truncated);
        }
        if !stderr.is_empty() {
            if !result_text.is_empty() {
                result_text.push('\n');
            }
            result_text.push_str("STDERR:\n");
            let truncated = truncate_output(&stderr, MAX_OUTPUT_SIZE);
            result_text.push_str(&truncated);
        }

        if exit_code != 0 {
            if result_text.is_empty() {
                result_text = format!("Command failed with exit code {exit_code}");
            } else {
                result_text.push_str(&format!("\n\nExit code: {exit_code}"));
            }
            return Ok(ToolOutput::error(result_text));
        }

        if result_text.is_empty() {
            result_text = "(no output)".to_string();
        }

        Ok(ToolOutput::text(result_text))
    }
}

/// Truncate output if it exceeds the maximum size.
fn truncate_output(output: &str, max_size: usize) -> String {
    if output.len() <= max_size {
        output.to_string()
    } else {
        let half = max_size / 2;
        let start = &output[..half];
        let end = &output[output.len() - half..];
        format!(
            "{start}\n\n... (output truncated, {} characters omitted) ...\n\n{end}",
            output.len() - max_size
        )
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
    async fn test_bash_echo() {
        let tool = BashTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "command": "echo hello"
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
    async fn test_bash_failure() {
        let tool = BashTool::new();
        let mut ctx = make_context();

        let input = serde_json::json!({
            "command": "exit 1"
        });

        let result = tool.execute(input, &mut ctx).await.unwrap();
        assert!(result.is_error);
    }

    #[test]
    fn test_is_read_only() {
        assert!(is_read_only_command("ls -la"));
        assert!(is_read_only_command("cat file.txt"));
        assert!(is_read_only_command("git status"));
        assert!(!is_read_only_command("rm -rf /"));
        assert!(!is_read_only_command("ls && rm foo"));
        assert!(!is_read_only_command("echo foo > bar"));
    }

    #[test]
    fn test_truncate_output() {
        let short = "hello";
        assert_eq!(truncate_output(short, 100), "hello");

        let long = "a".repeat(200);
        let result = truncate_output(&long, 100);
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_tool_properties() {
        let tool = BashTool::new();
        assert_eq!(tool.name(), "Bash");
        assert!(!tool.is_concurrent_safe());
    }
}
