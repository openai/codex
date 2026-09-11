#![allow(clippy::expect_used)]
//! Exercise realtime TLS selection without mutating the test process's trust environment.

use std::io;
use std::net::TcpListener;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_api::Provider;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeOutputModality;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeSessionMode;
use codex_api::RealtimeWebsocketClient;
use codex_api::RetryConfig;
use codex_protocol::protocol::RealtimeVoice;
use http::HeaderMap;
use pretty_assertions::assert_eq;

const ADDRESS_ENV: &str = "CODEX_TEST_REALTIME_TLS_ADDRESS";
const TRUST_ENV: &str = "CODEX_TEST_REALTIME_TLS_TRUST";

#[test]
fn realtime_tls_selects_system_and_custom_trust() {
    if let Ok(address) = std::env::var(ADDRESS_ENV) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(check_connection(address));
        return;
    }

    codex_utils_rustls_provider::ensure_rustls_crypto_provider();
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let temp = tempfile::TempDir::new().unwrap();
    let ca = temp.path().join("ca.pem");
    std::fs::write(&ca, certificate.cert.pem()).unwrap();
    let config = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                certificate.signing_key.into(),
            )
            .unwrap(),
    );

    for trust in ["system", "custom"] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(/*nonblocking*/ true).unwrap();
        let config = config.clone();
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(/*secs*/ 30);
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "TLS client did not connect");
                        std::thread::sleep(Duration::from_millis(/*millis*/ 10));
                    }
                    Err(error) => panic!("TLS accept failed: {error}"),
                }
            };
            stream.set_nonblocking(/*nonblocking*/ false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(/*secs*/ 30)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(/*secs*/ 30)))
                .unwrap();
            let tls =
                rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), stream);
            match tungstenite::accept(tls) {
                Ok(mut socket) => {
                    assert_eq!(trust, "custom", "system trust must reject the generated CA");
                    let message = socket.read().unwrap().into_text().unwrap();
                    let update: serde_json::Value = serde_json::from_str(&message).unwrap();
                    assert_eq!(update["type"], "session.update");
                }
                Err(_) => assert_eq!(trust, "system", "custom CA must allow WSS"),
            }
        });
        // Re-execute this test so each connection uses its own CA environment, including on Windows.
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "realtime_tls_selects_system_and_custom_trust",
                "--nocapture",
            ])
            .env(ADDRESS_ENV, format!("localhost:{}", address.port()))
            .env(TRUST_ENV, trust)
            .env_remove("CODEX_CA_CERTIFICATE")
            .env_remove("SSL_CERT_FILE");
        if trust == "custom" {
            child.env("CODEX_CA_CERTIFICATE", &ca).env(
                "SSL_CERT_FILE",
                temp.path().join("missing-lower-priority-ca.pem"),
            );
        }
        let output = child.output().unwrap();
        server.join().unwrap();
        assert!(
            output.status.success(),
            "{trust}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

async fn check_connection(address: String) {
    let client = RealtimeWebsocketClient::new(Provider {
        name: "local TLS test".into(),
        base_url: format!("https://{address}"),
        query_params: None,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(/*millis*/ 1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        },
        stream_idle_timeout: Duration::from_secs(/*secs*/ 5),
    });
    let result = tokio::time::timeout(
        Duration::from_secs(/*secs*/ 20),
        client.connect(
            RealtimeSessionConfig {
                instructions: "TLS test".into(),
                initial_items: Vec::new(),
                delegation_ack_filler: None,
                model: Some("realtime-test-model".into()),
                session_id: None,
                event_parser: RealtimeEventParser::V1,
                session_mode: RealtimeSessionMode::Conversational,
                output_modality: RealtimeOutputModality::Audio,
                voice: RealtimeVoice::Cove,
            },
            HeaderMap::new(),
            HeaderMap::new(),
        ),
    )
    .await
    .expect("TLS connection should finish");
    match std::env::var(TRUST_ENV)
        .expect("trust scenario should be set")
        .as_str()
    {
        "custom" => {
            result.expect("configured CA should permit realtime WSS");
        }
        "system" => {
            let error = match result {
                Ok(_) => panic!("system trust accepted an untrusted certificate"),
                Err(error) => error.to_string(),
            };
            assert!(error.to_lowercase().contains("certificate"), "{error}");
        }
        other => panic!("unknown trust scenario: {other}"),
    }
}
