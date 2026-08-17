//! Policy-aware HTTP transports for OpenTelemetry exporters.

use crate::BuildRouteAwareHttpClientError;
use crate::ClientRouteClass;
use crate::HttpClientFactory;
use crate::OutboundProxyPolicy;
use crate::OutboundProxyRoute;
use crate::custom_ca::BuildCustomCaTransportError;
use crate::custom_ca::build_blocking_reqwest_client_with_custom_ca;
use opentelemetry_http::HttpClient;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Optional collector-specific roots and client identity for telemetry export.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TelemetryClientTlsConfig {
    pub ca_certificate: Option<PathBuf>,
    pub client_certificate: Option<PathBuf>,
    pub client_private_key: Option<PathBuf>,
}

/// Failure while preparing a proxy-aware telemetry exporter transport.
#[derive(Debug, Error)]
pub enum BuildTelemetryHttpClientError {
    #[error("failed to read {}: {source}", path.display())]
    ReadTlsFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to parse certificate {}: {source}", path.display())]
    InvalidCertificate {
        path: PathBuf,
        #[source]
        source: reqwest::Error,
    },

    #[error(
        "failed to parse client identity using {} and {}: {source}",
        certificate_path.display(),
        private_key_path.display()
    )]
    InvalidClientIdentity {
        certificate_path: PathBuf,
        private_key_path: PathBuf,
        #[source]
        source: reqwest::Error,
    },

    #[error("client_certificate and client_private_key must both be provided for mTLS")]
    IncompleteClientIdentity,

    #[error(transparent)]
    Route(#[from] BuildRouteAwareHttpClientError),

    #[error(transparent)]
    CustomCa(#[from] BuildCustomCaTransportError),
}

struct PreparedTlsConfig {
    root_certificate: Option<reqwest::Certificate>,
    client_identity: Option<reqwest::Identity>,
}

impl HttpClientFactory {
    /// Builds an asynchronous exporter client for one fixed collector endpoint.
    pub fn build_async_telemetry_client(
        &self,
        endpoint: &str,
        timeout: Duration,
        tls: &TelemetryClientTlsConfig,
    ) -> Result<impl HttpClient + 'static + use<>, BuildTelemetryHttpClientError> {
        let tls = prepare_tls_config(tls)?;
        let mut builder = reqwest::Client::builder().timeout(timeout);
        if let Some(certificate) = tls.root_certificate {
            builder = builder
                .tls_built_in_root_certs(false)
                .add_root_certificate(certificate);
        }
        if let Some(identity) = tls.client_identity {
            builder = builder.identity(identity).https_only(true);
        }
        if self.outbound_proxy_policy() == OutboundProxyPolicy::RespectSystemProxy {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }

        self.build_reqwest_client(builder, endpoint, ClientRouteClass::Other)
            .map_err(Into::into)
    }

    /// Builds a blocking exporter client for one fixed collector endpoint.
    ///
    /// Callers inside a Tokio runtime must construct this on a blocking-capable
    /// thread, just as they would for any other blocking reqwest client.
    pub fn build_blocking_telemetry_client(
        &self,
        endpoint: &str,
        timeout: Duration,
        tls: &TelemetryClientTlsConfig,
    ) -> Result<impl HttpClient + 'static + use<>, BuildTelemetryHttpClientError> {
        let tls = prepare_tls_config(tls)?;
        let mut builder = reqwest::blocking::Client::builder().timeout(timeout);
        if let Some(certificate) = tls.root_certificate {
            builder = builder
                .tls_built_in_root_certs(false)
                .add_root_certificate(certificate);
        }
        if let Some(identity) = tls.client_identity {
            builder = builder.identity(identity).https_only(true);
        }
        if self.outbound_proxy_policy() == OutboundProxyPolicy::RespectSystemProxy {
            builder = builder.redirect(reqwest::redirect::Policy::none());
        }

        builder = match self.resolve_proxy_route(endpoint) {
            OutboundProxyRoute::TransportDefault => builder,
            OutboundProxyRoute::Direct => builder.no_proxy(),
            OutboundProxyRoute::Proxy { url, no_proxy } => {
                let proxy = reqwest::Proxy::all(&url).map_err(|_| {
                    BuildRouteAwareHttpClientError::InvalidProxyConfig {
                        route_class: ClientRouteClass::Other,
                    }
                })?;
                let no_proxy = no_proxy.as_deref().and_then(reqwest::NoProxy::from_string);
                builder.proxy(proxy.no_proxy(no_proxy))
            }
        };

        build_blocking_reqwest_client_with_custom_ca(builder).map_err(Into::into)
    }
}

fn prepare_tls_config(
    tls: &TelemetryClientTlsConfig,
) -> Result<PreparedTlsConfig, BuildTelemetryHttpClientError> {
    let root_certificate = tls
        .ca_certificate
        .as_ref()
        .map(|path| {
            let pem = read_tls_file(path)?;
            reqwest::Certificate::from_pem(&pem).map_err(|source| {
                BuildTelemetryHttpClientError::InvalidCertificate {
                    path: path.clone(),
                    source,
                }
            })
        })
        .transpose()?;

    let client_identity = match (&tls.client_certificate, &tls.client_private_key) {
        (Some(certificate_path), Some(private_key_path)) => {
            let mut pem = read_tls_file(certificate_path)?;
            pem.extend_from_slice(&read_tls_file(private_key_path)?);
            Some(reqwest::Identity::from_pem(&pem).map_err(|source| {
                BuildTelemetryHttpClientError::InvalidClientIdentity {
                    certificate_path: certificate_path.clone(),
                    private_key_path: private_key_path.clone(),
                    source,
                }
            })?)
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(BuildTelemetryHttpClientError::IncompleteClientIdentity);
        }
        (None, None) => None,
    };

    Ok(PreparedTlsConfig {
        root_certificate,
        client_identity,
    })
}

fn read_tls_file(path: &Path) -> Result<Vec<u8>, BuildTelemetryHttpClientError> {
    fs::read(path).map_err(|source| BuildTelemetryHttpClientError::ReadTlsFile {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
#[path = "telemetry_client_tests.rs"]
mod tests;
