//! Error mapping from anthropic-sdk to codex-api errors.

use crate::error::ApiError;
use anthropic_sdk::AnthropicError;
use http::StatusCode;
use std::time::Duration;

/// Map an AnthropicError to an ApiError.
pub fn map_error(err: AnthropicError) -> ApiError {
    match err {
        AnthropicError::RateLimited { retry_after } => ApiError::Retryable {
            message: "Rate limit exceeded".to_string(),
            delay: retry_after.or(Some(Duration::from_secs(1))),
        },

        AnthropicError::InternalServerError => ApiError::Retryable {
            message: "Internal server error".to_string(),
            delay: Some(Duration::from_secs(2)),
        },

        AnthropicError::Network(e) => ApiError::Retryable {
            message: format!("Network error: {e}"),
            delay: Some(Duration::from_millis(500)),
        },

        AnthropicError::Authentication(msg) => ApiError::Api {
            status: StatusCode::UNAUTHORIZED,
            message: msg,
        },

        AnthropicError::PermissionDenied(msg) => ApiError::Api {
            status: StatusCode::FORBIDDEN,
            message: msg,
        },

        AnthropicError::BadRequest(msg) => {
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

        AnthropicError::Api {
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

        AnthropicError::Configuration(msg) => ApiError::Api {
            status: StatusCode::BAD_REQUEST,
            message: format!("Configuration error: {msg}"),
        },

        AnthropicError::Validation(msg) => ApiError::Api {
            status: StatusCode::BAD_REQUEST,
            message: format!("Validation error: {msg}"),
        },

        AnthropicError::Serialization(e) => ApiError::Stream(format!("Serialization error: {e}")),

        AnthropicError::NotFound(msg) => ApiError::Api {
            status: StatusCode::NOT_FOUND,
            message: msg,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_map_rate_limited() {
        let err = AnthropicError::RateLimited {
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
        let err = AnthropicError::InternalServerError;
        assert!(matches!(map_error(err), ApiError::Retryable { .. }));
    }

    #[test]
    fn test_map_authentication_error() {
        let err = AnthropicError::Authentication("Invalid API key".to_string());
        match map_error(err) {
            ApiError::Api { status, .. } => {
                assert_eq!(status, StatusCode::UNAUTHORIZED);
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }

    #[test]
    fn test_map_context_window_exceeded() {
        let err = AnthropicError::BadRequest("context length exceeded".to_string());
        assert!(matches!(map_error(err), ApiError::ContextWindowExceeded));
    }

    #[test]
    fn test_map_quota_exceeded() {
        let err = AnthropicError::Api {
            status: 402,
            message: "insufficient quota".to_string(),
            request_id: None,
        };
        assert!(matches!(map_error(err), ApiError::QuotaExceeded));
    }

    #[test]
    fn test_map_server_error() {
        let err = AnthropicError::Api {
            status: 503,
            message: "Service unavailable".to_string(),
            request_id: None,
        };
        assert!(matches!(map_error(err), ApiError::Retryable { .. }));
    }

    #[test]
    fn test_map_network_error() {
        let err = AnthropicError::Network(
            reqwest::Client::new()
                .get("invalid://")
                .build()
                .unwrap_err(),
        );
        assert!(matches!(map_error(err), ApiError::Retryable { .. }));
    }
}
