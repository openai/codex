use super::service::TlsConnector;
use crate::transport::{
    tls::{Certificate, Identity},
    Error,
};
use http::Uri;
use tokio_rustls::rustls::pki_types::TrustAnchor;

/// Configures TLS settings for endpoints.
#[derive(Debug, Clone, Default)]
pub struct ClientTlsConfig {
    domain: Option<String>,
    certs: Vec<Certificate>,
    trust_anchors: Vec<TrustAnchor<'static>>,
    identity: Option<Identity>,
    assume_http2: bool,
    #[cfg(feature = "tls-native-roots")]
    with_native_roots: bool,
    #[cfg(feature = "tls-webpki-roots")]
    with_webpki_roots: bool,
    use_key_log: bool,
}

impl ClientTlsConfig {
    /// Creates a new `ClientTlsConfig` using Rustls.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the domain name against which to verify the server's TLS certificate.
    pub fn domain_name(self, domain_name: impl Into<String>) -> Self {
        ClientTlsConfig {
            domain: Some(domain_name.into()),
            ..self
        }
    }

    /// Adds the CA Certificate against which to verify the server's TLS certificate.
    pub fn ca_certificate(self, ca_certificate: Certificate) -> Self {
        let mut certs = self.certs;
        certs.push(ca_certificate);
        ClientTlsConfig { certs, ..self }
    }

    /// Adds the multiple CA Certificates against which to verify the server's TLS certificate.
    pub fn ca_certificates(self, ca_certificates: impl IntoIterator<Item = Certificate>) -> Self {
        let mut certs = self.certs;
        certs.extend(ca_certificates);
        ClientTlsConfig { certs, ..self }
    }

    /// Adds the trust anchor which to verify the server's TLS certificate.
    pub fn trust_anchor(self, trust_anchor: TrustAnchor<'static>) -> Self {
        let mut trust_anchors = self.trust_anchors;
        trust_anchors.push(trust_anchor);
        ClientTlsConfig {
            trust_anchors,
            ..self
        }
    }

    /// Adds the multiple trust anchors which to verify the server's TLS certificate.
    pub fn trust_anchors(
        mut self,
        trust_anchors: impl IntoIterator<Item = TrustAnchor<'static>>,
    ) -> Self {
        self.trust_anchors.extend(trust_anchors);
        self
    }

    /// Sets the client identity to present to the server.
    pub fn identity(self, identity: Identity) -> Self {
        ClientTlsConfig {
            identity: Some(identity),
            ..self
        }
    }

    /// If true, the connector should assume that the server supports HTTP/2,
    /// even if it doesn't provide protocol negotiation via ALPN.
    pub fn assume_http2(self, assume_http2: bool) -> Self {
        ClientTlsConfig {
            assume_http2,
            ..self
        }
    }

    /// Use key log as specified by the `SSLKEYLOGFILE` environment variable.
    pub fn use_key_log(self) -> Self {
        ClientTlsConfig {
            use_key_log: true,
            ..self
        }
    }

    /// Enables the platform's trusted certs.
    #[cfg(feature = "tls-native-roots")]
    pub fn with_native_roots(self) -> Self {
        ClientTlsConfig {
            with_native_roots: true,
            ..self
        }
    }

    /// Enables the webpki roots.
    #[cfg(feature = "tls-webpki-roots")]
    pub fn with_webpki_roots(self) -> Self {
        ClientTlsConfig {
            with_webpki_roots: true,
            ..self
        }
    }

    /// Activates all TLS roots enabled through `tls-*-roots` feature flags
    pub fn with_enabled_roots(self) -> Self {
        let config = self;

        #[cfg(feature = "tls-native-roots")]
        let config = config.with_native_roots();
        #[cfg(feature = "tls-webpki-roots")]
        let config = config.with_webpki_roots();

        config
    }

    pub(crate) fn into_tls_connector(self, uri: &Uri) -> Result<TlsConnector, crate::BoxError> {
        let domain = match &self.domain {
            Some(domain) => domain,
            None => uri.host().ok_or_else(Error::new_invalid_uri)?,
        };
        TlsConnector::new(
            self.certs,
            self.trust_anchors,
            self.identity,
            domain,
            self.assume_http2,
            self.use_key_log,
            #[cfg(feature = "tls-native-roots")]
            self.with_native_roots,
            #[cfg(feature = "tls-webpki-roots")]
            self.with_webpki_roots,
        )
    }
}
