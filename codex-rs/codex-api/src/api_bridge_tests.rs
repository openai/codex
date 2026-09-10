use super::*;
use base64::Engine;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::RateLimitReachedType;
use pretty_assertions::assert_eq;

#[test]
fn map_api_error_maps_server_overloaded() {
    let err = map_api_error(ApiError::ServerOverloaded);
    assert!(matches!(err.details(), CodexErrorDetails::ServerOverloaded));
}

#[test]
fn map_api_error_preserves_retry_delay() {
    let retry_delay = std::time::Duration::from_secs(17);
    for (error, expected_code, expected_message) in [
        (
            ApiError::Retryable {
                message: "retry later".to_string(),
                delay: Some(retry_delay),
            },
            CodexErrorInfo::Other,
            "stream disconnected before completion: retry later",
        ),
        (
            ApiError::RateLimitExceeded {
                message: "retry later".to_string(),
                delay: Some(retry_delay),
            },
            CodexErrorInfo::RateLimitExceeded,
            "rate limit exceeded: retry later",
        ),
    ] {
        let err = map_api_error(error);
        assert_eq!(
            (
                err.to_codex_protocol_error(),
                err.retry_delay(),
                err.is_retryable(),
                err.http_status_code_value(),
                err.to_string(),
            ),
            (
                expected_code,
                Some(retry_delay),
                true,
                None,
                expected_message.to_string(),
            )
        );
    }
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

    assert!(matches!(err.details(), CodexErrorDetails::ServerOverloaded));
}

#[test]
fn map_api_error_maps_cloudflare_blocked_response_to_user_message() {
    let mut headers = HeaderMap::new();
    headers.insert(CF_RAY_HEADER, http::HeaderValue::from_static("ray-id"));
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::FORBIDDEN,
        url: Some("http://example.com/blocked".to_string()),
        headers: Some(headers),
        body: Some(
            "<html><body>Cloudflare error: Sorry, you have been blocked</body></html>".to_string(),
        ),
    }));

    let CodexErrorDetails::UnexpectedStatus(err) = err.details() else {
        panic!("expected CodexErrorDetails::UnexpectedStatus, got {err:?}");
    };
    assert_eq!(
        err.user_message.as_deref(),
        Some(
            "Access blocked by Cloudflare. This usually happens when connecting from a restricted region (status 403 Forbidden)"
        )
    );
    assert_eq!(
        err.to_string(),
        "Access blocked by Cloudflare. This usually happens when connecting from a restricted region (status 403 Forbidden), url: http://example.com/blocked, cf-ray: ray-id"
    );
}

#[test]
fn map_api_error_maps_cyber_policy_from_400_body() {
    let body = serde_json::json!({
        "error": {
            "message": "This request has been flagged for potentially high-risk cyber activity.",
            "type": "invalid_request",
            "param": null,
            "code": "cyber_policy"
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::BAD_REQUEST,
        url: Some("http://example.com/v1/responses".to_string()),
        headers: None,
        body: Some(body),
    }));

    let CodexErrorDetails::CyberPolicy { message } = err.details() else {
        panic!("expected CodexErrorDetails::CyberPolicy, got {err:?}");
    };
    assert_eq!(
        message,
        "This request has been flagged for potentially high-risk cyber activity."
    );
}

#[test]
fn map_api_error_maps_wrapped_websocket_cyber_policy_from_400_body() {
    let body = serde_json::json!({
        "type": "error",
        "status": 400,
        "error": {
            "message": "This websocket request was flagged.",
            "type": "invalid_request",
            "code": "cyber_policy"
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::BAD_REQUEST,
        url: Some("ws://example.com/v1/responses".to_string()),
        headers: None,
        body: Some(body),
    }));

    let CodexErrorDetails::CyberPolicy { message } = err.details() else {
        panic!("expected CodexErrorDetails::CyberPolicy, got {err:?}");
    };
    assert_eq!(message, "This websocket request was flagged.");
}

#[test]
fn map_api_error_uses_cyber_policy_fallback_for_missing_message() {
    let body = serde_json::json!({
        "error": {
            "code": "cyber_policy"
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::BAD_REQUEST,
        url: Some("http://example.com/v1/responses".to_string()),
        headers: None,
        body: Some(body),
    }));

    let CodexErrorDetails::CyberPolicy { message } = err.details() else {
        panic!("expected CodexErrorDetails::CyberPolicy, got {err:?}");
    };
    assert_eq!(
        message,
        "This request has been flagged for possible cybersecurity risk."
    );
}

#[test]
fn map_api_error_maps_misalignment_policy_violation_from_400_body() {
    assert_misalignment_policy_violation_from_http_body(http::StatusCode::BAD_REQUEST);
}

#[test]
fn map_api_error_maps_misalignment_policy_violation_from_403_body() {
    assert_misalignment_policy_violation_from_http_body(http::StatusCode::FORBIDDEN);
}

fn assert_misalignment_policy_violation_from_http_body(status: http::StatusCode) {
    let body = serde_json::json!({
        "error": {
            "message": "This request violated the misalignment policy.",
            "type": "invalid_request_error",
            "code": "misalignment_policy_violation"
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status,
        url: Some("http://example.com/v1/responses".to_string()),
        headers: None,
        body: Some(body),
    }));

    let CodexErrorDetails::MisalignmentPolicyViolation {
        message,
        misalignment,
    } = err.details()
    else {
        panic!("expected CodexErrorDetails::MisalignmentPolicyViolation, got {err:?}");
    };
    assert_eq!(message, "This request violated the misalignment policy.");
    assert_eq!(misalignment, &None);
    assert!(!err.is_retryable());
}

#[test]
fn map_api_error_preserves_misalignment_details_from_403_body() {
    let body = serde_json::json!({
        "error": {
            "message": "This request violated the misalignment policy.",
            "code": "misalignment_policy_violation",
            "misalignment": {
                "error_type": "unauthorized_data_transfer",
                "detailed_explanation": "The agent attempted an external transfer.",
                "steer": { "message": "Do not transfer the user's files." }
            }
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::FORBIDDEN,
        url: Some("http://example.com/v1/responses".to_string()),
        headers: None,
        body: Some(body),
    }));

    let CodexErrorDetails::MisalignmentPolicyViolation {
        message,
        misalignment,
    } = err.details()
    else {
        panic!("expected CodexErrorDetails::MisalignmentPolicyViolation, got {err:?}");
    };
    assert_eq!(message, "This request violated the misalignment policy.");
    assert_eq!(
        misalignment,
        &Some(MisalignmentErrorDetails {
            error_type: Some("unauthorized_data_transfer".to_string()),
            detailed_explanation: Some("The agent attempted an external transfer.".to_string()),
            steer: Some(codex_protocol::protocol::MisalignmentSteer {
                message: "Do not transfer the user's files.".to_string(),
            }),
        })
    );
    assert!(!err.is_retryable());
}

#[test]
fn map_api_error_preserves_misalignment_details_from_wrapped_websocket_error() {
    let body = serde_json::json!({
        "type": "error",
        "status": 403,
        "error": {
            "message": "This websocket request violated the misalignment policy.",
            "code": "misalignment_policy_violation",
            "misalignment": {
                "error_type": "future_safety_category",
                "detailed_explanation": "The agent attempted an external transfer.",
                "steer": { "message": "Do not transfer the user's files." }
            }
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::FORBIDDEN,
        url: Some("ws://example.com/v1/responses".to_string()),
        headers: None,
        body: Some(body),
    }));

    let CodexErrorDetails::MisalignmentPolicyViolation {
        message,
        misalignment,
    } = err.details()
    else {
        panic!("expected CodexErrorDetails::MisalignmentPolicyViolation, got {err:?}");
    };
    assert_eq!(
        message,
        "This websocket request violated the misalignment policy."
    );
    assert_eq!(
        misalignment,
        &Some(MisalignmentErrorDetails {
            error_type: Some("future_safety_category".to_string()),
            detailed_explanation: Some("The agent attempted an external transfer.".to_string()),
            steer: Some(codex_protocol::protocol::MisalignmentSteer {
                message: "Do not transfer the user's files.".to_string(),
            }),
        })
    );
    assert!(!err.is_retryable());
}

#[test]
fn map_api_error_keeps_unknown_400_errors_generic() {
    let body = serde_json::json!({
        "error": {
            "message": "Some other bad request.",
            "code": "some_other_policy"
        }
    })
    .to_string();
    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::BAD_REQUEST,
        url: Some("http://example.com/v1/responses".to_string()),
        headers: None,
        body: Some(body.clone()),
    }));

    let CodexErrorDetails::InvalidRequest(message) = err.details() else {
        panic!("expected CodexErrorDetails::InvalidRequest, got {err:?}");
    };
    assert_eq!(message, &body);
}

#[test]
fn map_api_error_distinguishes_http_quota_errors_from_rate_limits() {
    for error in [
        serde_json::json!({"type": "insufficient_quota"}),
        serde_json::json!({"code": "insufficient_quota"}),
        serde_json::json!({"code": "credit_balance_exhausted"}),
        serde_json::json!({"code": "organization_spend_limit_exceeded"}),
        serde_json::json!({"code": "project_spend_limit_exceeded"}),
        serde_json::json!({"code": "organization_usage_limit_exceeded"}),
        serde_json::json!({"type": "rate_limit_error", "code": "rate_limit_exceeded"}),
        serde_json::json!({"type": "rate_limit_error", "code": "slow_down"}),
    ] {
        let expected = if error["type"] == "rate_limit_error" {
            CodexErrorInfo::ResponseTooManyFailedAttempts {
                http_status_code: Some(429),
            }
        } else {
            CodexErrorInfo::UsageLimitExceeded
        };
        let err = map_api_error(ApiError::Transport(TransportError::Http {
            status: http::StatusCode::TOO_MANY_REQUESTS,
            url: None,
            headers: None,
            body: Some(serde_json::json!({"error": error}).to_string()),
        }));

        assert_eq!(err.to_codex_protocol_error(), expected, "{error}");
    }
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

    let CodexErrorDetails::UsageLimitReached(usage_limit) = err.details() else {
        panic!("expected CodexErrorDetails::UsageLimitReached, got {err:?}");
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

    let CodexErrorDetails::UsageLimitReached(usage_limit) = err.details() else {
        panic!("expected CodexErrorDetails::UsageLimitReached, got {err:?}");
    };
    assert_eq!(
        usage_limit
            .rate_limits
            .as_ref()
            .and_then(|snapshot| snapshot.limit_name.as_deref()),
        None
    );
}

#[test]
fn map_api_error_copies_rate_limit_reached_type_to_usage_limit_snapshot() {
    for (active_limit, expected_limit_id) in [(None, "codex"), (Some("codex_other"), "codex_other")]
    {
        let mut headers = HeaderMap::new();
        if let Some(active_limit) = active_limit {
            headers.insert(
                ACTIVE_LIMIT_HEADER,
                http::HeaderValue::from_static(active_limit),
            );
        }
        for (name, value) in [
            ("x-codex-credits-has-credits", "true"),
            ("x-codex-credits-unlimited", "false"),
            ("x-codex-credits-balance", ""),
            (
                "x-codex-rate-limit-reached-type",
                "workspace_member_usage_limit_reached",
            ),
        ] {
            headers.insert(name, http::HeaderValue::from_static(value));
        }
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

        let CodexErrorDetails::UsageLimitReached(usage_limit) = err.details() else {
            panic!("expected CodexErrorDetails::UsageLimitReached, got {err:?}");
        };
        assert_eq!(
            usage_limit.rate_limit_reached_type,
            Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached)
        );
        let snapshot = usage_limit
            .rate_limits
            .as_ref()
            .expect("usage limit snapshot");
        assert_eq!(snapshot.limit_id.as_deref(), Some(expected_limit_id));
        assert_eq!(
            snapshot.rate_limit_reached_type,
            Some(RateLimitReachedType::WorkspaceMemberUsageLimitReached)
        );
        assert_eq!(
            snapshot.credits.as_ref().map(|credits| (
                credits.has_credits,
                credits.unlimited,
                credits.balance.as_deref()
            )),
            Some((true, false, None))
        );
    }
}

#[test]
fn map_api_error_ignores_unparseable_rate_limit_reached_type_headers() {
    let values = [
        http::HeaderValue::from_static("future_rate_limit_reached_type"),
        http::HeaderValue::from_bytes(&[0xff]).expect("valid opaque header value"),
    ];

    for value in values {
        let mut headers = HeaderMap::new();
        headers.insert("x-codex-rate-limit-reached-type", value);
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

        let CodexErrorDetails::UsageLimitReached(usage_limit) = err.details() else {
            panic!("expected CodexErrorDetails::UsageLimitReached, got {err:?}");
        };
        assert_eq!(usage_limit.rate_limit_reached_type, None);
    }
}

#[test]
fn map_api_error_extracts_identity_auth_details_from_headers() {
    let mut headers = HeaderMap::new();
    headers.insert(REQUEST_ID_HEADER, http::HeaderValue::from_static("req-401"));
    headers.insert(CF_RAY_HEADER, http::HeaderValue::from_static("ray-401"));
    headers.insert(
        X_OPENAI_AUTHORIZATION_ERROR_HEADER,
        http::HeaderValue::from_static("missing_authorization_header"),
    );
    let x_error_json =
        base64::engine::general_purpose::STANDARD.encode(r#"{"error":{"code":"token_expired"}}"#);
    headers.insert(
        X_ERROR_JSON_HEADER,
        http::HeaderValue::from_str(&x_error_json).expect("valid x-error-json header"),
    );

    let err = map_api_error(ApiError::Transport(TransportError::Http {
        status: http::StatusCode::UNAUTHORIZED,
        url: Some("https://chatgpt.com/backend-api/codex/models".to_string()),
        headers: Some(headers),
        body: Some(r#"{"detail":"Unauthorized"}"#.to_string()),
    }));

    let CodexErrorDetails::UnexpectedStatus(err) = err.details() else {
        panic!("expected CodexErrorDetails::UnexpectedStatus, got {err:?}");
    };
    assert_eq!(err.request_id.as_deref(), Some("req-401"));
    assert_eq!(err.cf_ray.as_deref(), Some("ray-401"));
    assert_eq!(
        err.identity_authorization_error.as_deref(),
        Some("missing_authorization_header")
    );
    assert_eq!(err.identity_error_code.as_deref(), Some("token_expired"));
}
