//! Bash tool for executing shell commands.
//!
//! Delegates to [`ShellExecutor`] for command execution, CWD tracking,
//! and background task management.

use super::prompts;
use crate::context::ToolContext;
use crate::error::Result;
use crate::tool::Tool;
use async_trait::async_trait;
use cocode_protocol::ApprovalRequest;
use cocode_protocol::ConcurrencySafety;
use cocode_protocol::PermissionResult;
use cocode_protocol::RiskSeverity;
use cocode_protocol::RiskType;
use cocode_protocol::SecurityRisk;
use cocode_protocol::ToolOutput;
use cocode_shell::CommandResult;
use serde_json::Value;

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

    async fn check_permission(&self, input: &Value, _ctx: &ToolContext) -> PermissionResult {
        let command = match input.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => return PermissionResult::Passthrough,
        };

        // Read-only commands are always allowed
        if is_read_only_command(command) {
            return PermissionResult::Allowed;
        }

        // Run security analysis using cocode-shell-parser
        let (_, analysis) = cocode_shell_parser::parse_and_analyze(command);

        if analysis.has_risks() {
            // Allow-phase risks → Deny immediately (injection vectors)
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

            // Ask-phase risks → NeedsApproval with risk details
            let ask_phase_risks =
                analysis.risks_by_phase(cocode_shell_parser::security::RiskPhase::Ask);
            if !ask_phase_risks.is_empty() {
                let risks: Vec<SecurityRisk> = ask_phase_risks
                    .iter()
                    .map(|r| SecurityRisk {
                        risk_type: match r.kind {
                            cocode_shell_parser::security::RiskKind::NetworkExfiltration => {
                                RiskType::Network
                            }
                            cocode_shell_parser::security::RiskKind::PrivilegeEscalation => {
                                RiskType::Elevated
                            }
                            cocode_shell_parser::security::RiskKind::FileSystemTampering => {
                                RiskType::Destructive
                            }
                            cocode_shell_parser::security::RiskKind::SensitiveRedirect => {
                                RiskType::SensitiveFile
                            }
                            cocode_shell_parser::security::RiskKind::CodeExecution => {
                                RiskType::SystemConfig
                            }
                            _ => RiskType::Unknown,
                        },
                        severity: match r.level {
                            cocode_shell_parser::security::RiskLevel::Low => RiskSeverity::Low,
                            cocode_shell_parser::security::RiskLevel::Medium => {
                                RiskSeverity::Medium
                            }
                            cocode_shell_parser::security::RiskLevel::High => RiskSeverity::High,
                            cocode_shell_parser::security::RiskLevel::Critical => {
                                RiskSeverity::Critical
                            }
                        },
                        message: r.message.clone(),
                    })
                    .collect();

                let description = if command.len() > 120 {
                    format!("{}...", &command[..120])
                } else {
                    command.to_string()
                };

                return PermissionResult::NeedsApproval {
                    request: ApprovalRequest {
                        request_id: format!(
                            "bash-security-{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_nanos())
                                .unwrap_or(0)
                        ),
                        tool_name: "Bash".to_string(),
                        description,
                        risks,
                        allow_remember: true,
                        proposed_prefix_pattern: None,
                    },
                };
            }
        }

        // Non-read-only command with no detected risks → still needs approval
        PermissionResult::NeedsApproval {
            request: ApprovalRequest {
                request_id: format!(
                    "bash-cmd-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0)
                ),
                tool_name: "Bash".to_string(),
                description: if command.len() > 120 {
                    format!("{}...", &command[..120])
                } else {
                    command.to_string()
                },
                risks: vec![],
                allow_remember: true,
                proposed_prefix_pattern: None,
            },
        }
    }

    async fn execute(&self, input: Value, ctx: &mut ToolContext) -> Result<ToolOutput> {
        let command = input["command"].as_str().ok_or_else(|| {
            crate::error::tool_error::InvalidInputSnafu {
                message: "command must be a string",
            }
            .build()
        })?;

        let timeout_ms = input["timeout"]
            .as_i64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS * 1000);
        let timeout_secs = (timeout_ms / 1000).min(MAX_TIMEOUT_SECS);
        let run_in_background = input["run_in_background"].as_bool().unwrap_or(false);

        // Emit progress
        let desc = input["description"].as_str().unwrap_or("Executing command");
        ctx.emit_progress(desc).await;

        // Background execution — delegate to ShellExecutor
        if run_in_background {
            let task_id = ctx
                .shell_executor
                .spawn_background(command)
                .await
                .map_err(|e| {
                    crate::error::tool_error::ExecutionFailedSnafu {
                        message: e.to_string(),
                    }
                    .build()
                })?;

            return Ok(ToolOutput::text(format!(
                "Background task started with ID: {task_id}\n\
                 Use TaskOutput tool with task_id=\"{task_id}\" to retrieve output."
            )));
        }

        // Foreground execution — delegate to ShellExecutor with CWD tracking
        let result = ctx
            .shell_executor
            .execute_with_cwd_tracking(command, timeout_secs)
            .await;

        // Sync CWD back to ctx
        if let Some(ref new_cwd) = result.new_cwd {
            ctx.cwd = new_cwd.clone();
        }

        // Convert CommandResult → ToolOutput
        format_command_result(&result)
    }
}

/// Convert a [`CommandResult`] to a [`ToolOutput`].
fn format_command_result(result: &CommandResult) -> Result<ToolOutput> {
    let mut text = String::new();
    if !result.stdout.is_empty() {
        text.push_str(&result.stdout);
    }
    if !result.stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("STDERR:\n");
        text.push_str(&result.stderr);
    }

    if result.exit_code != 0 {
        if text.is_empty() {
            text = format!("Command failed with exit code {}", result.exit_code);
        } else {
            text.push_str(&format!("\n\nExit code: {}", result.exit_code));
        }
        return Ok(ToolOutput::error(text));
    }

    if text.is_empty() {
        text = "(no output)".to_string();
    }
    Ok(ToolOutput::text(text))
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
    fn test_tool_properties() {
        let tool = BashTool::new();
        assert_eq!(tool.name(), "Bash");
        assert!(!tool.is_concurrent_safe());
    }
}
