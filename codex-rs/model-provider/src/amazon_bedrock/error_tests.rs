use std::num::NonZeroU64;

use codex_api::ApiError;
use codex_api::TransportError;
use codex_model_provider_info::AwsCredentialExportConfig;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErrorDetails;
use http::HeaderMap;
use http::HeaderValue;
use http::StatusCode;
use pretty_assertions::assert_eq;

use super::AmazonBedrockModelProvider;
use super::error::BEDROCK_EXPIRED_SIGNATURE_MESSAGE;
use super::error::CREDENTIAL_EXPORT_CONFIG_ERROR_PREFIX;
use super::error::is_refreshable_auth_error;
use super::error::map_api_error;
use crate::ModelProvider;

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

#[test]
fn classifies_only_refreshable_bedrock_auth_failures() {
    let cases = [
        (
            StatusCode::UNAUTHORIZED,
            Some("ExpiredTokenException"),
            true,
        ),
        (
            StatusCode::UNAUTHORIZED,
            Some("AccessDeniedException"),
            true,
        ),
        (StatusCode::UNAUTHORIZED, Some(""), true),
        (StatusCode::UNAUTHORIZED, None, true),
        (StatusCode::FORBIDDEN, Some("ExpiredTokenException"), true),
        (
            StatusCode::FORBIDDEN,
            Some("UnrecognizedClientException"),
            true,
        ),
        (StatusCode::FORBIDDEN, Some("InvalidClientTokenId"), true),
        (StatusCode::FORBIDDEN, Some("AccessDeniedException"), false),
        (
            StatusCode::FORBIDDEN,
            Some("The security token included in the request is invalid"),
            false,
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            Some("ExpiredTokenException"),
            false,
        ),
        (StatusCode::BAD_REQUEST, Some("RequestExpired"), false),
    ];

    for (status, body, expected) in cases {
        let error = TransportError::Http {
            status,
            url: Some(BEDROCK_RESPONSES_URL.to_string()),
            headers: None,
            body: body.map(str::to_string),
        };
        assert_eq!(
            is_refreshable_auth_error(&error),
            expected,
            "{status}: {body:?}"
        );
    }

    for (error, expected) in [
        (
            TransportError::Build("failed to load AWS credentials: expired".to_string()),
            true,
        ),
        (
            TransportError::Network("failed to load AWS credentials: unavailable".to_string()),
            true,
        ),
        (
            TransportError::Build("request URL is not a valid URI".to_string()),
            false,
        ),
    ] {
        assert_eq!(is_refreshable_auth_error(&error), expected, "{error}");
    }
}

#[test]
fn credential_export_errors_distinguish_configuration_failures_from_retryable_auth() {
    let provider = AmazonBedrockModelProvider::new(
        ModelProviderInfo::create_amazon_bedrock_provider(Some(ModelProviderAwsAuthInfo {
            profile: None,
            region: Some("us-west-2".to_string()),
            credential_export: Some(AwsCredentialExportConfig {
                command: "export-credentials".to_string(),
                args: Vec::new(),
                timeout_ms: NonZeroU64::new(5_000).expect("non-zero timeout"),
            }),
            auth_refresh: None,
        })),
        /*auth_manager*/ None,
    );
    for (error, recoverable, retryable) in [
        (
            ApiError::Transport(TransportError::Build(format!(
                "{CREDENTIAL_EXPORT_CONFIG_ERROR_PREFIX} invalid credentials JSON"
            ))),
            false,
            false,
        ),
        (
            ApiError::Transport(TransportError::Build(
                "failed to load AWS credentials: export command exited with status 1".to_string(),
            )),
            true,
            true,
        ),
        (
            http_error(StatusCode::UNAUTHORIZED, "ExpiredTokenException"),
            true,
            true,
        ),
        (
            http_error(StatusCode::FORBIDDEN, "ExpiredTokenException"),
            true,
            true,
        ),
        (
            http_error(StatusCode::FORBIDDEN, "AccessDeniedException"),
            false,
            true,
        ),
        (
            ApiError::Transport(TransportError::Network("offline".to_string())),
            false,
            true,
        ),
    ] {
        let ApiError::Transport(transport) = &error else {
            unreachable!("all test cases are transport errors");
        };
        assert_eq!(provider.is_recoverable_auth_error(transport), recoverable);
        assert_eq!(
            provider
                .map_api_error(error)
                .retry_delay(/*retry_count*/ 1)
                .is_some(),
            retryable
        );
    }
}
