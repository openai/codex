use async_trait::async_trait;
use std::sync::Arc;

use crate::function_tool::FunctionCallError;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::McpToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::ResponseInputItem;

pub struct McpHandler;

pub enum McpHandlerOutput {
    Mcp(McpToolOutput),
    Function(FunctionToolOutput),
}

impl crate::tools::context::ToolOutput for McpHandlerOutput {
    fn log_preview(&self) -> String {
        match self {
            Self::Mcp(output) => output.log_preview(),
            Self::Function(output) => output.log_preview(),
        }
    }

    fn success_for_logging(&self) -> bool {
        match self {
            Self::Mcp(output) => output.success_for_logging(),
            Self::Function(output) => output.success_for_logging(),
        }
    }

    fn into_response(self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        match self {
            Self::Mcp(output) => output.into_response(call_id, payload),
            Self::Function(output) => output.into_response(call_id, payload),
        }
    }
}

#[async_trait]
impl ToolHandler for McpHandler {
    type Output = McpHandlerOutput;

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
                Ok(McpHandlerOutput::Mcp(McpToolOutput { result }))
            }
            ResponseInputItem::FunctionCallOutput { output, .. } => {
                let success = output.success;
                match output.body {
                    codex_protocol::models::FunctionCallOutputBody::Text(text) => Ok(
                        McpHandlerOutput::Function(FunctionToolOutput::from_text(text, success)),
                    ),
                    codex_protocol::models::FunctionCallOutputBody::ContentItems(content) => {
                        Ok(McpHandlerOutput::Function(
                            FunctionToolOutput::from_content(content, success),
                        ))
                    }
                }
            }
            _ => Err(FunctionCallError::RespondToModel(
                "mcp handler received unexpected response variant".to_string(),
            )),
        }
    }
}
