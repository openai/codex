use std::collections::BTreeMap;
use std::time::Duration;

use codex_otel::MetricsConfig;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn emits_connection_metrics() {
    let (telemetry, metrics, exporter) = test_telemetry();

    let connection = telemetry.connection_started(ConnectionTransport::WebSocket);
    drop(connection);
    metrics.shutdown().expect("shutdown metrics");

    let metrics = latest_metrics(&exporter);
    assert_eq!(
        metric_points(&metrics, CONNECTIONS_TOTAL_METRIC),
        vec![(
            1.0,
            BTreeMap::from([
                ("result".to_string(), "accepted".to_string()),
                ("transport".to_string(), "websocket".to_string()),
            ]),
        )]
    );
    assert_eq!(
        metric_points(&metrics, CONNECTIONS_ACTIVE_METRIC),
        vec![(
            0.0,
            BTreeMap::from([("transport".to_string(), "websocket".to_string())]),
        )]
    );
    assert_metric_metadata(
        &metrics,
        CONNECTIONS_ACTIVE_METRIC,
        CONNECTIONS_ACTIVE_DESCRIPTION,
        "",
    );
    assert_metric_metadata(
        &metrics,
        CONNECTIONS_TOTAL_METRIC,
        CONNECTIONS_TOTAL_DESCRIPTION,
        "",
    );
}

#[test]
fn emits_request_metrics() {
    let (telemetry, metrics, exporter) = test_telemetry();

    telemetry.request_completed("process/start", "success", Duration::from_millis(12));
    metrics.shutdown().expect("shutdown metrics");

    let metrics = latest_metrics(&exporter);
    assert_eq!(
        metric_points(&metrics, REQUESTS_TOTAL_METRIC),
        vec![(
            1.0,
            BTreeMap::from([
                ("method".to_string(), "process/start".to_string()),
                ("result".to_string(), "success".to_string()),
            ]),
        )]
    );
    assert_eq!(histogram_count(&metrics, REQUEST_DURATION_METRIC), 1);
    assert_eq!(histogram_sum(&metrics, REQUEST_DURATION_METRIC), 0.012);
    assert_metric_metadata(
        &metrics,
        REQUESTS_TOTAL_METRIC,
        REQUESTS_TOTAL_DESCRIPTION,
        "",
    );
    assert_metric_metadata(
        &metrics,
        REQUEST_DURATION_METRIC,
        REQUEST_DURATION_DESCRIPTION,
        "s",
    );
}

#[test]
fn emits_process_metrics() {
    let (telemetry, metrics, exporter) = test_telemetry();

    telemetry.process_started();
    telemetry.process_finished("terminated", Duration::from_millis(34));
    metrics.shutdown().expect("shutdown metrics");

    let metrics = latest_metrics(&exporter);
    assert_eq!(
        metric_points(&metrics, PROCESSES_ACTIVE_METRIC),
        vec![(0.0, BTreeMap::new())]
    );
    assert_eq!(
        metric_points(&metrics, PROCESSES_FINISHED_TOTAL_METRIC),
        vec![(
            1.0,
            BTreeMap::from([("result".to_string(), "terminated".to_string())]),
        )]
    );
    assert_eq!(histogram_count(&metrics, PROCESS_DURATION_METRIC), 1);
    assert_metric_metadata(
        &metrics,
        PROCESSES_ACTIVE_METRIC,
        PROCESSES_ACTIVE_DESCRIPTION,
        "",
    );
    assert_metric_metadata(
        &metrics,
        PROCESSES_FINISHED_TOTAL_METRIC,
        PROCESSES_FINISHED_TOTAL_DESCRIPTION,
        "",
    );
    assert_metric_metadata(
        &metrics,
        PROCESS_DURATION_METRIC,
        PROCESS_DURATION_DESCRIPTION,
        "s",
    );
}

#[test]
fn emits_remote_registration_metrics() {
    let (telemetry, metrics, exporter) = test_telemetry();

    telemetry.remote_registration_completed("success", Duration::from_millis(5));
    metrics.shutdown().expect("shutdown metrics");

    let metrics = latest_metrics(&exporter);
    assert_eq!(
        metric_points(&metrics, REMOTE_REGISTRATION_TOTAL_METRIC),
        vec![(
            1.0,
            BTreeMap::from([("result".to_string(), "success".to_string())]),
        )]
    );
    assert_eq!(
        histogram_count(&metrics, REMOTE_REGISTRATION_DURATION_METRIC),
        1
    );
    assert_metric_metadata(
        &metrics,
        REMOTE_REGISTRATION_TOTAL_METRIC,
        REMOTE_REGISTRATION_TOTAL_DESCRIPTION,
        "",
    );
    assert_metric_metadata(
        &metrics,
        REMOTE_REGISTRATION_DURATION_METRIC,
        REMOTE_REGISTRATION_DURATION_DESCRIPTION,
        "s",
    );
}

#[test]
fn emits_remote_websocket_metrics() {
    let (telemetry, metrics, exporter) = test_telemetry();

    let websocket = telemetry.remote_websocket_connected();
    telemetry.remote_websocket_connect_completed("success", Duration::from_millis(7));
    telemetry.remote_websocket_reconnect("connect_failed");
    drop(websocket);
    metrics.shutdown().expect("shutdown metrics");

    let metrics = latest_metrics(&exporter);
    assert_eq!(
        metric_points(&metrics, REMOTE_WEBSOCKET_CONNECT_TOTAL_METRIC),
        vec![(
            1.0,
            BTreeMap::from([("result".to_string(), "success".to_string())]),
        )]
    );
    assert_eq!(
        metric_points(&metrics, REMOTE_WEBSOCKET_ACTIVE_METRIC),
        vec![(0.0, BTreeMap::new())]
    );
    assert_eq!(
        metric_points(&metrics, REMOTE_WEBSOCKET_RECONNECTS_METRIC),
        vec![(
            1.0,
            BTreeMap::from([("reason".to_string(), "connect_failed".to_string())]),
        )]
    );
    assert_eq!(
        histogram_count(&metrics, REMOTE_WEBSOCKET_CONNECT_DURATION_METRIC),
        1
    );
    for (name, description, unit) in [
        (
            REMOTE_WEBSOCKET_ACTIVE_METRIC,
            REMOTE_WEBSOCKET_ACTIVE_DESCRIPTION,
            "",
        ),
        (
            REMOTE_WEBSOCKET_CONNECT_TOTAL_METRIC,
            REMOTE_WEBSOCKET_CONNECT_TOTAL_DESCRIPTION,
            "",
        ),
        (
            REMOTE_WEBSOCKET_CONNECT_DURATION_METRIC,
            REMOTE_WEBSOCKET_CONNECT_DURATION_DESCRIPTION,
            "s",
        ),
        (
            REMOTE_WEBSOCKET_RECONNECTS_METRIC,
            REMOTE_WEBSOCKET_RECONNECTS_DESCRIPTION,
            "",
        ),
    ] {
        assert_metric_metadata(&metrics, name, description, unit);
    }
}

fn test_telemetry() -> (
    ExecServerTelemetry,
    codex_otel::MetricsClient,
    InMemoryMetricExporter,
) {
    let exporter = InMemoryMetricExporter::default();
    let metrics = codex_otel::MetricsClient::new(MetricsConfig::in_memory(
        "test",
        "codex-exec-server",
        env!("CARGO_PKG_VERSION"),
        exporter.clone(),
    ))
    .expect("metrics");
    (
        ExecServerTelemetry::new(Some(metrics.clone())),
        metrics,
        exporter,
    )
}

fn latest_metrics(exporter: &InMemoryMetricExporter) -> ResourceMetrics {
    exporter
        .get_finished_metrics()
        .expect("finished metrics")
        .into_iter()
        .last()
        .expect("metrics export")
}

fn find_metric<'a>(resource_metrics: &'a ResourceMetrics, name: &str) -> &'a Metric {
    resource_metrics
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == name)
        .unwrap_or_else(|| panic!("metric {name} missing"))
}

fn metric_points(
    resource_metrics: &ResourceMetrics,
    name: &str,
) -> Vec<(f64, BTreeMap<String, String>)> {
    match find_metric(resource_metrics, name).data() {
        AggregatedMetrics::I64(MetricData::Gauge(gauge)) => gauge
            .data_points()
            .map(|point| (point.value() as f64, attributes_to_map(point.attributes())))
            .collect(),
        AggregatedMetrics::U64(MetricData::Sum(sum)) => sum
            .data_points()
            .map(|point| (point.value() as f64, attributes_to_map(point.attributes())))
            .collect(),
        _ => panic!("unexpected metric data for {name}"),
    }
}

fn histogram_count(resource_metrics: &ResourceMetrics, name: &str) -> u64 {
    match find_metric(resource_metrics, name).data() {
        AggregatedMetrics::F64(MetricData::Histogram(histogram)) => histogram
            .data_points()
            .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::count)
            .sum(),
        _ => panic!("unexpected histogram data for {name}"),
    }
}

fn histogram_sum(resource_metrics: &ResourceMetrics, name: &str) -> f64 {
    match find_metric(resource_metrics, name).data() {
        AggregatedMetrics::F64(MetricData::Histogram(histogram)) => histogram
            .data_points()
            .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::sum)
            .sum(),
        _ => panic!("unexpected histogram data for {name}"),
    }
}

fn assert_metric_metadata(
    resource_metrics: &ResourceMetrics,
    name: &str,
    description: &str,
    unit: &str,
) {
    let metric = find_metric(resource_metrics, name);
    assert_eq!(metric.description(), description);
    assert_eq!(metric.unit(), unit);
}

fn attributes_to_map<'a>(
    attributes: impl Iterator<Item = &'a KeyValue>,
) -> BTreeMap<String, String> {
    attributes
        .map(|attribute| {
            (
                attribute.key.as_str().to_string(),
                attribute.value.as_str().to_string(),
            )
        })
        .collect()
}
