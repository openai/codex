//! Maps OTEL requirements and masks exporter credentials in app-server responses.

use codex_app_server_protocol::OtelExporter;
use codex_app_server_protocol::OtelHttpProtocol;
use codex_app_server_protocol::OtelRequirements;
use codex_app_server_protocol::OtelTlsConfig;
use codex_app_server_protocol::OtelToolResultLogConfig;
use codex_app_server_protocol::SerdeRedactedString;
use codex_config::OtelRequirementsToml;
use codex_config::types::OtelExporterKind;
use codex_config::types::OtelHttpProtocol as CoreOtelHttpProtocol;
use codex_config::types::OtelTlsConfig as CoreOtelTlsConfig;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

pub(crate) fn map_otel_requirements_to_api(otel: OtelRequirementsToml) -> OtelRequirements {
    OtelRequirements {
        tool_result: otel.tool_result.map(|value| OtelToolResultLogConfig {
            max_bytes: value.max_bytes,
        }),
        log_user_prompt: otel.log_user_prompt,
        log_agent_responses: otel.log_agent_responses,
        log_guardian_assessments: otel.log_guardian_assessments,
        environment: otel.environment,
        exporter: otel.exporter.map(map_otel_exporter_to_api),
        trace_exporter: otel.trace_exporter.map(map_otel_exporter_to_api),
        metrics_exporter: otel.metrics_exporter.map(map_otel_exporter_to_api),
        span_attributes: otel.span_attributes,
        tracestate: otel.tracestate,
    }
}

fn map_otel_exporter_to_api(exporter: OtelExporterKind) -> OtelExporter {
    let response_headers = |headers: HashMap<String, _>| {
        headers
            .into_iter()
            .map(|(name, value)| (name, SerdeRedactedString::from(value)))
            .collect()
    };
    match exporter {
        OtelExporterKind::None => OtelExporter::None,
        OtelExporterKind::Statsig => OtelExporter::Statsig,
        OtelExporterKind::OtlpHttp {
            endpoint,
            headers,
            protocol,
            tls,
        } => OtelExporter::OtlpHttp {
            endpoint,
            headers: response_headers(headers),
            protocol: match protocol {
                CoreOtelHttpProtocol::Binary => OtelHttpProtocol::Binary,
                CoreOtelHttpProtocol::Json => OtelHttpProtocol::Json,
            },
            tls: tls.map(map_otel_tls_to_api),
        },
        OtelExporterKind::OtlpGrpc {
            endpoint,
            headers,
            tls,
        } => OtelExporter::OtlpGrpc {
            endpoint,
            headers: response_headers(headers),
            tls: tls.map(map_otel_tls_to_api),
        },
    }
}

fn map_otel_tls_to_api(tls: CoreOtelTlsConfig) -> OtelTlsConfig {
    OtelTlsConfig {
        ca_certificate: tls.ca_certificate.map(Into::into),
        client_certificate: tls.client_certificate.map(Into::into),
        client_private_key: tls.client_private_key.map(Into::into),
    }
}

/// Masks exporter header values before OTEL configuration crosses the API boundary.
pub(crate) fn redact_otel_exporter_headers(otel: &mut JsonValue) {
    for field in ["exporter", "trace_exporter", "metrics_exporter"] {
        let Some(exporter) = otel.get_mut(field).and_then(JsonValue::as_object_mut) else {
            continue;
        };

        for config in exporter.values_mut() {
            let Some(headers) = config.get_mut("headers").and_then(JsonValue::as_object_mut) else {
                continue;
            };

            for value in headers.values_mut() {
                *value = SerdeRedactedString::PLACEHOLDER.into();
            }
        }
    }
}
