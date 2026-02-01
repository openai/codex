//! Command handler: executes an external process.
//!
//! The command receives the full `HookContext` as JSON on stdin and is expected to
//! return a JSON response on stdout. The response can be:
//!
//! 1. A `HookResult` (legacy format with `action` tag):
//!    ```json
//!    { "action": "continue" }
//!    { "action": "reject", "reason": "..." }
//!    { "action": "modify_input", "new_input": {...} }
//!    { "action": "continue_with_context", "additional_context": "..." }
//!    ```
//!
//! 2. A `HookOutput` (Claude Code v2.1.7 format):
//!    ```json
//!    { "continue_execution": true }
//!    { "continue_execution": false, "stop_reason": "..." }
//!    { "continue_execution": true, "updated_input": {...} }
//!    { "continue_execution": true, "additional_context": "..." }
//!    ```
//!
//! Environment variables set for the command:
//! - `CLAUDE_PROJECT_DIR` - Project root (working directory)
//! - `CLAUDE_SESSION_ID` - Current session ID
//! - `HOOK_EVENT` - Event type name (e.g., "pre_tool_use")
//! - `HOOK_TOOL_NAME` - Tool name (if applicable, otherwise empty)
//!
//! If the process exits with a non-zero status or produces invalid JSON,
//! the hook returns `Continue`.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tracing::debug;
use tracing::warn;

use crate::context::HookContext;
use crate::result::HookResult;

/// Claude Code v2.1.7 compatible hook output format.
///
/// This format is an alternative to `HookResult` that external commands can return.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookOutput {
    /// Whether execution should continue. If false, the action is blocked.
    pub continue_execution: bool,

    /// Reason for blocking (used when `continue_execution` is false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// Replacement input (used to modify tool input before execution).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<Value>,

    /// Additional context to inject into the conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,

    /// If true, the hook is running asynchronously.
    ///
    /// When a command returns `{ "async": true }`, it indicates that the hook
    /// has spawned a background process that will complete later. The main
    /// execution continues immediately, and the async hook's result will be
    /// delivered via system reminders when it completes.
    #[serde(default, rename = "async")]
    pub is_async: bool,
}

impl HookOutput {
    /// Converts this output to a HookResult, optionally with a hook name for async results.
    pub fn into_result(self, hook_name: Option<&str>) -> HookResult {
        if self.is_async {
            // Generate a unique task ID for async hooks
            let task_id = format!("async-{}", uuid::Uuid::new_v4());
            return HookResult::Async {
                task_id,
                hook_name: hook_name.unwrap_or("unknown").to_string(),
            };
        }

        if !self.continue_execution {
            return HookResult::Reject {
                reason: self
                    .stop_reason
                    .unwrap_or_else(|| "Hook blocked execution".to_string()),
            };
        }

        if let Some(new_input) = self.updated_input {
            return HookResult::ModifyInput { new_input };
        }

        if self.additional_context.is_some() {
            return HookResult::ContinueWithContext {
                additional_context: self.additional_context,
            };
        }

        HookResult::Continue
    }
}

impl From<HookOutput> for HookResult {
    fn from(output: HookOutput) -> Self {
        output.into_result(None)
    }
}

/// Executes an external command as a hook handler.
pub struct CommandHandler;

impl CommandHandler {
    /// Runs the specified command, passing the full `HookContext` as JSON on stdin.
    ///
    /// Environment variables are set to provide context:
    /// - `CLAUDE_PROJECT_DIR` - Working directory / project root
    /// - `CLAUDE_SESSION_ID` - Current session ID
    /// - `HOOK_EVENT` - Event type (e.g., "pre_tool_use")
    /// - `HOOK_TOOL_NAME` - Tool name if applicable
    ///
    /// The process stdout is parsed as either `HookResult` (legacy) or `HookOutput`
    /// (Claude Code v2.1.7 format). On any error the handler falls back to `Continue`.
    pub async fn execute(command: &str, args: &[String], ctx: &HookContext) -> HookResult {
        let ctx_json = match serde_json::to_string(ctx) {
            Ok(j) => j,
            Err(e) => {
                warn!("Failed to serialize hook context: {e}");
                return HookResult::Continue;
            }
        };

        debug!(command, ?args, event_type = %ctx.event_type, "Executing command hook");

        let result = tokio::process::Command::new(command)
            .args(args)
            .current_dir(&ctx.working_dir)
            // Set environment variables for the command
            .env(
                "CLAUDE_PROJECT_DIR",
                ctx.working_dir.to_string_lossy().as_ref(),
            )
            .env("CLAUDE_SESSION_ID", &ctx.session_id)
            .env("HOOK_EVENT", ctx.event_type.as_str())
            .env("HOOK_TOOL_NAME", ctx.tool_name.as_deref().unwrap_or(""))
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

        // Write full context JSON to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            if let Err(e) = stdin.write_all(ctx_json.as_bytes()).await {
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

        parse_hook_response(stdout.trim())
    }
}

/// Parses hook command output, supporting both `HookResult` and `HookOutput` formats.
fn parse_hook_response(stdout: &str) -> HookResult {
    // Try parsing as HookResult first (has "action" field)
    if let Ok(result) = serde_json::from_str::<HookResult>(stdout) {
        return result;
    }

    // Try parsing as HookOutput (Claude Code v2.1.7 format with "continue_execution" field)
    if let Ok(output) = serde_json::from_str::<HookOutput>(stdout) {
        return output.into();
    }

    warn!("Failed to parse hook command output as HookResult or HookOutput");
    HookResult::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::HookEventType;
    use std::path::PathBuf;

    fn make_ctx() -> HookContext {
        HookContext::new(
            HookEventType::PreToolUse,
            "test-session".to_string(),
            PathBuf::from("/tmp"),
        )
    }

    #[tokio::test]
    async fn test_execute_echo_command() {
        // Use `echo` which ignores stdin and writes to stdout
        let ctx = make_ctx();
        let result =
            CommandHandler::execute("echo", &[r#"{"action":"continue"}"#.to_string()], &ctx).await;
        // echo output includes a newline, should parse as Continue
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_execute_nonexistent_command() {
        let ctx = make_ctx();
        let result =
            CommandHandler::execute("this-command-definitely-does-not-exist-12345", &[], &ctx)
                .await;
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_execute_failing_command() {
        let ctx = make_ctx();
        let result = CommandHandler::execute("false", &[], &ctx).await;
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_hook_output_continue() {
        let output = HookOutput {
            continue_execution: true,
            stop_reason: None,
            updated_input: None,
            additional_context: None,
            is_async: false,
        };
        let result: HookResult = output.into();
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_hook_output_reject() {
        let output = HookOutput {
            continue_execution: false,
            stop_reason: Some("Not allowed".to_string()),
            updated_input: None,
            additional_context: None,
            is_async: false,
        };
        let result: HookResult = output.into();
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "Not allowed");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_hook_output_reject_default_reason() {
        let output = HookOutput {
            continue_execution: false,
            stop_reason: None,
            updated_input: None,
            additional_context: None,
            is_async: false,
        };
        let result: HookResult = output.into();
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "Hook blocked execution");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_hook_output_modify_input() {
        let output = HookOutput {
            continue_execution: true,
            stop_reason: None,
            updated_input: Some(serde_json::json!({"modified": true})),
            additional_context: None,
            is_async: false,
        };
        let result: HookResult = output.into();
        if let HookResult::ModifyInput { new_input } = result {
            assert_eq!(new_input["modified"], true);
        } else {
            panic!("Expected ModifyInput");
        }
    }

    #[test]
    fn test_hook_output_additional_context() {
        let output = HookOutput {
            continue_execution: true,
            stop_reason: None,
            updated_input: None,
            additional_context: Some("Extra info".to_string()),
            is_async: false,
        };
        let result: HookResult = output.into();
        if let HookResult::ContinueWithContext { additional_context } = result {
            assert_eq!(additional_context, Some("Extra info".to_string()));
        } else {
            panic!("Expected ContinueWithContext");
        }
    }

    #[test]
    fn test_hook_output_async() {
        let output = HookOutput {
            continue_execution: true,
            stop_reason: None,
            updated_input: None,
            additional_context: None,
            is_async: true,
        };
        let result = output.into_result(Some("test-hook"));
        if let HookResult::Async { task_id, hook_name } = result {
            assert!(task_id.starts_with("async-"));
            assert_eq!(hook_name, "test-hook");
        } else {
            panic!("Expected Async");
        }
    }

    #[test]
    fn test_parse_hook_response_hook_result() {
        let json = r#"{"action":"continue"}"#;
        let result = parse_hook_response(json);
        assert!(matches!(result, HookResult::Continue));

        let json = r#"{"action":"reject","reason":"blocked"}"#;
        let result = parse_hook_response(json);
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "blocked");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_parse_hook_response_hook_output() {
        let json = r#"{"continue_execution":true}"#;
        let result = parse_hook_response(json);
        assert!(matches!(result, HookResult::Continue));

        let json = r#"{"continue_execution":false,"stop_reason":"nope"}"#;
        let result = parse_hook_response(json);
        if let HookResult::Reject { reason } = result {
            assert_eq!(reason, "nope");
        } else {
            panic!("Expected Reject");
        }
    }

    #[test]
    fn test_parse_hook_response_invalid() {
        let result = parse_hook_response("not json at all");
        assert!(matches!(result, HookResult::Continue));

        let result = parse_hook_response(r#"{"unknown":"format"}"#);
        assert!(matches!(result, HookResult::Continue));
    }

    #[test]
    fn test_hook_output_serde() {
        let output = HookOutput {
            continue_execution: true,
            stop_reason: None,
            updated_input: Some(serde_json::json!({"key": "value"})),
            additional_context: Some("context".to_string()),
            is_async: false,
        };
        let json = serde_json::to_string(&output).expect("serialize");
        let parsed: HookOutput = serde_json::from_str(&json).expect("deserialize");
        assert!(parsed.continue_execution);
        assert!(parsed.updated_input.is_some());
        assert_eq!(parsed.additional_context, Some("context".to_string()));
        assert!(!parsed.is_async);
    }

    #[test]
    fn test_hook_output_serde_async() {
        let json = r#"{"continue_execution":true,"async":true}"#;
        let parsed: HookOutput = serde_json::from_str(json).expect("deserialize");
        assert!(parsed.continue_execution);
        assert!(parsed.is_async);
    }

    #[test]
    fn test_parse_hook_response_async() {
        let json = r#"{"continue_execution":true,"async":true}"#;
        let result = parse_hook_response(json);
        assert!(matches!(result, HookResult::Async { .. }));
    }
}
