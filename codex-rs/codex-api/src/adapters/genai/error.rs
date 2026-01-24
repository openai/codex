//! Error mapping from google-genai to codex-api errors.

use crate::error::ApiError;
use google_genai::error::GenAiError;
use http::StatusCode;
use std::time::Duration;

/// Map a GenAiError to an ApiError.
pub fn map_error(err: GenAiError) -> ApiError {
    match err {
        GenAiError::ContextLengthExceeded(_) => ApiError::ContextWindowExceeded,

        GenAiError::QuotaExceeded(_) => ApiError::QuotaExceeded,

        GenAiError::Api {
            code,
            message,
            status,
        } => {
            // Check for retryable errors
            if code == 429 || code >= 500 {
                return ApiError::Retryable {
                    message: format!("[{code}] {status}: {message}"),
                    delay: if code == 429 {
                        Some(Duration::from_secs(1))
                    } else {
                        None
                    },
                };
            }

            // Map specific API errors
            let status_code =
                StatusCode::from_u16(code as u16).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            ApiError::Api {
                status: status_code,
                message: format!("{status}: {message}"),
            }
        }

        GenAiError::Network(msg) => {
            // Network errors are generally retryable
            ApiError::Retryable {
                message: msg,
                delay: Some(Duration::from_millis(500)),
            }
        }

        GenAiError::Configuration(msg) => ApiError::Api {
            status: StatusCode::BAD_REQUEST,
            message: format!("Configuration error: {msg}"),
        },

        GenAiError::Parse(msg) => ApiError::Stream(format!("Parse error: {msg}")),

        GenAiError::Validation(msg) => ApiError::Api {
            status: StatusCode::BAD_REQUEST,
            message: format!("Validation error: {msg}"),
        },

        GenAiError::ContentBlocked(msg) => ApiError::Api {
            status: StatusCode::FORBIDDEN,
            message: format!("Content blocked: {msg}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_map_context_length_exceeded() {
        let err = GenAiError::ContextLengthExceeded("too long".to_string());
        assert!(matches!(map_error(err), ApiError::ContextWindowExceeded));
    }

    #[test]
    fn test_map_quota_exceeded() {
        let err = GenAiError::QuotaExceeded("rate limit".to_string());
        assert!(matches!(map_error(err), ApiError::QuotaExceeded));
    }

    #[test]
    fn test_map_rate_limit_api_error() {
        let err = GenAiError::Api {
            code: 429,
            message: "Too many requests".to_string(),
            status: "RESOURCE_EXHAUSTED".to_string(),
        };
        match map_error(err) {
            ApiError::Retryable { delay, .. } => {
                assert!(delay.is_some());
            }
            other => panic!("expected Retryable, got {other:?}"),
        }
    }

    #[test]
    fn test_map_server_error() {
        let err = GenAiError::Api {
            code: 503,
            message: "Service unavailable".to_string(),
            status: "UNAVAILABLE".to_string(),
        };
        assert!(matches!(map_error(err), ApiError::Retryable { .. }));
    }

    #[test]
    fn test_map_client_error() {
        let err = GenAiError::Api {
            code: 400,
            message: "Bad request".to_string(),
            status: "INVALID_ARGUMENT".to_string(),
        };
        match map_error(err) {
            ApiError::Api { status, .. } => {
                assert_eq!(status, StatusCode::BAD_REQUEST);
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }

    #[test]
    fn test_map_network_error() {
        let err = GenAiError::Network("connection reset".to_string());
        assert!(matches!(map_error(err), ApiError::Retryable { .. }));
    }

    #[test]
    fn test_map_content_blocked() {
        let err = GenAiError::ContentBlocked("safety filter".to_string());
        match map_error(err) {
            ApiError::Api { status, message } => {
                assert_eq!(status, StatusCode::FORBIDDEN);
                assert!(message.contains("Content blocked"));
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }
}
