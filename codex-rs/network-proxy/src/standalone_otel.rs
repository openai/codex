use codex_otel::config::OtelExporter;
use codex_otel::config::OtelHttpProtocol;
use codex_otel::config::OtelSettings;
use codex_otel::config::OtelTlsConfig;
use codex_otel::otel_provider::OtelProvider;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;

const DEFAULT_OTEL_ENVIRONMENT: &str = "dev";
const STANDALONE_SERVICE_NAME: &str = "codex_network_proxy";

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct StandaloneOtelConfigToml {
    #[serde(rename = "log_user_prompt")]
    pub(crate) _log_user_prompt: Option<bool>,
    pub(crate) environment: Option<String>,
    pub(crate) exporter: Option<StandaloneOtelExporterKind>,
    pub(crate) trace_exporter: Option<StandaloneOtelExporterKind>,
    pub(crate) metrics_exporter: Option<StandaloneOtelExporterKind>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StandaloneOtelHttpProtocol {
    Binary,
    Json,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct StandaloneOtelTlsConfigToml {
    pub(crate) ca_certificate: Option<AbsolutePathBuf>,
    pub(crate) client_certificate: Option<AbsolutePathBuf>,
    pub(crate) client_private_key: Option<AbsolutePathBuf>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum StandaloneOtelExporterKind {
    None,
    Statsig,
    OtlpHttp {
        endpoint: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        protocol: StandaloneOtelHttpProtocol,
        #[serde(default)]
        tls: Option<StandaloneOtelTlsConfigToml>,
    },
    OtlpGrpc {
        endpoint: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default)]
        tls: Option<StandaloneOtelTlsConfigToml>,
    },
}

#[cfg(test)]
#[derive(Debug, Deserialize, Default)]
struct StandaloneConfigToml {
    #[serde(default)]
    otel: Option<StandaloneOtelConfigToml>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedStandaloneOtelConfig {
    pub(crate) environment: String,
    pub(crate) exporter: StandaloneOtelExporterKind,
    pub(crate) trace_exporter: StandaloneOtelExporterKind,
    pub(crate) metrics_exporter: StandaloneOtelExporterKind,
}

#[cfg(test)]
pub(crate) fn parse_otel_config(raw: &str) -> Result<StandaloneOtelConfigToml, toml::de::Error> {
    let config: StandaloneConfigToml = toml::from_str(raw)?;
    Ok(config.otel.unwrap_or_default())
}

pub(crate) fn resolve_otel_config(
    config: StandaloneOtelConfigToml,
) -> ResolvedStandaloneOtelConfig {
    let exporter = config.exporter.unwrap_or(StandaloneOtelExporterKind::None);
    let trace_exporter = config.trace_exporter.unwrap_or_else(|| exporter.clone());
    let metrics_exporter = config
        .metrics_exporter
        .unwrap_or(StandaloneOtelExporterKind::Statsig);

    ResolvedStandaloneOtelConfig {
        environment: config
            .environment
            .unwrap_or_else(|| DEFAULT_OTEL_ENVIRONMENT.to_string()),
        exporter,
        trace_exporter,
        metrics_exporter,
    }
}

pub(crate) fn build_provider(
    config: StandaloneOtelConfigToml,
    codex_home: PathBuf,
    service_version: &str,
) -> Result<Option<OtelProvider>, Box<dyn Error>> {
    let resolved = resolve_otel_config(config);
    let settings = OtelSettings {
        environment: resolved.environment,
        service_name: STANDALONE_SERVICE_NAME.to_string(),
        service_version: service_version.to_string(),
        codex_home,
        exporter: to_otel_exporter(resolved.exporter),
        trace_exporter: to_otel_exporter(resolved.trace_exporter),
        metrics_exporter: to_otel_exporter(resolved.metrics_exporter),
        runtime_metrics: false,
    };
    OtelProvider::from(&settings)
}

fn to_otel_exporter(exporter: StandaloneOtelExporterKind) -> OtelExporter {
    match exporter {
        StandaloneOtelExporterKind::None => OtelExporter::None,
        StandaloneOtelExporterKind::Statsig => OtelExporter::Statsig,
        StandaloneOtelExporterKind::OtlpHttp {
            endpoint,
            headers,
            protocol,
            tls,
        } => OtelExporter::OtlpHttp {
            endpoint,
            headers,
            protocol: to_otel_http_protocol(protocol),
            tls: to_otel_tls_config(tls),
        },
        StandaloneOtelExporterKind::OtlpGrpc {
            endpoint,
            headers,
            tls,
        } => OtelExporter::OtlpGrpc {
            endpoint,
            headers,
            tls: to_otel_tls_config(tls),
        },
    }
}

fn to_otel_http_protocol(protocol: StandaloneOtelHttpProtocol) -> OtelHttpProtocol {
    match protocol {
        StandaloneOtelHttpProtocol::Binary => OtelHttpProtocol::Binary,
        StandaloneOtelHttpProtocol::Json => OtelHttpProtocol::Json,
    }
}

fn to_otel_tls_config(config: Option<StandaloneOtelTlsConfigToml>) -> Option<OtelTlsConfig> {
    config.map(|config| OtelTlsConfig {
        ca_certificate: config.ca_certificate,
        client_certificate: config.client_certificate,
        client_private_key: config.client_private_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    #[test]
    fn parse_minimal_config_uses_core_defaults() {
        let parsed = parse_otel_config(
            r#"
[network]
enabled = true
"#,
        )
        .unwrap();
        let resolved = resolve_otel_config(parsed);

        assert_eq!(
            resolved,
            ResolvedStandaloneOtelConfig {
                environment: "dev".to_string(),
                exporter: StandaloneOtelExporterKind::None,
                trace_exporter: StandaloneOtelExporterKind::None,
                metrics_exporter: StandaloneOtelExporterKind::Statsig,
            }
        );
    }

    #[test]
    fn parse_otlp_http_exporter_defaults_trace_to_exporter() {
        let parsed = parse_otel_config(
            r#"
[otel]
environment = "staging"
exporter = { otlp-http = { endpoint = "https://collector.example/v1/logs", protocol = "json", headers = { "x-api-key" = "abc" } } }
"#,
        )
        .unwrap();
        let resolved = resolve_otel_config(parsed);

        assert_eq!(
            resolved,
            ResolvedStandaloneOtelConfig {
                environment: "staging".to_string(),
                exporter: StandaloneOtelExporterKind::OtlpHttp {
                    endpoint: "https://collector.example/v1/logs".to_string(),
                    headers: HashMap::from([("x-api-key".to_string(), "abc".to_string())]),
                    protocol: StandaloneOtelHttpProtocol::Json,
                    tls: None,
                },
                trace_exporter: StandaloneOtelExporterKind::OtlpHttp {
                    endpoint: "https://collector.example/v1/logs".to_string(),
                    headers: HashMap::from([("x-api-key".to_string(), "abc".to_string())]),
                    protocol: StandaloneOtelHttpProtocol::Json,
                    tls: None,
                },
                metrics_exporter: StandaloneOtelExporterKind::Statsig,
            }
        );
    }

    #[test]
    fn parse_trace_exporter_independently_of_log_exporter() {
        let parsed = parse_otel_config(
            r#"
[otel]
trace_exporter = { otlp-grpc = { endpoint = "https://collector.example:4317" } }
"#,
        )
        .unwrap();
        let resolved = resolve_otel_config(parsed);

        assert_eq!(
            resolved,
            ResolvedStandaloneOtelConfig {
                environment: "dev".to_string(),
                exporter: StandaloneOtelExporterKind::None,
                trace_exporter: StandaloneOtelExporterKind::OtlpGrpc {
                    endpoint: "https://collector.example:4317".to_string(),
                    headers: HashMap::new(),
                    tls: None,
                },
                metrics_exporter: StandaloneOtelExporterKind::Statsig,
            }
        );
    }

    #[test]
    fn parse_log_user_prompt_field_without_error() {
        let parsed = parse_otel_config(
            r#"
[otel]
log_user_prompt = true
"#,
        )
        .unwrap();
        let resolved = resolve_otel_config(parsed);

        assert_eq!(
            resolved,
            ResolvedStandaloneOtelConfig {
                environment: "dev".to_string(),
                exporter: StandaloneOtelExporterKind::None,
                trace_exporter: StandaloneOtelExporterKind::None,
                metrics_exporter: StandaloneOtelExporterKind::Statsig,
            }
        );
    }

    #[test]
    fn parse_unknown_otel_field_forwards_compatibly() {
        let parsed = parse_otel_config(
            r#"
[otel]
future_field = "ignored"
"#,
        )
        .unwrap();
        let resolved = resolve_otel_config(parsed);

        assert_eq!(
            resolved,
            ResolvedStandaloneOtelConfig {
                environment: "dev".to_string(),
                exporter: StandaloneOtelExporterKind::None,
                trace_exporter: StandaloneOtelExporterKind::None,
                metrics_exporter: StandaloneOtelExporterKind::Statsig,
            }
        );
    }
}
