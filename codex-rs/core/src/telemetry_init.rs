use std::path::PathBuf;

use opentelemetry_sdk::trace::Tracer;

use crate::config::Config;
use crate::config_types::TelemetryExporterKind as Kind;
use codex_telemetry as telemetry;

/// Build an OpenTelemetry tracer and guard from the app Config.
///
/// Returns `None` when telemetry is disabled.
pub fn build_otel_layer_from_config(
    config: &Config,
    service_name: &str,
    service_version: &str,
) -> Option<(telemetry::Guard, Tracer)> {
    let exporter = match config.telemetry.exporter {
        Kind::None => telemetry::Exporter::None,
        Kind::OtlpFile => telemetry::Exporter::OtlpFile {
            path: PathBuf::new(),
            rotate_mb: config.telemetry.rotate_mb,
        },
        Kind::OtlpHttp => telemetry::Exporter::OtlpHttp {
            endpoint: config
                .telemetry
                .endpoint
                .clone()
                .unwrap_or_else(|| "http://localhost:4318".to_string()),
            headers: config
                .telemetry
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        },
        Kind::OtlpGrpc => telemetry::Exporter::OtlpGrpc {
            endpoint: config
                .telemetry
                .endpoint
                .clone()
                .unwrap_or_else(|| "http://localhost:4317".to_string()),
            headers: config
                .telemetry
                .headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        },
    };

    telemetry::build_layer(&telemetry::Settings {
        enabled: config.telemetry.enabled,
        exporter,
        service_name: service_name.to_string(),
        service_version: service_version.to_string(),
        codex_home: Some(config.codex_home.clone()),
    })
}
