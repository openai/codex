//! Certificate rejection coverage for Windows platform TLS.

use super::build_windows_platform_tls_config;
use std::sync::Arc;

#[test]
fn platform_tls_rejects_a_self_signed_server() {
    let client_config =
        build_windows_platform_tls_config().expect("configure platform verification");
    let certified_key = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .expect("generate untrusted certificate");
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![certified_key.cert.der().clone()],
            certified_key.signing_key.into(),
        )
        .expect("configure test server");
    let mut client = rustls::ClientConnection::new(
        client_config,
        "localhost".try_into().expect("valid server name"),
    )
    .expect("create client");
    let mut server = rustls::ServerConnection::new(Arc::new(server_config)).expect("create server");

    // Drive the real handshake without opening ports or changing system trust.
    for _ in 0..10 {
        let mut outbound = Vec::new();
        client
            .write_tls(&mut outbound)
            .expect("write client records");
        server
            .read_tls(&mut outbound.as_slice())
            .expect("read client records");
        server
            .process_new_packets()
            .expect("process client records");
        outbound.clear();
        server
            .write_tls(&mut outbound)
            .expect("write server records");
        client
            .read_tls(&mut outbound.as_slice())
            .expect("read server records");
        if let Err(error) = client.process_new_packets() {
            assert!(
                matches!(error, rustls::Error::InvalidCertificate(_)),
                "{error:?}"
            );
            return;
        }
        assert!(
            client.is_handshaking(),
            "untrusted server must not complete TLS"
        );
    }
    panic!("expected certificate rejection during handshake");
}
