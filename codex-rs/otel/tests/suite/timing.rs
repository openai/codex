use crate::harness::attributes_to_map;
use crate::harness::build_metrics_with_defaults;
use crate::harness::histogram_data;
use crate::harness::latest_metrics;
use codex_otel::metrics::MetricsError;
use codex_otel::metrics::Result;
use pretty_assertions::assert_eq;
use std::time::Duration;

// Ensures duration recording maps to histogram output.
#[test]
fn record_duration_records_histogram() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    metrics.record_duration(
        "codex.request_latency",
        Duration::from_millis(15),
        &[("route", "chat")],
    )?;
    metrics.shutdown()?;

    let (bounds, bucket_counts, sum, count) =
        histogram_data(&latest_metrics(&exporter), "codex.request_latency");
    assert!(!bounds.is_empty());
    assert_eq!(bucket_counts.iter().sum::<u64>(), 1);
    assert_eq!(sum, 15.0);
    assert_eq!(count, 1);

    Ok(())
}

// Ensures time_result returns the closure output and records timing.
#[test]
fn time_result_records_success() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    let value = metrics.time_result("codex.request_latency", &[("route", "chat")], || Ok("ok"))?;
    assert_eq!(value, "ok");
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let (bounds, bucket_counts, _sum, count) =
        histogram_data(&resource_metrics, "codex.request_latency");
    assert!(!bounds.is_empty());
    assert_eq!(count, 1);
    assert_eq!(bucket_counts.iter().sum::<u64>(), 1);
    let attrs = attributes_to_map(
        match crate::harness::find_metric(&resource_metrics, "codex.request_latency").and_then(
            |metric| match metric.data() {
                opentelemetry_sdk::metrics::data::AggregatedMetrics::F64(data) => match data {
                    opentelemetry_sdk::metrics::data::MetricData::Histogram(histogram) => histogram
                        .data_points()
                        .next()
                        .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::attributes),
                    _ => None,
                },
                _ => None,
            },
        ) {
            Some(attrs) => attrs,
            None => panic!("attributes missing"),
        },
    );
    assert_eq!(attrs.get("route").map(String::as_str), Some("chat"));

    Ok(())
}

// Ensures time_result propagates errors but still records timing.
#[test]
fn time_result_records_on_error() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    let err = metrics
        .time_result(
            "codex.request_latency",
            &[("route", "chat")],
            || -> Result<&'static str> { Err(MetricsError::EmptyMetricName) },
        )
        .unwrap_err();
    assert!(matches!(err, MetricsError::EmptyMetricName));
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let (bounds, bucket_counts, _sum, count) =
        histogram_data(&resource_metrics, "codex.request_latency");
    assert!(!bounds.is_empty());
    assert_eq!(bucket_counts.iter().sum::<u64>(), 1);
    assert_eq!(count, 1);
    let attrs = attributes_to_map(
        match crate::harness::find_metric(&resource_metrics, "codex.request_latency").and_then(
            |metric| match metric.data() {
                opentelemetry_sdk::metrics::data::AggregatedMetrics::F64(data) => match data {
                    opentelemetry_sdk::metrics::data::MetricData::Histogram(histogram) => histogram
                        .data_points()
                        .next()
                        .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::attributes),
                    _ => None,
                },
                _ => None,
            },
        ) {
            Some(attrs) => attrs,
            None => panic!("attributes missing"),
        },
    );
    assert_eq!(attrs.get("route").map(String::as_str), Some("chat"));

    Ok(())
}
