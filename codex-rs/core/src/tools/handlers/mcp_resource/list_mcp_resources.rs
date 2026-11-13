use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolEnvironments;
use crate::tools::context::ToolInvocation;
use crate::tools::handlers::mcp_resource_spec::create_list_mcp_resources_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::openai_models::ToolMessage;
use codex_protocol::protocol::McpInvocation;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::ListResourceArgs;
use super::ListResourcesPayload;
use super::ResourceOperation;
use super::model_can_access_mcp_server;
use super::resource_arguments;
use super::resource_environments;
use super::run_resource_operation;

pub struct ListMcpResourcesHandler {
    spec: ToolSpec,
}

impl ToolExecutor<ToolInvocation> for ListMcpResourcesHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("list_mcp_resources")
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

impl ListMcpResourcesHandler {
    pub fn new(messages: Option<&ToolMessage>, include_environment_id: bool) -> Self {
        Self {
            spec: create_list_mcp_resources_tool(messages, include_environment_id),
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
        let target = args.target(&step_context);

        let invocation = McpInvocation {
            server: args.server.clone().unwrap_or_else(|| "codex".to_string()),
            environment_id: target
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .and_then(|(key, _)| key.scoped_environment_id())
                .map(str::to_owned),
            tool: "list_mcp_resources".to_string(),
            arguments: arguments.clone(),
        };

        run_resource_operation(&session, &step_context, &call_id, invocation, async {
            if let Some((server_key, params)) = target? {
                let result = mcp
                    .list_resources(server_key, params)
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!("resources/list failed: {err:#}"))
                    })?;
                Ok(ListResourcesPayload::from_single_server(
                    server_key, result, exposure,
                ))
            } else {
                let resources = mcp
                    .list_all_resources(|server_key| {
                        model_can_access_mcp_server(turn.as_ref(), server_key.name())
                    })
                    .await;
                Ok(ListResourcesPayload::from_all_servers(resources, exposure))
            }
        })
        .await
    }
}

impl CoreToolRuntime for ListMcpResourcesHandler {
    fn environments_for_input(
        &self,
        invocation: &ToolInvocation,
    ) -> Result<ToolEnvironments, FunctionCallError> {
        resource_environments(invocation, ResourceOperation::List)
    }
}
