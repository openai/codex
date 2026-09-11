//! Handshake coverage for the custom trust path retained by Windows realtime connections.

use super::CODEX_CA_CERT_ENV;
use super::ConfiguredCaBundle;
use super::build_rustls_client_config;
use pretty_assertions::assert_eq;
use rcgen::BasicConstraints;
use rcgen::CertificateParams;
use rcgen::CertifiedIssuer;
use rcgen::IsCa;
use rcgen::KeyPair;
use std::sync::Arc;

#[test]
fn custom_intermediate_trust_preserves_hostname_validation() {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let root = CertifiedIssuer::self_signed(params.clone(), KeyPair::generate().unwrap()).unwrap();
    let intermediate =
        CertifiedIssuer::signed_by(params, KeyPair::generate().unwrap(), &root).unwrap();
    let leaf_key = KeyPair::generate().unwrap();
    let leaf = CertificateParams::new(vec!["localhost".to_string()])
        .unwrap()
        .signed_by(&leaf_key, &intermediate)
        .unwrap();
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("intermediate.pem");
    std::fs::write(&path, intermediate.pem()).unwrap();
    let config = build_rustls_client_config(Some(&ConfiguredCaBundle {
        source_env: CODEX_CA_CERT_ENV,
        path,
    }))
    .unwrap();
    let server = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![leaf.der().clone()], leaf_key.into())
            .unwrap(),
    );

    assert_eq!(
        handshake(config.clone(), server.clone(), "localhost"),
        Ok(())
    );
    let error = handshake(config, server, "wrong.example").unwrap_err();
    assert!(
        matches!(
            error,
            rustls::Error::InvalidCertificate(
                rustls::CertificateError::NotValidForName
                    | rustls::CertificateError::NotValidForNameContext { .. }
            )
        ),
        "{error:?}"
    );
}

fn handshake(
    config: Arc<rustls::ClientConfig>,
    server: Arc<rustls::ServerConfig>,
    hostname: &'static str,
) -> Result<(), rustls::Error> {
    let mut client = rustls::ClientConnection::new(config, hostname.try_into().unwrap()).unwrap();
    let mut server = rustls::ServerConnection::new(server).unwrap();
    for _ in 0..10 {
        let mut bytes = Vec::new();
        client.write_tls(&mut bytes).unwrap();
        server.read_tls(&mut bytes.as_slice()).unwrap();
        server.process_new_packets()?;
        bytes.clear();
        server.write_tls(&mut bytes).unwrap();
        client.read_tls(&mut bytes.as_slice()).unwrap();
        client.process_new_packets()?;
        if !client.is_handshaking() {
            return Ok(());
        }
    }
    panic!("handshake did not complete");
}
