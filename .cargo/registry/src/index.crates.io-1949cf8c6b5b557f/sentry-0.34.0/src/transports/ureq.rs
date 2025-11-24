use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "native-tls")]
use native_tls::TlsConnector;
#[cfg(feature = "rustls")]
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
#[cfg(feature = "rustls")]
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature};
#[cfg(feature = "rustls")]
use rustls::pki_types::{CertificateDer, ServerName, TrustAnchor, UnixTime};
#[cfg(feature = "rustls")]
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore};
use ureq::{Agent, AgentBuilder, Proxy};
#[cfg(feature = "rustls")]
use webpki_roots::TLS_SERVER_ROOTS;

use super::thread::TransportThread;

use crate::{sentry_debug, types::Scheme, ClientOptions, Envelope, Transport};

/// A [`Transport`] that sends events via the [`ureq`] library.
///
/// This is enabled by the `ureq` feature flag.
#[cfg_attr(doc_cfg, doc(cfg(feature = "ureq")))]
pub struct UreqHttpTransport {
    thread: TransportThread,
}

impl UreqHttpTransport {
    /// Creates a new Transport.
    pub fn new(options: &ClientOptions) -> Self {
        Self::new_internal(options, None)
    }

    /// Creates a new Transport that uses the specified [`ureq::Agent`].
    pub fn with_agent(options: &ClientOptions, agent: Agent) -> Self {
        Self::new_internal(options, Some(agent))
    }

    fn new_internal(options: &ClientOptions, agent: Option<Agent>) -> Self {
        let dsn = options.dsn.as_ref().unwrap();
        let scheme = dsn.scheme();
        let agent = agent.unwrap_or_else(|| {
            let mut builder = AgentBuilder::new();

            #[cfg(feature = "native-tls")]
            {
                let mut tls_connector_builder = TlsConnector::builder();

                if options.accept_invalid_certs {
                    tls_connector_builder.danger_accept_invalid_certs(true);
                }

                builder = builder.tls_connector(Arc::new(tls_connector_builder.build().unwrap()));
            }

            if options.accept_invalid_certs {
                #[cfg(feature = "rustls")]
                {
                    #[derive(Debug)]
                    struct NoVerifier;

                    impl ServerCertVerifier for NoVerifier {
                        fn verify_server_cert(
                            &self,
                            _end_entity: &CertificateDer<'_>,
                            _intermediates: &[CertificateDer<'_>],
                            _server_name: &ServerName<'_>,
                            _ocsp: &[u8],
                            _now: UnixTime,
                        ) -> Result<ServerCertVerified, rustls::Error> {
                            Ok(ServerCertVerified::assertion())
                        }

                        fn verify_tls12_signature(
                            &self,
                            message: &[u8],
                            cert: &CertificateDer<'_>,
                            dss: &DigitallySignedStruct,
                        ) -> Result<HandshakeSignatureValid, rustls::Error>
                        {
                            verify_tls12_signature(
                                message,
                                cert,
                                dss,
                                &rustls::crypto::ring::default_provider()
                                    .signature_verification_algorithms,
                            )
                        }

                        fn verify_tls13_signature(
                            &self,
                            message: &[u8],
                            cert: &CertificateDer<'_>,
                            dss: &DigitallySignedStruct,
                        ) -> Result<HandshakeSignatureValid, rustls::Error>
                        {
                            verify_tls13_signature(
                                message,
                                cert,
                                dss,
                                &rustls::crypto::ring::default_provider()
                                    .signature_verification_algorithms,
                            )
                        }

                        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
                            rustls::crypto::ring::default_provider()
                                .signature_verification_algorithms
                                .supported_schemes()
                        }
                    }

                    let mut root_store = RootCertStore::empty();
                    root_store.extend(TLS_SERVER_ROOTS.iter().map(TrustAnchor::to_owned));
                    let mut config = ClientConfig::builder()
                        .with_root_certificates(root_store)
                        .with_no_client_auth();
                    config
                        .dangerous()
                        .set_certificate_verifier(Arc::new(NoVerifier));
                    builder = builder.tls_config(Arc::new(config));
                }
            }

            match (scheme, &options.http_proxy, &options.https_proxy) {
                (Scheme::Https, _, Some(proxy)) => match Proxy::new(proxy) {
                    Ok(proxy) => {
                        builder = builder.proxy(proxy);
                    }
                    Err(err) => {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                },
                (_, Some(proxy), _) => match Proxy::new(proxy) {
                    Ok(proxy) => {
                        builder = builder.proxy(proxy);
                    }
                    Err(err) => {
                        sentry_debug!("invalid proxy: {:?}", err);
                    }
                },
                _ => {}
            }

            builder.build()
        });
        let user_agent = options.user_agent.clone();
        let auth = dsn.to_auth(Some(&user_agent)).to_string();
        let url = dsn.envelope_api_url().to_string();

        let thread = TransportThread::new(move |envelope, rl| {
            let mut body = Vec::new();
            envelope.to_writer(&mut body).unwrap();
            let request = agent
                .post(&url)
                .set("X-Sentry-Auth", &auth)
                .send_bytes(&body);

            match request {
                Ok(response) => {
                    if let Some(sentry_header) = response.header("x-sentry-rate-limits") {
                        rl.update_from_sentry_header(sentry_header);
                    } else if let Some(retry_after) = response.header("retry-after") {
                        rl.update_from_retry_after(retry_after);
                    } else if response.status() == 429 {
                        rl.update_from_429();
                    }

                    match response.into_string() {
                        Err(err) => {
                            sentry_debug!("Failed to read sentry response: {}", err);
                        }
                        Ok(text) => {
                            sentry_debug!("Get response: `{}`", text);
                        }
                    }
                }
                Err(err) => {
                    sentry_debug!("Failed to send envelope: {}", err);
                }
            }
        });
        Self { thread }
    }
}

impl Transport for UreqHttpTransport {
    fn send_envelope(&self, envelope: Envelope) {
        self.thread.send(envelope)
    }
    fn flush(&self, timeout: Duration) -> bool {
        self.thread.flush(timeout)
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        self.flush(timeout)
    }
}
