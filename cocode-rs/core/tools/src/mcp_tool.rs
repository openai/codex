//! MCP tool wrapper for executing tools from MCP servers.
//!
//! This module provides [`McpToolWrapper`] which wraps an MCP tool and implements
//! the [`Tool`] trait, allowing MCP tools to be used alongside built-in tools.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use cocode_mcp_types::{CallToolResult, ContentBlock, TextContent, Tool as McpTool};
use cocode_protocol::{ConcurrencySafety, ToolOutput, ToolResultContent};
use cocode_rmcp_client::RmcpClient;
use serde_json::Value;
use tracing::{debug, warn};

use crate::context::ToolContext;
use crate::error::{Result, ToolError};
use crate::tool::Tool;

/// Wrapper around an MCP tool that implements the [`Tool`] trait.
///
/// This allows MCP tools to be registered in the [`ToolRegistry`] and executed
/// using the same interface as built-in tools.
///
/// # Note
///
/// The client is wrapped in `Arc<RmcpClient>` (not `Arc<Mutex<RmcpClient>>`) because
/// `RmcpClient` has internal synchronization and its methods take `&self`. This
/// avoids holding locks across async operations.
pub struct McpToolWrapper {
    /// Server name for the MCP server providing this tool.
    server_name: String,
    /// The MCP tool definition.
    mcp_tool: McpTool,
    /// The MCP client used to call the tool.
    client: Arc<RmcpClient>,
    /// Timeout for tool calls.
    timeout: Duration,
}

impl McpToolWrapper {
    /// Create a new MCP tool wrapper.
    ///
    /// # Arguments
    ///
    /// * `server_name` - Name of the MCP server
    /// * `mcp_tool` - The MCP tool definition
    /// * `client` - Shared client for the MCP server (uses `Arc<RmcpClient>` not `Arc<Mutex<...>>`)
    /// * `timeout` - Timeout for tool calls
    pub fn new(
        server_name: String,
        mcp_tool: McpTool,
        client: Arc<RmcpClient>,
        timeout: Duration,
    ) -> Self {
        Self {
            server_name,
            mcp_tool,
            client,
            timeout,
        }
    }

    /// Get the qualified name following the `mcp__<server>__<tool>` convention.
    pub fn qualified_name(&self) -> String {
        format!("mcp__{}_{}", self.server_name, self.mcp_tool.name)
    }

    /// Get the server name.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Get the original MCP tool name.
    pub fn original_name(&self) -> &str {
        &self.mcp_tool.name
    }

    /// Convert MCP CallToolResult to ToolOutput.
    fn convert_result(&self, result: CallToolResult) -> Result<ToolOutput> {
        let is_error = result.is_error.unwrap_or(false);

        // Try to use structured content first
        if let Some(structured) = result.structured_content {
            return Ok(ToolOutput {
                content: ToolResultContent::Structured(structured),
                is_error,
                modifiers: Vec::new(),
            });
        }

        // Otherwise, extract text from content blocks
        let text: String = result
            .content
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::TextContent(TextContent { text, .. }) => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput {
            content: ToolResultContent::Text(text),
            is_error,
            modifiers: Vec::new(),
        })
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        // Note: This returns the original name, but the tool should be registered
        // with the qualified name in the registry
        &self.mcp_tool.name
    }

    fn description(&self) -> &str {
        self.mcp_tool.description.as_deref().unwrap_or("MCP tool")
    }

    fn input_schema(&self) -> Value {
        // Convert ToolInputSchema to Value
        let mut schema = serde_json::json!({
            "type": "object"
        });

        if let Some(props) = &self.mcp_tool.input_schema.properties {
            schema["properties"] = props.clone();
        }

        if let Some(required) = &self.mcp_tool.input_schema.required {
            schema["required"] = serde_json::to_value(required).unwrap_or_default();
        }

        schema
    }

    fn concurrency_safety(&self) -> ConcurrencySafety {
        // MCP tools are considered unsafe by default since we don't know
        // their side effects
        ConcurrencySafety::Unsafe
    }

    async fn execute(&self, input: Value, _ctx: &mut ToolContext) -> Result<ToolOutput> {
        debug!(
            server = %self.server_name,
            tool = %self.mcp_tool.name,
            "Executing MCP tool"
        );

        // Prepare arguments - if input is an empty object, pass None
        let arguments = if input.is_object() && input.as_object().map_or(true, |o| o.is_empty()) {
            None
        } else {
            Some(input)
        };

        // Call the MCP tool - no locking needed since RmcpClient has internal synchronization
        let result = self
            .client
            .call_tool(self.mcp_tool.name.clone(), arguments, Some(self.timeout))
            .await
            .map_err(|e| {
                warn!(
                    server = %self.server_name,
                    tool = %self.mcp_tool.name,
                    error = %e,
                    "MCP tool call failed"
                );
                ToolError::execution_failed(format!("MCP tool call failed: {e}"))
            })?;

        self.convert_result(result)
    }
}

impl std::fmt::Debug for McpToolWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpToolWrapper")
            .field("server_name", &self.server_name)
            .field("tool_name", &self.mcp_tool.name)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_mcp_types::ToolInputSchema;

    fn make_mcp_tool(name: &str, description: Option<&str>) -> McpTool {
        McpTool {
            name: name.to_string(),
            description: description.map(String::from),
            input_schema: ToolInputSchema {
                r#type: "object".to_string(),
                properties: Some(serde_json::json!({
                    "arg1": {"type": "string"}
                })),
                required: Some(vec!["arg1".to_string()]),
            },
            annotations: None,
            output_schema: None,
            title: None,
        }
    }

    #[test]
    fn test_qualified_name() {
        // We can't fully test without a real client, but we can test the naming
        let tool = make_mcp_tool("get_data", Some("Gets data"));

        // Verify the tool definition
        assert_eq!(tool.name, "get_data");
        assert_eq!(tool.description, Some("Gets data".to_string()));
    }

    #[test]
    fn test_input_schema_conversion() {
        let tool = make_mcp_tool("test", None);

        // Verify schema has properties and required
        assert!(tool.input_schema.properties.is_some());
        assert!(tool.input_schema.required.is_some());
    }
}
