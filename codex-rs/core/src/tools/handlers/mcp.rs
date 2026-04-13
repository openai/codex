use std::sync::Arc;
use std::time::Instant;

use crate::function_tool::FunctionCallError;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::tools::context::McpToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct McpHandler;
impl ToolHandler for McpHandler {
    type Output = McpToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Mcp
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
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

        let started = Instant::now();
        let result = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn,
            call_id.clone(),
            server,
            tool,
            arguments_str,
        )
        .await;

        Ok(McpToolOutput {
            result,
            wall_time: started.elapsed(),
        })
    }
}
