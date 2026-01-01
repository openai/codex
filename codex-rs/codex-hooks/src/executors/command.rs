//! Command hook executor.
//!
//! Executes shell commands aligned with Claude Code's `executeShellHook` function.
//!
//! ## Execution Flow
//!
//! 1. Spawn child process with shell=true
//! 2. Write JSON-stringified HookInput to stdin
//! 3. Collect stdout/stderr
//! 4. Check for async response marker `{"async": true}`
//! 5. Parse exit code and JSON output
//!
//! ## Exit Code Semantics (Claude Code alignment)
//!
//! - 0: Success - stdout parsed as JSON or treated as plain text
//! - 2: BLOCKING ERROR - stops execution, stderr used as error message
//! - 1, 3+: Non-blocking error - warns but continues
//!
//! ## Environment Variables
//!
//! - `CLAUDE_PROJECT_DIR`: Always set to project root
//! - `CLAUDE_ENV_FILE`: SessionStart only, path for hook to write env vars
//! - `CODEX_CODE_SHELL_PREFIX`: Optional command prefix (wraps the hook command)
//!
//! ## Shell Prefix
//!
//! The shell prefix can be configured in two ways (in priority order):
//! 1. Config-based: `shellPrefix` field in hooks.json
//! 2. Environment variable: `CODEX_CODE_SHELL_PREFIX`

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::warn;

use crate::error::HookError;
use crate::input::HookInput;
use crate::output::AsyncResponse;
use crate::output::HookOutcome;
use crate::output::HookOutput;
use crate::output::HookResult;
use crate::types::HookEventType;

/// Command execution configuration.
#[derive(Debug, Clone)]
pub struct CommandConfig {
    /// The shell command to execute.
    pub command: String,
    /// Timeout in seconds.
    pub timeout_secs: u32,
    /// Optional status message for progress display.
    pub status_message: Option<String>,
}

/// Result of command execution.
#[derive(Debug)]
pub struct CommandResult {
    /// Raw stdout output.
    pub stdout: String,
    /// Raw stderr output.
    pub stderr: String,
    /// Exit code from the process.
    pub exit_code: i32,
    /// Execution outcome.
    pub outcome: HookOutcome,
    /// Parsed JSON output (if stdout was valid JSON).
    pub parsed_output: Option<HookOutput>,
    /// Whether the hook declared async execution.
    pub is_async: bool,
    /// Async timeout if specified.
    pub async_timeout: Option<u64>,
}

/// Execute a shell command hook.
///
/// Spawns a shell process, passes the hook input as JSON via stdin,
/// and interprets the exit code and output according to Claude Code semantics.
///
/// # Arguments
///
/// * `config` - Command configuration
/// * `input` - The hook input to pass to the command
/// * `event_type` - The type of hook event (for environment setup)
/// * `cwd` - Working directory for the command
/// * `cancel` - Cancellation token for timeout/abort handling
/// * `hook_index` - Index of this hook in the execution order
/// * `shell_prefix` - Optional shell prefix from config (takes precedence over env var)
pub async fn execute_command(
    config: &CommandConfig,
    input: &HookInput,
    event_type: HookEventType,
    cwd: &Path,
    cancel: CancellationToken,
    hook_index: i32,
    shell_prefix: Option<&str>,
) -> Result<CommandResult, HookError> {
    let timeout = Duration::from_secs(config.timeout_secs as u64);

    debug!(
        command = %config.command,
        timeout_secs = config.timeout_secs,
        hook_index,
        "Executing command hook"
    );

    // Build environment
    let mut env_vars: Vec<(String, String)> = std::env::vars().collect();

    // Always set CLAUDE_PROJECT_DIR
    env_vars.push(("CLAUDE_PROJECT_DIR".to_string(), cwd.display().to_string()));

    // SessionStart gets CLAUDE_ENV_FILE
    if event_type == HookEventType::SessionStart {
        let env_file_path = get_env_file_path(hook_index, &input.session_id);
        env_vars.push(("CLAUDE_ENV_FILE".to_string(), env_file_path));
    }

    // Apply shell prefix (config takes precedence over env var)
    // This wraps the command with a prefix, e.g., "/custom/init.sh echo 'hello'"
    let effective_command = get_effective_command(&config.command, shell_prefix);

    // Spawn process
    let mut child = Command::new("sh")
        .args(["-c", &effective_command])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(cwd)
        .envs(env_vars)
        .spawn()
        .map_err(HookError::SpawnFailed)?;

    // Write JSON input to stdin
    let input_json = serde_json::to_string(input)?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input_json.as_bytes())
            .await
            .map_err(HookError::SpawnFailed)?;
        drop(stdin); // Close stdin to signal EOF
    }

    // Wait with timeout and cancellation
    let wait_future = child.wait_with_output();
    tokio::pin!(wait_future);

    let output = tokio::select! {
        _ = cancel.cancelled() => {
            return Ok(CommandResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: -1,
                outcome: HookOutcome::Cancelled,
                parsed_output: None,
                is_async: false,
                async_timeout: None,
            });
        }
        result = tokio::time::timeout(timeout, &mut wait_future) => {
            match result {
                Ok(Ok(output)) => output,
                Ok(Err(e)) => return Err(HookError::SpawnFailed(e)),
                Err(_) => {
                    // Timeout
                    return Err(HookError::Timeout);
                }
            }
        }
    };

    // Parse output
    parse_command_output(output, config)
}

/// Parse command output according to Claude Code semantics.
fn parse_command_output(
    output: std::process::Output,
    config: &CommandConfig,
) -> Result<CommandResult, HookError> {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(1);

    debug!(
        exit_code,
        stdout_len = stdout.len(),
        stderr_len = stderr.len(),
        "Command completed"
    );

    // Check for async response marker
    let (is_async, async_timeout) = check_async_response(&stdout);

    // Try to parse JSON if stdout starts with "{"
    let parsed_output = if stdout.trim().starts_with('{') {
        match serde_json::from_str::<HookOutput>(stdout.trim()) {
            Ok(output) => Some(output),
            Err(e) => {
                debug!(error = %e, "Failed to parse hook output as JSON");
                None
            }
        }
    } else {
        None
    };

    // Determine outcome based on exit code
    let outcome = match exit_code {
        0 => HookOutcome::Success,
        2 => {
            warn!(
                command = %config.command,
                stderr = %stderr,
                "Hook returned blocking error (exit code 2)"
            );
            HookOutcome::Blocking
        }
        _ => {
            warn!(
                command = %config.command,
                exit_code,
                stderr = %stderr,
                "Hook returned non-blocking error"
            );
            HookOutcome::NonBlockingError
        }
    };

    Ok(CommandResult {
        stdout,
        stderr,
        exit_code,
        outcome,
        parsed_output,
        is_async,
        async_timeout,
    })
}

/// Check if the output contains an async response marker.
fn check_async_response(stdout: &str) -> (bool, Option<u64>) {
    let trimmed = stdout.trim();
    if !trimmed.starts_with('{') {
        return (false, None);
    }

    // Try to parse as JSON and check for async marker
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if AsyncResponse::is_async_response(&value) {
            let timeout = value
                .get("asyncTimeout")
                .or_else(|| value.get("async_timeout"))
                .and_then(|v| v.as_u64());
            return (true, timeout);
        }
    }

    (false, None)
}

/// Get the path for the CLAUDE_ENV_FILE.
fn get_env_file_path(hook_index: i32, session_id: &str) -> String {
    let temp_dir = std::env::temp_dir();
    temp_dir
        .join(format!("claude_env_{session_id}_{hook_index}.env"))
        .display()
        .to_string()
}

/// Get the effective command with shell prefix applied.
///
/// Priority:
/// 1. Config-based shell_prefix (passed as parameter)
/// 2. CODEX_CODE_SHELL_PREFIX environment variable
/// 3. No prefix (use command as-is)
fn get_effective_command(command: &str, config_shell_prefix: Option<&str>) -> String {
    // First, try config-based prefix
    if let Some(prefix) = config_shell_prefix {
        if !prefix.is_empty() {
            debug!(prefix = %prefix, "Applying config shell prefix");
            return format!("{} {}", prefix, command);
        }
    }

    // Fall back to environment variable
    if let Ok(prefix) = std::env::var("CODEX_CODE_SHELL_PREFIX") {
        if !prefix.is_empty() {
            debug!(prefix = %prefix, "Applying CODEX_CODE_SHELL_PREFIX");
            return format!("{} {}", prefix, command);
        }
    }

    // No prefix
    command.to_string()
}

/// Convert a CommandResult to a HookResult.
impl From<CommandResult> for HookResult {
    fn from(cmd_result: CommandResult) -> Self {
        match cmd_result.outcome {
            HookOutcome::Success => HookResult {
                outcome: HookOutcome::Success,
                output: cmd_result.parsed_output,
                blocking_error: None,
                stdout: Some(cmd_result.stdout),
                stderr: Some(cmd_result.stderr),
                exit_code: Some(cmd_result.exit_code),
            },
            HookOutcome::Blocking => HookResult {
                outcome: HookOutcome::Blocking,
                output: cmd_result.parsed_output,
                blocking_error: Some(crate::output::BlockingError {
                    error: if cmd_result.stderr.is_empty() {
                        "Hook returned exit code 2".to_string()
                    } else {
                        cmd_result.stderr.clone()
                    },
                    command: String::new(), // Will be filled in by executor
                }),
                stdout: Some(cmd_result.stdout),
                stderr: Some(cmd_result.stderr),
                exit_code: Some(cmd_result.exit_code),
            },
            HookOutcome::NonBlockingError => HookResult {
                outcome: HookOutcome::NonBlockingError,
                output: cmd_result.parsed_output,
                blocking_error: None,
                stdout: Some(cmd_result.stdout),
                stderr: Some(cmd_result.stderr),
                exit_code: Some(cmd_result.exit_code),
            },
            HookOutcome::Cancelled => HookResult::cancelled(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_input() -> HookInput {
        HookInput {
            hook_event_name: HookEventType::PreToolUse,
            session_id: "test-session".to_string(),
            transcript_path: "/tmp/transcript.json".to_string(),
            cwd: "/tmp".to_string(),
            permission_mode: serde_json::Value::Null,
            event_data: crate::input::HookEventData::Empty {},
        }
    }

    #[tokio::test]
    async fn test_execute_simple_command() {
        let config = CommandConfig {
            command: "echo 'hello'".to_string(),
            timeout_secs: 5,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            None,
        )
        .await
        .expect("Command should succeed");

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.outcome, HookOutcome::Success);
        assert!(result.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_json_output() {
        let config = CommandConfig {
            command: r#"echo '{"continue": true}'"#.to_string(),
            timeout_secs: 5,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            None,
        )
        .await
        .expect("Command should succeed");

        assert!(result.parsed_output.is_some());
        let output = result.parsed_output.unwrap();
        assert_eq!(output.r#continue, Some(true));
    }

    #[tokio::test]
    async fn test_exit_code_2_blocking() {
        let config = CommandConfig {
            command: "exit 2".to_string(),
            timeout_secs: 5,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            None,
        )
        .await
        .expect("Command should complete");

        assert_eq!(result.exit_code, 2);
        assert_eq!(result.outcome, HookOutcome::Blocking);
    }

    #[tokio::test]
    async fn test_exit_code_1_non_blocking() {
        let config = CommandConfig {
            command: "exit 1".to_string(),
            timeout_secs: 5,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            None,
        )
        .await
        .expect("Command should complete");

        assert_eq!(result.exit_code, 1);
        assert_eq!(result.outcome, HookOutcome::NonBlockingError);
    }

    #[tokio::test]
    async fn test_timeout() {
        let config = CommandConfig {
            command: "sleep 10".to_string(),
            timeout_secs: 1,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            None,
        )
        .await;

        assert!(matches!(result, Err(HookError::Timeout)));
    }

    #[tokio::test]
    async fn test_cancellation() {
        let config = CommandConfig {
            command: "sleep 10".to_string(),
            timeout_secs: 60,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        // Cancel before starting
        cancel.cancel();

        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            None,
        )
        .await
        .expect("Should return cancelled result");

        assert_eq!(result.outcome, HookOutcome::Cancelled);
    }

    #[tokio::test]
    async fn test_async_response_detection() {
        let config = CommandConfig {
            command: r#"echo '{"async": true, "asyncTimeout": 15000}'"#.to_string(),
            timeout_secs: 5,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            None,
        )
        .await
        .expect("Command should succeed");

        assert!(result.is_async);
        assert_eq!(result.async_timeout, Some(15000));
    }

    #[tokio::test]
    async fn test_stdin_receives_input() {
        // Command that reads stdin and echoes it back
        let config = CommandConfig {
            command: "cat".to_string(),
            timeout_secs: 5,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            None,
        )
        .await
        .expect("Command should succeed");

        // stdout should contain the JSON input
        assert!(result.stdout.contains("hook_event_name"));
        assert!(result.stdout.contains("PreToolUse"));
    }

    #[test]
    fn test_check_async_response() {
        let (is_async, timeout) = check_async_response(r#"{"async": true}"#);
        assert!(is_async);
        assert!(timeout.is_none());

        let (is_async, timeout) = check_async_response(r#"{"async": true, "asyncTimeout": 5000}"#);
        assert!(is_async);
        assert_eq!(timeout, Some(5000));

        let (is_async, _) = check_async_response(r#"{"continue": true}"#);
        assert!(!is_async);

        let (is_async, _) = check_async_response("plain text");
        assert!(!is_async);
    }

    #[test]
    fn test_env_file_path() {
        let path = get_env_file_path(0, "session-123");
        assert!(path.contains("claude_env_session-123_0.env"));
    }

    #[test]
    fn test_get_effective_command_with_config_prefix() {
        // Config prefix takes precedence
        let result = get_effective_command("echo hello", Some("/prefix.sh"));
        assert_eq!(result, "/prefix.sh echo hello");
    }

    #[test]
    fn test_get_effective_command_no_prefix() {
        // No prefix = command as-is
        let result = get_effective_command("echo hello", None);
        assert_eq!(result, "echo hello");
    }

    #[test]
    fn test_get_effective_command_empty_prefix() {
        // Empty prefix = command as-is
        let result = get_effective_command("echo hello", Some(""));
        assert_eq!(result, "echo hello");
    }

    #[tokio::test]
    async fn test_config_shell_prefix_applied() {
        // Test that config-based shell prefix is applied
        let config = CommandConfig {
            command: "echo test".to_string(),
            timeout_secs: 5,
            status_message: None,
        };
        let input = make_input();
        let cwd = PathBuf::from("/tmp");
        let cancel = CancellationToken::new();

        // Use a shell prefix that echoes the final command
        let result = execute_command(
            &config,
            &input,
            HookEventType::PreToolUse,
            &cwd,
            cancel,
            0,
            Some("sh -c"), // Wrap with another shell
        )
        .await
        .expect("Command should succeed");

        // The command should execute successfully
        assert_eq!(result.exit_code, 0);
    }

    // Note: CODEX_CODE_SHELL_PREFIX env var integration test is skipped because
    // std::env::set_var is unsafe in Rust 2024 and environment variable
    // modification can cause race conditions in parallel tests.
    // The prefix logic is tested via the unit tests above.
}
