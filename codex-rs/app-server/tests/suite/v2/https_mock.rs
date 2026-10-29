//! Loopback TLS listener and its trusted test certificate.

use anyhow::Result;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls;

pub(super) async fn tls_listener(home: &Path) -> Result<(TcpListener, TlsAcceptor, PathBuf)> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])?;
    let ca_path = home.join("mock-ca.pem");
    std::fs::write(&ca_path, cert.cert.pem())?;
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(
        vec![cert.cert.der().clone()],
        rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
    )?;
    let acceptor = TlsAcceptor::from(Arc::new(tls));
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    Ok((listener, acceptor, ca_path))
}
