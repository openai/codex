//! Validated TLS material for strict, route-aware HTTP clients.
//! Explicit roots replace shared environment roots; PEM identities require HTTPS and rustls.

use std::fmt;

use crate::HttpError;

/// TLS configuration consumed by [`crate::HttpClientBuilder::build_with_tls`].
/// Keeping this separate from the builder prevents combining it with legacy fallback APIs.
#[derive(Clone, Default)]
pub struct HttpClientTlsConfig {
    pub(crate) root_certificate: Option<reqwest::Certificate>,
    pub(crate) client_identity: Option<reqwest::Identity>,
}

impl fmt::Debug for HttpClientTlsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpClientTlsConfig")
            .field("explicit_root", &self.root_certificate.is_some())
            .field("client_identity", &self.client_identity.is_some())
            .finish()
    }
}

impl HttpClientTlsConfig {
    /// Trusts this PEM certificate instead of system or environment-provided roots.
    pub fn with_root_certificate_pem(mut self, pem: &[u8]) -> Result<Self, HttpError> {
        self.root_certificate = Some(reqwest::Certificate::from_pem(pem)?);
        Ok(self)
    }

    /// Installs a PEM certificate chain and private key and requires HTTPS requests.
    pub fn with_client_identity_pem(mut self, pem: &[u8]) -> Result<Self, HttpError> {
        self.client_identity = Some(reqwest::Identity::from_pem(pem)?);
        Ok(self)
    }
}
