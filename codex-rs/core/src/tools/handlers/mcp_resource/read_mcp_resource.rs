use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::mcp_resource_spec::create_read_mcp_resource_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::protocol::McpInvocation;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use rmcp::model::ReadResourceRequestParams;

use super::ReadResourceArgs;
use super::ReadResourcePayload;
use super::ensure_model_can_access_mcp_server;
use super::normalize_required_string;
use super::parse_args;
use super::parse_arguments;
use super::run_resource_operation;

pub struct ReadMcpResourceHandler;

impl ToolExecutor<ToolInvocation> for ReadMcpResourceHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("read_mcp_resource")
    }

    fn spec(&self) -> ToolSpec {
        create_read_mcp_resource_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl ReadMcpResourceHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            step_context,
            call_id,
            payload,
            ..
        } = invocation;
        let turn = std::sync::Arc::clone(&step_context.turn);
        let mcp = &step_context.mcp;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "read_mcp_resource handler received unsupported payload".to_string(),
                ));
            }
        };

        let arguments = parse_arguments(arguments.as_str())?;
        let args: ReadResourceArgs = parse_args(arguments.clone())?;
        let ReadResourceArgs { server, uri } = args;
        let server = normalize_required_string("server", server)?;
        let uri = normalize_required_string("uri", uri)?;

        let invocation = McpInvocation {
            server: server.clone(),
            tool: "read_mcp_resource".to_string(),
            arguments: arguments.clone(),
        };

        run_resource_operation(&session, turn.as_ref(), &call_id, invocation, async {
            ensure_model_can_access_mcp_server(turn.as_ref(), &server)?;
            let result = mcp
                .read_resource(&server, ReadResourceRequestParams::new(uri.clone()))
                .await
                .map_err(|err| {
                    FunctionCallError::RespondToModel(format!("resources/read failed: {err:#}"))
                })?;

            Ok(ReadResourcePayload {
                server,
                uri,
                result,
            })
        })
        .await
    }
}

impl CoreToolRuntime for ReadMcpResourceHandler {}
