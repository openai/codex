use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolEnvironments;
use crate::tools::context::ToolInvocation;
use crate::tools::handlers::mcp_resource_spec::create_list_mcp_resource_templates_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::openai_models::ToolMessage;
use codex_protocol::protocol::McpInvocation;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::ListResourceArgs;
use super::ListResourceTemplatesPayload;
use super::ResourceOperation;
use super::model_can_access_mcp_server;
use super::resource_arguments;
use super::resource_environments;
use super::run_resource_operation;

pub struct ListMcpResourceTemplatesHandler {
    spec: ToolSpec,
}

impl ToolExecutor<ToolInvocation> for ListMcpResourceTemplatesHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("list_mcp_resource_templates")
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(self.handle_call(invocation))
    }
}

impl ListMcpResourceTemplatesHandler {
    pub fn new(messages: Option<&ToolMessage>, include_environment_id: bool) -> Self {
        Self {
            spec: create_list_mcp_resource_templates_tool(messages, include_environment_id),
        }
    }

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
        let exposure = mcp.config().mcp_server_catalog.exposure();

        let arguments = resource_arguments(&payload)?;
        let args = ListResourceArgs::parse(arguments.clone())?;

        let invocation = McpInvocation {
            server: args.server.clone().unwrap_or_else(|| "codex".to_string()),
            tool: "list_mcp_resource_templates".to_string(),
            arguments: arguments.clone(),
        };

        run_resource_operation(&session, &step_context, &call_id, invocation, async {
            if let Some((server_key, params)) = args.target(&step_context)? {
                let result = mcp
                    .list_resource_templates(server_key, params)
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "resources/templates/list failed: {err:#}"
                        ))
                    })?;
                Ok(ListResourceTemplatesPayload::from_single_server(
                    server_key, result, exposure,
                ))
            } else {
                let templates = mcp
                    .list_all_resource_templates(|server_key| {
                        model_can_access_mcp_server(turn.as_ref(), server_key.name())
                    })
                    .await;
                Ok(ListResourceTemplatesPayload::from_all_servers(
                    templates, exposure,
                ))
            }
        })
        .await
    }
}

impl CoreToolRuntime for ListMcpResourceTemplatesHandler {
    fn environments_for_input(
        &self,
        invocation: &ToolInvocation,
    ) -> Result<ToolEnvironments, FunctionCallError> {
        resource_environments(invocation, ResourceOperation::List)
    }
}
