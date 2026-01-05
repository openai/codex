use crate::config::OtelTlsConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use http::Uri;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT;
use opentelemetry_otlp::tonic_types::transport::Certificate as TonicCertificate;
use opentelemetry_otlp::tonic_types::transport::ClientTlsConfig;
use opentelemetry_otlp::tonic_types::transport::Identity as TonicIdentity;
use reqwest::Certificate as ReqwestCertificate;
use reqwest::Identity as ReqwestIdentity;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

pub(crate) fn build_header_map(headers: &std::collections::HashMap<String, String>) -> HeaderMap {
    let mut header_map = HeaderMap::new();
    for (key, value) in headers {
        if let Ok(name) = HeaderName::from_bytes(key.as_bytes())
            && let Ok(val) = HeaderValue::from_str(value)
        {
            header_map.insert(name, val);
        }
    }
    header_map
}

pub(crate) fn build_grpc_tls_config(
    endpoint: &str,
    tls_config: ClientTlsConfig,
    tls: &OtelTlsConfig,
) -> Result<ClientTlsConfig, Box<dyn Error>> {
    let uri: Uri = endpoint.parse()?;
    let host = uri.host().ok_or_else(|| {
        config_error(format!(
            "OTLP gRPC endpoint {endpoint} does not include a host"
        ))
    })?;

    let mut config = tls_config.domain_name(host.to_owned());

    if let Some(path) = tls.ca_certificate.as_ref() {
        let (pem, _) = read_bytes(path)?;
        config = config.ca_certificate(TonicCertificate::from_pem(pem));
    }

    match (&tls.client_certificate, &tls.client_private_key) {
        (Some(cert_path), Some(key_path)) => {
            let (cert_pem, _) = read_bytes(cert_path)?;
            let (key_pem, _) = read_bytes(key_path)?;
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

/// Build a blocking HTTP client with TLS configuration for OTLP HTTP exporters.
///
/// We use `reqwest::blocking::Client` because OTEL exporters run on dedicated
/// OS threads that are not necessarily backed by tokio.
pub(crate) fn build_http_client(
    tls: &OtelTlsConfig,
    timeout_var: &str,
) -> Result<reqwest::blocking::Client, Box<dyn Error>> {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| build_http_client_inner(tls, timeout_var))
    } else {
        build_http_client_inner(tls, timeout_var)
    }
}

fn build_http_client_inner(
    tls: &OtelTlsConfig,
    timeout_var: &str,
) -> Result<reqwest::blocking::Client, Box<dyn Error>> {
    let mut builder =
        reqwest::blocking::Client::builder().timeout(resolve_otlp_timeout(timeout_var));

    if let Some(path) = tls.ca_certificate.as_ref() {
        let (pem, location) = read_bytes(path)?;
        let certificate = ReqwestCertificate::from_pem(pem.as_slice()).map_err(|error| {
            config_error(format!(
                "failed to parse certificate {}: {error}",
                location.display()
            ))
        })?;
        builder = builder
            .tls_built_in_root_certs(false)
            .add_root_certificate(certificate);
    }

    match (&tls.client_certificate, &tls.client_private_key) {
        (Some(cert_path), Some(key_path)) => {
            let (mut cert_pem, cert_location) = read_bytes(cert_path)?;
            let (key_pem, key_location) = read_bytes(key_path)?;
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

pub(crate) fn resolve_otlp_timeout(signal_var: &str) -> Duration {
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

fn read_bytes(path: &AbsolutePathBuf) -> Result<(Vec<u8>, PathBuf), Box<dyn Error>> {
    match fs::read(path) {
        Ok(bytes) => Ok((bytes, path.to_path_buf())),
        Err(error) => Err(Box::new(io::Error::new(
            error.kind(),
            format!("failed to read {}: {error}", path.display()),
        ))),
    }
}

fn config_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::new(ErrorKind::InvalidData, message.into()))
}
