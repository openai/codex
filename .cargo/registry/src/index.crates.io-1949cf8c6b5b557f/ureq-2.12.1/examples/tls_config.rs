//! Example of how to make more in depth TLS config, like narrowing the allowed TLS versions.

use std::sync::Arc;

use native_tls::Protocol;
use rustls::version::TLS12;
use rustls::version::TLS13;

pub fn main() {
    let mut root_store = rustls::RootCertStore::empty();

    // Uncomment this to use native-certs

    // let certs = rustls_native_certs::load_native_certs().expect("Could not load platform certs");
    // for cert in certs {
    //     // Repackage the certificate DER bytes.
    //     let rustls_cert = rustls::Certificate(cert.0);
    //     root_store
    //         .add(&rustls_cert)
    //         .expect("Failed to add native certificate too root store");
    // }

    // This adds webpki_roots certs.
    root_store.roots = webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect();

    // This is how we narrow down the allowed TLS versions for rustls.
    let protocol_versions = &[&TLS12, &TLS13];

    // See rustls documentation for more configuration options.
    let tls_config = rustls::ClientConfig::builder_with_protocol_versions(protocol_versions)
        .with_root_certificates(root_store)
        .with_no_client_auth();

    // Build a ureq agent with the rustls config.
    let agent1 = ureq::builder().tls_config(Arc::new(tls_config)).build();

    let response1 = agent1.get("https://httpbin.org/get").call().unwrap();
    assert!(response1.status() == 200);

    ////////////////////////////////////////////////////////////

    // Narrow the accepted TLS versions for native-tls
    // See native-tls documentation for more configuration options.
    let tls_connector = native_tls::TlsConnector::builder()
        .min_protocol_version(Some(Protocol::Tlsv12))
        .max_protocol_version(Some(Protocol::Tlsv12))
        .build()
        .unwrap();

    // Build a ureq agent with the native-tls config.
    let agent2 = ureq::builder()
        .tls_connector(Arc::new(tls_connector))
        .build();

    let response2 = agent2.get("https://httpbin.org/get").call().unwrap();
    assert!(response2.status() == 200);
}
