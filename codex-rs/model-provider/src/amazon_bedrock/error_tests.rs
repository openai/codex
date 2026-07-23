use codex_api::ApiError;
use codex_api::TransportError;
use codex_protocol::error::CodexErrorDetails;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use pretty_assertions::assert_eq;

use super::error::BEDROCK_EXPIRED_SIGNATURE_MESSAGE;
use super::error::map_api_error;

const BEDROCK_RESPONSES_URL: &str = "https://bedrock-mantle.us-east-2.api.aws/openai/v1/responses";

fn http_error(status: StatusCode, body: &str) -> ApiError {
    let mut headers = HeaderMap::new();
    headers.insert("x-request-id", HeaderValue::from_static("req-bedrock"));
    ApiError::Transport(TransportError::Http {
        status,
        url: Some(BEDROCK_RESPONSES_URL.to_string()),
        headers: Some(headers),
        body: Some(body.to_string()),
    })
}

#[test]
fn expired_signature_has_actionable_guidance() {
    let error = map_api_error(http_error(
        StatusCode::UNAUTHORIZED,
        "Signature expired: 20260609T133205Z is now earlier than 20260614T062525Z",
    ));

    let CodexErrorDetails::UnexpectedStatus(response) = error.details() else {
        panic!("expected unexpected status error, got {error:?}");
    };
    assert_eq!(
        response.user_message.as_deref(),
        Some(BEDROCK_EXPIRED_SIGNATURE_MESSAGE)
    );
    assert_eq!(
        error.to_string(),
        format!(
            "{BEDROCK_EXPIRED_SIGNATURE_MESSAGE}, url: {BEDROCK_RESPONSES_URL}, request id: req-bedrock"
        )
    );
}

#[test]
fn other_unauthorized_errors_remain_generic() {
    let error = map_api_error(http_error(
        StatusCode::UNAUTHORIZED,
        "The security token included in the request is invalid",
    ));

    let CodexErrorDetails::UnexpectedStatus(response) = error.details() else {
        panic!("expected unexpected status error, got {error:?}");
    };
    assert_eq!(response.user_message, None);
    assert_eq!(
        error.to_string(),
        format!(
            "unexpected status 401 Unauthorized: The security token included in the request is invalid, url: {BEDROCK_RESPONSES_URL}, request id: req-bedrock"
        )
    );
}

#[test]
fn signature_errors_with_other_statuses_remain_generic() {
    let error = map_api_error(http_error(
        StatusCode::FORBIDDEN,
        "Signature expired: old is now earlier than new",
    ));

    let CodexErrorDetails::UnexpectedStatus(response) = error.details() else {
        panic!("expected unexpected status error, got {error:?}");
    };
    assert_eq!(response.user_message, None);
}
