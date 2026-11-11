//! Prepares MCP approval actions using current bindings without dispatching or remembering approval.

use codex_mcp::ToolInfo;
use codex_protocol::protocol::McpInvocation;
use codex_tools::FunctionCallError;

use super::McpToolApprovalRequirement;
use super::mcp_tool_approval_policy;
use super::mcp_tool_metadata;
use super::parse_mcp_tool_arguments;
use super::prepare_mcp_tool_approval;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::McpHandler;
use crate::tools::sandboxing::ApprovalAction;

pub(crate) async fn prepare_approval_action(
    invocation: &ToolInvocation,
    advertised_tool: &ToolInfo,
) -> Result<Option<ApprovalAction>, FunctionCallError> {
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return Err(FunctionCallError::RespondToModel(
            "mcp handler received unsupported payload".into(),
        ));
    };
    let arguments = parse_mcp_tool_arguments(arguments)
        .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
    let call = invocation
        .session
        .prepare_mcp_call(advertised_tool)
        .await
        .map_err(|err| FunctionCallError::RespondToModel(format!("{err:#}")))?;
    let mcp_invocation = McpInvocation {
        server: advertised_tool.server_name().to_owned(),
        tool: advertised_tool.tool.name.to_string(),
        arguments,
    };
    let metadata = mcp_tool_metadata(
        call.tool_info(),
        call.plugin_id(),
        mcp_invocation.arguments.as_ref(),
    )
    .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
    let policy = mcp_tool_approval_policy(&call, &metadata).ok_or_else(|| {
        FunctionCallError::RespondToModel("MCP tool call blocked by app configuration".into())
    })?;
    match prepare_mcp_tool_approval(
        &invocation.session,
        &invocation.step_context,
        &invocation.call_id,
        &mcp_invocation,
        &McpHandler::hook_tool_name_for(advertised_tool),
        &metadata,
        call.config(),
        call.permission_source(),
        call.server_environment_id(),
        call.server_key(),
        policy,
    )
    .await
    {
        McpToolApprovalRequirement::NotRequired | McpToolApprovalRequirement::Remembered => {
            Ok(None)
        }
        McpToolApprovalRequirement::Required { action, .. } => Ok(Some(*action)),
    }
}
