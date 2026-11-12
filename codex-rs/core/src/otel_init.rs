use crate::config::Config;
use codex_config::types::OtelExporterKind as Kind;
use codex_config::types::OtelHttpProtocol as Protocol;
use codex_features::Feature;
use codex_login::default_client::originator;
use codex_otel::OtelExporter;
use codex_otel::OtelHeadersHelper;
use codex_otel::OtelHttpProtocol;
use codex_otel::OtelProvider;
use codex_otel::OtelSettings;
use codex_otel::OtelTlsConfig as OtelTlsSettings;
use std::error::Error;
use std::num::NonZeroU64;
use std::time::Duration;

const DEFAULT_HEADERS_HELPER_REFRESH_INTERVAL_MS: u64 = 29 * 60 * 1_000;

/// Build an OpenTelemetry provider from the app Config.
///
/// Returns `None` when OTEL export is disabled.
pub fn build_provider(
    config: &Config,
    service_version: &str,
    service_name_override: Option<&str>,
    default_analytics_enabled: bool,
) -> Result<Option<OtelProvider>, Box<dyn Error>> {
    let to_otel_exporter = |kind: &Kind| match kind {
        Kind::None => OtelExporter::None,
        Kind::Statsig => OtelExporter::Statsig,
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
                    .map(|(name, value)| (name.clone(), value.as_str().to_owned()))
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
                .map(|(name, value)| (name.clone(), value.as_str().to_owned()))
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
    // Analytics consent controls the built-in exporter, not explicit OTLP collectors.
    let metrics_exporter = if matches!(config.otel.metrics_exporter, Kind::Statsig)
        && !config
            .analytics_enabled
            .unwrap_or(default_analytics_enabled)
    {
        OtelExporter::None
    } else {
        to_otel_exporter(&config.otel.metrics_exporter)
    };

    let originator = originator();
    let service_name = service_name_override.unwrap_or(originator.value.as_str());
    let runtime_metrics = config.features.enabled(Feature::RuntimeMetrics);

    let managed = config
        .config_layer_stack
        .requirements_toml()
        .otel
        .as_deref();
    let headers_helper = config
        .otel
        .headers_helper
        .as_ref()
        .map(|command| OtelHeadersHelper {
            command: command.clone(),
            apply_to_logs: managed.is_none_or(|otel| otel.exporter.is_none()),
            apply_to_traces: managed.is_none_or(|otel| otel.trace_exporter.is_none()),
            apply_to_metrics: managed.is_none_or(|otel| otel.metrics_exporter.is_none()),
            refresh_interval: Duration::from_millis(
                config
                    .otel
                    .headers_helper_refresh_interval_ms
                    .map_or(DEFAULT_HEADERS_HELPER_REFRESH_INTERVAL_MS, NonZeroU64::get),
            ),
        });
    OtelProvider::try_new(&OtelSettings {
        headers_helper,
        http_client_factory: config.http_client_factory(),
        service_name: service_name.to_string(),
        service_version: service_version.to_string(),
        codex_home: config.codex_home.to_path_buf(),
        environment: config.otel.environment.to_string(),
        exporter,
        trace_exporter,
        metrics_exporter,
        runtime_metrics,
        span_attributes: config.otel.span_attributes.clone(),
        tracestate: config.otel.tracestate.clone(),
    })
}

pub fn record_process_start(otel: Option<&OtelProvider>, originator: &str) {
    let Some(metrics) = otel.and_then(OtelProvider::metrics) else {
        return;
    };
    let _ = codex_otel::record_process_start_once(metrics, originator);
}

pub fn install_sqlite_telemetry(otel: Option<&OtelProvider>, originator: &str) {
    let Some(metrics) = otel.and_then(OtelProvider::metrics) else {
        return;
    };
    let telemetry = codex_rollout::sqlite_telemetry_recorder(metrics.clone(), originator);
    let _ = codex_state::install_process_db_telemetry(telemetry);
}
