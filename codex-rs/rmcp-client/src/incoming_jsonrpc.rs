//! Compatibility decoding for multi-round-trip tool results that rmcp's
//! untagged result union does not yet represent correctly.

use rmcp::model::InputRequiredResult;
use rmcp::model::JsonRpcResponse;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::model::ServerResult;
use serde_json::Value;

/// Decode at the transport boundary, before rmcp's untagged `ServerResult`
/// deserializer can mistake a modern result for a completed tool call.
pub(crate) fn deserialize_incoming_jsonrpc_message(
    bytes: &[u8],
) -> serde_json::Result<ServerJsonRpcMessage> {
    let message: Value = serde_json::from_slice(bytes)?;

    if message
        .pointer("/result/resultType")
        .and_then(Value::as_str)
        == Some("input_required")
    {
        let response: JsonRpcResponse<InputRequiredResult> = serde_json::from_value(message)?;
        return Ok(ServerJsonRpcMessage::response(
            ServerResult::InputRequiredResult(response.result),
            response.id,
        ));
    }

    serde_json::from_value(message)
}

/// Normalize an SSE payload before rmcp parses it internally. SSE does not
/// expose a typed-message hook, so response-level MRTR metadata cannot be
/// retained until the upstream untagged-result decoder is fixed.
pub(crate) fn normalize_sse_jsonrpc_message(payload: &str, modern_session: bool) -> Option<String> {
    if !modern_session {
        return None;
    }

    let mut message: Value = serde_json::from_str(payload).ok()?;
    if message
        .pointer("/result/resultType")
        .and_then(Value::as_str)
        != Some("input_required")
    {
        return None;
    }

    message
        .get_mut("result")
        .and_then(Value::as_object_mut)?
        .remove("_meta")?;
    serde_json::to_string(&message).ok()
}

#[cfg(test)]
#[path = "incoming_jsonrpc_tests.rs"]
mod tests;
