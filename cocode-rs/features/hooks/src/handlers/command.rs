//! Command handler: executes an external process.
//!
//! The command receives the hook context as JSON on stdin and is expected to
//! return a JSON `HookResult` on stdout. If the process exits with a non-zero
//! status or produces invalid JSON, the hook returns `Continue`.

use std::path::Path;

use serde_json::Value;
use tracing::{debug, warn};

use crate::result::HookResult;

/// Executes an external command as a hook handler.
pub struct CommandHandler;

impl CommandHandler {
    /// Runs the specified command, passing `input` as JSON on stdin.
    ///
    /// The working directory is set to `cwd`. The process stdout is parsed as
    /// a `HookResult`. On any error the handler falls back to `Continue`.
    pub async fn execute(command: &str, args: &[String], input: &Value, cwd: &Path) -> HookResult {
        let input_json = match serde_json::to_string(input) {
            Ok(j) => j,
            Err(e) => {
                warn!("Failed to serialize hook input: {e}");
                return HookResult::Continue;
            }
        };

        debug!(command, ?args, "Executing command hook");

        let result = tokio::process::Command::new(command)
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let mut child = match result {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to spawn hook command '{command}': {e}");
                return HookResult::Continue;
            }
        };

        // Write input to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            if let Err(e) = stdin.write_all(input_json.as_bytes()).await {
                warn!("Failed to write to hook command stdin: {e}");
            }
            drop(stdin);
        }

        let output = match child.wait_with_output().await {
            Ok(o) => o,
            Err(e) => {
                warn!("Failed to wait for hook command: {e}");
                return HookResult::Continue;
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                command,
                exit_code = output.status.code().unwrap_or(-1),
                stderr = %stderr,
                "Hook command exited with error"
            );
            return HookResult::Continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return HookResult::Continue;
        }

        match serde_json::from_str::<HookResult>(stdout.trim()) {
            Ok(result) => result,
            Err(e) => {
                warn!("Failed to parse hook command output as HookResult: {e}");
                HookResult::Continue
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_execute_echo_command() {
        // Use `echo` which ignores stdin and writes to stdout
        let result = CommandHandler::execute(
            "echo",
            &[r#"{"action":"continue"}"#.to_string()],
            &serde_json::json!({}),
            &PathBuf::from("/tmp"),
        )
        .await;
        // echo output includes a newline, should parse as Continue
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_execute_nonexistent_command() {
        let result = CommandHandler::execute(
            "this-command-definitely-does-not-exist-12345",
            &[],
            &serde_json::json!({}),
            &PathBuf::from("/tmp"),
        )
        .await;
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_execute_failing_command() {
        let result =
            CommandHandler::execute("false", &[], &serde_json::json!({}), &PathBuf::from("/tmp"))
                .await;
        assert!(matches!(result, HookResult::Continue));
    }
}
