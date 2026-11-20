//! Prepares Guardian review requests using current bindings without dispatching or remembering approval.

use codex_mcp::ToolInfo;
use codex_protocol::protocol::McpInvocation;
use codex_tools::FunctionCallError;

use super::build_guardian_mcp_tool_review_request;
use super::capture_environment_permissions;
use super::mcp_tool_approval_policy;
use super::mcp_tool_metadata;
use super::parse_mcp_tool_arguments;
use crate::guardian::GuardianApprovalRequest;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;

pub(crate) async fn prepare_review_request(
    invocation: &ToolInvocation,
    advertised_tool: &ToolInfo,
) -> Result<GuardianApprovalRequest, FunctionCallError> {
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
        environment_id: advertised_tool
            .server_key()
            .scoped_environment_id()
            .map(str::to_owned),
        tool: advertised_tool.tool.name.to_string(),
        arguments,
    };
    let metadata = mcp_tool_metadata(
        call.tool_info(),
        call.plugin_id(),
        mcp_invocation.arguments.as_ref(),
    )
    .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))?;
    if mcp_tool_approval_policy(&call, &metadata).is_none() {
        return Err(FunctionCallError::RespondToModel(
            "MCP tool call blocked by app configuration".into(),
        ));
    }
    Ok(build_guardian_mcp_tool_review_request(
        &invocation.call_id,
        &mcp_invocation,
        Some(&metadata),
        capture_environment_permissions(call.permission_source()),
    ))
}
