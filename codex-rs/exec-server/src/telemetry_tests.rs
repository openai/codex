use std::collections::BTreeMap;
use std::time::Duration;

use codex_otel::MetricsConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;

use super::CONNECTIONS_TOTAL_METRIC;
use super::ConnectionTransport;
use super::ExecServerTelemetry;
use super::REMOTE_RECONNECTS_TOTAL_METRIC;
use super::REMOTE_RENDEZVOUS_METRICS;
use super::REQUESTS_TOTAL_METRIC;
use super::TRANSPORT_POLICY_CELL_TAG;
use super::TRANSPORT_POLICY_STATE_TAG;
use crate::ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION;
use crate::EnvironmentRegistryTransportPolicy;

#[test]
fn policy_cell_and_state_tag_request_connection_setup_and_reconnect_metrics() {
    let exporter = InMemoryMetricExporter::default();
    let metrics = codex_otel::MetricsClient::new(MetricsConfig::in_memory(
        "test",
        "codex-exec-server",
        env!("CARGO_PKG_VERSION"),
        exporter.clone(),
    ))
    .expect("metrics");
    let telemetry = ExecServerTelemetry::new(metrics.clone());
    let active_c11 = EnvironmentRegistryTransportPolicy {
        version: ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
        assignment_epoch: "experiment-1".to_string(),
        outbound_tcp_nodelay: true,
        rendezvous_accepted_tcp_nodelay: true,
    };
    telemetry.set_transport_policy(&active_c11);

    drop(telemetry.connection_started(ConnectionTransport::Relay));
    telemetry.request_completed("process/start", "success", Duration::from_millis(1));
    telemetry.remote_rendezvous_completed("success", Duration::from_millis(1));
    telemetry.remote_reconnect("disconnected");
    for policy in [
        EnvironmentRegistryTransportPolicy::default(),
        EnvironmentRegistryTransportPolicy {
            version: ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
            assignment_epoch: "off".to_string(),
            outbound_tcp_nodelay: true,
            rendezvous_accepted_tcp_nodelay: true,
        },
        EnvironmentRegistryTransportPolicy {
            version: ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION,
            assignment_epoch: "experiment-1".to_string(),
            outbound_tcp_nodelay: false,
            rendezvous_accepted_tcp_nodelay: false,
        },
        EnvironmentRegistryTransportPolicy {
            version: ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION + 1,
            assignment_epoch: "future".to_string(),
            outbound_tcp_nodelay: true,
            rendezvous_accepted_tcp_nodelay: true,
        },
    ] {
        telemetry.set_transport_policy(&policy);
        telemetry.request_completed("process/start", "success", Duration::from_millis(1));
    }

    metrics.shutdown().expect("shutdown metrics");
    let resource_metrics = exporter
        .get_finished_metrics()
        .expect("finished metrics")
        .into_iter()
        .last()
        .expect("metrics export");
    let expected_names = [
        CONNECTIONS_TOTAL_METRIC,
        REQUESTS_TOTAL_METRIC,
        REMOTE_RENDEZVOUS_METRICS.total_name,
        REMOTE_RECONNECTS_TOTAL_METRIC,
    ];
    let policy_tags_by_metric = resource_metrics
        .scope_metrics()
        .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        .filter(|metric| expected_names.contains(&metric.name()))
        .map(|metric| {
            let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                panic!("expected counter metric for {}", metric.name());
            };
            let mut policy_tags = sum
                .data_points()
                .map(|point| {
                    let attributes = point.attributes().collect::<Vec<_>>();
                    let tag = |key| {
                        attributes
                            .iter()
                            .find(|attribute| attribute.key.as_str() == key)
                            .map(|attribute| attribute.value.as_str().into_owned())
                            .unwrap_or_else(|| panic!("missing {key} tag"))
                    };
                    (
                        tag(TRANSPORT_POLICY_CELL_TAG),
                        tag(TRANSPORT_POLICY_STATE_TAG),
                    )
                })
                .collect::<Vec<_>>();
            policy_tags.sort();
            (metric.name().to_string(), policy_tags)
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        policy_tags_by_metric,
        BTreeMap::from([
            (
                CONNECTIONS_TOTAL_METRIC.to_string(),
                vec![("c11".to_string(), "active".to_string())]
            ),
            (
                REMOTE_RECONNECTS_TOTAL_METRIC.to_string(),
                vec![("c11".to_string(), "active".to_string())],
            ),
            (
                REMOTE_RENDEZVOUS_METRICS.total_name.to_string(),
                vec![("c11".to_string(), "active".to_string())],
            ),
            (
                REQUESTS_TOTAL_METRIC.to_string(),
                vec![
                    ("c00".to_string(), "active".to_string()),
                    ("c00".to_string(), "inactive".to_string()),
                    ("c00".to_string(), "legacy".to_string()),
                    ("c00".to_string(), "unknown".to_string()),
                    ("c11".to_string(), "active".to_string()),
                ],
            ),
        ])
    );
}
