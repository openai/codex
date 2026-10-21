//! Validated TLS material for strict, route-aware HTTP clients.
//! Explicit roots replace shared environment roots; PEM identities require HTTPS and rustls.

use std::fmt;
#[cfg(windows)]
use std::io;
#[cfg(windows)]
use std::sync::Arc;

#[cfg(windows)]
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
#[cfg(windows)]
use rustls::ClientConfig;
#[cfg(windows)]
use rustls::RootCertStore;
#[cfg(windows)]
use rustls_pki_types::CertificateDer;
#[cfg(windows)]
use rustls_pki_types::PrivateKeyDer;
#[cfg(windows)]
use rustls_pki_types::pem::PemObject;

use crate::HttpError;
#[cfg(windows)]
use crate::WindowsProxyTls;

/// TLS configuration consumed by [`crate::HttpClientBuilder::build_with_tls`].
/// Keeping this separate from the builder prevents combining it with legacy fallback APIs.
#[derive(Clone, Default)]
pub struct HttpClientTlsConfig {
    pub(crate) root_certificate: Option<TlsMaterial<reqwest::Certificate>>,
    pub(crate) client_identity: Option<TlsMaterial<reqwest::Identity>>,
}

#[derive(Clone)]
pub(crate) struct TlsMaterial<T> {
    pub(crate) reqwest: T,
    // Reqwest does not expose its TLS material to the Windows proxy connector.
    #[cfg(windows)]
    pem: Arc<[u8]>,
}

impl<T> TlsMaterial<T> {
    fn from_pem(
        pem: &[u8],
        parse: impl FnOnce(&[u8]) -> Result<T, reqwest::Error>,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            reqwest: parse(pem)?,
            #[cfg(windows)]
            pem: pem.into(),
        })
    }
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
        self.root_certificate = Some(TlsMaterial::from_pem(pem, reqwest::Certificate::from_pem)?);
        Ok(self)
    }

    /// Installs a PEM certificate chain and private key and requires HTTPS requests.
    pub fn with_client_identity_pem(mut self, pem: &[u8]) -> Result<Self, HttpError> {
        self.client_identity = Some(TlsMaterial::from_pem(pem, reqwest::Identity::from_pem)?);
        Ok(self)
    }

    #[cfg(windows)]
    pub(crate) fn windows_proxy_tls(&self) -> io::Result<WindowsProxyTls> {
        if self.root_certificate.is_none() && self.client_identity.is_none() {
            return Ok(WindowsProxyTls::Default);
        }
        let roots = if let Some(root) = &self.root_certificate {
            let mut roots = RootCertStore::empty();
            for certificate in CertificateDer::pem_slice_iter(&root.pem) {
                roots
                    .add(certificate.map_err(io::Error::other)?)
                    .map_err(io::Error::other)?;
            }
            roots
        } else {
            crate::custom_ca::build_rustls_root_store_with_custom_ca().map_err(io::Error::other)?
        };
        ensure_rustls_crypto_provider();
        let builder = ClientConfig::builder().with_root_certificates(roots);
        let config = if let Some(identity) = &self.client_identity {
            let certificates = CertificateDer::pem_slice_iter(&identity.pem)
                .collect::<Result<Vec<_>, _>>()
                .map_err(io::Error::other)?;
            let key = PrivateKeyDer::pem_slice_iter(&identity.pem)
                .last()
                .transpose()
                .map_err(io::Error::other)?
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing private key"))?;
            builder
                .with_client_auth_cert(certificates, key)
                .map_err(io::Error::other)?
        } else {
            builder.with_no_client_auth()
        };
        Ok(WindowsProxyTls::Custom(Arc::new(config)))
    }
}
