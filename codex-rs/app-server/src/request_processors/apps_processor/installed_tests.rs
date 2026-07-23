use super::APPS_INSTALLED_DURATION_METRIC;
use super::AppsInstalledSnapshotMetrics;
use super::record_apps_installed_metrics;
use anyhow::Result;
use codex_app_server_protocol::AppsInstalledResponse;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ScopeMetrics;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Instant;

fn test_metrics() -> Result<MetricsClient> {
    Ok(MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-app-server",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )?)
}

#[test]
fn installed_duration_records_one_sample_per_success_with_legacy_comparison_dimensions()
-> Result<()> {
    let metrics = test_metrics()?;
    let response = AppsInstalledResponse { apps: Vec::new() };

    record_apps_installed_metrics(
        &metrics,
        Instant::now(),
        /*force_refresh*/ false,
        /*retained_previous_snapshot*/ false,
        "not_requested",
        AppsInstalledSnapshotMetrics {
            age: None,
            tool_count: 0,
        },
        Some(&response),
    );
    record_apps_installed_metrics(
        &metrics,
        Instant::now(),
        /*force_refresh*/ true,
        /*retained_previous_snapshot*/ false,
        "success",
        AppsInstalledSnapshotMetrics {
            age: None,
            tool_count: 0,
        },
        Some(&response),
    );

    let snapshot = metrics.snapshot()?;
    let metric = snapshot
        .scope_metrics()
        .flat_map(ScopeMetrics::metrics)
        .find(|metric| metric.name() == APPS_INSTALLED_DURATION_METRIC)
        .expect("installed duration metric should be recorded");
    let mut points = match metric.data() {
        AggregatedMetrics::F64(MetricData::Histogram(histogram)) => histogram
            .data_points()
            .map(|point| {
                let attributes = point
                    .attributes()
                    .map(|attribute| {
                        (
                            attribute.key.as_str().to_string(),
                            attribute.value.as_str().to_string(),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                (attributes, point.count())
            })
            .collect::<Vec<_>>(),
        _ => panic!("installed duration should be a floating-point histogram"),
    };
    points.sort_by(|(left, _), (right, _)| left.cmp(right));

    assert_eq!(
        points,
        vec![
            (
                BTreeMap::from([
                    ("force_refresh".to_string(), "false".to_string()),
                    ("outcome".to_string(), "success".to_string()),
                    ("path".to_string(), "installed".to_string()),
                    ("refresh".to_string(), "not_requested".to_string()),
                    ("reload".to_string(), "false".to_string()),
                    (
                        "retained_previous_snapshot".to_string(),
                        "false".to_string()
                    ),
                ]),
                1,
            ),
            (
                BTreeMap::from([
                    ("force_refresh".to_string(), "true".to_string()),
                    ("outcome".to_string(), "success".to_string()),
                    ("path".to_string(), "installed".to_string()),
                    ("refresh".to_string(), "success".to_string()),
                    ("reload".to_string(), "true".to_string()),
                    (
                        "retained_previous_snapshot".to_string(),
                        "false".to_string()
                    ),
                ]),
                1,
            ),
        ]
    );

    Ok(())
}

#[test]
fn installed_duration_does_not_record_failed_requests() -> Result<()> {
    let metrics = test_metrics()?;

    record_apps_installed_metrics(
        &metrics,
        Instant::now(),
        /*force_refresh*/ true,
        /*retained_previous_snapshot*/ true,
        "error",
        AppsInstalledSnapshotMetrics {
            age: None,
            tool_count: 0,
        },
        /*response*/ None,
    );

    let snapshot = metrics.snapshot()?;
    assert!(
        snapshot
            .scope_metrics()
            .flat_map(ScopeMetrics::metrics)
            .all(|metric| metric.name() != APPS_INSTALLED_DURATION_METRIC),
        "failed installed requests must not record a successful duration sample",
    );

    Ok(())
}
