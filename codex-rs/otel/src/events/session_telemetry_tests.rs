use super::AuthEnvTelemetryMetadata;
use super::SessionTelemetry;
use crate::TelemetryAuthMode;
use crate::metrics::MetricsClient;
use crate::metrics::MetricsConfig;
use crate::metrics::names::INLINE_IMAGE_REQUEST_LIMIT_METRIC;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use eventsource_stream::Event as StreamEvent;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

fn auth_env_metadata() -> AuthEnvTelemetryMetadata {
    AuthEnvTelemetryMetadata {
        openai_api_key_env_present: true,
        codex_api_key_env_present: false,
        codex_api_key_env_enabled: true,
        provider_env_key_name: Some("configured".to_string()),
        provider_env_key_present: Some(true),
        refresh_token_url_override_present: true,
    }
}

fn test_session_telemetry_with_metrics() -> SessionTelemetry {
    let exporter = InMemoryMetricExporter::default();
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "codex-otel", env!("CARGO_PKG_VERSION"), exporter)
            .with_runtime_reader(),
    )
    .expect("in-memory metrics client");
    SessionTelemetry::new(
        ThreadId::new(),
        "gpt-test",
        "gpt-test",
        Some("account-id".to_string()),
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test-originator".to_string(),
        /*log_user_prompts*/ false,
        "test-terminal".to_string(),
        SessionSource::Cli,
    )
    .with_auth_env(auth_env_metadata())
    .with_metrics_without_metadata_tags(metrics)
}

fn find_metric<'a>(resource_metrics: &'a ResourceMetrics, name: &str) -> &'a Metric {
    for scope_metrics in resource_metrics.scope_metrics() {
        for metric in scope_metrics.metrics() {
            if metric.name() == name {
                return metric;
            }
        }
    }
    panic!("metric {name} missing");
}

fn attributes_to_map<'a>(
    attributes: impl Iterator<Item = &'a KeyValue>,
) -> BTreeMap<String, String> {
    attributes
        .map(|kv| (kv.key.as_str().to_string(), kv.value.as_str().to_string()))
        .collect()
}

fn metric_point(resource_metrics: &ResourceMetrics, name: &str) -> (BTreeMap<String, String>, u64) {
    let metric = find_metric(resource_metrics, name);
    match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                let point = points[0];
                (attributes_to_map(point.attributes()), point.value())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    }
}

#[test]
fn log_sse_event_records_inline_image_limit_metric_for_response_failed() {
    let session_telemetry = test_session_telemetry_with_metrics();
    let sse_response: std::result::Result<
        Option<std::result::Result<StreamEvent, eventsource_stream::EventStreamError<&str>>>,
        tokio::time::error::Elapsed,
    > = Ok(Some(Ok(StreamEvent {
        event: "response.failed".to_string(),
        data: r#"{"type":"response.failed","response":{"error":{"code":"max_images_per_request","message":"Invalid request."}}}"#
            .to_string(),
        id: String::new(),
        retry: None,
    })));

    session_telemetry.log_sse_event(&sse_response, Duration::from_millis(25));

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, value) = metric_point(&snapshot, INLINE_IMAGE_REQUEST_LIMIT_METRIC);
    assert_eq!(value, 1);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("bytes_exceeded".to_string(), "false".to_string()),
            ("images_exceeded".to_string(), "true".to_string()),
            ("outcome".to_string(), "upstream_rejected".to_string()),
        ])
    );
}

#[test]
fn record_websocket_event_records_inline_image_limit_metric_for_response_failed() {
    let session_telemetry = test_session_telemetry_with_metrics();
    let websocket_response: std::result::Result<
        Option<std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>,
        codex_api::ApiError,
    > = Ok(Some(Ok(Message::Text(
        r#"{"type":"response.failed","response":{"error":{"code":"max_images_per_request","message":"Invalid request."}}}"#
            .into(),
    ))));

    session_telemetry.record_websocket_event(&websocket_response, Duration::from_millis(25));

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, value) = metric_point(&snapshot, INLINE_IMAGE_REQUEST_LIMIT_METRIC);
    assert_eq!(value, 1);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("bytes_exceeded".to_string(), "false".to_string()),
            ("images_exceeded".to_string(), "true".to_string()),
            ("outcome".to_string(), "upstream_rejected".to_string()),
        ])
    );
}
