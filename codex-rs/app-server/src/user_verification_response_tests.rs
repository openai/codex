use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn accepted_device_proof_uses_existing_content_and_discards_metadata() {
    assert_eq!(
        from_client_result(Ok(Ok(json!({
            "action": "accept", "content": {"credentialId": "AQID", "signature": "BAUG"},
            "_meta": {"ignored": "untrusted"},
        })))),
        McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Accept,
            content: Some(json!({"credentialId": "AQID", "signature": "BAUG"})),
            meta: None,
        }
    );
}

#[test]
fn invalid_response_or_accept_without_proof_cancels() {
    for (response, reason) in [
        (json!({"invalid": "response"}), "invalidResponse"),
        (
            json!({"action": "accept", "content": null, "_meta": null}),
            "invalidProof",
        ),
        (
            json!({"action": "accept", "content": {}, "_meta": null}),
            "invalidProof",
        ),
    ] {
        assert_eq!(
            from_client_result(Ok(Ok(response))),
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Cancel,
                content: None,
                meta: Some(json!({"openai/userVerificationReason": reason})),
            }
        );
    }
}

#[test]
fn terminal_response_keeps_only_a_known_reason() {
    for (input, expected) in [
        (
            json!({"openai/userVerificationReason": "timeout", "secret": "discard"}),
            Some(json!({"openai/userVerificationReason": "timeout"})),
        ),
        (
            json!({"openai/userVerificationReason": "arbitrary secret"}),
            None,
        ),
        (json!(null), None),
    ] {
        assert_eq!(
            from_client_result(Ok(Ok(
                json!({"action": "cancel", "content": {"signature": "discard"}, "_meta": input})
            ))),
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Cancel,
                content: None,
                meta: expected
            }
        );
    }
}

#[tokio::test]
async fn runtime_failures_keep_their_reason_without_error_text() {
    for (data, reason) in [
        (Some(json!({"reason": "turnTransition"})), "interrupted"),
        (None, "responseError"),
    ] {
        let response = from_client_result(Ok(Err(codex_app_server_protocol::JSONRPCErrorError {
            code: -1,
            message: "private error details".into(),
            data,
        })));
        assert_eq!(
            response,
            McpServerElicitationRequestResponse {
                action: McpServerElicitationAction::Cancel,
                content: None,
                meta: Some(json!({"openai/userVerificationReason": reason}))
            }
        );
    }
    let (sender, receiver) = oneshot::channel();
    drop(sender);
    assert_eq!(
        from_client_result(receiver.await),
        McpServerElicitationRequestResponse {
            action: McpServerElicitationAction::Cancel,
            content: None,
            meta: Some(json!({"openai/userVerificationReason": "responseChannelClosed"}))
        }
    );
}
