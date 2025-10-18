use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct OtelSettings {
    pub environment: String,
    pub service_name: String,
    pub service_version: String,
    pub codex_home: PathBuf,
    pub exporter: OtelExporter,
}

#[derive(Clone, Debug)]
pub enum OtelHttpProtocol {
    /// HTTP protocol with binary protobuf
    Binary,
    /// HTTP protocol with JSON payload
    Json,
}

#[derive(Clone, Debug)]
pub enum OtelExporter {
    None,
    OtlpGrpc {
        endpoint: String,
        headers: HashMap<String, String>,
    },
    OtlpHttp {
        endpoint: String,
        headers: HashMap<String, String>,
        protocol: OtelHttpProtocol,
    },
}
