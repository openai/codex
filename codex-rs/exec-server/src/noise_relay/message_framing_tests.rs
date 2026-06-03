use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use pretty_assertions::assert_eq;

use super::JsonRpcMessageDecoder;
use super::MAX_NOISE_JSONRPC_MESSAGE_LEN;
use super::NOISE_RECORD_PLAINTEXT_LEN;
use super::frame_jsonrpc_message;
use crate::ExecServerError;

#[test]
fn fragments_and_reassembles_large_jsonrpc_message() {
    let message = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "large/test".to_string(),
        params: Some(serde_json::json!({
            "data": "x".repeat(128 * 1024),
        })),
    });
    let framed = frame_jsonrpc_message(&message).unwrap();
    assert!(framed.len() > 128 * 1024);

    let mut decoder = JsonRpcMessageDecoder::default();
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
