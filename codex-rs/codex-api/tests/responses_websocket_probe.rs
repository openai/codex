#![allow(clippy::expect_used)]
//! Exercise TLS, upgrade, rejection, and post-upgrade fault evidence with local wire fixtures.

use codex_api::ApiError;
use codex_api::AuthProvider;
use codex_api::Provider;
use codex_api::ResponsesWebsocketClient;
use codex_api::ResponsesWebsocketProbeFailureStage;
use codex_api::RetryConfig;
use codex_api::TransportError;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;

struct NoAuth;
impl AuthProvider for NoAuth {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) -> Result<(), codex_api::AuthError> {
        Ok(())
    }
}

fn probe_client(base_url: String) -> ResponsesWebsocketClient {
    ResponsesWebsocketClient::new(
        Provider {
            name: "probe fixture".into(),
            base_url,
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
        },
        Arc::new(NoAuth),
    )
}

#[tokio::test]
async fn probe_distinguishes_untrusted_certificates_from_io_failures() -> anyhow::Result<()> {
    codex_utils_rustls_provider::ensure_rustls_crypto_provider();
    let timeout = Duration::from_secs(/*secs*/ 5);
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let config = Arc::new(
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.cert.der().clone()],
                certificate.signing_key.into(),
            )?,
    );
    for (serve_tls, expected_stage) in [
        (true, ResponsesWebsocketProbeFailureStage::Tls),
        (false, ResponsesWebsocketProbeFailureStage::Unknown),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let config = Arc::clone(&config);
        let server = tokio::spawn(async move {
            let (socket, _) = tokio::time::timeout(timeout, listener.accept()).await??;
            if serve_tls {
                let mut socket = socket.into_std()?;
                socket.set_nonblocking(/*nonblocking*/ false)?;
                socket.set_read_timeout(Some(timeout))?;
                socket.set_write_timeout(Some(timeout))?;
                let mut connection = rustls::ServerConnection::new(config)?;
                tokio::task::spawn_blocking(move || {
                    connection
                        .complete_io(&mut socket)
                        .expect_err("client must reject the untrusted certificate");
                })
                .await?;
            }
            // Closing before the handshake produces an ordinary I/O failure.
            Ok::<_, anyhow::Error>(())
        });
        let client = probe_client(format!("https://localhost:{}", address.port()));
        let error = tokio::time::timeout(
            timeout,
            client.probe_handshake(
                &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                HeaderMap::new(),
                HeaderMap::new(),
                Duration::from_millis(/*millis*/ 25),
            ),
        )
        .await
        .expect("probe finishes")
        .expect_err("handshake must fail");
        assert_eq!(
            (error.stage, error.status, error.headers),
            (expected_stage, None, None)
        );
        server.await??;
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Reply {
    EmptyClose,
    CloseWithReason,
    HoldOpen,
    Rejected,
    MalformedFrame,
}

#[tokio::test]
async fn probe_preserves_evidence_and_redacts_wire_secrets() {
    for (reply, expected_status, expected_close) in [
        (Reply::EmptyClose, 101, Some(("1005", ""))),
        (
            Reply::CloseWithReason,
            101,
            Some(("1008", "private-close-reason")),
        ),
        (Reply::HoldOpen, 101, None),
        (Reply::Rejected, 403, None),
        (Reply::MalformedFrame, 101, None),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept fixture");
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(socket.read_u8().await.expect("read handshake"));
                assert!(request.len() < 8192);
            }
            let headers = concat!(
                "x-request-id: fixture-request\r\n",
                "x-oai-request-id: fixture-oai-request\r\n",
                "cf-ray: fixture-ray\r\n",
                "x-ms-request-id: fixture-ms-one\r\n",
                "x-ms-request-id: fixture-ms-two\r\n",
                "x-reasoning-included: true\r\n",
                "openai-model: fixture-model\r\n",
                "set-cookie: private-cookie\r\n",
            );
            let response = match reply {
                Reply::Rejected => format!(
                    "HTTP/1.1 403 Forbidden\r\n{headers}content-length: 12\r\n\r\nprivate-body"
                ),
                Reply::EmptyClose
                | Reply::CloseWithReason
                | Reply::HoldOpen
                | Reply::MalformedFrame => {
                    let request = String::from_utf8(request).expect("HTTP request");
                    let key = request
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("sec-websocket-key")
                                .then(|| value.trim())
                        })
                        .expect("WebSocket key");
                    let accept = derive_accept_key(key.as_bytes());
                    format!(
                        "HTTP/1.1 101 Switching Protocols\r\nconnection: Upgrade\r\nupgrade: websocket\r\nsec-websocket-accept: {accept}\r\n{headers}\r\n"
                    )
                }
            };
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write handshake");
            match reply {
                Reply::EmptyClose => socket.write_all(&[0x88, 0]).await.expect("write frame"),
                Reply::CloseWithReason => {
                    let mut frame = vec![0x88, 22, 0x03, 0xf0];
                    frame.extend_from_slice(b"private-close-reason");
                    socket.write_all(&frame).await.expect("write frame");
                }
                Reply::HoldOpen => tokio::time::sleep(Duration::from_millis(/*millis*/ 50)).await,
                // Unassigned opcode 3 does not depend on extension negotiation.
                Reply::MalformedFrame => socket.write_all(&[0x83, 0]).await.expect("write frame"),
                Reply::Rejected => (),
            }
        });
        let client = probe_client(format!("http://{address}"));
        let immediate_close_timeout = if reply == Reply::HoldOpen {
            Duration::from_millis(/*millis*/ 25)
        } else {
            Duration::from_secs(/*secs*/ 1)
        };
        let result = tokio::time::timeout(
            Duration::from_secs(/*secs*/ 5),
            client.probe_handshake(
                &HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
                HeaderMap::new(),
                HeaderMap::new(),
                immediate_close_timeout,
            ),
        )
        .await
        .expect("probe finishes");
        let (status, headers, close) = match result {
            Ok(probe) => {
                assert!(matches!(
                    reply,
                    Reply::EmptyClose | Reply::CloseWithReason | Reply::HoldOpen
                ));
                assert_eq!(
                    (probe.reasoning_included, probe.server_model_present),
                    (true, true)
                );
                let debug = format!("{probe:?}");
                for secret in [
                    "fixture-request",
                    "fixture-oai-request",
                    "fixture-ray",
                    "fixture-ms",
                    "private-cookie",
                    "private-close-reason",
                ] {
                    assert!(!debug.contains(secret), "{debug}");
                }
                let close = probe
                    .immediate_close
                    .map(|close| (close.code, close.reason));
                (probe.status, probe.headers, close)
            }
            Err(error) => {
                assert_eq!(error.to_string(), "Responses WebSocket probe failed");
                assert!(!format!("{error:?}").contains("private-"));
                assert_eq!(error.stage, ResponsesWebsocketProbeFailureStage::Unknown);
                match *error.error {
                    ApiError::Transport(TransportError::Http {
                        status,
                        headers,
                        body,
                        ..
                    }) => {
                        assert!(matches!(reply, Reply::Rejected));
                        assert_eq!(body.as_deref(), Some("private-body"));
                        (status, headers.expect("rejected headers"), None)
                    }
                    _ => {
                        assert!(matches!(reply, Reply::MalformedFrame));
                        (
                            error.status.expect("received 101"),
                            error.headers.expect("received headers"),
                            None,
                        )
                    }
                }
            }
        };
        let mut expected_headers = HeaderMap::new();
        for (name, value) in [
            ("x-request-id", "fixture-request"),
            ("x-oai-request-id", "fixture-oai-request"),
            ("cf-ray", "fixture-ray"),
            ("x-ms-request-id", "fixture-ms-one"),
            ("x-ms-request-id", "fixture-ms-two"),
        ] {
            expected_headers.append(name, HeaderValue::from_static(value));
        }
        if matches!(reply, Reply::Rejected) {
            for (name, value) in [
                ("x-reasoning-included", "true"),
                ("openai-model", "fixture-model"),
                ("set-cookie", "private-cookie"),
                ("content-length", "12"),
            ] {
                expected_headers.append(name, HeaderValue::from_static(value));
            }
        }
        assert_eq!(
            (
                status.as_u16(),
                headers,
                close
                    .as_ref()
                    .map(|(code, reason)| (code.as_str(), reason.as_str())),
            ),
            (expected_status, expected_headers, expected_close)
        );
        server.await.expect("fixture completes");
    }
}
