use codex_api::ApiError;
use codex_api::TransportError;
use http::StatusCode;
use pretty_assertions::assert_eq;

use super::MAX_ERROR_MESSAGE_CHARS;
use super::image_api_error_message;

#[test]
fn image_api_error_message_extracts_codex_backend_error_envelopes() {
    let cases = [
        (
            r#"{"error":{"message":"bad image request"}}"#,
            "bad image request",
        ),
        (r#"{"detail":"Forbidden"}"#, "Forbidden"),
        (
            r#"{"detail":{"message":"workspace access denied","code":"codex_workspace_access_denied"}}"#,
            "workspace access denied",
        ),
        (
            r#"{"detail":{"message":"current message"},"error":{"message":"legacy message"}}"#,
            "legacy message",
        ),
    ];

    for (body, expected) in cases {
        assert_eq!(
            image_api_error_message(http_error(body.to_string())),
            expected
        );
    }
}

#[test]
fn image_api_error_message_truncates_plain_text_body() {
    let body = "x".repeat(MAX_ERROR_MESSAGE_CHARS + 1);

    let message = image_api_error_message(http_error(body));

    assert_eq!(message.chars().count(), MAX_ERROR_MESSAGE_CHARS);
    assert!(message.ends_with('…'));
}

#[test]
fn image_api_error_message_truncates_json_message() {
    let backend_message = "x".repeat(MAX_ERROR_MESSAGE_CHARS + 1);
    let body = serde_json::json!({
        "error": {
            "message": backend_message,
        },
    })
    .to_string();

    let message = image_api_error_message(http_error(body));

    assert_eq!(message.chars().count(), MAX_ERROR_MESSAGE_CHARS);
    assert!(message.ends_with('…'));
    assert!(!message.contains("error"));
}

fn http_error(body: String) -> ApiError {
    ApiError::Transport(TransportError::Http {
        status: StatusCode::BAD_REQUEST,
        url: None,
        headers: None,
        body: Some(body),
    })
}
