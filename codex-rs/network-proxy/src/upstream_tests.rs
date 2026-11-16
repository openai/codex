use super::*;
use crate::config::NetworkProxyConfig;
use crate::state::network_proxy_state_for_policy;
use pretty_assertions::assert_eq;
use rama_http::StatusCode;
use rama_http::Version;
use rama_net::address::Host;
use rama_net::address::HostWithPort;
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

    let mut config = NetworkProxyConfig::default();
    config.set_allowed_domains(vec!["localhost".to_string()]);
    let state = Arc::new(network_proxy_state_for_policy(config));
    let client = UpstreamClient::direct_with_tls_root_store(state, roots);
    let response = client
        .serve(
            Request::builder()
                .version(Version::HTTP_2)
                .uri(format!("https://localhost:{}/", address.port()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    server.await.unwrap();
}

#[test]
fn private_ip_upstream_routing_is_opt_in() {
    let proxy = ProxyAddress::try_from("http://127.0.0.1:43128").unwrap();
    for enabled in [false, true] {
        let config = ProxyConfig {
            http: Some(proxy.clone()),
            https: Some(proxy.clone()),
            proxy_private_ips_via_upstream: enabled,
            ..ProxyConfig::default()
        };
        for (host, use_proxy) in [
            ("10.0.0.1", enabled),
            ("172.16.0.1", enabled),
            ("192.168.0.1", enabled),
            ("100.68.58.50", enabled),
            ("fd00::1", enabled),
            ("::ffff:100.68.58.50", enabled),
            ("localhost", false),
            ("127.0.0.1", false),
            ("::1", false),
            ("169.254.169.254", false),
            ("fe80::1", false),
            ("0.0.0.0", false),
            ("224.0.0.1", false),
            ("192.0.2.1", false),
            ("240.0.0.1", false),
            ("example.com", true),
            ("100.63.255.255", true),
            ("100.128.0.0", true),
        ] {
            let target = HostWithPort::new(host.parse::<Host>().unwrap(), /*port*/ 443);
            for is_secure in [false, true] {
                assert_eq!(
                    config.proxy_for_target(&target, is_secure),
                    use_proxy.then(|| proxy.clone()),
                    "host={host}, enabled={enabled}, secure={is_secure}"
                );
            }
        }
    }
}

async fn capture_proxy_request(
    uri: String,
    listener: TcpListener,
    proxy_config: ProxyConfig,
) -> String {
    let network = NetworkProxyConfig {
        allow_local_binding: Some(true),
        ..NetworkProxyConfig::default()
    };
    let state = Arc::new(network_proxy_state_for_policy(network));
    let client = UpstreamClient::new(
        proxy_config,
        TargetCheckedTcpConnector::new(state),
        client_root_certs(),
    );
    let request = Request::builder()
        .uri(uri)
        .header(TOOL_CALL_HEADER, "from-command")
        .body(Body::empty())
        .unwrap();

    let capture = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            bytes.push(socket.read_u8().await.unwrap());
            assert!(bytes.len() < 8192);
        }
        socket
            .write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        String::from_utf8(bytes).unwrap()
    };
    let (wire, _) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(capture, client.serve(request))
    })
    .await
    .expect("request reached proxy or direct destination");
    wire
}

#[tokio::test]
async fn tool_call_header_is_scoped_to_the_configured_upstream_and_execution() {
    async fn exchange(scheme: &str, call_id: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let proxy = ProxyAddress::try_from(addr.to_string()).unwrap();
        let endpoint = ProxyAddress::try_from(format!("http://{addr}")).unwrap();
        let config = ProxyConfig {
            http: Some(proxy.clone()),
            https: Some(proxy),
            attribution: Some(ToolCallAttribution {
                endpoint,
                call_id: HeaderValue::from_static(call_id),
            }),
            ..ProxyConfig::default()
        };
        capture_proxy_request(format!("{scheme}://same.example/path"), listener, config).await
    }

    let (http, https) = tokio::join!(exchange("http", "call-A"), exchange("https", "exec-B"));
    assert!(http.starts_with("GET "));
    assert!(https.starts_with("CONNECT same.example:443 "));
    for (wire, expected) in [(http, "call-A"), (https, "exec-B")] {
        let ids: Vec<_> = wire
            .lines()
            .filter_map(|line| line.split_once(": "))
            .filter(|(name, _)| name.eq_ignore_ascii_case(TOOL_CALL_HEADER.as_str()))
            .map(|(_, value)| value)
            .collect();
        assert_eq!(ids, vec![expected]);
    }
}

#[tokio::test]
async fn direct_and_other_proxy_requests_strip_tool_call_headers() {
    let trusted = ProxyAddress::try_from("http://other-proxy.test:8889").unwrap();
    for (scheme, use_proxy, enabled) in [
        ("http", false, true),
        ("http", true, false),
        ("http", true, true),
        ("https", true, true),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let proxy = ProxyAddress::try_from(format!("http://{address}")).unwrap();
        let uri = if use_proxy {
            format!("{scheme}://same.example/path")
        } else {
            format!("http://{address}/path")
        };
        let config = ProxyConfig {
            http: use_proxy.then(|| proxy.clone()),
            https: use_proxy.then_some(proxy),
            attribution: enabled.then(|| ToolCallAttribution {
                endpoint: trusted.clone(),
                call_id: HeaderValue::from_static("call-A"),
            }),
            ..ProxyConfig::default()
        };
        let wire = capture_proxy_request(uri, listener, config).await;
        assert!(
            !wire
                .to_ascii_lowercase()
                .contains(TOOL_CALL_HEADER.as_str())
        );
    }
}

#[tokio::test]
async fn split_upstreams_attribute_only_the_selected_hop_regardless_of_origin_port() {
    for (uri, secure) in [
        ("http://same.example:443/path", false),
        ("https://same.example:80/path", true),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let active =
            ProxyAddress::try_from(format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let other = ProxyAddress::try_from("http://unused-proxy.test:8889").unwrap();
        let (http, https) = if secure {
            (other, active)
        } else {
            (active, other)
        };
        let config = ProxyConfig {
            http: Some(http),
            https: Some(https.clone()),
            attribution: Some(ToolCallAttribution {
                endpoint: https,
                call_id: HeaderValue::from_static("exec-A"),
            }),
            ..ProxyConfig::default()
        };
        let wire = capture_proxy_request(uri.to_string(), listener, config).await;
        assert_eq!(
            wire.to_ascii_lowercase()
                .contains(TOOL_CALL_HEADER.as_str()),
            secure
        );
        if secure {
            assert!(wire.starts_with("CONNECT same.example:80 "));
            assert!(wire.contains("exec-A"));
        }
    }
}
