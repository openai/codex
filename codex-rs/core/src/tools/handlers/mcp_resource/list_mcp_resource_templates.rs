use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::mcp_resource_spec::create_list_mcp_resource_templates_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::protocol::McpInvocation;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::ListResourceArgs;
use super::ListResourceTemplatesPayload;
use super::model_can_access_mcp_server;
use super::parse_args_with_default;
use super::parse_arguments;
use super::run_resource_operation;

pub struct ListMcpResourceTemplatesHandler;

impl ToolExecutor<ToolInvocation> for ListMcpResourceTemplatesHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("list_mcp_resource_templates")
    }

    fn spec(&self) -> ToolSpec {
        create_list_mcp_resource_templates_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl ListMcpResourceTemplatesHandler {
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
                    "list_mcp_resource_templates handler received unsupported payload".to_string(),
                ));
            }
        };

        let arguments = parse_arguments(arguments.as_str())?;
        let args: ListResourceArgs = parse_args_with_default(arguments.clone())?;
        let args = args.normalized();

        let invocation = McpInvocation {
            server: args.server.clone().unwrap_or_else(|| "codex".to_string()),
            tool: "list_mcp_resource_templates".to_string(),
            arguments: arguments.clone(),
        };

        run_resource_operation(&session, turn.as_ref(), &call_id, invocation, async {
            if let Some((server_name, params)) = args.target(turn.as_ref())? {
                let result = mcp
                    .list_resource_templates(&server_name, params)
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "resources/templates/list failed: {err:#}"
                        ))
                    })?;
                Ok(ListResourceTemplatesPayload::from_single_server(
                    server_name,
                    result,
                ))
            } else {
                let templates = mcp
                    .list_all_resource_templates(|server_name| {
                        model_can_access_mcp_server(turn.as_ref(), server_name)
                    })
                    .await;
                Ok(ListResourceTemplatesPayload::from_all_servers(templates))
            }
        })
        .await
    }
}

impl CoreToolRuntime for ListMcpResourceTemplatesHandler {}
