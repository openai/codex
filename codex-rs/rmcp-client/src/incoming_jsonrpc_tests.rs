use pretty_assertions::assert_eq;
use rmcp::model::JsonRpcMessage;
use rmcp::model::ServerResult;
use serde_json::json;

use super::deserialize_incoming_jsonrpc_message;
use super::normalize_sse_jsonrpc_message;

#[test]
fn discovery_accepts_metadata_namespaced_server_identity() {
    let message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "resultType": "complete",
            "supportedVersions": ["2026-07-28"],
            "capabilities": {"tools": {}},
            "ttlMs": 0,
            "cacheScope": "private",
            "_meta": {
                "io.modelcontextprotocol/serverInfo": {
                    "name": "modern-server",
                    "version": "1.0.0",
                },
                "retained": "metadata",
            },
        },
    });

    let decoded = deserialize_incoming_jsonrpc_message(&serde_json::to_vec(&message).unwrap())
        .expect("metadata-only identity must decode as discovery");
    let JsonRpcMessage::Response(response) = decoded else {
        panic!("expected a discovery response");
    };
    let ServerResult::DiscoverResult(result) = response.result else {
        panic!("metadata-only identity must not become a completed tool result");
    };
    assert_eq!(result.server_info.name, "modern-server");
    assert_eq!(
        result.meta.and_then(|meta| meta.get("retained").cloned()),
        Some(json!("metadata"))
    );
}

#[test]
fn extension_result_with_discovery_like_fields_remains_a_custom_result() {
    let result = json!({
        "supportedVersions": ["custom-extension-v1"],
        "capabilities": {"custom": true},
    });
    let message = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "result": result,
    });

    let decoded = deserialize_incoming_jsonrpc_message(&serde_json::to_vec(&message).unwrap())
        .expect("legacy extension result must deserialize normally");
    let JsonRpcMessage::Response(response) = decoded else {
        panic!("expected a legacy extension response");
    };
    let ServerResult::CustomResult(custom) = response.result else {
        panic!("legacy extension response must remain a custom result");
    };
    assert_eq!(custom.0, result);
}

#[test]
fn input_required_discriminator_wins_over_tool_result_metadata() {
    let message = json!({
        "jsonrpc": "2.0",
        "id": "tool-call",
        "result": {
            "resultType": "input_required",
            "requestState": "opaque-server-state",
            "_meta": {"retained": "response-metadata"},
        },
    });

    let decoded = deserialize_incoming_jsonrpc_message(&serde_json::to_vec(&message).unwrap())
        .expect("input-required result must decode");
    let JsonRpcMessage::Response(response) = decoded else {
        panic!("expected an input-required response");
    };
    let ServerResult::InputRequiredResult(result) = response.result else {
        panic!("input_required must not become an empty completed tool result");
    };
    assert_eq!(result.request_state.as_deref(), Some("opaque-server-state"));
    assert_eq!(
        result.meta.and_then(|meta| meta.get("retained").cloned()),
        Some(json!("response-metadata"))
    );
}

#[test]
fn sse_discovery_promotes_metadata_namespaced_identity() {
    let message = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "resultType": "complete",
            "supportedVersions": ["2026-07-28"],
            "capabilities": {},
            "ttlMs": 0,
            "cacheScope": "private",
            "_meta": {
                "io.modelcontextprotocol/serverInfo": {
                    "name": "sse-server",
                    "version": "1.0.0",
                },
            },
        },
    });
    let normalized =
        normalize_sse_jsonrpc_message(&message.to_string(), /*modern_session*/ true)
            .expect("metadata-only SSE discovery must be normalized");
    let normalized: serde_json::Value = serde_json::from_str(&normalized).unwrap();
    assert_eq!(normalized["result"]["serverInfo"]["name"], "sse-server");
}

#[test]
fn legacy_sse_payload_preserves_intermediate_response_metadata() {
    let message = json!({
        "jsonrpc": "2.0",
        "id": "legacy-extension",
        "result": {
            "resultType": "input_required",
            "_meta": {"preserved": "legacy-extension-metadata"},
        },
    });

    assert_eq!(
        normalize_sse_jsonrpc_message(&message.to_string(), /*modern_session*/ false),
        None
    );
}
