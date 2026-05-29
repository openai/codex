//! Subprocess coverage for custom CA behavior that must build a real reqwest client.
//!
//! These tests intentionally run through `custom_ca_probe` and
//! `build_reqwest_client_for_subprocess_tests` instead of calling the helper in-process. The
//! detailed explanation of what "hermetic" means here lives in `codex_client::custom_ca`; these
//! tests add the process-level half of that contract by scrubbing inherited CA environment
//! variables before each subprocess launch. Most assertions here cover CA file selection, PEM
//! parsing, and user-facing errors. The HTTPS probes go further and perform real POSTs against
//! locally generated certificates, including through a TLS-intercepting CONNECT proxy.

use codex_utils_cargo_bin::cargo_bin;
use rcgen::BasicConstraints;
use rcgen::CertificateParams;
use rcgen::CertifiedIssuer;
use rcgen::DistinguishedName;
use rcgen::DnType;
use rcgen::ExtendedKeyUsagePurpose;
use rcgen::IsCa;
use rcgen::KeyPair;
use rcgen::KeyUsagePurpose;
use rcgen::PKCS_ECDSA_P256_SHA256;
use rustls_pki_types::CertificateDer;
use rustls_pki_types::PrivateKeyDer;
use std::fs;
use std::io;
use std::io::Read;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use tempfile::TempDir;

const CODEX_CA_CERT_ENV: &str = "CODEX_CA_CERTIFICATE";
const PROBE_PROXY_ENV: &str = "CODEX_CUSTOM_CA_PROBE_PROXY";
const PROBE_TLS13_ENV: &str = "CODEX_CUSTOM_CA_PROBE_TLS13";
const PROBE_URL_ENV: &str = "CODEX_CUSTOM_CA_PROBE_URL";
const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";
const PROXY_ENV_VARS: &[&str] = &[
    "HTTP_PROXY",
    "http_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "ALL_PROXY",
    "all_proxy",
    "NO_PROXY",
    "no_proxy",
];

const TEST_CERT_1: &str = include_str!("fixtures/test-ca.pem");
const TEST_CERT_2: &str = include_str!("fixtures/test-intermediate.pem");
const TRUSTED_TEST_CERT: &str = include_str!("fixtures/test-ca-trusted.pem");

struct Tls13Material {
    ca_cert_pem: String,
    server_cert: CertificateDer<'static>,
    server_key: PrivateKeyDer<'static>,
}

struct Tls13TestServer {
    ca_cert_pem: String,
    request_rx: mpsc::Receiver<Result<String, String>>,
    url: String,
}

struct PlainHttpOrigin {
    request_rx: mpsc::Receiver<Result<String, String>>,
    url: String,
}

struct TlsInterceptingProxy {
    ca_cert_pem: String,
    request_rx: mpsc::Receiver<Result<String, String>>,
    url: String,
}

fn write_cert_file(temp_dir: &TempDir, name: &str, contents: &str) -> PathBuf {
    let path = temp_dir.path().join(name);
    fs::write(&path, contents).unwrap_or_else(|error| {
        panic!("write cert fixture failed for {}: {error}", path.display())
    });
    path
}

fn probe_command() -> Command {
    let mut cmd = Command::new(
        cargo_bin("custom_ca_probe")
            .unwrap_or_else(|error| panic!("failed to locate custom_ca_probe: {error}")),
    );
    // `Command` inherits the parent environment by default, so scrub CA-related variables first or
    // these tests can accidentally pass/fail based on the developer shell or CI runner.
    cmd.env_remove(CODEX_CA_CERT_ENV);
    cmd.env_remove(PROBE_PROXY_ENV);
    cmd.env_remove(PROBE_TLS13_ENV);
    cmd.env_remove(PROBE_URL_ENV);
    cmd.env_remove(SSL_CERT_FILE_ENV);
    for env_var in PROXY_ENV_VARS {
        cmd.env_remove(env_var);
    }
    cmd
}

fn run_probe(envs: &[(&str, &Path)]) -> std::process::Output {
    let mut cmd = probe_command();
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output()
        .unwrap_or_else(|error| panic!("failed to run custom_ca_probe: {error}"))
}

fn run_probe_posting_to_tls13_server(envs: &[(&str, &Path)], url: &str) -> std::process::Output {
    let mut cmd = probe_command();
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.env(PROBE_TLS13_ENV, "1");
    cmd.env(PROBE_URL_ENV, url);
    cmd.output()
        .unwrap_or_else(|error| panic!("failed to run custom_ca_probe: {error}"))
}

fn run_probe_posting_through_tls_intercepting_proxy(
    envs: &[(&str, &Path)],
    url: &str,
    proxy_url: &str,
) -> std::process::Output {
    let mut cmd = probe_command();
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.env(PROBE_PROXY_ENV, proxy_url);
    cmd.env(PROBE_TLS13_ENV, "1");
    cmd.env(PROBE_URL_ENV, url);
    cmd.output()
        .unwrap_or_else(|error| panic!("failed to run custom_ca_probe: {error}"))
}

fn spawn_tls13_test_server() -> Tls13TestServer {
    codex_utils_rustls_provider::ensure_rustls_crypto_provider();
    let material = generate_tls13_material();
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .unwrap_or_else(|error| panic!("bind TLS test server: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("set TLS test server nonblocking: {error}"));
    let port = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("TLS test server addr: {error}"))
        .port();
    let config = Arc::new(
        rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
            .with_no_client_auth()
            .with_single_cert(vec![material.server_cert], material.server_key)
            .unwrap_or_else(|error| panic!("TLS 1.3 server config: {error}")),
    );
    let (request_tx, request_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = accept_tls13_request(listener, config);
        let _ = request_tx.send(result.map_err(|error| error.to_string()));
    });

    Tls13TestServer {
        ca_cert_pem: material.ca_cert_pem,
        request_rx,
        url: format!("https://127.0.0.1:{port}/oauth/token"),
    }
}

fn spawn_plain_http_origin() -> PlainHttpOrigin {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .unwrap_or_else(|error| panic!("bind plain HTTP origin: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("set plain HTTP origin nonblocking: {error}"));
    let port = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("plain HTTP origin addr: {error}"))
        .port();
    let (request_tx, request_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = accept_plain_http_origin_request(listener);
        let _ = request_tx.send(result.map_err(|error| error.to_string()));
    });

    PlainHttpOrigin {
        request_rx,
        url: format!("https://127.0.0.1:{port}/oauth/token"),
    }
}

fn spawn_tls_intercepting_proxy() -> TlsInterceptingProxy {
    codex_utils_rustls_provider::ensure_rustls_crypto_provider();
    let material = generate_tls13_material();
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .unwrap_or_else(|error| panic!("bind TLS intercepting proxy: {error}"));
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|error| panic!("set TLS intercepting proxy nonblocking: {error}"));
    let port = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("TLS intercepting proxy addr: {error}"))
        .port();
    let config = Arc::new(
        rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
            .with_no_client_auth()
            .with_single_cert(vec![material.server_cert], material.server_key)
            .unwrap_or_else(|error| panic!("TLS intercepting proxy config: {error}")),
    );
    let (request_tx, request_rx) = mpsc::channel();

    thread::spawn(move || {
        let result = accept_tls_intercepting_proxy_request(listener, config);
        let _ = request_tx.send(result.map_err(|error| error.to_string()));
    });

    TlsInterceptingProxy {
        ca_cert_pem: material.ca_cert_pem,
        request_rx,
        url: format!("http://127.0.0.1:{port}"),
    }
}

fn generate_tls13_material() -> Tls13Material {
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    let mut ca_distinguished_name = DistinguishedName::new();
    ca_distinguished_name.push(DnType::CommonName, "codex test CA");
    ca_params.distinguished_name = ca_distinguished_name;
    let ca_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
        .unwrap_or_else(|error| panic!("generate test CA key pair: {error}"));
    let ca = CertifiedIssuer::self_signed(ca_params, ca_key_pair)
        .unwrap_or_else(|error| panic!("generate test CA certificate: {error}"));

    let mut server_params =
        CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])
            .unwrap_or_else(|error| panic!("create test server certificate params: {error}"));
    server_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    server_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    let server_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
        .unwrap_or_else(|error| panic!("generate test server key pair: {error}"));
    let server_cert = server_params
        .signed_by(&server_key_pair, &ca)
        .unwrap_or_else(|error| panic!("generate test server certificate: {error}"));

    Tls13Material {
        ca_cert_pem: ca.pem(),
        server_cert: server_cert.der().clone(),
        server_key: PrivateKeyDer::from(server_key_pair),
    }
}

fn accept_plain_http_origin_request(listener: TcpListener) -> io::Result<String> {
    let mut stream = accept_with_timeout(listener, Duration::from_secs(5))?;
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let request = read_http_message(&mut stream)?;
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")?;
    stream.flush()?;
    Ok(request)
}

fn accept_tls13_request(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
) -> io::Result<String> {
    let stream = accept_with_timeout(listener, Duration::from_secs(5))?;
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let connection = rustls::ServerConnection::new(config).map_err(io::Error::other)?;
    let mut tls = rustls::StreamOwned::new(connection, stream);
    let request = read_http_message(&mut tls)?;
    tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")?;
    tls.flush()?;
    Ok(request)
}

fn accept_tls_intercepting_proxy_request(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
) -> io::Result<String> {
    let mut stream = accept_with_timeout(listener, Duration::from_secs(5))?;
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let connect_request = read_http_message(&mut stream)?;
    let origin_authority = connect_authority_from_request(&connect_request)?;
    stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
    stream.flush()?;

    let connection = rustls::ServerConnection::new(config).map_err(io::Error::other)?;
    let mut tls = rustls::StreamOwned::new(connection, stream);
    let request = read_http_message(&mut tls)?;

    let mut origin = TcpStream::connect(origin_authority.as_str())?;
    origin.set_read_timeout(Some(Duration::from_secs(5)))?;
    origin.set_write_timeout(Some(Duration::from_secs(5)))?;
    origin.write_all(request.as_bytes())?;
    origin.flush()?;
    let response = read_http_message(&mut origin)?;

    tls.write_all(response.as_bytes())?;
    tls.flush()?;
    Ok(request)
}

fn connect_authority_from_request(request: &str) -> io::Result<String> {
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty CONNECT request"))?;
    let mut parts = request_line.split_whitespace();
    match (parts.next(), parts.next(), parts.next()) {
        (Some("CONNECT"), Some(authority), Some(_version)) => Ok(authority.to_string()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid CONNECT request line: {request_line}"),
        )),
    }
}

fn accept_with_timeout(listener: TcpListener, timeout: Duration) -> io::Result<TcpStream> {
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "timed out waiting for TLS test client",
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(error),
        }
    }
}

fn read_http_message(stream: &mut impl Read) -> io::Result<String> {
    let mut buffer = Vec::new();
    let mut chunk = [0; 1024];
    loop {
        let bytes_read = stream.read(&mut chunk)?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if let Some(header_end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            let body_start = header_end + 4;
            let headers = String::from_utf8_lossy(&buffer[..body_start]);
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find_map(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if buffer.len() >= body_start + content_length {
                break;
            }
        }
    }
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

fn assert_token_exchange_request(request: &str) {
    assert!(
        request.starts_with("POST /oauth/token HTTP/1.1"),
        "unexpected request:\n{request}"
    );
    assert!(
        request.contains("grant_type=authorization_code&code=test"),
        "unexpected request body:\n{request}"
    );
}

#[test]
fn uses_codex_ca_cert_env() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ca.pem", TEST_CERT_1);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn falls_back_to_ssl_cert_file() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ssl.pem", TEST_CERT_1);

    let output = run_probe(&[(SSL_CERT_FILE_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn prefers_codex_ca_cert_over_ssl_cert_file() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "ca.pem", TEST_CERT_1);
    let bad_path = write_cert_file(&temp_dir, "bad.pem", "");

    let output = run_probe(&[
        (CODEX_CA_CERT_ENV, cert_path.as_path()),
        (SSL_CERT_FILE_ENV, bad_path.as_path()),
    ]);

    assert!(output.status.success());
}

#[test]
fn handles_multi_certificate_bundle() {
    let temp_dir = TempDir::new().expect("tempdir");
    let bundle = format!("{TEST_CERT_1}\n{TEST_CERT_2}");
    let cert_path = write_cert_file(&temp_dir, "bundle.pem", &bundle);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn posts_to_tls13_server_using_custom_ca_bundle() {
    let temp_dir = TempDir::new().expect("tempdir");
    let server = spawn_tls13_test_server();
    let cert_path = write_cert_file(&temp_dir, "tls-ca.pem", &server.ca_cert_pem);

    let output =
        run_probe_posting_to_tls13_server(&[(CODEX_CA_CERT_ENV, cert_path.as_path())], &server.url);
    let server_result = server.request_rx.recv_timeout(Duration::from_secs(5));

    assert!(
        output.status.success(),
        "custom_ca_probe failed\nstdout:\n{}\nstderr:\n{}\nserver:\n{server_result:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let request = server_result
        .expect("TLS test server should report a request")
        .expect("TLS test server should accept the probe request");
    assert_token_exchange_request(&request);
}

#[test]
fn posts_to_token_origin_through_tls_intercepting_proxy_with_custom_ca_bundle() {
    let temp_dir = TempDir::new().expect("tempdir");
    let origin = spawn_plain_http_origin();
    let proxy = spawn_tls_intercepting_proxy();
    let cert_path = write_cert_file(&temp_dir, "proxy-ca.pem", &proxy.ca_cert_pem);

    let output = run_probe_posting_through_tls_intercepting_proxy(
        &[(CODEX_CA_CERT_ENV, cert_path.as_path())],
        &origin.url,
        &proxy.url,
    );
    let proxy_result = proxy.request_rx.recv_timeout(Duration::from_secs(5));
    let origin_result = origin.request_rx.recv_timeout(Duration::from_secs(5));

    assert!(
        output.status.success(),
        "custom_ca_probe failed\nstdout:\n{}\nstderr:\n{}\nproxy:\n{proxy_result:?}\norigin:\n{origin_result:?}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let proxy_request = proxy_result
        .expect("TLS intercepting proxy should report a request")
        .expect("TLS intercepting proxy should accept the probe request");
    let origin_request = origin_result
        .expect("plain HTTP origin should report a request")
        .expect("plain HTTP origin should accept the forwarded request");
    assert_token_exchange_request(&proxy_request);
    assert_token_exchange_request(&origin_request);
}

#[test]
fn rejects_empty_pem_file_with_hint() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "empty.pem", "");

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no certificates found in PEM file"));
    assert!(stderr.contains("CODEX_CA_CERTIFICATE"));
    assert!(stderr.contains("SSL_CERT_FILE"));
}

#[test]
fn rejects_malformed_pem_with_hint() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(
        &temp_dir,
        "malformed.pem",
        "-----BEGIN CERTIFICATE-----\nMIIBroken",
    );

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse PEM file"));
    assert!(stderr.contains("CODEX_CA_CERTIFICATE"));
    assert!(stderr.contains("SSL_CERT_FILE"));
}

#[test]
fn accepts_openssl_trusted_certificate() {
    let temp_dir = TempDir::new().expect("tempdir");
    let cert_path = write_cert_file(&temp_dir, "trusted.pem", TRUSTED_TEST_CERT);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}

#[test]
fn accepts_bundle_with_crl() {
    let temp_dir = TempDir::new().expect("tempdir");
    let crl = "-----BEGIN X509 CRL-----\nMIIC\n-----END X509 CRL-----";
    let bundle = format!("{TEST_CERT_1}\n{crl}");
    let cert_path = write_cert_file(&temp_dir, "bundle_crl.pem", &bundle);

    let output = run_probe(&[(CODEX_CA_CERT_ENV, cert_path.as_path())]);

    assert!(output.status.success());
}
