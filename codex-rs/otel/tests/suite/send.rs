use crate::harness::attributes_to_map;
use crate::harness::build_metrics_with_defaults;
use crate::harness::find_metric;
use crate::harness::gauge_i64;
use crate::harness::histogram_data;
use crate::harness::histogram_f64;
use crate::harness::latest_metrics;
use crate::harness::sum_u64;
use codex_otel::Result;
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
    assert_eq!(counter.description.as_ref(), "Total number of Codex turns.");
    let counter_sum = sum_u64(counter);
    assert_eq!(counter_sum.data_points.len(), 1);
    assert_eq!(counter_sum.data_points[0].value, 1);
    let counter_attributes = attributes_to_map(counter_sum.data_points[0].attributes.iter());

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

    let histogram = histogram_f64(
        find_metric(&resource_metrics, "codex.tool_latency")
            .expect("codex.tool_latency histogram should exist"),
    );
    let histogram_attrs = attributes_to_map(
        histogram
            .data_points
            .first()
            .expect("codex.tool_latency histogram attributes should exist")
            .attributes
            .iter(),
    );
    let expected_histogram_attributes = BTreeMap::from([
        ("service".to_string(), "codex-cli".to_string()),
        ("env".to_string(), "prod".to_string()),
        ("tool".to_string(), "shell".to_string()),
    ]);
    assert_eq!(histogram_attrs, expected_histogram_attributes);

    let gauge = find_metric(&resource_metrics, "codex.active").expect("gauge metric missing");
    assert_eq!(
        gauge.description.as_ref(),
        "Number of active Codex operations."
    );
    let gauge_point = gauge_i64(gauge).data_points.first().expect("gauge point");
    assert_eq!(gauge_point.value, 2);
    assert_eq!(
        attributes_to_map(gauge_point.attributes.iter()),
        BTreeMap::from([
            ("component".to_string(), "test".to_string()),
            ("env".to_string(), "prod".to_string()),
            ("service".to_string(), "codex-cli".to_string()),
        ])
    );

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
    let alpha_sum = sum_u64(alpha_metric);
    assert_eq!(alpha_sum.data_points.len(), 1);
    let alpha_point = &alpha_sum.data_points[0];
    assert_eq!(alpha_point.value, 1);
    let alpha_attrs = attributes_to_map(alpha_point.attributes.iter());
    let expected_alpha_attrs = BTreeMap::from([
        ("component".to_string(), "alpha".to_string()),
        ("env".to_string(), "dev".to_string()),
        ("region".to_string(), "us".to_string()),
        ("service".to_string(), "codex-cli".to_string()),
    ]);
    assert_eq!(alpha_attrs, expected_alpha_attrs);

    let beta_metric =
        find_metric(&resource_metrics, "codex.beta").expect("codex.beta metric missing");
    let beta_sum = sum_u64(beta_metric);
    assert_eq!(beta_sum.data_points.len(), 1);
    let beta_point = &beta_sum.data_points[0];
    assert_eq!(beta_point.value, 2);
    let beta_attrs = attributes_to_map(beta_point.attributes.iter());
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
    let points = &sum_u64(counter).data_points;
    assert_eq!(points.len(), 1);
    let point = &points[0];
    assert_eq!(point.value, 1);
    let attrs = attributes_to_map(point.attributes.iter());
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
    let points = &sum_u64(counter).data_points;
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
