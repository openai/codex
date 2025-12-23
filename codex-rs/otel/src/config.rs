use std::collections::HashMap;
use std::path::PathBuf;

use crate::metrics::MetricsConfig;
use codex_utils_absolute_path::AbsolutePathBuf;

#[cfg(feature = "statsig-default-metrics-exporter")]
pub fn statsig_default_metrics_exporter() -> OtelExporter {
    let headers = std::collections::HashMap::from([(
        "statsig-api-key".to_string(),
        "client-MkRuleRQBd6qakfnDYqJVR9JuXcY57Ljly3vi5JVUIO".to_string(),
    )]);

    OtelExporter::OtlpHttp {
        endpoint: "https://ab.chatgpt.com".to_string(),
        headers,
        protocol: OtelHttpProtocol::Json,
        tls: None,
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
    pub metrics: Option<MetricsConfig>,
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
