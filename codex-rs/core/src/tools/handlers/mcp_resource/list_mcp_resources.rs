use std::time::Instant;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::function_call_output_content_items_to_text;
use codex_protocol::protocol::McpInvocation;
use codex_tools::ToolName;

use rmcp::model::PaginatedRequestParams;

use super::ListResourcesArgs;
use super::ListResourcesPayload;
use super::call_tool_result_from_content;
use super::emit_tool_call_begin;
use super::emit_tool_call_end;
use super::normalize_optional_string;
use super::parse_args_with_default;
use super::parse_arguments;
use super::serialize_function_output;

pub struct ListMcpResourcesHandler;

impl ToolHandler for ListMcpResourcesHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("list_mcp_resources")
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "MCP resource listing reads through the session-owned manager guard"
    )]
    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "list_mcp_resources handler received unsupported payload".to_string(),
                ));
            }
        };

        let arguments = parse_arguments(arguments.as_str())?;
        let args: ListResourcesArgs = parse_args_with_default(arguments.clone())?;
        let ListResourcesArgs { server, cursor } = args;
        let server = normalize_optional_string(server);
        let cursor = normalize_optional_string(cursor);

        let invocation = McpInvocation {
            server: server.clone().unwrap_or_else(|| "codex".to_string()),
            tool: "list_mcp_resources".to_string(),
            arguments: arguments.clone(),
        };

        emit_tool_call_begin(&session, turn.as_ref(), &call_id, invocation.clone()).await;
        let start = Instant::now();

        let payload_result: Result<ListResourcesPayload, FunctionCallError> = async {
            if let Some(server_name) = server.clone() {
                let params = cursor.clone().map(|value| PaginatedRequestParams {
                    meta: None,
                    cursor: Some(value),
                });
                let result = session
                    .list_resources(&server_name, params)
                    .await
                    .map_err(|err| {
                        FunctionCallError::RespondToModel(format!("resources/list failed: {err:#}"))
                    })?;
                Ok(ListResourcesPayload::from_single_server(
                    server_name,
                    result,
                ))
            } else {
                if cursor.is_some() {
                    return Err(FunctionCallError::RespondToModel(
                        "cursor can only be used when a server is specified".to_string(),
                    ));
                }

                let resources = session
                    .services
                    .mcp_connection_manager
                    .read()
                    .await
                    .list_all_resources()
                    .await;
                Ok(ListResourcesPayload::from_all_servers(resources))
            }
        }
        .await;

        match payload_result {
            Ok(payload) => match serialize_function_output(payload) {
                Ok(output) => {
                    let content = function_call_output_content_items_to_text(&output.body)
                        .unwrap_or_default();
                    let duration = start.elapsed();
                    emit_tool_call_end(
                        &session,
                        turn.as_ref(),
                        &call_id,
                        invocation,
                        duration,
                        Ok(call_tool_result_from_content(&content, output.success)),
                    )
                    .await;
                    Ok(output)
                }
                Err(err) => {
                    let duration = start.elapsed();
                    let message = err.to_string();
                    emit_tool_call_end(
                        &session,
                        turn.as_ref(),
                        &call_id,
                        invocation,
                        duration,
                        Err(message.clone()),
                    )
                    .await;
                    Err(err)
                }
            },
            Err(err) => {
                let duration = start.elapsed();
                let message = err.to_string();
                emit_tool_call_end(
                    &session,
                    turn.as_ref(),
                    &call_id,
                    invocation,
                    duration,
                    Err(message.clone()),
                )
                .await;
                Err(err)
            }
        }
    }
}
