use crate::harness::attributes_to_map;
use crate::harness::build_metrics_with_defaults;
use crate::harness::histogram_data;
use crate::harness::latest_metrics;
use codex_otel::Result;
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

    let resource_metrics = latest_metrics(&exporter);
    let (bounds, bucket_counts, sum, count) =
        histogram_data(&resource_metrics, "codex.request_latency");
    assert!(!bounds.is_empty());
    assert_eq!(bucket_counts.iter().sum::<u64>(), 1);
    assert_eq!(sum, 15.0);
    assert_eq!(count, 1);
    let metric = crate::harness::find_metric(&resource_metrics, "codex.request_latency")
        .unwrap_or_else(|| panic!("metric codex.request_latency missing"));
    assert_eq!(metric.unit(), "ms");
    assert_eq!(metric.description(), "Duration in milliseconds.");

    Ok(())
}

#[test]
fn record_duration_seconds_uses_fractional_seconds_and_scaled_buckets() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    for duration in [
        Duration::from_millis(200),
        Duration::from_secs(1),
        Duration::from_millis(4900),
    ] {
        metrics.record_duration_seconds_with_description(
            "codex.request_duration_seconds",
            "Duration of Codex requests in seconds.",
            duration,
            &[("method", "initialize")],
        )?;
    }
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let (bounds, bucket_counts, sum, count) =
        histogram_data(&resource_metrics, "codex.request_duration_seconds");
    assert_eq!(
        bounds,
        vec![
            0.0, 0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
        ]
    );
    assert_eq!(
        bucket_counts,
        vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0]
    );
    assert!((sum - 6.1).abs() < f64::EPSILON * 8.0);
    assert_eq!(count, 3);
    let metric = crate::harness::find_metric(&resource_metrics, "codex.request_duration_seconds")
        .unwrap_or_else(|| panic!("metric codex.request_duration_seconds missing"));
    assert_eq!(metric.unit(), "s");
    assert_eq!(
        metric.description(),
        "Duration of Codex requests in seconds."
    );

    Ok(())
}

// Ensures time_result returns the closure output and records timing.
#[test]
fn timer_result_records_success() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    {
        let timer = metrics.start_timer("codex.request_latency", &[("route", "chat")]);
        assert!(timer.is_ok());
    }

    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let (bounds, bucket_counts, _sum, count) =
        histogram_data(&resource_metrics, "codex.request_latency");
    assert!(!bounds.is_empty());
    assert_eq!(count, 1);
    assert_eq!(bucket_counts.iter().sum::<u64>(), 1);
    let metric = crate::harness::find_metric(&resource_metrics, "codex.request_latency")
        .unwrap_or_else(|| panic!("metric codex.request_latency missing"));
    assert_eq!(metric.unit(), "ms");
    assert_eq!(metric.description(), "Duration in milliseconds.");
    let attrs = attributes_to_map(
        match crate::harness::find_metric(&resource_metrics, "codex.request_latency").and_then(
            |metric| match metric.data() {
                opentelemetry_sdk::metrics::data::AggregatedMetrics::F64(
                    opentelemetry_sdk::metrics::data::MetricData::Histogram(histogram),
                ) => histogram
                    .data_points()
                    .next()
                    .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::attributes),
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
