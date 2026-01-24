//! Error mapping from openai-sdk to codex-api errors.

use crate::error::ApiError;
use http::StatusCode;
use openai_sdk::OpenAIError;
use std::time::Duration;

/// Map an OpenAIError to an ApiError.
pub fn map_error(err: OpenAIError) -> ApiError {
    match err {
        OpenAIError::RateLimited { retry_after } => ApiError::Retryable {
            message: "Rate limit exceeded".to_string(),
            delay: retry_after.or(Some(Duration::from_secs(1))),
        },

        OpenAIError::InternalServerError => ApiError::Retryable {
            message: "Internal server error".to_string(),
            delay: Some(Duration::from_secs(2)),
        },

        OpenAIError::Network(e) => ApiError::Retryable {
            message: format!("Network error: {e}"),
            delay: Some(Duration::from_millis(500)),
        },

        OpenAIError::Authentication(msg) => ApiError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: msg,
        },

        OpenAIError::BadRequest(msg) => {
            // Check for context window errors
            if msg.contains("context")
                || msg.contains("token")
                || msg.contains("too long")
                || msg.contains("maximum")
            {
                ApiError::ContextWindowExceeded
            } else {
                ApiError::Api {
                    status: StatusCode::BAD_REQUEST,
                    message: msg,
                }
            }
        }

        OpenAIError::ContextWindowExceeded => ApiError::ContextWindowExceeded,

        OpenAIError::QuotaExceeded => ApiError::QuotaExceeded,

        OpenAIError::PreviousResponseNotFound => ApiError::PreviousResponseNotFound,

        OpenAIError::Api {
            status,
            message,
            request_id: _,
        } => {
            // Check for retryable status codes
            if status >= 500 || status == 429 {
                return ApiError::Retryable {
                    message: format!("[{status}] {message}"),
                    delay: if status == 429 {
                        Some(Duration::from_secs(1))
                    } else {
                        Some(Duration::from_secs(2))
                    },
                };
            }

            // Check for context window exceeded in message
            if message.contains("context")
                || message.contains("max_tokens")
                || message.contains("too long")
            {
                return ApiError::ContextWindowExceeded;
            }

            // Check for quota exceeded
            if message.contains("quota") || message.contains("insufficient") {
                return ApiError::QuotaExceeded;
            }

            ApiError::Api {
                status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                message,
            }
        }

        OpenAIError::Configuration(msg) => ApiError::Api {
            status: StatusCode::BAD_REQUEST,
            message: format!("Configuration error: {msg}"),
        },

        OpenAIError::Validation(msg) => ApiError::Api {
            status: StatusCode::BAD_REQUEST,
            message: format!("Validation error: {msg}"),
        },

        OpenAIError::Serialization(e) => ApiError::Stream(format!("Serialization error: {e}")),

        OpenAIError::Parse(msg) => ApiError::Stream(format!("Parse error: {msg}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_map_rate_limited() {
        let err = OpenAIError::RateLimited {
            retry_after: Some(Duration::from_secs(5)),
        };
        match map_error(err) {
            ApiError::Retryable { delay, .. } => {
                assert_eq!(delay, Some(Duration::from_secs(5)));
            }
            other => panic!("expected Retryable, got {other:?}"),
        }
    }

    #[test]
    fn test_map_internal_server_error() {
        let err = OpenAIError::InternalServerError;
        assert!(matches!(map_error(err), ApiError::Retryable { .. }));
    }

    #[test]
    fn test_map_authentication_error() {
        let err = OpenAIError::Authentication("Invalid API key".to_string());
        match map_error(err) {
            ApiError::Api { status, .. } => {
                assert_eq!(status, StatusCode::UNAUTHORIZED);
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }

    #[test]
    fn test_map_context_window_exceeded() {
        let err = OpenAIError::ContextWindowExceeded;
        assert!(matches!(map_error(err), ApiError::ContextWindowExceeded));
    }

    #[test]
    fn test_map_quota_exceeded() {
        let err = OpenAIError::QuotaExceeded;
        assert!(matches!(map_error(err), ApiError::QuotaExceeded));
    }

    #[test]
    fn test_map_previous_response_not_found() {
        let err = OpenAIError::PreviousResponseNotFound;
        assert!(matches!(map_error(err), ApiError::PreviousResponseNotFound));
    }

    #[test]
    fn test_map_server_error() {
        let err = OpenAIError::Api {
            status: 503,
            message: "Service unavailable".to_string(),
            request_id: None,
        };
        assert!(matches!(map_error(err), ApiError::Retryable { .. }));
    }
}
