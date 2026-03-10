use async_trait::async_trait;
use std::sync::Arc;

use crate::function_tool::FunctionCallError;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::tools::context::ContentToolOutput;
use crate::tools::context::McpToolOutput;
use crate::tools::context::TextToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutputBox;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::ResponseInputItem;

pub struct McpHandler;

#[async_trait]
impl ToolHandler for McpHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Mcp
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutputBox, FunctionCallError> {
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

        let response = handle_mcp_tool_call(
            Arc::clone(&session),
            &turn,
            call_id.clone(),
            server,
            tool,
            arguments_str,
        )
        .await;

        match response {
            ResponseInputItem::McpToolCallOutput { result, .. } => {
                Ok(Box::new(McpToolOutput { result }))
            }
            ResponseInputItem::FunctionCallOutput { output, .. } => {
                let success = output.success;
                match output.body {
                    codex_protocol::models::FunctionCallOutputBody::Text(text) => {
                        Ok(Box::new(TextToolOutput { text, success }))
                    }
                    codex_protocol::models::FunctionCallOutputBody::ContentItems(content) => {
                        Ok(Box::new(ContentToolOutput { content, success }))
                    }
                }
            }
            _ => Err(FunctionCallError::RespondToModel(
                "mcp handler received unexpected response variant".to_string(),
            )),
        }
    }
}
