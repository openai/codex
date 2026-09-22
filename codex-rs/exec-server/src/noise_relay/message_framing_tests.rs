use codex_exec_server_protocol::EXEC_OUTPUT_DELTA_METHOD;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::JSONRPCNotification;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use pretty_assertions::assert_eq;

use super::JsonRpcMessageDecoder;
use super::MAX_NOISE_JSONRPC_MESSAGE_LEN;
use super::NOISE_RECORD_PLAINTEXT_LEN;
use super::frame_jsonrpc_message;
use crate::ExecServerError;
use crate::client_inbound_request_limit::MAX_CLIENT_INBOUND_REQUEST_LEN;

#[test]
fn fragments_and_reassembles_large_jsonrpc_message() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: EXEC_OUTPUT_DELTA_METHOD.to_string(),
        params: Some(serde_json::json!({
            "data": "x".repeat(128 * 1024),
        })),
    });
    let framed = frame_jsonrpc_message(&message).unwrap();
    assert!(framed.len() > 128 * 1024);

    let mut decoder = JsonRpcMessageDecoder::client();
    let mut decoded = Vec::new();
    for record in framed.chunks(NOISE_RECORD_PLAINTEXT_LEN) {
        decoded.extend(decoder.push(record).unwrap());
    }

    assert_eq!(decoded, vec![message]);
}

#[test]
fn rejects_declared_message_length_above_limit_without_payload() {
    let mut decoder = JsonRpcMessageDecoder::default();
    let declared_len = (MAX_NOISE_JSONRPC_MESSAGE_LEN as u32 + 1).to_be_bytes();

    assert!(matches!(
        decoder.push(&declared_len),
        Err(ExecServerError::Protocol(message))
            if message == "Noise relay JSON-RPC message has invalid length"
    ));
}

#[test]
fn rejects_oversized_plaintext_record() {
    let mut decoder = JsonRpcMessageDecoder::default();

    assert!(matches!(
        decoder.push(&vec![0; NOISE_RECORD_PLAINTEXT_LEN + 1]),
        Err(ExecServerError::Protocol(message))
            if message == "Noise relay plaintext record exceeds maximum length"
    ));
}

#[test]
fn client_decoder_rejects_oversized_request_and_preserves_large_response() {
    let request = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "network/policyRequest".to_string(),
        params: Some(serde_json::json!({
            "padding": "x".repeat(MAX_CLIENT_INBOUND_REQUEST_LEN),
        })),
        trace: None,
    });
    let request = frame_jsonrpc_message(&request).unwrap();
    let mut client = JsonRpcMessageDecoder::client();
    assert!(matches!(
        client.push(&request),
        Err(ExecServerError::Protocol(message))
            if message == format!(
                "Noise relay JSON-RPC message exceeds maximum length of {MAX_CLIENT_INBOUND_REQUEST_LEN} bytes"
        )
    ));

    let mut client = JsonRpcMessageDecoder::client();
    let split = MAX_CLIENT_INBOUND_REQUEST_LEN / 2;
    assert!(client.push(&request[..split]).unwrap().is_empty());
    assert!(matches!(
        client.push(&request[split..]),
        Err(ExecServerError::Protocol(message))
            if message == format!(
                "Noise relay JSON-RPC message exceeds maximum length of {MAX_CLIENT_INBOUND_REQUEST_LEN} bytes"
            )
    ));

    let response = JSONRPCMessage::Response(JSONRPCResponse {
        id: RequestId::Integer(1),
        result: serde_json::json!({
            "padding": "x".repeat(MAX_CLIENT_INBOUND_REQUEST_LEN),
        }),
    });
    let framed = frame_jsonrpc_message(&response).unwrap();
    let mut client = JsonRpcMessageDecoder::client();
    assert_eq!(client.push(&framed).unwrap(), vec![response]);
}

#[test]
fn executor_decoder_preserves_large_request() {
    let request = JSONRPCMessage::Request(JSONRPCRequest {
        id: RequestId::Integer(1),
        method: "process/start".to_string(),
        params: Some(serde_json::json!({
            "padding": "x".repeat(MAX_CLIENT_INBOUND_REQUEST_LEN),
        })),
        trace: None,
    });
    let framed = frame_jsonrpc_message(&request).unwrap();
    let mut executor = JsonRpcMessageDecoder::default();
    assert_eq!(executor.push(&framed).unwrap(), vec![request]);
}

#[test]
fn reassembles_many_messages_from_one_record() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "small/test".to_string(),
        params: None,
    });
    let framed = frame_jsonrpc_message(&message).expect("frame message");
    let message_count = NOISE_RECORD_PLAINTEXT_LEN / framed.len();
    let record = framed.repeat(message_count);

    let mut decoder = JsonRpcMessageDecoder::default();
    let decoded = decoder.push(&record).expect("decode record");

    assert_eq!(decoded, vec![message; message_count]);
}
