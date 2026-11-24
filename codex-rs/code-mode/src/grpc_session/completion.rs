use codex_code_mode_protocol::CodeModeToolResult;
use codex_code_mode_protocol::grpc;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use prost::Message;

pub(super) fn request(
    session_id: &str,
    invocation_id: &str,
    result: Result<CodeModeToolResult, String>,
) -> grpc::CompleteToolCallRequest {
    request_with_maximum(session_id, invocation_id, result, MAX_FRAME_BYTES)
}

fn request_with_maximum(
    session_id: &str,
    invocation_id: &str,
    result: Result<CodeModeToolResult, String>,
    maximum_message_bytes: usize,
) -> grpc::CompleteToolCallRequest {
    let reply = result.and_then(|result| {
        let (value, format) = match result {
            CodeModeToolResult::Json(value) => (value, grpc::ToolResultFormat::Json),
            CodeModeToolResult::McpCallToolResult(value) => {
                (value, grpc::ToolResultFormat::McpCallToolResult)
            }
        };
        let output_json = serde_json::to_vec(&value)
            .map_err(|error| format!("failed to encode code-mode tool result: {error}"))?;
        Ok(grpc::ToolCallSucceeded {
            output_json,
            result_format: format as i32,
        })
    });
    let outcome = match reply {
        Ok(reply) => grpc::complete_tool_call_request::Outcome::Succeeded(reply),
        Err(message) => {
            grpc::complete_tool_call_request::Outcome::Failed(grpc::ToolCallFailed { message })
        }
    };
    let mut request = grpc::CompleteToolCallRequest {
        session_id: session_id.to_string(),
        invocation_id: invocation_id.to_string(),
        outcome: Some(outcome),
    };
    let encoded_bytes = request.encoded_len();
    if encoded_bytes > maximum_message_bytes {
        request.outcome = Some(grpc::complete_tool_call_request::Outcome::Failed(
            grpc::ToolCallFailed {
                message: format!(
                    "code-mode tool result of {encoded_bytes} encoded bytes exceeds the gRPC message limit of {maximum_message_bytes} bytes"
                ),
            },
        ));
    }
    request
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;
