use async_trait::async_trait;
use std::sync::Arc;

use crate::function_tool::FunctionCallError;
use crate::mcp_tool_call::QualifiedMcpTool;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::mcp::CallToolResult;

pub struct McpHandler;
#[async_trait]
impl ToolHandler for McpHandler {
    type Output = CallToolResult;

    fn kind(&self) -> ToolKind {
        ToolKind::Mcp
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
            tool_namespace,
            payload,
            ..
        } = invocation;

        let payload = match payload {
            ToolPayload::Mcp {
                server,
                tool,
                raw_arguments,
            } => (server, tool, raw_arguments),
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "mcp handler received unsupported payload".to_string(),
                ));
            }
        };

        let (server, tool, raw_arguments) = payload;
        let arguments_str = raw_arguments;

        let output = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn,
            call_id.clone(),
            QualifiedMcpTool {
                name: &tool_name,
                namespace: tool_namespace.as_deref(),
            },
            server,
            tool,
            arguments_str,
        )
        .await;

        Ok(output)
    }
}
