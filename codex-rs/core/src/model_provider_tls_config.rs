//! TLS configuration for model providers.
//!
//! This module contains types for configuring TLS/mTLS connections to model providers.

use crate::default_client_config::TlsConfig;
use codex_utils_absolute_path::AbsolutePathBuf;
use serde::Deserialize;
use serde::Serialize;

/// TLS configuration for mutual TLS (mTLS) authentication with model providers.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ModelProviderTlsConfig {
    /// Path to a custom CA certificate (PEM format) to trust when connecting to this provider.
    /// Relative paths are resolved against the directory containing the config file.
    pub ca_certificate: Option<AbsolutePathBuf>,

    /// Path to the client certificate (PEM format) for mutual TLS authentication.
    /// Must be provided together with `client_private_key`.
    /// Relative paths are resolved against the directory containing the config file.
    pub client_certificate: Option<AbsolutePathBuf>,

    /// Path to the client private key (PEM format) for mutual TLS authentication.
    /// Must be provided together with `client_certificate`.
    /// Relative paths are resolved against the directory containing the config file.
    pub client_private_key: Option<AbsolutePathBuf>,
}

impl ModelProviderTlsConfig {
    /// Convert to the TlsConfig type used by HTTP clients.
    pub fn to_tls_config(&self) -> TlsConfig {
        TlsConfig {
            ca_certificate: self.ca_certificate.clone(),
            client_certificate: self.client_certificate.clone(),
            client_private_key: self.client_private_key.clone(),
        }
    }
}
