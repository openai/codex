use std::collections::HashMap;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;

pub(crate) const STATSIG_OTLP_HTTP_ENDPOINT: &str = "https://ab.chatgpt.com/otlp/v1/metrics";
pub(crate) const STATSIG_API_KEY_HEADER: &str = "statsig-api-key";
pub(crate) const STATSIG_API_KEY: &str = "client-MkRuleRQBd6qakfnDYqJVR9JuXcY57Ljly3vi5JVUIO";

pub(crate) fn resolve_exporter(exporter: &OtelExporter) -> OtelExporter {
    match exporter {
        OtelExporter::Statsig => {
            // Keep the built-in Statsig default off in debug builds so
            // incremental local development and test runs do not emit
            // best-effort OTEL traffic unless a test or binary opts into an
            // explicit exporter configuration.
            if cfg!(debug_assertions) {
                return OtelExporter::None;
            }

            OtelExporter::OtlpHttp {
                endpoint: STATSIG_OTLP_HTTP_ENDPOINT.to_string(),
                headers: HashMap::from([(
                    STATSIG_API_KEY_HEADER.to_string(),
                    STATSIG_API_KEY.to_string(),
                )]),
                protocol: OtelHttpProtocol::Json,
                tls: None,
            }
        }
        _ => exporter.clone(),
    }
}

#[derive(Clone, Debug)]
pub struct OtelSettings {
    pub environment: String,
    pub service_name: String,
    pub service_version: String,
    pub codex_home: PathBuf,
    pub exporter: OtelExporter,
    pub trace_exporter: OtelExporter,
    pub metrics_exporter: OtelExporter,
    pub runtime_metrics: bool,
}

#[derive(Clone, Debug)]
pub enum OtelHttpProtocol {
    /// HTTP protocol with binary protobuf
    Binary,
    /// HTTP protocol with JSON payload
    Json,
}

#[derive(Clone, Debug, Default)]
pub struct OtelTlsConfig {
    pub ca_certificate: Option<AbsolutePathBuf>,
    pub client_certificate: Option<AbsolutePathBuf>,
    pub client_private_key: Option<AbsolutePathBuf>,
}

#[derive(Clone, Debug)]
pub enum OtelExporter {
    None,
    /// Statsig metrics ingestion exporter using Codex-internal defaults.
    ///
    /// This is intended for metrics only.
    Statsig,
    OtlpGrpc {
        endpoint: String,
        headers: HashMap<String, String>,
        tls: Option<OtelTlsConfig>,
    },
    OtlpHttp {
        endpoint: String,
        headers: HashMap<String, String>,
        protocol: OtelHttpProtocol,
        tls: Option<OtelTlsConfig>,
    },
}

#[cfg(test)]
mod tests {
    use super::OtelExporter;
    use super::resolve_exporter;

    #[test]
    fn statsig_default_metrics_exporter_is_disabled_in_debug_builds() {
        assert!(matches!(
            resolve_exporter(&OtelExporter::Statsig),
            OtelExporter::None
        ));
    }
}
