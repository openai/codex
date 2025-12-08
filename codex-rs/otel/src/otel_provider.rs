use crate::config::OtelExporter;
use crate::config::OtelHttpProtocol;
use crate::config::OtelSettings;
use crate::config::OtelTlsConfig;
use http::Uri;
use opentelemetry::KeyValue;
use opentelemetry_otlp::LogExporter;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_LOGS_TIMEOUT;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT;
use opentelemetry_otlp::Protocol;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_semantic_conventions as semconv;
use reqwest::Certificate as ReqwestCertificate;
use reqwest::Identity as ReqwestIdentity;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use std::env;
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::io::{self};
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tonic::metadata::MetadataMap;
use tonic::transport::Certificate as TonicCertificate;
use tonic::transport::ClientTlsConfig;
use tonic::transport::Identity as TonicIdentity;
use tracing::debug;

const ENV_ATTRIBUTE: &str = "env";

pub struct OtelProvider {
    pub logger: SdkLoggerProvider,
}

impl OtelProvider {
    pub fn shutdown(&self) {
        let _ = self.logger.shutdown();
    }

    pub fn from(settings: &OtelSettings) -> Result<Option<Self>, Box<dyn Error>> {
        let resource = Resource::builder()
            .with_service_name(settings.service_name.clone())
            .with_attributes(vec![
                KeyValue::new(
                    semconv::attribute::SERVICE_VERSION,
                    settings.service_version.clone(),
                ),
                KeyValue::new(ENV_ATTRIBUTE, settings.environment.clone()),
            ])
            .build();

        let mut builder = SdkLoggerProvider::builder().with_resource(resource);

        match &settings.exporter {
            OtelExporter::None => {
                debug!("No exporter enabled in OTLP settings.");
                return Ok(None);
            }
            OtelExporter::OtlpGrpc {
                endpoint,
                headers,
                tls,
            } => {
                debug!("Using OTLP Grpc exporter: {endpoint}");

                let mut header_map = HeaderMap::new();
                for (key, value) in headers {
                    if let Ok(name) = HeaderName::from_bytes(key.as_bytes())
                        && let Ok(val) = HeaderValue::from_str(value)
                    {
                        header_map.insert(name, val);
                    }
                }

                let base_tls_config = ClientTlsConfig::new()
                    .with_enabled_roots()
                    .assume_http2(true);

                let tls_config = match tls.as_ref() {
                    Some(tls) => build_grpc_tls_config(
                        endpoint,
                        base_tls_config,
                        tls,
                        settings.codex_home.as_path(),
                    )?,
                    None => base_tls_config,
                };

                let exporter = LogExporter::builder()
                    .with_tonic()
                    .with_endpoint(endpoint)
                    .with_metadata(MetadataMap::from_headers(header_map))
                    .with_tls_config(tls_config)
                    .build()?;

                builder = builder.with_batch_exporter(exporter);
            }
            OtelExporter::OtlpHttp {
                endpoint,
                headers,
                protocol,
                tls,
            } => {
                debug!("Using OTLP Http exporter: {endpoint}");

                let protocol = match protocol {
                    OtelHttpProtocol::Binary => Protocol::HttpBinary,
                    OtelHttpProtocol::Json => Protocol::HttpJson,
                };

                let mut exporter_builder = LogExporter::builder()
                    .with_http()
                    .with_endpoint(endpoint)
                    .with_protocol(protocol)
                    .with_headers(headers.clone());

                if let Some(tls) = tls.as_ref() {
                    let client = build_http_client(tls, settings.codex_home.as_path())?;
                    exporter_builder = exporter_builder.with_http_client(client);
                }

                let exporter = exporter_builder.build()?;

                builder = builder.with_batch_exporter(exporter);
            }
        }

        Ok(Some(Self {
            logger: builder.build(),
        }))
    }
}

impl Drop for OtelProvider {
    fn drop(&mut self) {
        let _ = self.logger.shutdown();
    }
}

fn build_grpc_tls_config(
    endpoint: &str,
    tls_config: ClientTlsConfig,
    tls: &OtelTlsConfig,
    codex_home: &Path,
) -> Result<ClientTlsConfig, Box<dyn Error>> {
    let uri: Uri = endpoint.parse()?;
    let host = uri.host().ok_or_else(|| {
        config_error(format!(
            "OTLP gRPC endpoint {endpoint} does not include a host"
        ))
    })?;

    let mut config = tls_config.domain_name(host.to_owned());

    if let Some(path) = tls.ca_certificate.as_ref() {
        let (pem, _) = read_bytes(codex_home, path)?;
        config = config.ca_certificate(TonicCertificate::from_pem(pem));
    }

    match (&tls.client_certificate, &tls.client_private_key) {
        (Some(cert_path), Some(key_path)) => {
            let (cert_pem, _) = read_bytes(codex_home, cert_path)?;
            let (key_pem, _) = read_bytes(codex_home, key_path)?;
            config = config.identity(TonicIdentity::from_pem(cert_pem, key_pem));
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(config_error(
                "client_certificate and client_private_key must both be provided for mTLS",
            ));
        }
        (None, None) => {}
    }

    Ok(config)
}

/// Build a blocking HTTP client with TLS configuration for the OTLP HTTP exporter.
///
/// We use `reqwest::blocking::Client` instead of the async client because the
/// `opentelemetry_sdk` `BatchLogProcessor` spawns a dedicated OS thread that uses
/// `futures_executor::block_on()` rather than tokio. When the async reqwest client's
/// timeout calls `tokio::time::sleep()`, it panics with "no reactor running".
fn build_http_client(
    tls: &OtelTlsConfig,
    codex_home: &Path,
) -> Result<reqwest::blocking::Client, Box<dyn Error>> {
    // Wrap in block_in_place because reqwest::blocking::Client creates its own
    // internal tokio runtime, which would panic if built directly from an async context.
    tokio::task::block_in_place(|| build_http_client_inner(tls, codex_home))
}

fn build_http_client_inner(
    tls: &OtelTlsConfig,
    codex_home: &Path,
) -> Result<reqwest::blocking::Client, Box<dyn Error>> {
    let mut builder = reqwest::blocking::Client::builder()
        .timeout(resolve_otlp_timeout(OTEL_EXPORTER_OTLP_LOGS_TIMEOUT));

    if let Some(path) = tls.ca_certificate.as_ref() {
        let (pem, location) = read_bytes(codex_home, path)?;
        let certificate = ReqwestCertificate::from_pem(pem.as_slice()).map_err(|error| {
            config_error(format!(
                "failed to parse certificate {}: {error}",
                location.display()
            ))
        })?;
        // Disable built-in root certificates and use only our custom CA
        builder = builder
            .tls_built_in_root_certs(false)
            .add_root_certificate(certificate);
    }

    match (&tls.client_certificate, &tls.client_private_key) {
        (Some(cert_path), Some(key_path)) => {
            let (mut cert_pem, cert_location) = read_bytes(codex_home, cert_path)?;
            let (key_pem, key_location) = read_bytes(codex_home, key_path)?;
            cert_pem.extend_from_slice(key_pem.as_slice());
            let identity = ReqwestIdentity::from_pem(cert_pem.as_slice()).map_err(|error| {
                config_error(format!(
                    "failed to parse client identity using {} and {}: {error}",
                    cert_location.display(),
                    key_location.display()
                ))
            })?;
            builder = builder.identity(identity).https_only(true);
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(config_error(
                "client_certificate and client_private_key must both be provided for mTLS",
            ));
        }
        (None, None) => {}
    }

    builder
        .build()
        .map_err(|error| Box::new(error) as Box<dyn Error>)
}

fn resolve_otlp_timeout(signal_var: &str) -> Duration {
    if let Some(timeout) = read_timeout_env(signal_var) {
        return timeout;
    }
    if let Some(timeout) = read_timeout_env(OTEL_EXPORTER_OTLP_TIMEOUT) {
        return timeout;
    }
    OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT
}

fn read_timeout_env(var: &str) -> Option<Duration> {
    let value = env::var(var).ok()?;
    let parsed = value.parse::<i64>().ok()?;
    if parsed < 0 {
        return None;
    }
    Some(Duration::from_millis(parsed as u64))
}

fn read_bytes(base: &Path, provided: &PathBuf) -> Result<(Vec<u8>, PathBuf), Box<dyn Error>> {
    let resolved = resolve_config_path(base, provided);
    match fs::read(&resolved) {
        Ok(bytes) => Ok((bytes, resolved)),
        Err(error) => Err(Box::new(io::Error::new(
            error.kind(),
            format!("failed to read {}: {error}", resolved.display()),
        ))),
    }
}

fn resolve_config_path(base: &Path, provided: &PathBuf) -> PathBuf {
    if provided.is_absolute() {
        provided.clone()
    } else {
        base.join(provided)
    }
}

fn config_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidData, message.into()))
}
