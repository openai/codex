use async_trait::async_trait;
use codex_lua_runtime::{LuaRuntime, LuaRuntimeConfig};
use serde::Deserialize;
use serde_json::Value as JsonValue;

use crate::function_tool::FunctionCallError;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::registry::{ToolHandler, ToolKind};

/// Tool handler for executing Lua scripts
pub struct LuaHandler {
    runtime: LuaRuntime,
}

impl LuaHandler {
    /// Create a new Lua handler with the given configuration
    pub fn new(config: LuaRuntimeConfig) -> Result<Self, FunctionCallError> {
        let runtime = LuaRuntime::new(config).map_err(|e| {
            FunctionCallError::Fatal(format!("Failed to initialize Lua runtime: {}", e))
        })?;

        Ok(Self { runtime })
    }

    /// Create a new Lua handler with default configuration
    pub fn new_default() -> Result<Self, FunctionCallError> {
        Self::new(LuaRuntimeConfig::default())
    }
}

/// JSON arguments accepted by the `lua_execute` tool handler.
#[derive(Deserialize)]
struct LuaExecuteArgs {
    /// The Lua script to execute
    script: String,
    is_path: Option<bool>,
    /// Optional arguments to pass to the script (available as global `args`)
    #[serde(default)]
    args: Option<JsonValue>,
}

#[async_trait]
impl ToolHandler for LuaHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "lua_execute handler received unsupported payload".to_string(),
                ));
            }
        };

        let execute_args: LuaExecuteArgs = serde_json::from_str(&arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to parse function arguments: {err:?}"
            ))
        })?;

        let LuaExecuteArgs {
            script,
            args,
            is_path,
        } = execute_args;

        // Execute the Lua script
        let result = if is_path.unwrap_or(false) {
            self.runtime.execute_script_from_file(&script, args).await
        } else {
            self.runtime.execute_script(&script, args).await
        }
        .map_err(|e| FunctionCallError::RespondToModel(format!("Lua execution error: {}", e)))?;

        // Convert the result to a formatted string
        let content = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lua_handler_basic() {
        let handler = LuaHandler::new_default().unwrap();

        let payload = ToolPayload::Function {
            arguments: r#"{"script": "return 1 + 1"}"#.to_string(),
        };

        let invocation = ToolInvocation {
            payload,
            session: None,
            turn: None,
            tracker: None,
        };

        let output = handler.handle(invocation).await.unwrap();

        if let ToolOutput::Function { content, .. } = output {
            assert_eq!(content.trim(), "2");
        } else {
            panic!("Expected Function output");
        }
    }

    #[tokio::test]
    async fn test_lua_handler_with_args() {
        let handler = LuaHandler::new_default().unwrap();

        let payload = ToolPayload::Function {
            arguments: r#"{"script": "return args.x + args.y", "args": {"x": 10, "y": 20}}"#
                .to_string(),
        };

        let invocation = ToolInvocation {
            payload,
            session: None,
            turn: None,
            tracker: None,
        };

        let output = handler.handle(invocation).await.unwrap();

        if let ToolOutput::Function { content, .. } = output {
            assert_eq!(content.trim(), "30");
        } else {
            panic!("Expected Function output");
        }
    }

    #[tokio::test]
    async fn test_lua_handler_error() {
        let handler = LuaHandler::new_default().unwrap();

        let payload = ToolPayload::Function {
            arguments: r#"{"script": "return undefined_variable"}"#.to_string(),
        };

        let invocation = ToolInvocation {
            payload,
            session: None,
            turn: None,
            tracker: None,
        };

        let result = handler.handle(invocation).await;
        assert!(result.is_err());
    }
}
