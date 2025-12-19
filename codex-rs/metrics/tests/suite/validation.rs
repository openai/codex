use codex_metrics::HistogramBuckets;
use codex_metrics::MetricsBatch;
use codex_metrics::MetricsClient;
use codex_metrics::MetricsConfig;
use codex_metrics::MetricsError;
use codex_metrics::Result;

// Validates invalid DSNs are rejected early.
#[test]
fn invalid_dsn_reports_error() -> Result<()> {
    assert!(matches!(
        MetricsClient::new(MetricsConfig::new("not a dsn")),
        Err(MetricsError::InvalidDsn { .. })
    ));
    Ok(())
}

// Ensures invalid tag components are rejected during config build.
#[test]
fn invalid_tag_component_is_rejected() -> Result<()> {
    let err = MetricsConfig::default()
        .with_tag("bad key", "value")
        .unwrap_err();
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag key" && value == "bad key"
    ));
    Ok(())
}

// Ensures the reserved histogram bucketing tag key is rejected in config defaults.
#[test]
fn reserved_tag_key_is_rejected_in_config() -> Result<()> {
    let err = MetricsConfig::default().with_tag("le", "10").unwrap_err();
    assert!(matches!(
        err,
        MetricsError::ReservedTagKey { key } if key == "le"
    ));
    Ok(())
}

// Ensures per-metric tag keys are validated.
#[test]
fn counter_rejects_invalid_tag_key() {
    let mut batch = MetricsBatch::new();
    let err = batch
        .counter("codex.turns", 1, &[("bad key", "value")])
        .unwrap_err();
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag key" && value == "bad key"
    ));
}

// Ensures per-metric tag keys cannot use reserved histogram bucketing keys.
#[test]
fn counter_rejects_reserved_tag_key() {
    let mut batch = MetricsBatch::new();
    let err = batch
        .counter("codex.turns", 1, &[("le", "10")])
        .unwrap_err();
    assert!(matches!(
        err,
        MetricsError::ReservedTagKey { key } if key == "le"
    ));
}

// Ensures per-metric tag values are validated.
#[test]
fn histogram_rejects_invalid_tag_value() -> Result<()> {
    let mut batch = MetricsBatch::new();
    let buckets = HistogramBuckets::from_values(&[10])?;
    let err = batch
        .histogram(
            "codex.request_latency",
            3,
            &buckets,
            &[("route", "bad value")],
        )
        .unwrap_err();
    assert!(matches!(
        err,
        MetricsError::InvalidTagComponent { label, value }
            if label == "tag value" && value == "bad value"
    ));
    Ok(())
}

// Ensures histogram calls reject reserved tag keys even though they internally add `le`.
#[test]
fn histogram_rejects_reserved_tag_key() -> Result<()> {
    let mut batch = MetricsBatch::new();
    let buckets = HistogramBuckets::from_values(&[10])?;
    let err = batch
        .histogram("codex.request_latency", 3, &buckets, &[("le", "10")])
        .unwrap_err();
    assert!(matches!(
        err,
        MetricsError::ReservedTagKey { key } if key == "le"
    ));
    Ok(())
}

// Ensures invalid metric names are rejected when building a batch.
#[test]
fn counter_rejects_invalid_metric_name() -> Result<()> {
    let mut batch = MetricsBatch::new();
    let err = batch.counter("bad name", 1, &[]).unwrap_err();
    assert!(matches!(
        err,
        MetricsError::InvalidMetricName { name } if name == "bad name"
    ));
    Ok(())
}

// Ensures empty histogram bucket lists are rejected.
#[test]
fn empty_buckets_are_rejected() {
    let err = HistogramBuckets::from_values(&[]).unwrap_err();
    assert!(matches!(err, MetricsError::EmptyBuckets));
}

// Ensures range overflow is detected when building buckets.
#[test]
fn range_overflow_is_reported() {
    let err = HistogramBuckets::from_range(i64::MAX - 1, i64::MAX, 2).unwrap_err();
    assert!(matches!(err, MetricsError::BucketRangeOverflow { .. }));
}
