//! KillShell tool handler.
//!
//! Terminates running background shell commands.

use async_trait::async_trait;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::shell_background::SharedBackgroundShellStore;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

/// Arguments for KillShell tool.
#[derive(Debug, Deserialize)]
pub struct KillShellArgs {
    /// Shell ID to terminate.
    pub shell_id: String,
}

/// Handler for KillShell tool.
pub struct KillShellHandler {
    store: SharedBackgroundShellStore,
}

impl KillShellHandler {
    /// Create a new KillShell handler using the global store.
    pub fn new() -> Self {
        use crate::shell_background::get_global_shell_store;
        Self {
            store: get_global_shell_store(),
        }
    }
}

impl std::fmt::Debug for KillShellHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KillShellHandler").finish()
    }
}

#[async_trait]
impl ToolHandler for KillShellHandler {
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
                    "Invalid payload type for KillShell".to_string(),
                ));
            }
        };

        let args: KillShellArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        match self.store.kill(&args.shell_id) {
            Ok(()) => Ok(ToolOutput::Function {
                content: serde_json::json!({
                    "shell_id": args.shell_id,
                    "status": "killed",
                    "message": "Shell command terminated successfully",
                })
                .to_string(),
                content_items: None,
                success: Some(true),
            }),
            Err(error) => Ok(ToolOutput::Function {
                content: serde_json::json!({
                    "shell_id": args.shell_id,
                    "status": "error",
                    "message": error,
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
    fn test_kill_shell_handler_kind() {
        let handler = KillShellHandler::new();
        assert_eq!(handler.kind(), ToolKind::Function);
    }

    #[test]
    fn test_parse_kill_shell_args() {
        let args: KillShellArgs =
            serde_json::from_str(r#"{"shell_id": "shell-123"}"#).expect("Should parse");
        assert_eq!(args.shell_id, "shell-123");
    }
}
