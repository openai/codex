use codex_api::ApiError;
use codex_api::TransportError;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use http::StatusCode;

pub(super) const CREDENTIAL_EXPORT_CONFIG_ERROR_PREFIX: &str =
    "AWS credential export configuration error:";

pub(super) const BEDROCK_EXPIRED_SIGNATURE_MESSAGE: &str = concat!(
    "Amazon Bedrock rejected the request because its AWS signature has expired. ",
    "Refresh your AWS credentials and retry. If `AWS_BEARER_TOKEN_BEDROCK` is set, ",
    "update or unset it, then restart Codex",
);

pub(super) fn map_api_error(error: ApiError) -> CodexErr {
    if let ApiError::Transport(TransportError::Build(message)) = &error
        && message.starts_with(CREDENTIAL_EXPORT_CONFIG_ERROR_PREFIX)
    {
        return CodexErr::InvalidRequest(message.clone());
    }
    let error = codex_api::map_api_error(error);
    if let CodexErrorDetails::UnexpectedStatus(response) = error.details()
        && response.status == StatusCode::UNAUTHORIZED
        && response.body.contains("Signature expired:")
    {
        let mut response = response.clone();
        response.user_message = Some(BEDROCK_EXPIRED_SIGNATURE_MESSAGE.to_string());
        let mapped_error = CodexErr::new(CodexErrorDetails::UnexpectedStatus(response));
        return match error.retry_after() {
            Some(retry_after) => mapped_error.with_retry_after(retry_after),
            None => mapped_error,
        };
    }
    error
}

pub(super) fn is_refreshable_auth_error(error: &TransportError) -> bool {
    match error {
        TransportError::Build(message) | TransportError::Network(message) => {
            message.starts_with("failed to load AWS credentials:")
        }
        TransportError::Http { status, .. } if *status == StatusCode::UNAUTHORIZED => true,
        TransportError::Http {
            status,
            body: Some(body),
            ..
        } if *status == StatusCode::FORBIDDEN => {
            let body = body.to_ascii_lowercase();
            [
                "expiredtoken",
                "unrecognizedclientexception",
                "invalidclienttokenid",
            ]
            .iter()
            .any(|error_code| body.contains(error_code))
        }
        _ => false,
    }
}
