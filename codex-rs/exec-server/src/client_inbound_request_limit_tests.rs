use codex_exec_server_protocol::EXEC_CLOSED_METHOD;
use codex_exec_server_protocol::EXEC_EXITED_METHOD;
use codex_exec_server_protocol::EXEC_OUTPUT_DELTA_METHOD;
use codex_exec_server_protocol::HTTP_REQUEST_BODY_DELTA_METHOD;
use codex_exec_server_protocol::HttpRequestBodyDeltaNotification;
use codex_exec_server_protocol::JSONRPCMessage;
use codex_exec_server_protocol::MAX_HTTP_BODY_DELTA_BYTES;

use super::MAX_CLIENT_INBOUND_NOTIFICATION_LEN;
use super::MAX_CLIENT_INBOUND_REQUEST_LEN;
use super::client_inbound_message_exceeded_limit;

#[test]
fn rejects_oversized_request_hidden_in_raw_value_wrapper() {
    let request = serde_json::json!({
        "method": "network/policyRequest",
        "id": 1,
        "params": { "padding": "x".repeat(MAX_CLIENT_INBOUND_REQUEST_LEN) },
    });
    let encoded = serde_json::to_vec(&serde_json::json!({
        "$serde_json::private::RawValue": request.to_string(),
    }))
    .expect("wrapped request should serialize");

    let message = serde_json::from_slice::<JSONRPCMessage>(&encoded);
    assert!(matches!(&message, Ok(JSONRPCMessage::Request(_))));
    assert_eq!(
        client_inbound_message_exceeded_limit(
            message.as_ref(),
            encoded.len(),
            MAX_CLIENT_INBOUND_REQUEST_LEN,
        ),
        Some(MAX_CLIENT_INBOUND_REQUEST_LEN)
    );
}

#[test]
fn bounds_notifications_without_rejecting_streamed_http_bodies() {
    let streamed_body = serde_json::to_vec(&serde_json::json!({
        "method": HTTP_REQUEST_BODY_DELTA_METHOD,
        "params": HttpRequestBodyDeltaNotification {
            request_id: "request".to_string(),
            seq: 1,
            delta: vec![0; MAX_HTTP_BODY_DELTA_BYTES].into(),
            done: false,
            error: None,
        },
    }))
    .expect("streamed body notification should serialize");
    let message = serde_json::from_slice::<JSONRPCMessage>(&streamed_body);
    assert_eq!(
        client_inbound_message_exceeded_limit(
            message.as_ref(),
            streamed_body.len(),
            MAX_CLIENT_INBOUND_REQUEST_LEN,
        ),
        None
    );

    for method in [
        EXEC_OUTPUT_DELTA_METHOD,
        EXEC_EXITED_METHOD,
        EXEC_CLOSED_METHOD,
    ] {
        let notification = serde_json::to_vec(&serde_json::json!({
            "method": method,
            "params": { "padding": "x".repeat(MAX_CLIENT_INBOUND_REQUEST_LEN) },
        }))
        .expect("process notification should serialize");
        let message = serde_json::from_slice::<JSONRPCMessage>(&notification);
        assert_eq!(
            client_inbound_message_exceeded_limit(
                message.as_ref(),
                notification.len(),
                MAX_CLIENT_INBOUND_REQUEST_LEN,
            ),
            None
        );
    }

    let oversized_notification = serde_json::to_vec(&serde_json::json!({
        "method": "hostile/notification",
        "params": { "padding": "x".repeat(MAX_CLIENT_INBOUND_REQUEST_LEN) },
    }))
    .expect("oversized notification should serialize");
    assert!(oversized_notification.len() < MAX_CLIENT_INBOUND_NOTIFICATION_LEN);
    let message = serde_json::from_slice::<JSONRPCMessage>(&oversized_notification);
    assert_eq!(
        client_inbound_message_exceeded_limit(
            message.as_ref(),
            oversized_notification.len(),
            MAX_CLIENT_INBOUND_REQUEST_LEN,
        ),
        Some(MAX_CLIENT_INBOUND_REQUEST_LEN)
    );
}

#[test]
fn preserves_oversized_responses() {
    let response = serde_json::to_vec(&serde_json::json!({
        "id": 1,
        "result": { "padding": "x".repeat(MAX_CLIENT_INBOUND_NOTIFICATION_LEN) },
    }))
    .expect("response should serialize");
    let message = serde_json::from_slice::<JSONRPCMessage>(&response);

    assert_eq!(
        client_inbound_message_exceeded_limit(
            message.as_ref(),
            response.len(),
            MAX_CLIENT_INBOUND_REQUEST_LEN,
        ),
        None
    );
}
