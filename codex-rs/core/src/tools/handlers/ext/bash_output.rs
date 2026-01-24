//! BashOutput tool handler.
//!
//! Retrieves output from background shell commands with tweakcc read support.
//!
//! ## Incremental Output
//!
//! By default, each call returns new output since the last read (tweakcc mode).
//! Use `offset=0` to read from the beginning, or specify a byte offset to read from.
//!
//! ## Environment Variables
//!
//! - `CODEX_BASH_MAX_OUTPUT_LENGTH`: Maximum bytes per call (default: 100000)

use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

use crate::function_tool::FunctionCallError;
use crate::shell_background::SharedBackgroundShellStore;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

/// Default maximum output length per call.
const DEFAULT_MAX_OUTPUT_LENGTH: usize = 100_000;

/// Environment variable for max output length override.
const CODEX_BASH_MAX_OUTPUT_LENGTH_ENV: &str = "CODEX_BASH_MAX_OUTPUT_LENGTH";

/// Arguments for BashOutput tool.
#[derive(Debug, Deserialize)]
pub struct BashOutputArgs {
    /// Shell ID to retrieve output for (also accepts bash_id for compatibility).
    #[serde(alias = "bash_id")]
    pub shell_id: String,
    /// Whether to block waiting for completion.
    #[serde(default = "default_block")]
    pub block: bool,
    /// Timeout in milliseconds.
    #[serde(default = "default_timeout")]
    pub timeout: i64,
    /// Optional regex pattern to filter output lines.
    #[serde(default)]
    pub filter: Option<String>,
}

fn default_block() -> bool {
    true
}

fn default_timeout() -> i64 {
    30000 // 30 seconds default
}

/// Get maximum output length from environment or use default.
fn get_max_output_length() -> usize {
    std::env::var(CODEX_BASH_MAX_OUTPUT_LENGTH_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_MAX_OUTPUT_LENGTH)
}

/// Handler for BashOutput tool.
pub struct BashOutputHandler {
    store: SharedBackgroundShellStore,
}

impl BashOutputHandler {
    /// Create a new BashOutput handler using the global store.
    pub fn new() -> Self {
        use crate::shell_background::get_global_shell_store;
        Self {
            store: get_global_shell_store(),
        }
    }
}

impl std::fmt::Debug for BashOutputHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BashOutputHandler").finish()
    }
}

#[async_trait]
impl ToolHandler for BashOutputHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for BashOutput".to_string(),
                ));
            }
        };

        let args: BashOutputArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // Clamp timeout to max 10 minutes
        let timeout_ms = args.timeout.clamp(0, 600000) as u64;
        let timeout = Duration::from_millis(timeout_ms);

        let limit = get_max_output_length();

        match self
            .store
            .get_output(
                &args.shell_id,
                args.block,
                timeout,
                args.filter.as_deref(),
                limit,
            )
            .await
        {
            Some(output) => {
                // Trigger cleanup opportunistically
                self.store.cleanup_old(Duration::from_secs(3600)); // 1 hour

                let success = output.status == "completed";

                Ok(ToolOutput::Function {
                    content: serde_json::to_string(&output).unwrap_or_else(|_| {
                        serde_json::json!({
                            "shellId": output.shell_id,
                            "command": output.command,
                            "status": output.status,
                            "exitCode": output.exit_code,
                            "stdout": output.stdout,
                            "stderr": output.stderr,
                            "stdoutLines": output.stdout_lines,
                            "stderrLines": output.stderr_lines,
                            "timestamp": output.timestamp,
                            "hasMore": output.has_more,
                        })
                        .to_string()
                    }),
                    content_items: None,
                    success: Some(success),
                })
            }
            None => Ok(ToolOutput::Function {
                content: serde_json::json!({
                    "shellId": args.shell_id,
                    "status": "not_found",
                    "message": "No shell found with that shell_id",
                })
                .to_string(),
                content_items: None,
                success: Some(false),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_output_handler_kind() {
        let handler = BashOutputHandler::new();
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_parse_bash_output_args() {
        let args: BashOutputArgs =
            serde_json::from_str(r#"{"shell_id": "shell-123"}"#).expect("Should parse");
        assert_eq!(args.shell_id, "shell-123");
        assert!(args.block); // default
        assert_eq!(args.timeout, 30000); // default
        assert!(args.filter.is_none()); // default: no filter
    }

    #[test]
    fn test_parse_bash_output_args_with_options() {
        let args: BashOutputArgs =
            serde_json::from_str(r#"{"shell_id": "shell-123", "block": false, "timeout": 5000}"#)
                .expect("Should parse");
        assert_eq!(args.shell_id, "shell-123");
        assert!(!args.block);
        assert_eq!(args.timeout, 5000);
    }

    #[test]
    fn test_parse_bash_output_args_with_filter() {
        let args: BashOutputArgs =
            serde_json::from_str(r#"{"shell_id": "shell-123", "filter": "error|warn"}"#)
                .expect("Should parse");
        assert_eq!(args.shell_id, "shell-123");
        assert_eq!(args.filter, Some("error|warn".to_string()));
    }

    #[test]
    fn test_parse_bash_output_args_bash_id_alias() {
        // bash_id is an alias for shell_id (for compatibility)
        let args: BashOutputArgs =
            serde_json::from_str(r#"{"bash_id": "shell-456"}"#).expect("Should parse");
        assert_eq!(args.shell_id, "shell-456");
    }

    #[test]
    fn test_default_max_output_length() {
        // Just verify the default constant is reasonable
        assert_eq!(DEFAULT_MAX_OUTPUT_LENGTH, 100_000);
    }
}
