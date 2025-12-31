//! Shell extension for background execution.
//!
//! This module provides the entry point for running shell commands in the background.
//! It intercepts shell execution requests and handles `run_in_background` parameter.

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::function_tool::FunctionCallError;
use crate::shell::Shell;
use crate::shell_background::SharedBackgroundShellStore;
use crate::shell_background::SharedOutputBuffer;
use crate::shell_background::ShellResult;
use crate::shell_background::get_global_shell_store;
use crate::tools::context::ToolOutput;
use codex_protocol::ConversationId;
use codex_protocol::models::ShellCommandToolCallParams;
use codex_protocol::models::ShellToolCallParams;
use serde::Deserialize;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Maximum output size per stream (stdout/stderr) in bytes (10MB).
const MAX_OUTPUT_SIZE: usize = 10 * 1024 * 1024;

/// Extended shell parameters for background execution.
#[derive(Debug, Default, Deserialize)]
pub struct ShellBackgroundParams {
    /// Run command in background and return immediately.
    #[serde(default)]
    pub run_in_background: bool,
    /// Description for the background task (shown in system reminders).
    pub description: Option<String>,
    /// Additional environment variables to set for the command.
    #[serde(default)]
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// Try to extract background params from shell arguments JSON.
pub fn parse_background_params(arguments: &str) -> ShellBackgroundParams {
    serde_json::from_str(arguments).unwrap_or_default()
}

/// Run a shell command in the background.
///
/// Returns a ToolOutput with the shell_id for later retrieval via BashOutput.
pub async fn run_shell_background(
    command: Vec<String>,
    description: Option<String>,
    cwd: Option<std::path::PathBuf>,
    env: Option<std::collections::HashMap<String, String>>,
    conversation_id: Option<ConversationId>,
    store: &SharedBackgroundShellStore,
) -> ToolOutput {
    let command_str = command.join(" ");
    let desc = description.unwrap_or_else(|| {
        // Generate a description from the command
        if command_str.len() > 50 {
            format!("{}...", &command_str[..47])
        } else {
            command_str.clone()
        }
    });

    // Phase 1: Register pending before spawn (returns shell_id, token, and stdout/stderr buffers)
    let (shell_id, token, stdout_buffer, stderr_buffer) =
        store.register_pending_with_buffer(conversation_id, command_str.clone(), desc.clone());

    // Phase 2: Spawn the background task with streaming output support
    let cwd_clone = cwd.clone();
    let handle = tokio::spawn(async move {
        execute_shell_command_streaming(
            command,
            cwd_clone,
            env,
            token,
            stdout_buffer,
            stderr_buffer,
        )
        .await
    });

    // Set the handle to transition to Running
    store.set_running(&shell_id, handle);

    // Return immediately with shell_id
    ToolOutput::Function {
        content: serde_json::json!({
            "status": "background_started",
            "shell_id": shell_id,
            "description": desc,
            "message": "Command started in background. Use BashOutput tool to retrieve results, or KillShell to terminate."
        })
        .to_string(),
        content_items: None,
        success: Some(true),
    }
}

// ============================================================================
// High-level intercept functions for shell.rs
// ============================================================================

/// Try to handle shell tool (array command) as background execution.
///
/// Returns `Some(ToolOutput)` if `run_in_background: true`, `None` otherwise.
/// This is called from `ShellHandler::handle()` to intercept background requests.
pub async fn try_handle_shell_background(
    arguments: &str,
    turn: &TurnContext,
    conversation_id: ConversationId,
) -> Result<Option<ToolOutput>, FunctionCallError> {
    let bg_params = parse_background_params(arguments);
    if !bg_params.run_in_background {
        return Ok(None);
    }

    let params: ShellToolCallParams = serde_json::from_str(arguments).map_err(|e| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
    })?;

    let cwd = Some(turn.resolve_path(params.workdir.clone()));
    let store = get_global_shell_store();
    let output = run_shell_background(
        params.command,
        bg_params.description,
        cwd,
        bg_params.env,
        Some(conversation_id),
        &store,
    )
    .await;

    Ok(Some(output))
}

/// Try to handle shell_command tool (string command) as background execution.
///
/// Returns `Some(ToolOutput)` if `run_in_background: true`, `None` otherwise.
/// This is called from `ShellCommandHandler::handle()` to intercept background requests.
pub async fn try_handle_shell_command_background(
    arguments: &str,
    session: &Session,
    turn: &TurnContext,
    conversation_id: ConversationId,
) -> Result<Option<ToolOutput>, FunctionCallError> {
    let bg_params = parse_background_params(arguments);
    if !bg_params.run_in_background {
        return Ok(None);
    }

    let params: ShellCommandToolCallParams = serde_json::from_str(arguments).map_err(|e| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
    })?;

    let shell = session.user_shell();
    let command = derive_shell_command(shell.as_ref(), &params.command, params.login);
    let cwd = Some(turn.resolve_path(params.workdir.clone()));
    let store = get_global_shell_store();
    let output = run_shell_background(
        command,
        bg_params.description,
        cwd,
        bg_params.env,
        Some(conversation_id),
        &store,
    )
    .await;

    Ok(Some(output))
}

/// Derive shell command args (duplicated from ShellCommandHandler to avoid circular deps).
fn derive_shell_command(shell: &Shell, command: &str, login: Option<bool>) -> Vec<String> {
    let use_login_shell = login.unwrap_or(true);
    shell.derive_exec_args(command, use_login_shell)
}

// ============================================================================
// Low-level execution functions
// ============================================================================

/// Kill a process and its entire process group (Unix only).
///
/// On Unix, this sends SIGTERM to the entire process group to ensure child
/// processes spawned by the shell are also terminated.
#[cfg(unix)]
async fn kill_process_group(child: &mut tokio::process::Child) {
    use libc::SIGTERM;
    use libc::kill;
    use libc::pid_t;

    if let Some(pid) = child.id() {
        // Send SIGTERM to the entire process group (negative PID)
        // The process group ID equals the PID since we set process_group(0) at spawn
        unsafe {
            // Kill the process group (negative pid means process group)
            let _ = kill(-(pid as pid_t), SIGTERM);
        }
    }
    // Also call kill() on the child as a fallback
    let _ = child.kill().await;
}

/// Kill a process (Windows fallback - no process group support).
#[cfg(not(unix))]
async fn kill_process_group(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
}

/// Execute a shell command with streaming output to separate stdout/stderr buffers.
///
/// This version writes output incrementally to shared buffers during execution,
/// allowing real-time reading via BashOutput tool before the command completes.
///
/// Supports:
/// - Streaming output to separate stdout/stderr buffers
/// - Bounded output (MAX_OUTPUT_SIZE per stream)
/// - Graceful cancellation via cancellation token
/// - Process group kill on Unix
async fn execute_shell_command_streaming(
    command: Vec<String>,
    cwd: Option<std::path::PathBuf>,
    env: Option<std::collections::HashMap<String, String>>,
    cancel_token: CancellationToken,
    stdout_buffer: SharedOutputBuffer,
    stderr_buffer: SharedOutputBuffer,
) -> ShellResult {
    if command.is_empty() {
        return ShellResult {
            output: String::new(),
            exit_code: None,
            success: false,
            error: Some("Empty command".to_string()),
        };
    }

    let program = &command[0];
    let args = &command[1..];

    let mut cmd = Command::new(program);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    if let Some(envs) = env {
        cmd.envs(envs);
    }

    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    match cmd.spawn() {
        Ok(mut child) => {
            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();

            // Read streams and write to separate buffers incrementally
            let stdout_buf = stdout_buffer.clone();
            let stderr_buf = stderr_buffer.clone();
            let (stdout_truncated, stderr_truncated) = tokio::select! {
                result = read_streams_to_buffers(stdout_pipe, stderr_pipe, stdout_buf, stderr_buf) => result,
                _ = cancel_token.cancelled() => {
                    kill_process_group(&mut child).await;
                    let (stdout_content, _) = stdout_buffer.take_all();
                    let (stderr_content, _) = stderr_buffer.take_all();
                    return ShellResult {
                        output: format!("{}\n{}", stdout_content, stderr_content),
                        exit_code: None,
                        success: false,
                        error: Some("Command was cancelled".to_string()),
                    };
                }
            };

            // Wait for process completion
            let wait_result = tokio::select! {
                result = child.wait() => result,
                _ = cancel_token.cancelled() => {
                    kill_process_group(&mut child).await;
                    let (stdout_content, _) = stdout_buffer.take_all();
                    let (stderr_content, _) = stderr_buffer.take_all();
                    return ShellResult {
                        output: format!("{}\n{}", stdout_content, stderr_content),
                        exit_code: None,
                        success: false,
                        error: Some("Command was cancelled while waiting".to_string()),
                    };
                }
            };

            match wait_result {
                Ok(status) => {
                    // Add truncation notice if needed
                    if stdout_truncated {
                        stdout_buffer.append(&format!(
                            "\n[stdout truncated - exceeded {}MB limit]",
                            MAX_OUTPUT_SIZE / (1024 * 1024)
                        ));
                    }
                    if stderr_truncated {
                        stderr_buffer.append(&format!(
                            "\n[stderr truncated - exceeded {}MB limit]",
                            MAX_OUTPUT_SIZE / (1024 * 1024)
                        ));
                    }

                    let (stdout_content, _) = stdout_buffer.take_all();
                    let (stderr_content, _) = stderr_buffer.take_all();
                    ShellResult {
                        output: format!("{}\n{}", stdout_content, stderr_content),
                        exit_code: status.code(),
                        success: status.success(),
                        error: None,
                    }
                }
                Err(e) => {
                    let (stdout_content, _) = stdout_buffer.take_all();
                    let (stderr_content, _) = stderr_buffer.take_all();
                    ShellResult {
                        output: format!("{}\n{}", stdout_content, stderr_content),
                        exit_code: None,
                        success: false,
                        error: Some(format!("Failed to wait for process: {e}")),
                    }
                }
            }
        }
        Err(e) => ShellResult {
            output: String::new(),
            exit_code: None,
            success: false,
            error: Some(format!("Failed to spawn process: {e}")),
        },
    }
}

/// Read stdout and stderr streams, writing incrementally to separate buffers.
/// Returns (stdout_truncated, stderr_truncated).
async fn read_streams_to_buffers(
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
    stdout_buffer: SharedOutputBuffer,
    stderr_buffer: SharedOutputBuffer,
) -> (bool, bool) {
    // Read both streams concurrently to their respective buffers
    let (stdout_truncated, stderr_truncated) = tokio::join!(
        read_stream_to_buffer_simple(stdout, stdout_buffer),
        read_stream_to_buffer_simple(stderr, stderr_buffer),
    );

    (stdout_truncated, stderr_truncated)
}

/// Read a single stream and write chunks to a buffer.
async fn read_stream_to_buffer_simple<R: AsyncReadExt + Unpin>(
    pipe: Option<R>,
    buffer: SharedOutputBuffer,
) -> bool {
    let Some(mut pipe) = pipe else {
        return false;
    };

    let mut total_read = 0;
    let mut truncated = false;
    let mut chunk = [0u8; 8192];

    loop {
        match pipe.read(&mut chunk).await {
            Ok(0) => break, // EOF
            Ok(n) => {
                let remaining = MAX_OUTPUT_SIZE.saturating_sub(total_read);
                if remaining == 0 {
                    truncated = true;
                    break;
                }
                let to_take = n.min(remaining);
                let text = String::from_utf8_lossy(&chunk[..to_take]);
                buffer.append(&text);
                total_read += to_take;
                if to_take < n {
                    truncated = true;
                    break;
                }
            }
            Err(e) => {
                warn!("Error reading stream: {e}");
                break;
            }
        }
    }

    truncated
}

/// Execute a shell command and return the result.
///
/// Supports:
/// - Bounded output reading (MAX_OUTPUT_SIZE per stream)
/// - Graceful cancellation via cancellation token
/// - Error logging for read failures
/// - Process group kill on Unix to terminate child processes
/// - Optional environment variables
#[allow(dead_code)]
async fn execute_shell_command(
    command: Vec<String>,
    cwd: Option<std::path::PathBuf>,
    env: Option<std::collections::HashMap<String, String>>,
    cancel_token: CancellationToken,
) -> ShellResult {
    if command.is_empty() {
        return ShellResult {
            output: String::new(),
            exit_code: None,
            success: false,
            error: Some("Empty command".to_string()),
        };
    }

    let program = &command[0];
    let args = &command[1..];

    let mut cmd = Command::new(program);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    // Apply additional environment variables if provided
    if let Some(envs) = env {
        cmd.envs(envs);
    }

    // On Unix, create a new process group so we can kill all child processes together
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            // Set process group ID to the PID of this process
            // This makes the spawned process the leader of a new process group
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    match cmd.spawn() {
        Ok(mut child) => {
            // Read stdout and stderr with size limits
            let stdout_pipe = child.stdout.take();
            let stderr_pipe = child.stderr.take();

            // Read both streams concurrently with cancellation support
            let (stdout_result, stderr_result) = tokio::select! {
                result = async {
                    let stdout = read_stream_bounded_stdout(stdout_pipe).await;
                    let stderr = read_stream_bounded_stderr(stderr_pipe).await;
                    (stdout, stderr)
                } => result,
                _ = cancel_token.cancelled() => {
                    // Graceful cancellation - kill the child process and its process group
                    kill_process_group(&mut child).await;
                    return ShellResult {
                        output: String::new(),
                        exit_code: None,
                        success: false,
                        error: Some("Command was cancelled".to_string()),
                    };
                }
            };

            // Wait for completion with cancellation support
            let wait_result = tokio::select! {
                result = child.wait() => result,
                _ = cancel_token.cancelled() => {
                    kill_process_group(&mut child).await;
                    return ShellResult {
                        output: format_output(&stdout_result.0, &stderr_result.0),
                        exit_code: None,
                        success: false,
                        error: Some("Command was cancelled while waiting".to_string()),
                    };
                }
            };

            match wait_result {
                Ok(status) => {
                    let exit_code = status.code();
                    let output = format_output_with_truncation(
                        &stdout_result.0,
                        stdout_result.1,
                        &stderr_result.0,
                        stderr_result.1,
                    );

                    ShellResult {
                        output,
                        exit_code,
                        success: status.success(),
                        error: None,
                    }
                }
                Err(e) => ShellResult {
                    output: format_output(&stdout_result.0, &stderr_result.0),
                    exit_code: None,
                    success: false,
                    error: Some(format!("Failed to wait for process: {e}")),
                },
            }
        }
        Err(e) => ShellResult {
            output: String::new(),
            exit_code: None,
            success: false,
            error: Some(format!("Failed to spawn process: {e}")),
        },
    }
}

/// Read stdout with bounded size limit. Returns (output, was_truncated).
async fn read_stream_bounded_stdout(pipe: Option<tokio::process::ChildStdout>) -> (String, bool) {
    read_stream_bounded_impl(pipe, "stdout").await
}

/// Read stderr with bounded size limit. Returns (output, was_truncated).
async fn read_stream_bounded_stderr(pipe: Option<tokio::process::ChildStderr>) -> (String, bool) {
    read_stream_bounded_impl(pipe, "stderr").await
}

/// Generic implementation for reading any async stream with size limit.
async fn read_stream_bounded_impl<R: AsyncReadExt + Unpin>(
    pipe: Option<R>,
    stream_name: &str,
) -> (String, bool) {
    let Some(mut pipe) = pipe else {
        return (String::new(), false);
    };

    let mut buffer = Vec::with_capacity(8192);
    let mut total_read = 0;
    let mut truncated = false;
    let mut chunk = [0u8; 8192];

    loop {
        match pipe.read(&mut chunk).await {
            Ok(0) => break, // EOF
            Ok(n) => {
                let remaining = MAX_OUTPUT_SIZE.saturating_sub(total_read);
                if remaining == 0 {
                    truncated = true;
                    break;
                }
                let to_take = n.min(remaining);
                buffer.extend_from_slice(&chunk[..to_take]);
                total_read += to_take;
                if to_take < n {
                    truncated = true;
                    break;
                }
            }
            Err(e) => {
                warn!("Error reading {stream_name}: {e}");
                break;
            }
        }
    }

    (String::from_utf8_lossy(&buffer).into_owned(), truncated)
}

/// Format output combining stdout and stderr.
fn format_output(stdout: &str, stderr: &str) -> String {
    if stderr.is_empty() {
        stdout.to_string()
    } else if stdout.is_empty() {
        stderr.to_string()
    } else {
        format!("{stdout}\n--- stderr ---\n{stderr}")
    }
}

/// Format output with truncation notices.
fn format_output_with_truncation(
    stdout: &str,
    stdout_truncated: bool,
    stderr: &str,
    stderr_truncated: bool,
) -> String {
    let mut output = format_output(stdout, stderr);

    if stdout_truncated || stderr_truncated {
        output.push_str("\n\n[Output truncated - exceeded ");
        output.push_str(&format!("{}MB limit", MAX_OUTPUT_SIZE / (1024 * 1024)));
        if stdout_truncated && stderr_truncated {
            output.push_str(" on both stdout and stderr");
        } else if stdout_truncated {
            output.push_str(" on stdout");
        } else {
            output.push_str(" on stderr");
        }
        output.push(']');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell_background::BackgroundShellStore;
    use std::sync::Arc;

    #[test]
    fn test_parse_background_params_empty() {
        let params = parse_background_params("{}");
        assert!(!params.run_in_background);
        assert!(params.description.is_none());
    }

    #[test]
    fn test_parse_background_params_with_background() {
        let params = parse_background_params(r#"{"run_in_background": true}"#);
        assert!(params.run_in_background);
    }

    #[test]
    fn test_parse_background_params_with_description() {
        let params = parse_background_params(
            r#"{"run_in_background": true, "description": "Running tests"}"#,
        );
        assert!(params.run_in_background);
        assert_eq!(params.description, Some("Running tests".to_string()));
    }

    #[test]
    fn test_parse_background_params_with_env() {
        let params = parse_background_params(
            r#"{"run_in_background": true, "env": {"MY_VAR": "my_value", "OTHER": "123"}}"#,
        );
        assert!(params.run_in_background);
        let env = params.env.expect("env should be present");
        assert_eq!(env.get("MY_VAR"), Some(&"my_value".to_string()));
        assert_eq!(env.get("OTHER"), Some(&"123".to_string()));
    }

    #[test]
    fn test_parse_background_params_run_in_background() {
        // Default is false
        assert!(!parse_background_params(r#"{"command": "ls"}"#).run_in_background);
        // Explicit true
        assert!(parse_background_params(r#"{"run_in_background": true}"#).run_in_background);
    }

    #[tokio::test]
    async fn test_execute_shell_command_simple() {
        let token = CancellationToken::new();
        let result = execute_shell_command(
            vec!["echo".to_string(), "hello".to_string()],
            None,
            None,
            token,
        )
        .await;
        assert!(result.success);
        assert!(result.output.contains("hello"));
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_execute_shell_command_empty() {
        let token = CancellationToken::new();
        let result = execute_shell_command(vec![], None, None, token).await;
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_execute_shell_command_cancellation() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        // Cancel immediately
        token.cancel();

        let result = execute_shell_command(
            vec!["sleep".to_string(), "10".to_string()],
            None,
            None,
            token_clone,
        )
        .await;

        assert!(!result.success);
        assert!(
            result
                .error
                .as_ref()
                .map_or(false, |e| e.contains("cancelled"))
        );
    }

    #[tokio::test]
    async fn test_run_shell_background() {
        let store = Arc::new(BackgroundShellStore::new());
        let output = run_shell_background(
            vec!["echo".to_string(), "test".to_string()],
            Some("Test echo".to_string()),
            None,
            None,
            None, // conversation_id
            &store,
        )
        .await;

        let ToolOutput::Function {
            content, success, ..
        } = output
        else {
            panic!("Expected function output");
        };

        assert_eq!(success, Some(true));
        assert!(content.contains("background_started"));
        assert!(content.contains("shell-"));
    }

    #[test]
    fn test_format_output() {
        assert_eq!(format_output("stdout", ""), "stdout");
        assert_eq!(format_output("", "stderr"), "stderr");
        assert_eq!(
            format_output("stdout", "stderr"),
            "stdout\n--- stderr ---\nstderr"
        );
    }

    #[test]
    fn test_format_output_with_truncation() {
        let output = format_output_with_truncation("out", true, "err", false);
        assert!(output.contains("[Output truncated"));
        assert!(output.contains("on stdout"));

        let output = format_output_with_truncation("out", true, "err", true);
        assert!(output.contains("on both stdout and stderr"));
    }
}
