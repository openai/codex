use super::*;
use pretty_assertions::assert_eq;

#[test]
fn map_api_error_maps_server_overloaded() {
    let err = map_api_error(ApiError::ServerOverloaded);
    assert!(matches!(err, CodexErr::ServerOverloaded));
}

#[test]
fn map_api_error_maps_server_overloaded_from_503_body() {
    let body = serde_json::json!({
        "error": {
            "code": "server_is_overloaded"
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::SERVICE_UNAVAILABLE,
        url: Some("http://example.com/v1/responses".to_string()),
        headers: None,
        body: Some(body),
    }));

    assert!(matches!(err, CodexErr::ServerOverloaded));
}

#[test]
fn map_api_error_maps_usage_limit_limit_name_header() {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACTIVE_LIMIT_HEADER,
        http::HeaderValue::from_static("codex_other"),
    );
    headers.insert(
        "x-codex-other-limit-name",
        http::HeaderValue::from_static("codex_other"),
    );
    let body = serde_json::json!({
        "error": {
            "type": "usage_limit_reached",
            "plan_type": "pro",
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::TOO_MANY_REQUESTS,
        url: Some("http://example.com/v1/responses".to_string()),
        headers: Some(headers),
        body: Some(body),
    }));

    let CodexErr::UsageLimitReached(usage_limit) = err else {
        panic!("expected CodexErr::UsageLimitReached, got {err:?}");
    };
    assert_eq!(
        usage_limit
            .rate_limits
            .as_ref()
            .and_then(|snapshot| snapshot.limit_name.as_deref()),
        Some("codex_other")
    );
}

#[test]
fn map_api_error_does_not_fallback_limit_name_to_limit_id() {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACTIVE_LIMIT_HEADER,
        http::HeaderValue::from_static("codex_other"),
    );
    let body = serde_json::json!({
        "error": {
            "type": "usage_limit_reached",
            "plan_type": "pro",
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::TOO_MANY_REQUESTS,
        url: Some("http://example.com/v1/responses".to_string()),
        headers: Some(headers),
        body: Some(body),
    }));

    let CodexErr::UsageLimitReached(usage_limit) = err else {
        panic!("expected CodexErr::UsageLimitReached, got {err:?}");
    };
    assert_eq!(
        usage_limit
            .rate_limits
            .as_ref()
            .and_then(|snapshot| snapshot.limit_name.as_deref()),
        None
    );
}
