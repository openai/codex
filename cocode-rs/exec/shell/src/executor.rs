//! Shell command executor with timeout and background support.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::AsyncReadExt;
use tokio::sync::{Mutex, Notify};

use crate::background::{BackgroundProcess, BackgroundTaskRegistry};
use crate::command::CommandResult;

/// Default command timeout in seconds.
const DEFAULT_TIMEOUT_SECS: i64 = 120;

/// Maximum output size in bytes before truncation (30KB).
const MAX_OUTPUT_BYTES: i64 = 30_000;

/// Shell command executor.
///
/// Provides async execution of shell commands with timeout support,
/// output capture, and background task management.
#[derive(Debug, Clone)]
pub struct ShellExecutor {
    /// Default timeout for command execution in seconds.
    pub default_timeout_secs: i64,
    /// Working directory for command execution.
    pub cwd: PathBuf,
    /// Registry for background tasks.
    pub background_registry: BackgroundTaskRegistry,
}

impl ShellExecutor {
    /// Creates a new executor with the given working directory.
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            default_timeout_secs: DEFAULT_TIMEOUT_SECS,
            cwd,
            background_registry: BackgroundTaskRegistry::new(),
        }
    }

    /// Executes a shell command with the given timeout.
    ///
    /// The command is run via `bash -c` with the executor's working directory.
    /// Output is truncated if it exceeds the maximum size limit.
    ///
    /// If the command times out, a `CommandResult` is returned with exit code -1
    /// and a timeout message in stderr.
    pub async fn execute(&self, command: &str, timeout_secs: i64) -> CommandResult {
        let start = Instant::now();

        let timeout = if timeout_secs > 0 {
            timeout_secs
        } else {
            self.default_timeout_secs
        };

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(timeout as u64),
            self.run_command(command),
        )
        .await;

        let duration_ms = start.elapsed().as_millis() as i64;

        match result {
            Ok(cmd_result) => {
                let mut cmd_result = cmd_result;
                cmd_result.duration_ms = duration_ms;
                cmd_result
            }
            Err(_) => CommandResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: format!("Command timed out after {timeout} seconds"),
                duration_ms,
                truncated: false,
            },
        }
    }

    /// Spawns a command in the background and returns a task ID.
    ///
    /// The command output is captured asynchronously and can be retrieved
    /// via the background registry using the returned task ID.
    pub async fn spawn_background(&self, command: &str) -> Result<String, String> {
        let task_id = format!("bg-{}", uuid_simple());
        let output = Arc::new(Mutex::new(String::new()));
        let completed = Arc::new(Notify::new());

        let process = BackgroundProcess {
            id: task_id.clone(),
            command: command.to_string(),
            output: Arc::clone(&output),
            completed: Arc::clone(&completed),
        };

        self.background_registry
            .register(task_id.clone(), process)
            .await;

        let cwd = self.cwd.clone();
        let cmd_str = command.to_string();
        let registry = self.background_registry.clone();
        let bg_task_id = task_id.clone();

        tokio::spawn(async move {
            let child = tokio::process::Command::new("bash")
                .arg("-c")
                .arg(&cmd_str)
                .current_dir(&cwd)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn();

            match child {
                Ok(mut child) => {
                    // Read stdout
                    if let Some(mut stdout) = child.stdout.take() {
                        let output = Arc::clone(&output);
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 4096];
                            loop {
                                match stdout.read(&mut buf).await {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if let Ok(text) = String::from_utf8(buf[..n].to_vec()) {
                                            let mut out = output.lock().await;
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

            // Remove from registry when done
            registry.stop(&bg_task_id).await;
        });

        Ok(task_id)
    }

    /// Internal: runs a command and captures output.
    async fn run_command(&self, command: &str) -> CommandResult {
        let child = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(&self.cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let child = match child {
            Ok(c) => c,
            Err(e) => {
                return CommandResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Failed to spawn command: {e}"),
                    duration_ms: 0,
                    truncated: false,
                };
            }
        };

        let output = match child.wait_with_output().await {
            Ok(o) => o,
            Err(e) => {
                return CommandResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Failed to wait for command: {e}"),
                    duration_ms: 0,
                    truncated: false,
                };
            }
        };

        let exit_code = output.status.code().unwrap_or(-1);
        let (stdout, truncated_stdout) = truncate_output(&output.stdout);
        let (stderr, truncated_stderr) = truncate_output(&output.stderr);

        CommandResult {
            exit_code,
            stdout,
            stderr,
            duration_ms: 0, // Will be set by caller
            truncated: truncated_stdout || truncated_stderr,
        }
    }
}

/// Truncates output bytes to a string, returning (text, was_truncated).
fn truncate_output(bytes: &[u8]) -> (String, bool) {
    let max = MAX_OUTPUT_BYTES as usize;
    if bytes.len() > max {
        let truncated_bytes = &bytes[..max];
        let text = String::from_utf8_lossy(truncated_bytes).into_owned();
        (text, true)
    } else {
        let text = String::from_utf8_lossy(bytes).into_owned();
        (text, false)
    }
}

/// Generates a simple unique identifier (timestamp-based).
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_simple_command() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("echo hello", 10).await;
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello");
        assert!(result.stderr.is_empty());
        assert!(!result.truncated);
        assert!(result.duration_ms >= 0);
    }

    #[tokio::test]
    async fn test_execute_failing_command() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("exit 42", 10).await;
        assert_eq!(result.exit_code, 42);
    }

    #[tokio::test]
    async fn test_execute_with_stderr() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("echo err >&2", 10).await;
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stderr.trim(), "err");
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let result = executor.execute("sleep 30", 1).await;
        assert_eq!(result.exit_code, -1);
        assert!(result.stderr.contains("timed out"));
    }

    #[tokio::test]
    async fn test_execute_uses_cwd() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let executor = ShellExecutor::new(tmp.path().to_path_buf());
        let result = executor.execute("pwd", 10).await;
        assert_eq!(result.exit_code, 0);
        // The output should contain the temp dir path
        let output_path = result.stdout.trim();
        // On macOS, /tmp may resolve to /private/tmp
        assert!(
            output_path.contains(tmp.path().to_str().expect("path to str"))
                || tmp
                    .path()
                    .to_str()
                    .expect("path to str")
                    .contains(output_path),
            "Expected cwd to match temp dir: output={output_path}, temp={}",
            tmp.path().display()
        );
    }

    #[tokio::test]
    async fn test_default_timeout() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        assert_eq!(executor.default_timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[tokio::test]
    async fn test_spawn_background() {
        let executor = ShellExecutor::new(std::env::temp_dir());
        let task_id = executor
            .spawn_background("echo background-test")
            .await
            .expect("spawn");
        assert!(task_id.starts_with("bg-"));

        // Wait a bit for the background task to complete
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    #[test]
    fn test_truncate_output_small() {
        let data = b"hello world";
        let (text, truncated) = truncate_output(data);
        assert_eq!(text, "hello world");
        assert!(!truncated);
    }

    #[test]
    fn test_truncate_output_large() {
        let data = vec![b'x'; 50_000];
        let (text, truncated) = truncate_output(&data);
        assert_eq!(text.len(), MAX_OUTPUT_BYTES as usize);
        assert!(truncated);
    }

    #[test]
    fn test_uuid_simple_uniqueness() {
        let a = uuid_simple();
        // Small sleep to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(1));
        let b = uuid_simple();
        assert_ne!(a, b);
    }
}
