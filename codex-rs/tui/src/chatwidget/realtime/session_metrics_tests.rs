//! Failure accounting distinguishes internal retry cleanup from a requested stop.

use super::super::StartupRetry;
use super::*;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Instant;

#[tokio::test]
async fn terminal_error_during_retry_cleanup_records_one_failure() {
    for (retry, expected_failure) in [
        (StartupRetry::WaitingForStop, true),
        (StartupRetry::Used, false),
    ] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        activate_voice(&mut chat);
        chat.realtime_conversation.phase = RealtimeConversationPhase::Stopping;
        chat.realtime_conversation.startup_retry = retry;

        chat.on_realtime_error(
            "connection lost".into(),
            RealtimeFailureCause::ConnectionUnknown,
        );

        assert_eq!(
            chat.realtime_conversation.failure_recorded,
            expected_failure
        );
        chat.on_realtime_error(
            "duplicate error".into(),
            RealtimeFailureCause::ConnectionUnknown,
        );
        assert_eq!(
            chat.realtime_conversation.failure_recorded,
            expected_failure
        );
    }
}

#[tokio::test]
async fn completed_voice_duration_uses_terminal_success_or_failure_outcome() {
    for (failed, expected_outcome) in [(false, "success"), (true, "failure")] {
        let (mut chat, _sender, _events, _ops) = make_chatwidget_manual_with_sender().await;
        let metrics = MetricsClient::new(
            MetricsConfig::in_memory(
                "test",
                "codex-cli",
                env!("CARGO_PKG_VERSION"),
                InMemoryMetricExporter::default(),
            )
            .with_runtime_reader(),
        )
        .expect("in-memory metrics should initialize");
        chat.session_telemetry = chat
            .session_telemetry
            .clone()
            .with_metrics_without_metadata_tags(metrics);
        activate_voice(&mut chat);
        chat.realtime_conversation.active_since = Some(Instant::now());
        chat.capture_realtime_session_duration();
        if failed {
            chat.record_realtime_disconnect_failure();
        }

        chat.reset_realtime_conversation();

        let metrics = chat
            .session_telemetry
            .snapshot_metrics()
            .expect("runtime metrics snapshot should be available");
        let duration = metrics
            .scope_metrics()
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .find(|metric| metric.name() == "codex.voice.session.duration")
            .expect("voice duration metric should be recorded");
        let duration_attributes = match duration.data() {
            AggregatedMetrics::F64(MetricData::Histogram(histogram)) => histogram
                .data_points()
                .next()
                .expect("voice duration should have one datapoint")
                .attributes()
                .map(|attribute| {
                    (
                        attribute.key.as_str().to_string(),
                        attribute.value.as_str().to_string(),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
            _ => panic!("voice duration should be an f64 histogram"),
        };
        assert_eq!(
            duration_attributes,
            BTreeMap::from([("outcome".to_string(), expected_outcome.to_string())])
        );

        let failure = metrics
            .scope_metrics()
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .find(|metric| metric.name() == "codex.voice.session.failure");
        if failed {
            let failure_attributes = match failure
                .expect("failed voice session should record a failure")
                .data()
            {
                AggregatedMetrics::U64(MetricData::Sum(sum)) => sum
                    .data_points()
                    .next()
                    .expect("voice failure should have one datapoint")
                    .attributes()
                    .map(|attribute| {
                        (
                            attribute.key.as_str().to_string(),
                            attribute.value.as_str().to_string(),
                        )
                    })
                    .collect::<BTreeMap<_, _>>(),
                _ => panic!("voice failure should be a u64 sum"),
            };
            assert_eq!(
                failure_attributes,
                BTreeMap::from([
                    ("cause".to_string(), "app_server_disconnect".to_string()),
                    ("phase".to_string(), "active".to_string()),
                ])
            );
        } else {
            assert!(failure.is_none());
        }
    }
}
