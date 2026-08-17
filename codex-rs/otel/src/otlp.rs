use crate::config::OtelTlsConfig;
use codex_http_client::HttpClientFactory;
use codex_http_client::TelemetryClientTlsConfig;
use codex_http_client::TelemetryHttpClient;
use codex_utils_absolute_path::AbsolutePathBuf;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;
use http::Uri;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT;
use opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT;
use opentelemetry_otlp::tonic_types::transport::Certificate as TonicCertificate;
use opentelemetry_otlp::tonic_types::transport::ClientTlsConfig;
use opentelemetry_otlp::tonic_types::transport::Identity as TonicIdentity;
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

/// Build a policy-aware blocking HTTP client for OTLP HTTP exporters.
///
/// OTEL exporters run on dedicated OS threads that are not necessarily backed
/// by Tokio, so logs and metrics require a blocking-compatible transport.
pub(crate) fn build_http_client(
    http_client_factory: &HttpClientFactory,
    endpoint: &str,
    tls: Option<&OtelTlsConfig>,
    timeout_var: &str,
) -> Result<impl TelemetryHttpClient + 'static + use<>, Box<dyn Error>> {
    let tls = telemetry_tls_config(tls);
    let timeout = resolve_otlp_timeout(timeout_var);
    if current_tokio_runtime_is_multi_thread() {
        tokio::task::block_in_place(|| {
            http_client_factory
                .build_blocking_telemetry_client(endpoint, timeout, &tls)
                .map_err(|error| Box::new(error) as Box<dyn Error>)
        })
    } else if tokio::runtime::Handle::try_current().is_ok() {
        let http_client_factory = http_client_factory.clone();
        let endpoint = endpoint.to_owned();
        std::thread::spawn(move || {
            http_client_factory
                .build_blocking_telemetry_client(&endpoint, timeout, &tls)
                .map_err(|error| error.to_string())
        })
        .join()
        .map_err(|_| config_error("failed to join OTLP blocking HTTP client builder thread"))?
        .map_err(config_error)
    } else {
        http_client_factory
            .build_blocking_telemetry_client(endpoint, timeout, &tls)
            .map_err(|error| Box::new(error) as Box<dyn Error>)
    }
}

pub(crate) fn current_tokio_runtime_is_multi_thread() -> bool {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread,
        Err(_) => false,
    }
}

pub(crate) fn build_async_http_client(
    http_client_factory: &HttpClientFactory,
    endpoint: &str,
    tls: Option<&OtelTlsConfig>,
    timeout_var: &str,
) -> Result<impl TelemetryHttpClient + 'static + use<>, Box<dyn Error>> {
    http_client_factory
        .build_async_telemetry_client(
            endpoint,
            resolve_otlp_timeout(timeout_var),
            &telemetry_tls_config(tls),
        )
        .map_err(|error| Box::new(error) as Box<dyn Error>)
}

fn telemetry_tls_config(tls: Option<&OtelTlsConfig>) -> TelemetryClientTlsConfig {
    let Some(tls) = tls else {
        return TelemetryClientTlsConfig::default();
    };

    TelemetryClientTlsConfig {
        ca_certificate: tls
            .ca_certificate
            .as_ref()
            .map(codex_utils_absolute_path::AbsolutePathBuf::to_path_buf),
        client_certificate: tls
            .client_certificate
            .as_ref()
            .map(codex_utils_absolute_path::AbsolutePathBuf::to_path_buf),
        client_private_key: tls
            .client_private_key
            .as_ref()
            .map(codex_utils_absolute_path::AbsolutePathBuf::to_path_buf),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use codex_http_client::OutboundProxyPolicy;
    use pretty_assertions::assert_eq;
    use tokio::runtime::Builder;

    #[test]
    fn current_tokio_runtime_is_multi_thread_detects_runtime_flavor() {
        assert!(!current_tokio_runtime_is_multi_thread());

        let current_thread_runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");
        assert_eq!(
            current_thread_runtime.block_on(async { current_tokio_runtime_is_multi_thread() }),
            false
        );

        let multi_thread_runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("multi-thread runtime");
        assert_eq!(
            multi_thread_runtime.block_on(async { current_tokio_runtime_is_multi_thread() }),
            true
        );
    }

    #[test]
    fn build_http_client_works_in_current_thread_runtime() {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");

        let client = runtime.block_on(async {
            build_http_client(
                &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                "http://localhost:4318/v1/traces",
                Some(&OtelTlsConfig::default()),
                OTEL_EXPORTER_OTLP_TIMEOUT,
            )
        });

        assert!(client.is_ok());
    }
}
