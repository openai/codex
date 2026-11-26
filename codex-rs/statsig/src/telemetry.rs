//! Records bounded Statsig update and evaluation metrics without logging targeting values.

use std::time::Duration;

use codex_api::AuthError;
use codex_http_client::HttpError;
use codex_http_client::NetworkPolicyDenied;
use codex_otel::MetricsError;
use statsig_client::StatsigClient;
use statsig_client::StatsigError;

pub(crate) const POLICY_DENIED: &str = "Statsig request denied by network policy";

pub(crate) const WAITING_AUTH: &str = "waiting_auth";
pub(crate) const AUTH: &str = "auth";
pub(crate) const HTTP: &str = "http";

pub(crate) fn failure_category(error: &anyhow::Error) -> &'static str {
    if error.chain().any(|cause| {
        cause.is::<NetworkPolicyDenied>()
            || matches!(cause.downcast_ref(), Some(AuthError::Policy(_)))
    }) {
        return "policy";
    }
    if let Some(error) = error.downcast_ref::<HttpError>() {
        return if error.is_timeout() {
            "timeout"
        } else if matches!(error, HttpError::Policy(_)) {
            "policy"
        } else {
            "network"
        };
    }
    if let Some(error) = error.downcast_ref::<StatsigError>() {
        return match error {
            StatsigError::NetworkTimeout(_) | StatsigError::OperationTimeout { .. } => "timeout",
            StatsigError::Network(message) if message == POLICY_DENIED => "policy",
            StatsigError::Network(_) => "network",
            StatsigError::Http { .. } => HTTP,
            StatsigError::NoValues => "no_values",
            StatsigError::Json(_)
            | StatsigError::InvalidBootstrap { .. }
            | StatsigError::Utf8(_) => "invalid_response",
            _ => "other",
        };
    }
    if error.downcast_ref::<serde_json::Error>().is_some() {
        return "invalid_response";
    }
    // Only these fixed contexts can become metric labels.
    match error.downcast_ref::<&'static str>().copied() {
        Some(label @ (WAITING_AUTH | AUTH | HTTP)) => label,
        _ => "other",
    }
}

pub(crate) fn evaluation(name: &'static str, kind: &'static str, reason: &'static str) {
    if let Some(metrics) = codex_otel::global() {
        let record = |name| {
            metrics.counter(
                "codex.statsig.evaluation.count",
                /*inc*/ 1,
                &[("name", name), ("kind", kind), ("reason", reason)],
            )
        };
        if let Err(
            MetricsError::InvalidTagComponent { .. } | MetricsError::EmptyTagComponent { .. },
        ) = record(name)
        {
            let _ = record("invalid_name");
        }
    }
}

pub(crate) fn update(
    operation: &'static str,
    duration: Duration,
    failure: Option<&'static str>,
    sdk: Option<&StatsigClient>,
) {
    let Some(metrics) = codex_otel::global() else {
        return;
    };
    let result = match failure {
        None => "success",
        Some("timeout") => "timeout",
        Some(_) => "error",
    };
    let tags = [
        ("operation", operation),
        ("result", result),
        ("failure_category", failure.unwrap_or("none")),
    ];
    let _ = metrics.counter("codex.statsig.update.count", /*inc*/ 1, &tags);
    let _ = metrics.record_duration("codex.statsig.update.duration", duration, &tags);
    if failure.is_none()
        && let Some(sdk) = sdk
    {
        let state = sdk.current_state();
        for (name, count) in [
            ("codex.statsig.snapshot.gates", state.feature_gate_count()),
            (
                "codex.statsig.snapshot.configs",
                state.dynamic_config_count(),
            ),
        ] {
            let _ = metrics.histogram(name, count.min(i64::MAX as usize) as i64, &tags);
        }
    }
}
