//! Compatibility decoding for server messages whose modern wire shapes are
//! not yet represented correctly by rmcp's untagged result union.

use rmcp::model::DiscoverResult;
use rmcp::model::InputRequiredResult;
use rmcp::model::JsonRpcResponse;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::model::ServerResult;
use serde_json::Value;

// Remove this compatibility decoder once rmcp accepts namespaced server identity:
// https://github.com/modelcontextprotocol/rust-sdk/issues/1039
const SERVER_INFO_METADATA_KEY: &str = "io.modelcontextprotocol/serverInfo";

/// Decode at the transport boundary, before rmcp's untagged `ServerResult`
/// deserializer can mistake a modern result for a completed tool call.
pub(crate) fn deserialize_incoming_jsonrpc_message(
    bytes: &[u8],
) -> serde_json::Result<ServerJsonRpcMessage> {
    let mut message: Value = serde_json::from_slice(bytes)?;
    let changed_discovery = normalize_discovery_server_info(&mut message);

    let Some(result) = message.get("result") else {
        return serde_json::from_value(message);
    };

    if changed_discovery {
        let response: JsonRpcResponse<DiscoverResult> = serde_json::from_value(message)?;
        return Ok(ServerJsonRpcMessage::response(
            ServerResult::DiscoverResult(response.result),
            response.id,
        ));
    }

    if result.get("resultType").and_then(Value::as_str) == Some("input_required") {
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
    let changed_discovery = normalize_discovery_server_info(&mut message);
    let changed_mrtr = if message
        .pointer("/result/resultType")
        .and_then(Value::as_str)
        == Some("input_required")
    {
        message
            .get_mut("result")
            .and_then(Value::as_object_mut)
            .is_some_and(|result| result.remove("_meta").is_some())
    } else {
        false
    };

    if changed_discovery || changed_mrtr {
        serde_json::to_string(&message).ok()
    } else {
        None
    }
}

fn normalize_discovery_server_info(message: &mut Value) -> bool {
    let Some(result) = message.get_mut("result").and_then(Value::as_object_mut) else {
        return false;
    };
    if result.contains_key("serverInfo")
        || !result.contains_key("supportedVersions")
        || !result.contains_key("capabilities")
    {
        return false;
    }

    let Some(server_info) = result
        .get("_meta")
        .and_then(Value::as_object)
        .and_then(|meta| meta.get(SERVER_INFO_METADATA_KEY))
        .cloned()
    else {
        return false;
    };
    result.insert("serverInfo".to_owned(), server_info);
    true
}

#[cfg(test)]
#[path = "incoming_jsonrpc_tests.rs"]
mod tests;
