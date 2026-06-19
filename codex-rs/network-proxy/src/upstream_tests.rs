use super::*;
use pretty_assertions::assert_eq;
use rama_http::StatusCode;
use rama_tls_rustls::dep::pki_types::CertificateDer;
use rama_tls_rustls::dep::pki_types::PrivateKeyDer;
use rama_tls_rustls::dep::pki_types::pem::PemObject;
use rama_tls_rustls::dep::rcgen::BasicConstraints;
use rama_tls_rustls::dep::rcgen::CertificateParams;
use rama_tls_rustls::dep::rcgen::DistinguishedName;
use rama_tls_rustls::dep::rcgen::DnType;
use rama_tls_rustls::dep::rcgen::ExtendedKeyUsagePurpose;
use rama_tls_rustls::dep::rcgen::IsCa;
use rama_tls_rustls::dep::rcgen::Issuer;
use rama_tls_rustls::dep::rcgen::KeyPair;
use rama_tls_rustls::dep::rcgen::KeyUsagePurpose;
use rama_tls_rustls::dep::rcgen::PKCS_ECDSA_P256_SHA256;
use rama_tls_rustls::dep::tokio_rustls::TlsAcceptor;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

fn generate_ca(common_name: &str) -> (String, KeyPair) {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, common_name);
    params.distinguished_name = distinguished_name;
    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
    let cert = params.self_signed(&key_pair).unwrap();
    (cert.pem(), key_pair)
}

#[tokio::test]
async fn mitm_upstream_client_trusts_startup_custom_ca() {
    ensure_rustls_crypto_provider();
    let temp_dir = tempdir().unwrap();
    let startup_ca_path = temp_dir.path().join("startup-ca.pem");
    let managed_ca_path = temp_dir.path().join("managed-ca.pem");
    let (startup_ca_pem, startup_ca_key) = generate_ca("startup CA");
    let (managed_ca_pem, _) = generate_ca("managed MITM CA");
    fs::write(&startup_ca_path, &startup_ca_pem).unwrap();
    fs::write(&managed_ca_path, managed_ca_pem).unwrap();

    let issuer = Issuer::from_ca_cert_pem(&startup_ca_pem, startup_ca_key).unwrap();
    let mut server_params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    server_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    let server_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
    let server_cert = server_params.signed_by(&server_key, &issuer).unwrap();
    let server_cert = CertificateDer::from_pem_slice(server_cert.pem().as_bytes()).unwrap();
    let server_key = PrivateKeyDer::from_pem_slice(server_key.serialize_pem().as_bytes()).unwrap();
    let mut server_config =
        rustls::ServerConfig::builder_with_protocol_versions(rustls::ALL_VERSIONS)
            .with_no_client_auth()
            .with_single_cert(vec![server_cert], server_key)
            .unwrap();
    server_config.alpn_protocols = vec![b"http/1.1".to_vec()];

    let env = HashMap::from([(
        "SSL_CERT_FILE",
        startup_ca_path.to_string_lossy().into_owned(),
    )]);
    let roots =
        crate::certs::upstream_tls_root_store_for_cert_path(&managed_ca_path, &env).unwrap();
    let baseline_roots =
        crate::certs::upstream_tls_root_store_for_cert_path(&managed_ca_path, &HashMap::new())
            .unwrap();
    assert_eq!(roots.len(), baseline_roots.len() + 1);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut stream = acceptor.accept(stream).await.unwrap();
        let mut request = [0; 4096];
        let bytes_read = stream.read(&mut request).await.unwrap();
        assert!(bytes_read > 0);
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
    });

    let client =
        UpstreamClient::direct_with_allow_local_binding(/*allow_local_binding*/ true, roots);
    let response = client
        .serve(
            Request::builder()
                .uri(format!("https://localhost:{}/", address.port()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    server.await.unwrap();
}
