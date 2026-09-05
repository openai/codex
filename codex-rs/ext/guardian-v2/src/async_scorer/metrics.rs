//! Low-cardinality metrics for Guardian V2 classification, connections, and approval decisions.

use std::time::Duration;

use codex_api::ApiError;
use codex_api::TransportError;
use codex_extension_api::ExtensionMetrics;

use super::sampler::LunaSamplerError;

pub(super) const CLASSIFICATION_METRIC: &str = "codex.guardian_v2.classification";
pub(super) const CLASSIFICATION_DURATION_METRIC: &str =
    "codex.guardian_v2.classification.duration_ms";
pub(super) const CLASSIFICATION_RISK_METRIC: &str = "codex.guardian_v2.classification.risk";
pub(super) const FAST_DECISION_METRIC: &str = "codex.guardian_v2.fast_decision";
pub(super) const REVIEW_FALLBACK_METRIC: &str = "codex.guardian_v2.review_fallback";
pub(super) const TOOL_CALL_LAG_METRIC: &str = "codex.guardian_v2.tool_call_lag";

// Never use error messages as metric tags: they may contain server responses or credentials.
pub(super) fn sampler_failure_reason(error: &LunaSamplerError) -> &'static str {
    match error {
        LunaSamplerError::Provider(_) => "provider_error",
        LunaSamplerError::ConnectionTimeout => "connection_timeout",
        LunaSamplerError::MissingOutput => "missing_output",
        LunaSamplerError::OutputTooLarge => "output_too_large",
        LunaSamplerError::Superseded => "superseded",
        LunaSamplerError::IncompatibleCompaction => "incompatible_compaction",
        LunaSamplerError::Api(error) => match error {
            ApiError::Transport(TransportError::Http { status, .. })
            | ApiError::Api { status, .. } => match status.as_u16() {
                401 => "http_401",
                403 => "http_403",
                429 => "http_429",
                400..=499 => "http_4xx",
                500..=599 => "http_5xx",
                _ => "http_other",
            },
            ApiError::Transport(TransportError::Timeout) => "transport_timeout",
            ApiError::Transport(TransportError::Connection(_)) => "connection_error",
            ApiError::Transport(TransportError::Network(_)) => "network_error",
            ApiError::Transport(TransportError::RetryLimit) => "retry_limit",
            ApiError::Transport(TransportError::Build(_)) => "request_build_error",
            ApiError::Stream(_) => "stream_error",
            ApiError::ContextWindowExceeded => "context_window_exceeded",
            ApiError::QuotaExceeded => "quota_exceeded",
            ApiError::UsageNotIncluded => "usage_not_included",
            ApiError::Retryable { .. } => "retryable_api_error",
            ApiError::RateLimitExceeded { .. } | ApiError::RateLimit(_) => "rate_limit",
            ApiError::InvalidRequest { .. } => "invalid_request",
            ApiError::CyberPolicy { .. } | ApiError::MisalignmentPolicyViolation { .. } => {
                "policy_error"
            }
            ApiError::ServerOverloaded => "server_overloaded",
        },
    }
}

pub(super) fn record_classification(
    metrics: Option<&dyn ExtensionMetrics>,
    duration: Duration,
    outcome: &str,
    failure_reason: Option<&str>,
) {
    let Some(metrics) = metrics else {
        return;
    };
    let mut tags = vec![("outcome", outcome)];
    if let Some(reason) = failure_reason {
        tags.push(("failure_reason", reason));
    }
    metrics.counter(CLASSIFICATION_METRIC, /*inc*/ 1, &tags);
    metrics.histogram(
        CLASSIFICATION_DURATION_METRIC,
        i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        &tags,
    );
}

pub(super) fn record_classification_risk(metrics: Option<&dyn ExtensionMetrics>, risk_level: &str) {
    let Some(metrics) = metrics else {
        return;
    };
    metrics.counter(
        CLASSIFICATION_RISK_METRIC,
        /*inc*/ 1,
        &[("risk_level", risk_level)],
    );
}

pub(super) fn record_fast_decision(
    metrics: Option<&dyn ExtensionMetrics>,
    decision: &str,
    reason: &str,
) {
    let Some(metrics) = metrics else {
        return;
    };
    metrics.counter(
        FAST_DECISION_METRIC,
        /*inc*/ 1,
        &[("decision", decision), ("reason", reason)],
    );
}
