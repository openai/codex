//! Bash tool for executing shell commands.

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
use cocode_shell::BackgroundProcess;
use serde_json::Value;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::Notify;

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

        // Background execution
        if run_in_background {
            let task_id = format!("bg-{}", uuid_simple());
            let output = Arc::new(Mutex::new(String::new()));
            let completed = Arc::new(Notify::new());

            let process = BackgroundProcess {
                id: task_id.clone(),
                command: command.to_string(),
                output: Arc::clone(&output),
                completed: Arc::clone(&completed),
            };

            // Register the background task
            ctx.background_registry
                .register(task_id.clone(), process)
                .await;

            // Spawn the background process
            let cwd = ctx.cwd.clone();
            let cmd = command.to_string();

            tokio::spawn(async move {
                let child = Command::new("bash")
                    .arg("-c")
                    .arg(&cmd)
                    .current_dir(&cwd)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .kill_on_drop(true)
                    .spawn();

                match child {
                    Ok(mut child) => {
                        // Read stdout asynchronously
                        if let Some(mut stdout) = child.stdout.take() {
                            let output_clone = Arc::clone(&output);
                            tokio::spawn(async move {
                                let mut buf = vec![0u8; 4096];
                                loop {
                                    match stdout.read(&mut buf).await {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            if let Ok(text) = String::from_utf8(buf[..n].to_vec()) {
                                                let mut out = output_clone.lock().await;
                                                out.push_str(&text);
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            });
                        }

                        // Read stderr asynchronously
                        if let Some(mut stderr) = child.stderr.take() {
                            let output_clone = Arc::clone(&output);
                            tokio::spawn(async move {
                                let mut buf = vec![0u8; 4096];
                                loop {
                                    match stderr.read(&mut buf).await {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            if let Ok(text) = String::from_utf8(buf[..n].to_vec()) {
                                                let mut out = output_clone.lock().await;
                                                out.push_str(&text);
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            });
                        }

                        // Wait for process to complete
                        let _ = child.wait().await;
                    }
                    Err(e) => {
                        let mut out = output.lock().await;
                        out.push_str(&format!("Failed to spawn command: {e}"));
                    }
                }

                completed.notify_waiters();
                // Note: Task remains in registry until explicitly stopped or output retrieved
            });

            return Ok(ToolOutput::text(format!(
                "Background task started with ID: {task_id}\n\
                 Use TaskOutput tool with task_id=\"{task_id}\" to retrieve output."
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

/// Generates a simple unique identifier (timestamp-based).
fn uuid_simple() -> String {
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
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
