//! Compatibility decoding for modern discovery responses whose server identity
//! is represented in namespaced metadata instead of a top-level field.

use rmcp::model::DiscoverResult;
use rmcp::model::JsonRpcResponse;
use rmcp::model::ServerJsonRpcMessage;
use rmcp::model::ServerResult;
use serde_json::Value;

// Remove this compatibility decoder once rmcp accepts namespaced server identity:
// https://github.com/modelcontextprotocol/rust-sdk/issues/1039
const SERVER_INFO_METADATA_KEY: &str = "io.modelcontextprotocol/serverInfo";

/// Decode at the transport boundary, before rmcp's untagged result union can
/// mistake a metadata-only discovery response for a completed tool call.
pub(crate) fn deserialize_incoming_jsonrpc_message(
    bytes: &[u8],
) -> serde_json::Result<ServerJsonRpcMessage> {
    let mut message: Value = serde_json::from_slice(bytes)?;
    let Some(result) = message.get_mut("result").and_then(Value::as_object_mut) else {
        return serde_json::from_value(message);
    };

    if !result.contains_key("supportedVersions") || !result.contains_key("capabilities") {
        return serde_json::from_value(message);
    }

    let Some(server_info) = result
        .get("_meta")
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get(SERVER_INFO_METADATA_KEY))
        .cloned()
    else {
        return serde_json::from_value(message);
    };

    if !result.contains_key("serverInfo") {
        result.insert("serverInfo".to_owned(), server_info);
    }

    let response: JsonRpcResponse<DiscoverResult> = serde_json::from_value(message)?;
    Ok(ServerJsonRpcMessage::response(
        ServerResult::DiscoverResult(response.result),
        response.id,
    ))
}

#[cfg(test)]
#[path = "incoming_jsonrpc_tests.rs"]
mod tests;
