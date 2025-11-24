use std::fmt;
use std::sync::Arc;

use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{
    rustls::{
        crypto,
        pki_types::{ServerName, TrustAnchor},
        ClientConfig, ConfigBuilder, RootCertStore, WantsVerifier,
    },
    TlsConnector as RustlsConnector,
};

use super::io::BoxedIo;
use crate::transport::service::tls::{
    convert_certificate_to_pki_types, convert_identity_to_pki_types, TlsError, ALPN_H2,
};
use crate::transport::tls::{Certificate, Identity};

#[derive(Clone)]
pub(crate) struct TlsConnector {
    config: Arc<ClientConfig>,
    domain: Arc<ServerName<'static>>,
    assume_http2: bool,
}

impl TlsConnector {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ca_certs: Vec<Certificate>,
        trust_anchors: Vec<TrustAnchor<'static>>,
        identity: Option<Identity>,
        domain: &str,
        assume_http2: bool,
        use_key_log: bool,
        #[cfg(feature = "tls-native-roots")] with_native_roots: bool,
        #[cfg(feature = "tls-webpki-roots")] with_webpki_roots: bool,
    ) -> Result<Self, crate::BoxError> {
        fn with_provider(
            provider: Arc<crypto::CryptoProvider>,
        ) -> ConfigBuilder<ClientConfig, WantsVerifier> {
            ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .unwrap()
        }

        #[allow(unreachable_patterns)]
        let builder = match crypto::CryptoProvider::get_default() {
            Some(provider) => with_provider(provider.clone()),
            #[cfg(feature = "tls-ring")]
            None => with_provider(Arc::new(crypto::ring::default_provider())),
            #[cfg(feature = "tls-aws-lc")]
            None => with_provider(Arc::new(crypto::aws_lc_rs::default_provider())),
            // somehow tls is enabled, but neither of the crypto features are enabled.
            _ => ClientConfig::builder(),
        };

        let mut roots = RootCertStore::from_iter(trust_anchors);

        #[cfg(feature = "tls-native-roots")]
        if with_native_roots {
            let rustls_native_certs::CertificateResult { certs, errors, .. } =
                rustls_native_certs::load_native_certs();
            if !errors.is_empty() {
                tracing::debug!("errors occurred when loading native certs: {errors:?}");
            }
            if certs.is_empty() {
                return Err(TlsError::NativeCertsNotFound.into());
            }
            roots.add_parsable_certificates(certs);
        }

        #[cfg(feature = "tls-webpki-roots")]
        if with_webpki_roots {
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }

        for cert in ca_certs {
            roots.add_parsable_certificates(convert_certificate_to_pki_types(&cert)?);
        }

        let builder = builder.with_root_certificates(roots);
        let mut config = match identity {
            Some(identity) => {
                let (client_cert, client_key) = convert_identity_to_pki_types(&identity)?;
                builder.with_client_auth_cert(client_cert, client_key)?
            }
            None => builder.with_no_client_auth(),
        };

        if use_key_log {
            config.key_log = Arc::new(tokio_rustls::rustls::KeyLogFile::new());
        }

        config.alpn_protocols.push(ALPN_H2.into());
        Ok(Self {
            config: Arc::new(config),
            domain: Arc::new(ServerName::try_from(domain)?.to_owned()),
            assume_http2,
        })
    }

    pub(crate) async fn connect<I>(&self, io: I) -> Result<BoxedIo, crate::BoxError>
    where
        I: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let io = RustlsConnector::from(self.config.clone())
            .connect(self.domain.as_ref().to_owned(), io)
            .await?;

        // Generally we require ALPN to be negotiated, but if the user has
        // explicitly set `assume_http2` to true, we'll allow it to be missing.
        let (_, session) = io.get_ref();
        let alpn_protocol = session.alpn_protocol();
        if !(alpn_protocol == Some(ALPN_H2) || self.assume_http2) {
            return Err(TlsError::H2NotNegotiated.into());
        }
        Ok(BoxedIo::new(TokioIo::new(io)))
    }
}

impl fmt::Debug for TlsConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TlsConnector").finish()
    }
}
