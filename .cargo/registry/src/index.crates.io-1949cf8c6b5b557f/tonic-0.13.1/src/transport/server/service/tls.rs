use std::{fmt, sync::Arc};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::{
    rustls::{server::WebPkiClientVerifier, RootCertStore, ServerConfig},
    server::TlsStream,
    TlsAcceptor as RustlsAcceptor,
};

use crate::transport::{
    service::tls::{convert_certificate_to_pki_types, convert_identity_to_pki_types, ALPN_H2},
    Certificate, Identity,
};

#[derive(Clone)]
pub(crate) struct TlsAcceptor {
    inner: Arc<ServerConfig>,
}

impl TlsAcceptor {
    pub(crate) fn new(
        identity: &Identity,
        client_ca_root: Option<&Certificate>,
        client_auth_optional: bool,
        ignore_client_order: bool,
        use_key_log: bool,
    ) -> Result<Self, crate::BoxError> {
        let builder = ServerConfig::builder();

        let builder = match client_ca_root {
            None => builder.with_no_client_auth(),
            Some(cert) => {
                let mut roots = RootCertStore::empty();
                roots.add_parsable_certificates(convert_certificate_to_pki_types(cert)?);
                let verifier = if client_auth_optional {
                    WebPkiClientVerifier::builder(roots.into()).allow_unauthenticated()
                } else {
                    WebPkiClientVerifier::builder(roots.into())
                }
                .build()?;
                builder.with_client_cert_verifier(verifier)
            }
        };

        let (cert, key) = convert_identity_to_pki_types(identity)?;
        let mut config = builder.with_single_cert(cert, key)?;
        config.ignore_client_order = ignore_client_order;

        if use_key_log {
            config.key_log = Arc::new(tokio_rustls::rustls::KeyLogFile::new());
        }

        config.alpn_protocols.push(ALPN_H2.into());
        Ok(Self {
            inner: Arc::new(config),
        })
    }

    pub(crate) async fn accept<IO>(&self, io: IO) -> Result<TlsStream<IO>, crate::BoxError>
    where
        IO: AsyncRead + AsyncWrite + Unpin,
    {
        let acceptor = RustlsAcceptor::from(self.inner.clone());
        acceptor.accept(io).await.map_err(Into::into)
    }
}

impl fmt::Debug for TlsAcceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TlsAcceptor").finish()
    }
}
