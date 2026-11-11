use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolEnvironments;
use crate::tools::context::ToolInvocation;
use crate::tools::handlers::mcp_resource_spec::create_read_mcp_resource_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::openai_models::ToolMessage;
use codex_protocol::protocol::McpInvocation;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use rmcp::model::ReadResourceRequestParams;

use super::ReadResourceArgs;
use super::ReadResourcePayload;
use super::ResourceOperation;
use super::resolve_resource_server;
use super::resource_arguments;
use super::resource_environments;
use super::run_resource_operation;

pub struct ReadMcpResourceHandler {
    spec: ToolSpec,
}

impl ToolExecutor<ToolInvocation> for ReadMcpResourceHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("read_mcp_resource")
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

impl ReadMcpResourceHandler {
    pub fn new(messages: Option<&ToolMessage>, include_environment_id: bool) -> Self {
        Self {
            spec: create_read_mcp_resource_tool(messages, include_environment_id),
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
        let mcp = &step_context.mcp;

        let arguments = resource_arguments(&payload)?;
        let ReadResourceArgs {
            server,
            uri,
            environment_id,
        } = ReadResourceArgs::parse(arguments.clone())?;

        let invocation = McpInvocation {
            server: server.clone(),
            tool: "read_mcp_resource".to_string(),
            arguments: arguments.clone(),
        };

        run_resource_operation(&session, &step_context, &call_id, invocation, async {
            let server_key =
                resolve_resource_server(&step_context, &server, environment_id.as_deref())?;
            let result = mcp
                .read_resource(server_key, ReadResourceRequestParams::new(uri.clone()))
                .await
                .map_err(|err| {
                    FunctionCallError::RespondToModel(format!("resources/read failed: {err:#}"))
                })?;
            Ok(ReadResourcePayload {
                server: server_key.name().to_owned(),
                environment_id: mcp
                    .config()
                    .mcp_server_catalog
                    .exposure()
                    .environment_id(server_key)
                    .map(str::to_owned),
                uri,
                result,
            })
        })
        .await
    }
}

impl CoreToolRuntime for ReadMcpResourceHandler {
    fn environments_for_input(
        &self,
        invocation: &ToolInvocation,
    ) -> Result<ToolEnvironments, FunctionCallError> {
        resource_environments(invocation, ResourceOperation::Read)
    }
}
