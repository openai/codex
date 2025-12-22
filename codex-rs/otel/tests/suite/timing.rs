use crate::harness::parse_envelope;
use crate::harness::parse_statsd_line;
use crate::harness::spawn_server;
use codex_metrics::HistogramBuckets;
use codex_metrics::MetricsClient;
use codex_metrics::MetricsConfig;
use codex_metrics::MetricsError;
use codex_metrics::Result;
use pretty_assertions::assert_eq;
use std::time::Duration;

// Ensures duration recording maps to the expected bucket tag.
#[test]
fn record_duration_uses_matching_bucket() -> Result<()> {
    let (dsn, handle) = spawn_server(200);
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;
    let buckets = HistogramBuckets::from_values(&[10, 20])?;

    metrics.record_duration(
        "codex.request_latency",
        Duration::from_millis(15),
        &buckets,
        &[("route", "chat")],
    )?;
    metrics.shutdown()?;

    let captured = handle.join().expect("server thread");
    let envelope = parse_envelope(&captured.body);
    let lines: Vec<&str> = envelope.payload.split('\n').collect();
    assert_eq!(lines.len(), 2);

    let line = parse_statsd_line(lines[0]);
    assert_eq!(line.name, "codex.request_latency");
    assert_eq!(line.tags.get("route").map(String::as_str), Some("chat"));
    assert_eq!(line.tags.get("le").map(String::as_str), Some("20"));

    let line = parse_statsd_line(lines[1]);
    assert_eq!(line.tags.get("le").map(String::as_str), Some("inf"));

    Ok(())
}

// Ensures time_result returns the closure output and records timing.
#[test]
fn time_result_records_success() -> Result<()> {
    let (dsn, handle) = spawn_server(200);
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;
    let buckets = HistogramBuckets::from_values(&[10, 20])?;

    let value = metrics.time_result(
        "codex.request_latency",
        &buckets,
        &[("route", "chat")],
        || Ok("ok"),
    )?;
    assert_eq!(value, "ok");
    metrics.shutdown()?;

    let captured = handle.join().expect("server thread");
    let envelope = parse_envelope(&captured.body);
    let lines: Vec<&str> = envelope.payload.split('\n').collect();
    assert!(!lines.is_empty());
    let parsed: Vec<_> = lines.iter().copied().map(parse_statsd_line).collect();
    assert!(
        parsed
            .iter()
            .any(|line| { line.tags.get("le").map(String::as_str) == Some("inf") })
    );
    for line in parsed {
        assert_eq!(line.name, "codex.request_latency");
        assert_eq!(line.tags.get("route").map(String::as_str), Some("chat"));
        assert!(line.tags.contains_key("le"));
    }

    Ok(())
}

// Ensures time_result propagates errors but still records timing.
#[test]
fn time_result_records_on_error() -> Result<()> {
    let (dsn, handle) = spawn_server(200);
    let metrics = MetricsClient::new(MetricsConfig::new(dsn))?;
    let buckets = HistogramBuckets::from_values(&[10, 20])?;

    let err = metrics
        .time_result(
            "codex.request_latency",
            &buckets,
            &[("route", "chat")],
            || -> Result<&'static str> { Err(MetricsError::EmptyMetricName) },
        )
        .unwrap_err();
    assert!(matches!(err, MetricsError::EmptyMetricName));
    metrics.shutdown()?;

    let captured = handle.join().expect("server thread");
    let envelope = parse_envelope(&captured.body);
    let lines: Vec<&str> = envelope.payload.split('\n').collect();
    assert!(!lines.is_empty());
    let parsed: Vec<_> = lines.iter().copied().map(parse_statsd_line).collect();
    assert!(
        parsed
            .iter()
            .any(|line| { line.tags.get("le").map(String::as_str) == Some("inf") })
    );
    for line in parsed {
        assert_eq!(line.name, "codex.request_latency");
        assert_eq!(line.tags.get("route").map(String::as_str), Some("chat"));
        assert!(line.tags.contains_key("le"));
    }

    Ok(())
}
