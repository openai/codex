//! Review accounting with and without client events, including wall-clock skew.

use super::*;
use codex_otel::GUARDIAN_REVIEW_COUNT_METRIC;
use codex_otel::GUARDIAN_REVIEW_DURATION_METRIC;
use codex_otel::GUARDIAN_REVIEW_TOKEN_USAGE_METRIC;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::ThreadId;
use codex_protocol::approvals::NetworkApprovalProtocol;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

#[test_case::test_case(
    Some(GuardianAssessmentAction::NetworkAccess {
        target: "example.com".to_string(),
        host: "example.com".to_string(),
        protocol: NetworkApprovalProtocol::Https,
        port: 443,
    }),
    GuardianReviewedAction::NetworkAccess { protocol: NetworkApprovalProtocol::Https, port: 443 };
    "with client event"
)]
#[test_case::test_case(None, GuardianReviewedAction::NativeToolCall {}; "without client event")]
fn review_completion_uses_one_monotonic_duration(
    action: Option<GuardianAssessmentAction>,
    reviewed_action: GuardianReviewedAction,
) {
    let has_client_event = action.is_some();
    let exporter = InMemoryMetricExporter::default();
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "codex-core", env!("CARGO_PKG_VERSION"), exporter)
            .with_runtime_reader(),
    )
    .expect("in-memory metrics client");
    let telemetry = SessionTelemetry::new(
        ThreadId::new(),
        "test-model",
        "test-model",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_without_metadata_tags(metrics);
    let mut report = ReviewReport::new(ReviewMetadata {
        thread_id: "thread".to_string(),
        turn_id: "turn".to_string(),
        review_id: "review".to_string(),
        target_item_id: None,
        plugin_id: None,
        script_path: None,
        approval_request_source: GuardianApprovalRequestSource::MainTurn,
        reviewed_action,
        action,
        review_reason: GuardianReviewReason::Policy,
        timeout: Some(crate::REVIEW_TIMEOUT),
        model_context: ModelInvocationContext {
            model_slug: "test-model".to_string(),
            reasoning_effort: None,
        },
    });
    report.tracking.started_at_ms = 1_654_073_807_000;
    if let Some(event) = &mut report.started {
        event.started_at_ms =
            i64::try_from(report.tracking.started_at_ms).expect("start timestamp");
    }
    assert_eq!(report.started_event().is_some(), has_client_event);
    report.started_instant = Instant::now() - Duration::from_millis(456);
    let model: ModelInfo = serde_json::from_value(json!({
        "slug": "test-model",
        "display_name": "Test model",
        "supported_reasoning_levels": [],
        "shell_type": "disabled",
        "visibility": "none",
        "supported_in_api": false,
        "priority": 0,
        "support_verbosity": false,
        "truncation_policy": {"mode": "tokens", "limit": 1000},
        "experimental_supported_tools": []
    }))
    .expect("test model");

    let before = report.started_instant.elapsed();
    let (completed, elapsed) = report.complete(
        GuardianReviewOutcome::Error(crate::GuardianReviewError::Timeout),
        &model,
        /*require_guardian*/ true,
        GuardianReviewAnalyticsResult {
            attempt_count: 2,
            token_usage: Some(TokenUsage {
                input_tokens: 10,
                total_tokens: 10,
                ..Default::default()
            }),
            ..GuardianReviewAnalyticsResult::without_session()
        },
        /*completed_at_ms*/ 1_792_141_011_000,
    );
    let after = report.started_instant.elapsed();
    assert!((before..=after).contains(&elapsed));
    let elapsed_ms = u64::try_from(elapsed.as_millis()).expect("elapsed milliseconds");
    assert_eq!(
        completed
            .event
            .as_ref()
            .map(|event| (event.started_at_ms, event.completed_at_ms)),
        has_client_event.then_some((1_654_073_807_000, Some(1_792_141_011_000)))
    );

    // Simulate tracking later: the captured interval must not be measured again.
    report.started_instant -= Duration::from_secs(1);
    report.track(
        &telemetry,
        &AnalyticsEventsClient::disabled(),
        completed.analytics,
        /*completed_at_ms*/ 1_792_141_011_000,
        elapsed,
    );

    let snapshot = telemetry.snapshot_metrics().expect("metrics snapshot");
    let names = snapshot
        .scope_metrics()
        .flat_map(ScopeMetrics::metrics)
        .map(Metric::name)
        .collect::<std::collections::BTreeSet<_>>();
    for name in [
        GUARDIAN_REVIEW_COUNT_METRIC,
        GUARDIAN_REVIEW_DURATION_METRIC,
        GUARDIAN_REVIEW_TOKEN_USAGE_METRIC,
        "codex.guardian.review.attempt_count",
        "codex.guardian.review.retry_outcome",
    ] {
        assert!(names.contains(name), "missing {name}");
    }
    let metric = snapshot
        .scope_metrics()
        .flat_map(ScopeMetrics::metrics)
        .find(|metric| metric.name() == GUARDIAN_REVIEW_DURATION_METRIC)
        .expect("review duration metric");
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
        panic!("expected a duration histogram");
    };
    let points: Vec<_> = histogram.data_points().collect();
    assert_eq!(points.len(), 1);
    assert_eq!((points[0].count(), points[0].sum() as u64), (1, elapsed_ms));
}
