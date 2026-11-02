use crate::harness::attributes_to_map;
use crate::harness::build_metrics_with_defaults;
use crate::harness::find_metric;
use crate::harness::histogram_data;
use crate::harness::latest_metrics;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_BUCKETS;
use codex_otel::ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC;
use codex_otel::Result;
use codex_otel::THREAD_SKILLS_COUNT_METRIC_BUCKETS;
use codex_otel::THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_BUCKETS;
use codex_otel::THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC;
use codex_otel::THREAD_SKILLS_ENABLED_TOTAL_METRIC;
use codex_otel::THREAD_SKILLS_TRUNCATED_BUCKETS;
use codex_otel::THREAD_SKILLS_TRUNCATED_METRIC;
use codex_otel::THREAD_TOOLS_FRAGMENT_BYTES_METRIC;
use codex_otel::THREAD_TOOLS_METRIC_BUCKETS;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

// Ensures counters/histograms render with default + per-call tags.
#[test]
fn send_builds_payload_with_tags_and_histograms() -> Result<()> {
    let (metrics, exporter) =
        build_metrics_with_defaults(&[("service", "codex-cli"), ("env", "prod")])?;

    metrics.counter_with_description(
        "codex.turns",
        "Total number of Codex turns.",
        /*inc*/ 1,
        &[("model", "gpt-5.1"), ("env", "dev")],
    )?;
    metrics.histogram(
        "codex.tool_latency",
        /*value*/ 25,
        &[("tool", "shell")],
    )?;
    metrics.gauge_with_description(
        "codex.active",
        "Number of active Codex operations.",
        /*value*/ 2,
        &[("component", "test")],
    )?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);

    let counter = find_metric(&resource_metrics, "codex.turns").expect("counter metric missing");
    assert_eq!(counter.description(), "Total number of Codex turns.");
    let counter_attributes = match counter.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                assert_eq!(points[0].value(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    let expected_counter_attributes = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "dev".to_string()),
        ("model".to_string(), "gpt-5.1".to_string()),
    ]);
    assert_eq!(counter_attributes, expected_counter_attributes);

    let (bounds, bucket_counts, sum, count) =
        histogram_data(&resource_metrics, "codex.tool_latency");
    assert!(!bounds.is_empty());
    assert_eq!(bucket_counts.iter().sum::<u64>(), 1);
    assert_eq!(sum, 25.0);
    assert_eq!(count, 1);

    let histogram_attrs = attributes_to_map(
        find_metric(&resource_metrics, "codex.tool_latency")
            .and_then(|metric| match metric.data() {
                opentelemetry_sdk::metrics::data::AggregatedMetrics::F64(
                    opentelemetry_sdk::metrics::data::MetricData::Histogram(histogram),
                ) => histogram
                    .data_points()
                    .next()
                    .map(opentelemetry_sdk::metrics::data::HistogramDataPoint::attributes),
                _ => None,
            })
            .expect("codex.tool_latency histogram attributes should exist"),
    );
    let expected_histogram_attributes = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("tool".to_string(), "shell".to_string()),
    ]);
    assert_eq!(histogram_attrs, expected_histogram_attributes);

    let gauge = find_metric(&resource_metrics, "codex.active").expect("gauge metric missing");
    assert_eq!(gauge.description(), "Number of active Codex operations.");
    let gauge_point = match gauge.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::I64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Gauge(gauge) => {
                gauge.data_points().next().expect("gauge point")
            }
            _ => panic!("unexpected gauge aggregation"),
        },
        _ => panic!("unexpected gauge metric data type"),
    };
    assert_eq!(gauge_point.value(), 2);
    assert_eq!(
        attributes_to_map(gauge_point.attributes()),
        BTreeMap::from([
            ("component".to_string(), "test".to_string()),
            ("env".to_string(), "prod".to_string()),
            ("service".to_string(), "codex-cli".to_string()),
        ])
    );

    Ok(())
}

#[test]
fn histogram_uses_explicit_bucket_boundaries() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    metrics.histogram_with_boundaries(
        "codex.payload_bytes",
        /*value*/ 1024,
        &[256.0, 1024.0, 4096.0],
        &[],
    )?;
    metrics.shutdown()?;

    assert_eq!(
        histogram_data(&latest_metrics(&exporter), "codex.payload_bytes"),
        (vec![256.0, 1024.0, 4096.0], vec![0, 1, 0, 0], 1024.0, 1)
    );

    Ok(())
}

#[test]
fn additional_context_histogram_bounds_truncated_observations() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;
    let samples = [4_024, 4_027, 4_043];
    for value in samples {
        metrics.histogram_with_boundaries(
            ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC,
            value,
            ADDITIONAL_CONTEXT_VALUE_BYTES_BUCKETS.as_slice(),
            &[("stage", "after"), ("truncated", "true")],
        )?;
    }
    metrics.shutdown()?;

    let (bounds, counts, sum, count) = histogram_data(
        &latest_metrics(&exporter),
        ADDITIONAL_CONTEXT_VALUE_BYTES_METRIC,
    );
    assert_eq!((sum, count), (12_094.0, 3));
    assert_eq!(counts.iter().sum::<u64>(), count);
    assert_eq!(counts.last(), Some(&0));
    for (upper_bound, bucket_count) in bounds.iter().zip(&counts) {
        assert!(
            *bucket_count == 0 || *upper_bound <= 4_096.0,
            "post-truncation observations landed in a bucket ending at {upper_bound} bytes"
        );
    }

    Ok(())
}

#[test]
fn context_histograms_preserve_zero_and_family_ranges() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;
    let cases = [
        (
            THREAD_TOOLS_FRAGMENT_BYTES_METRIC,
            THREAD_TOOLS_METRIC_BUCKETS.as_slice(),
            32_768_i64,
            512,
        ),
        (
            THREAD_SKILLS_ENABLED_TOTAL_METRIC,
            THREAD_SKILLS_COUNT_METRIC_BUCKETS.as_slice(),
            512,
            514,
        ),
        (
            THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_METRIC,
            THREAD_SKILLS_DESCRIPTION_TRUNCATED_CHARS_BUCKETS.as_slice(),
            131_072,
            512,
        ),
        (
            THREAD_SKILLS_TRUNCATED_METRIC,
            THREAD_SKILLS_TRUNCATED_BUCKETS,
            1,
            3,
        ),
    ];
    for (name, boundaries, maximum, _) in cases {
        for value in [0, 1, maximum, maximum + 1] {
            metrics.histogram_with_boundaries(name, value, boundaries, &[])?;
        }
    }
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    for (name, expected_bounds, maximum, bucket_count) in cases {
        let (bounds, counts, sum, count) = histogram_data(&resource_metrics, name);
        assert_eq!(bounds.as_slice(), expected_bounds);
        assert_eq!(bounds.len(), bucket_count - 1);
        assert_eq!(bounds[0], 0.0);
        assert_eq!(bounds[1], 1.0);
        assert_eq!(bounds[bucket_count - 2], maximum as f64);
        assert!(bounds.windows(/*size*/ 2).all(|pair| pair[0] < pair[1]));
        let mut expected_counts = vec![0; bucket_count];
        for index in [0, 1, bucket_count - 2, bucket_count - 1] {
            expected_counts[index] += 1;
        }
        assert_eq!(counts, expected_counts);
        assert_eq!((sum, count), ((2 * maximum + 2) as f64, 4));
    }

    Ok(())
}

// Ensures defaults merge per line and overrides take precedence.
#[test]
fn send_merges_default_tags_per_line() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[
        ("service", "codex-cli"),
        ("env", "prod"),
        ("region", "us"),
    ])?;

    metrics.counter(
        "codex.alpha",
        /*inc*/ 1,
        &[("env", "dev"), ("component", "alpha")],
    )?;
    metrics.counter(
        "codex.beta",
        /*inc*/ 2,
        &[("service", "worker"), ("component", "beta")],
    )?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let alpha_metric =
        find_metric(&resource_metrics, "codex.alpha").expect("codex.alpha metric missing");
    let alpha_point = match alpha_metric.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                points[0]
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };
    assert_eq!(alpha_point.value(), 1);
    let alpha_attrs = attributes_to_map(alpha_point.attributes());
    let expected_alpha_attrs = BTreeMap::from([
        ("component".to_string(), "alpha".to_string()),
        ("env".to_string(), "dev".to_string()),
        ("region".to_string(), "us".to_string()),
        ("service".to_string(), "codex-cli".to_string()),
    ]);
    assert_eq!(alpha_attrs, expected_alpha_attrs);

    let beta_metric =
        find_metric(&resource_metrics, "codex.beta").expect("codex.beta metric missing");
    let beta_point = match beta_metric.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                points[0]
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };
    assert_eq!(beta_point.value(), 2);
    let beta_attrs = attributes_to_map(beta_point.attributes());
    let expected_beta_attrs = BTreeMap::from([
        ("component".to_string(), "beta".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("region".to_string(), "us".to_string()),
        ("service".to_string(), "worker".to_string()),
    ]);
    assert_eq!(beta_attrs, expected_beta_attrs);

    Ok(())
}

// Verifies enqueued metrics are delivered by the background worker.
#[test]
fn client_sends_enqueued_metric() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    metrics.counter("codex.turns", /*inc*/ 1, &[("model", "gpt-5.1")])?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let counter = find_metric(&resource_metrics, "codex.turns").expect("counter metric missing");
    let points = match counter.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                sum.data_points().collect::<Vec<_>>()
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };
    assert_eq!(points.len(), 1);
    let point = points[0];
    assert_eq!(point.value(), 1);
    let attrs = attributes_to_map(point.attributes());
    assert_eq!(attrs.get("model").map(String::as_str), Some("gpt-5.1"));

    Ok(())
}

// Ensures shutdown flushes successfully with in-memory exporters.
#[test]
fn shutdown_flushes_in_memory_exporter() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    metrics.counter("codex.turns", /*inc*/ 1, &[])?;
    metrics.shutdown()?;

    let resource_metrics = latest_metrics(&exporter);
    let counter = find_metric(&resource_metrics, "codex.turns").expect("counter metric missing");
    let points = match counter.data() {
        opentelemetry_sdk::metrics::data::AggregatedMetrics::U64(data) => match data {
            opentelemetry_sdk::metrics::data::MetricData::Sum(sum) => {
                sum.data_points().collect::<Vec<_>>()
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };
    assert_eq!(points.len(), 1);

    Ok(())
}

// Ensures shutting down without recording metrics does not export anything.
#[test]
fn shutdown_without_metrics_exports_nothing() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;

    metrics.shutdown()?;

    let finished = exporter.get_finished_metrics().unwrap();
    assert!(finished.is_empty(), "expected no metrics exported");
    Ok(())
}
