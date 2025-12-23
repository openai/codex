use crate::config::Config;
use crate::config::types::OtelExporterKind as Kind;
use crate::config::types::OtelHttpProtocol as Protocol;
use crate::default_client::originator;
use codex_otel::config::OtelExporter;
use codex_otel::config::OtelHttpProtocol;
use codex_otel::config::OtelSettings;
use codex_otel::config::OtelTlsConfig as OtelTlsSettings;
use codex_otel::metrics::MetricsConfig;
use codex_otel::traces::otel_provider::OtelProvider;
use std::error::Error;

#[cfg(feature = "statsig-default-metrics-exporter")]
use codex_otel::config::statsig_default_metrics_exporter;

/// Build an OpenTelemetry provider from the app Config.
///
/// Returns `None` when OTEL export is disabled.
pub fn build_provider(
    config: &Config,
    service_version: &str,
) -> Result<Option<OtelProvider>, Box<dyn Error>> {
    let to_otel_exporter = |kind: &Kind| match kind {
        Kind::None => OtelExporter::None,
        Kind::OtlpHttp {
            endpoint,
            headers,
            protocol,
            tls,
        } => {
            let protocol = match protocol {
                Protocol::Json => OtelHttpProtocol::Json,
                Protocol::Binary => OtelHttpProtocol::Binary,
            };

            OtelExporter::OtlpHttp {
                endpoint: endpoint.clone(),
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                protocol,
                tls: tls.as_ref().map(|config| OtelTlsSettings {
                    ca_certificate: config.ca_certificate.clone(),
                    client_certificate: config.client_certificate.clone(),
                    client_private_key: config.client_private_key.clone(),
                }),
            }
        }
        Kind::OtlpGrpc {
            endpoint,
            headers,
            tls,
        } => OtelExporter::OtlpGrpc {
            endpoint: endpoint.clone(),
            headers: headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            tls: tls.as_ref().map(|config| OtelTlsSettings {
                ca_certificate: config.ca_certificate.clone(),
                client_certificate: config.client_certificate.clone(),
                client_private_key: config.client_private_key.clone(),
            }),
        },
    };

    let exporter = to_otel_exporter(&config.otel.exporter);
    let trace_exporter = to_otel_exporter(&config.otel.trace_exporter);
    let metrics_exporter = to_otel_exporter(&config.otel.metrics_exporter);

    let metrics = match &metrics_exporter {
        OtelExporter::None => None,
        _ => Some(MetricsConfig::otlp(
            config.otel.environment.to_string(),
            originator().value.to_owned(),
            service_version.to_string(),
            metrics_exporter,
        )),
    };

    let metrics = metrics.or_else(|| default_metrics(config, service_version));

    OtelProvider::from(&OtelSettings {
        service_name: originator().value.to_owned(),
        service_version: service_version.to_string(),
        codex_home: config.codex_home.clone(),
        environment: config.otel.environment.to_string(),
        exporter,
        trace_exporter,
        metrics,
    })
}

#[cfg(feature = "statsig-default-metrics-exporter")]
fn default_metrics(config: &Config, service_version: &str) -> Option<MetricsConfig> {
    if is_test_process() {
        return None;
    }

    if matches!(config.otel.exporter, Kind::None)
        && matches!(config.otel.trace_exporter, Kind::None)
    {
        return None;
    }

    Some(MetricsConfig::otlp(
        config.otel.environment.to_string(),
        originator().value.to_owned(),
        service_version.to_string(),
        statsig_default_metrics_exporter(),
    ))
}

#[cfg(not(feature = "statsig-default-metrics-exporter"))]
fn default_metrics(_config: &Config, _service_version: &str) -> Option<MetricsConfig> {
    None
}

fn is_test_process() -> bool {
    std::env::var_os("RUST_TEST_THREADS").is_some()
}

/// Filter predicate for exporting only Codex-owned events via OTEL.
/// Keeps events that originated from codex_otel module
pub fn codex_export_filter(meta: &tracing::Metadata<'_>) -> bool {
    meta.target().starts_with("codex_otel")
}
