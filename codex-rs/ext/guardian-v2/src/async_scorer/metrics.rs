//! Low-cardinality metrics for Guardian V2 classification, connections, and approval decisions.

use std::time::Duration;

use codex_api::ApiError;
use codex_api::TransportError;
use codex_core::context::GuardianContextMode;
use codex_extension_api::ExtensionMetrics;

use super::decisions::DecisionsError;
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
        LunaSamplerError::InputTooLarge => "input_too_large",
        LunaSamplerError::QueueFull => "queue_full",
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
            ApiError::Transport(TransportError::ResponseTooLarge { .. }) => "response_too_large",
            ApiError::Transport(TransportError::Policy(_)) => "network_policy_denied",
            ApiError::Stream(_) | ApiError::ContentFilter => "stream_error",
            ApiError::ContextWindowExceeded => "context_window_exceeded",
            ApiError::QuotaExceeded => "quota_exceeded",
            ApiError::UsageNotIncluded => "usage_not_included",
            ApiError::Retryable { .. } => "retryable_api_error",
            ApiError::RateLimitExceeded { .. } | ApiError::RateLimit(_) => "rate_limit",
            ApiError::InvalidRequest { .. } | ApiError::InvalidPrompt { .. } => "invalid_request",
            ApiError::CyberPolicy { .. }
            | ApiError::BioPolicy { .. }
            | ApiError::MisalignmentPolicyViolation { .. } => "policy_error",
            ApiError::ServerOverloaded { .. } => "server_overloaded",
            ApiError::FlexUnavailable => "flex_unavailable",
        },
    }
}

pub(super) fn record_classification(
    metrics: Option<&dyn ExtensionMetrics>,
    context_mode: GuardianContextMode,
    duration: Duration,
    outcome: &str,
    failure_reason: Option<&str>,
) {
    let Some(metrics) = metrics else {
        return;
    };
    let mut tags = vec![
        ("outcome", outcome),
        ("context_mode", context_mode.as_str()),
    ];
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

pub(super) fn record_section_costs(
    metrics: Option<&dyn ExtensionMetrics>,
    costs: impl IntoIterator<Item = (&'static str, codex_guardian_context::SectionCost)>,
) {
    let Some(metrics) = metrics else {
        return;
    };
    for (section, cost) in costs {
        for (measurement, value) in cost.measurements() {
            metrics.histogram_with_boundaries(
                codex_guardian_context::SECTION_COST_METRIC,
                i64::try_from(value).unwrap_or(i64::MAX),
                codex_guardian_context::SECTION_COST_BOUNDARIES,
                &[
                    ("target", "async"),
                    ("section", section),
                    ("measurement", measurement),
                ],
            );
        }
    }
}

pub(super) fn record_request_tokens(
    metrics: Option<&dyn ExtensionMetrics>,
    existing: usize,
    total: usize,
) {
    let Some(metrics) = metrics else {
        return;
    };
    for (component, tokens) in [
        ("existing_context", existing),
        ("new_input", total.saturating_sub(existing)),
        ("total", total),
    ] {
        metrics.histogram_with_boundaries(
            codex_guardian_context::REQUEST_TOKENS_METRIC,
            i64::try_from(tokens).unwrap_or(i64::MAX),
            codex_guardian_context::REQUEST_TOKENS_BOUNDARIES,
            &[("target", "async"), ("component", component)],
        );
    }
}

pub(super) fn record_decisions_comparison_outcome(
    metrics: Option<&dyn ExtensionMetrics>,
    outcome: &str,
    reason: &str,
) {
    if let Some(metrics) = metrics {
        metrics.counter(
            "codex.guardian_v2.decisions_comparison",
            /*inc*/ 1,
            &[("outcome", outcome), ("reason", reason)],
        );
    }
}

// Keep all backend diagnostics bounded and free of raw transport data.
pub(super) fn decisions_failure_reason(error: DecisionsError) -> &'static str {
    match error {
        DecisionsError::Credentials => "provider_error",
        DecisionsError::ClientSetup => "request_build_error",
        DecisionsError::UnsupportedEvidence => "decisions_unsupported_evidence",
        DecisionsError::InputTooLarge => "input_too_large",
        DecisionsError::Timeout => "transport_timeout",
        // The adapter erases transport details; do not claim a specific network failure.
        DecisionsError::Transport => "decisions_transport",
        DecisionsError::Http(status) => match status {
            401 => "http_401",
            403 => "http_403",
            429 => "http_429",
            400..=499 => "http_4xx",
            500..=599 => "http_5xx",
            _ => "http_other",
        },
        DecisionsError::ResponseTooLarge => "response_too_large",
        DecisionsError::InvalidResponse => "invalid_output",
    }
}
