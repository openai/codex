use crate::harness::attributes_to_map;
use crate::harness::build_metrics_with_defaults;
use crate::harness::find_metric;
use crate::harness::histogram_data;
use crate::harness::latest_metrics;
use codex_otel::PLUGIN_INSTALL_ELICITATION_SENT_METRIC;
use codex_otel::PLUGIN_INSTALL_SUGGESTION_METRIC;
use codex_otel::Result;
use codex_otel::SessionTelemetry;
use codex_otel::TOOL_CALL_COUNT_METRIC;
use codex_otel::TOOL_CALL_DURATION_METRIC;
use codex_otel::TelemetryAuthMode;
use codex_protocol::ThreadId;
use codex_protocol::ToolName;
use codex_protocol::protocol::SessionSource;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;

#[test]
fn tool_metrics_keep_product_skus_separate_on_a_shared_client() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;
    let manager = SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_without_metadata_tags(metrics);
    let mut expected_counts = BTreeMap::new();
    let mut expected_durations = BTreeMap::new();

    for (sku, expected_sku, success, duration_ms) in [
        (Some("codex"), Some("codex"), true, 11),
        (Some("customer-specific-value"), Some("other"), false, 37),
        (Some("another-unrecognized-value"), Some("other"), false, 41),
        (Some(""), None, true, 43),
        (None, None, true, 47),
    ] {
        let telemetry = manager.clone().with_product_sku(sku);
        telemetry.tool_result_with_tags(
            &ToolName::plain("spawn_agent"),
            "call-1",
            "{}",
            Duration::from_millis(duration_ms),
            success,
            "result",
            &[("sandbox", "workspace")],
            &[],
        );
        let mut labels = BTreeMap::from([
            ("tool".to_string(), "spawn_agent".to_string()),
            ("success".to_string(), success.to_string()),
            ("sandbox".to_string(), "workspace".to_string()),
        ]);
        if let Some(sku) = expected_sku {
            labels.insert("product_sku".to_string(), sku.to_string());
        }
        *expected_counts.entry(labels.clone()).or_insert(0) += 1;
        let (count, sum) = expected_durations.entry(labels).or_insert((0, 0.0));
        *count += 1;
        *sum += duration_ms as f64;
    }
    manager.shutdown_metrics()?;
    let resource_metrics = latest_metrics(&exporter);
    let counter = find_metric(&resource_metrics, TOOL_CALL_COUNT_METRIC).expect("tool counter");
    let AggregatedMetrics::U64(MetricData::Sum(sum)) = counter.data() else {
        panic!("expected tool counter");
    };
    assert_eq!(
        sum.data_points()
            .map(|point| (attributes_to_map(point.attributes()), point.value()))
            .collect::<BTreeMap<_, _>>(),
        expected_counts,
    );
    let duration =
        find_metric(&resource_metrics, TOOL_CALL_DURATION_METRIC).expect("tool duration");
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = duration.data() else {
        panic!("expected tool duration histogram");
    };
    assert_eq!(
        histogram
            .data_points()
            .map(|point| (
                attributes_to_map(point.attributes()),
                (point.count(), point.sum()),
            ))
            .collect::<BTreeMap<_, _>>(),
        expected_durations,
    );
    Ok(())
}

// Ensures SessionTelemetry attaches metadata tags when forwarding metrics.
#[test]
fn manager_attaches_metadata_tags_to_metrics() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[("service", "codex-cli")])?;
    let manager = SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        Some("account-id".to_string()),
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test_originator".to_string(),
        /*log_user_prompts*/ true,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics(metrics);

    manager.counter(
        "codex.session_started",
        /*inc*/ 1,
        &[("source", "tui")],
    );
    for tokens in [32_000, 256_000] {
        manager.histogram_with_boundaries(
            "codex.request_tokens",
            tokens,
            &[16_000.0, 128_000.0, 512_000.0],
            &[("source", "tui")],
        );
    }
    manager.shutdown_metrics()?;

    let resource_metrics = latest_metrics(&exporter);
    assert_eq!(
        histogram_data(&resource_metrics, "codex.request_tokens"),
        (
            vec![16_000.0, 128_000.0, 512_000.0],
            vec![0, 1, 1, 0],
            288_000.0,
            2,
        )
    );
    let metric =
        find_metric(&resource_metrics, "codex.session_started").expect("counter metric missing");
    let attrs = match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    let expected = BTreeMap::from([
        (
            "app.version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
        (
            "auth_mode".to_string(),
            TelemetryAuthMode::ApiKey.to_string(),
        ),
        ("model".to_string(), "gpt-5.1".to_string()),
        ("originator".to_string(), "test_originator".to_string()),
        ("service".to_string(), "codex-cli".to_string()),
        ("session_source".to_string(), "cli".to_string()),
        ("source".to_string(), "tui".to_string()),
    ]);
    assert_eq!(attrs, expected);

    Ok(())
}

// Ensures metadata tagging can be disabled when recording via SessionTelemetry.
#[test]
fn manager_allows_disabling_metadata_tags() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;
    let manager = SessionTelemetry::new(
        ThreadId::new(),
        "gpt-4o",
        "gpt-4o",
        Some("account-id".to_string()),
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test_originator".to_string(),
        /*log_user_prompts*/ true,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_without_metadata_tags(metrics);

    manager.counter(
        "codex.session_started",
        /*inc*/ 1,
        &[("source", "tui")],
    );
    manager.shutdown_metrics()?;

    let resource_metrics = latest_metrics(&exporter);
    let metric =
        find_metric(&resource_metrics, "codex.session_started").expect("counter metric missing");
    let attrs = match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    let expected = BTreeMap::from([("source".to_string(), "tui".to_string())]);
    assert_eq!(attrs, expected);

    Ok(())
}

#[test]
fn manager_attaches_optional_service_name_tag() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;
    let manager = SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_service_name("my_app_server_client")
    .with_metrics(metrics);

    manager.counter("codex.session_started", /*inc*/ 1, &[]);
    manager.shutdown_metrics()?;

    let resource_metrics = latest_metrics(&exporter);
    let metric =
        find_metric(&resource_metrics, "codex.session_started").expect("counter metric missing");
    let attrs = match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    assert_eq!(
        attrs.get("service_name"),
        Some(&"my_app_server_client".to_string())
    );

    Ok(())
}

#[test]
fn manager_records_plugin_install_suggestion_metric() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;
    let manager = SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        Some("account-id".to_string()),
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_without_metadata_tags(metrics);

    manager.record_plugin_install_suggestion(
        "connector",
        "connector_calendar",
        "Google Calendar",
        "accept",
        /*user_confirmed*/ true,
        /*completed*/ false,
    );
    manager.shutdown_metrics()?;

    let resource_metrics = latest_metrics(&exporter);
    let metric = find_metric(&resource_metrics, PLUGIN_INSTALL_SUGGESTION_METRIC)
        .expect("plugin install suggestion metric missing");
    let attrs = match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    assert_eq!(
        attrs,
        BTreeMap::from([
            ("completed".to_string(), "false".to_string()),
            ("response_action".to_string(), "accept".to_string()),
            ("tool_type".to_string(), "connector".to_string()),
        ])
    );

    Ok(())
}

#[test]
fn manager_records_plugin_install_elicitation_sent_metric() -> Result<()> {
    let (metrics, exporter) = build_metrics_with_defaults(&[])?;
    let manager = SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        Some("account-id".to_string()),
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test_originator".to_string(),
        /*log_user_prompts*/ false,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_without_metadata_tags(metrics);

    manager.record_plugin_install_elicitation_sent("plugin", "slack@openai-curated", "Slack");
    manager.shutdown_metrics()?;

    let resource_metrics = latest_metrics(&exporter);
    let metric = find_metric(&resource_metrics, PLUGIN_INSTALL_ELICITATION_SENT_METRIC)
        .expect("plugin install elicitation sent metric missing");
    let attrs = match metric.data() {
        AggregatedMetrics::U64(data) => match data {
            MetricData::Sum(sum) => {
                let points: Vec<_> = sum.data_points().collect();
                assert_eq!(points.len(), 1);
                attributes_to_map(points[0].attributes())
            }
            _ => panic!("unexpected counter aggregation"),
        },
        _ => panic!("unexpected counter data type"),
    };

    assert_eq!(
        attrs,
        BTreeMap::from([("tool_type".to_string(), "plugin".to_string())])
    );

    Ok(())
}
