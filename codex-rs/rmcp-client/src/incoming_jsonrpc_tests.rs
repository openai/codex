use pretty_assertions::assert_eq;
use rmcp::model::JsonRpcMessage;
use rmcp::model::ServerResult;
use serde_json::json;

use super::deserialize_incoming_jsonrpc_message;

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
