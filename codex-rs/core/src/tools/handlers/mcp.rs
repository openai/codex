use async_trait::async_trait;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::orchestrator::ToolOrchestrator;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::runtimes::mcp::McpRuntime;
use crate::tools::runtimes::mcp::McpToolCallRequest;
use crate::tools::sandboxing::ToolCtx;
use crate::tools::sandboxing::ToolError;

pub struct McpHandler;

#[async_trait]
impl ToolHandler for McpHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Mcp
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            tool_name,
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
        let req = McpToolCallRequest {
            server,
            tool,
            raw_arguments,
            cwd: turn.cwd.clone(),
        };

        let mut orchestrator = ToolOrchestrator::new();
        let mut runtime = McpRuntime::new();
        let tool_ctx = ToolCtx {
            session: session.as_ref(),
            turn: turn.as_ref(),
            call_id: call_id.clone(),
            tool_name: tool_name.to_string(),
        };

        let response = orchestrator
            .run(&mut runtime, &req, &tool_ctx, &turn, turn.approval_policy)
            .await
            .map_err(map_tool_error)?;

        match response {
            codex_protocol::models::ResponseInputItem::McpToolCallOutput { result, .. } => {
                Ok(ToolOutput::Mcp { result })
            }
            codex_protocol::models::ResponseInputItem::FunctionCallOutput { output, .. } => {
                let codex_protocol::models::FunctionCallOutputPayload {
                    content,
                    content_items,
                    success,
                } = output;
                Ok(ToolOutput::Function {
                    content,
                    content_items,
                    success,
                })
            }
            _ => Err(FunctionCallError::RespondToModel(
                "mcp handler received unexpected response variant".to_string(),
            )),
        }
    }
}

fn map_tool_error(err: ToolError) -> FunctionCallError {
    match err {
        ToolError::Rejected(message) => FunctionCallError::RespondToModel(message),
        ToolError::Codex(other) => {
            FunctionCallError::RespondToModel(format!("mcp tool call failed: {other:?}"))
        }
    }
}
