use pretty_assertions::assert_eq;
use serde_json::json;

use super::JSONRPCError;
use super::JSONRPCErrorError;
use super::JSONRPCMessage;
use super::JSONRPCNotification;
use super::JSONRPCRequest;
use super::JSONRPCResponse;
use super::MAX_JSONRPC_VALUE_NODES;
use super::RequestId;

#[test]
fn round_trips_every_jsonrpc_message_variant() -> serde_json::Result<()> {
    let messages = [
        JSONRPCMessage::Request(JSONRPCRequest {
            id: RequestId::Integer(1),
            method: "request".to_string(),
            params: Some(json!({"items": [1, 2, 3]})),
            trace: None,
        }),
        JSONRPCMessage::Notification(JSONRPCNotification {
            method: "notification".to_string(),
            params: Some(json!({"enabled": true})),
        }),
        JSONRPCMessage::Response(JSONRPCResponse {
            id: RequestId::String("response".to_string()),
            result: json!({"value": "ok"}),
        }),
        JSONRPCMessage::Error(JSONRPCError {
            error: JSONRPCErrorError {
                code: -32603,
                data: Some(json!({"retryable": false})),
                message: "failed".to_string(),
            },
            id: RequestId::Integer(2),
        }),
    ];

    for expected in messages {
        let encoded = serde_json::to_string(&expected)?;
        let actual = serde_json::from_str::<JSONRPCMessage>(&encoded)?;
        assert_eq!(actual, expected);
    }

    Ok(())
}

#[test]
fn accepts_large_scalar_payload() -> serde_json::Result<()> {
    let expected = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "large".to_string(),
        params: Some(json!({"data": "x".repeat(MAX_JSONRPC_VALUE_NODES + 1)})),
    });

    let encoded = serde_json::to_string(&expected)?;
    let actual = serde_json::from_str::<JSONRPCMessage>(&encoded)?;

    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn rejects_compact_array_heap_amplification() {
    const REPRO_VALUE_COUNT: usize = 2_097_137;
    const REPRO_MESSAGE_BYTES: usize = 4_194_303;

    let mut encoded = String::with_capacity(REPRO_MESSAGE_BYTES);
    encoded.push_str(r#"{"method":"probe","params":["#);
    for index in 0..REPRO_VALUE_COUNT {
        if index != 0 {
            encoded.push(',');
        }
        encoded.push('0');
    }
    encoded.push_str("]}");
    assert_eq!(encoded.len(), REPRO_MESSAGE_BYTES);

    let error = serde_json::from_str::<JSONRPCMessage>(&encoded)
        .expect_err("amplification payload should exceed the JSON value limit");
    let expected_error = format!("exceeds the limit of {MAX_JSONRPC_VALUE_NODES} JSON values");
    assert!(
        error.to_string().contains(&expected_error),
        "unexpected error: {error}"
    );
}
