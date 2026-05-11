use std::time::Instant;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::mcp_resource_spec::create_read_mcp_resource_tool;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_protocol::models::function_call_output_content_items_to_text;
use codex_protocol::protocol::McpInvocation;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use rmcp::model::ReadResourceRequestParams;

use super::ReadResourceArgs;
use super::ReadResourcePayload;
use super::call_tool_result_from_content;
use super::emit_tool_call_begin;
use super::emit_tool_call_end;
use super::normalize_required_string;
use super::parse_args;
use super::parse_arguments;
use super::serialize_function_output;

pub struct ReadMcpResourceHandler;

impl ToolHandler for ReadMcpResourceHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("read_mcp_resource")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_read_mcp_resource_tool())
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

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

        emit_tool_call_begin(&session, turn.as_ref(), &call_id, invocation.clone()).await;
        let start = Instant::now();

        let payload_result: Result<ReadResourcePayload, FunctionCallError> = async {
            let result = session
                .read_resource(
                    &server,
                    ReadResourceRequestParams {
                        meta: None,
                        uri: uri.clone(),
                    },
                )
                .await
                .map_err(|err| {
                    FunctionCallError::RespondToModel(format!("resources/read failed: {err:#}"))
                })?;

            Ok(ReadResourcePayload {
                server,
                uri,
                result,
            })
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
