use super::*;
use codex_otel::MULTI_AGENT_SPAWN_PHASE_DURATION_METRIC;
use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn records_bounded_spawn_phase_metrics() {
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory(
            "test",
            "codex-core",
            env!("CARGO_PKG_VERSION"),
            InMemoryMetricExporter::default(),
        )
        .with_runtime_reader(),
    )
    .expect("create in-memory metrics client");
    let telemetry = SessionTelemetry::new(
        ThreadId::new(),
        "test-model",
        "test-model",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test-originator".to_string(),
        /*log_user_prompts*/ false,
        "test-terminal".to_string(),
        SessionSource::Cli,
    )
    .with_product_sku(Some("codex"))
    .with_metrics_without_metadata_tags(metrics.clone());

    record_spawn_success(
        &telemetry,
        Some(&SpawnAgentForkMode::LastNTurns(3)),
        MultiAgentVersion::V2,
        SpawnMeasurements {
            history_mode: ThreadHistoryMode::Paginated,
            residency_reservation: Some(Duration::from_millis(11)),
            fork_context: Some(Duration::from_millis(13)),
            child_create: Duration::from_millis(17),
            durability_wait: Duration::from_millis(19),
            input_admission: Duration::from_millis(23),
            total: Duration::from_millis(29),
        },
    );

    let snapshot = metrics.snapshot().expect("snapshot spawn metrics");
    let metric = snapshot
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .find(|metric| metric.name() == MULTI_AGENT_SPAWN_PHASE_DURATION_METRIC)
        .expect("spawn phase metric");
    let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = metric.data() else {
        panic!("expected spawn phase histogram");
    };
    let phases = histogram
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
            assert_eq!(attributes.len(), 5);
            assert_eq!(attributes["fork_mode"], "last_n");
            assert_eq!(attributes["history_mode"], "paginated");
            assert_eq!(attributes["multi_agent_version"], "v2");
            assert_eq!(attributes["product_sku"], "codex");
            (attributes["phase"].clone(), (point.count(), point.sum()))
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        phases,
        BTreeMap::from([
            ("child_create".to_string(), (1, 17.0)),
            ("durability_wait".to_string(), (1, 19.0)),
            ("fork_context".to_string(), (1, 13.0)),
            ("input_admission".to_string(), (1, 23.0)),
            ("residency_reservation".to_string(), (1, 11.0)),
            ("total".to_string(), (1, 29.0)),
        ])
    );
}
