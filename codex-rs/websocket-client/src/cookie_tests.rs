//! Real WebSocket handshakes exercising shared routing cookies and their transport boundaries.

use std::sync::Arc;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::OutboundProxyRoute;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use pretty_assertions::assert_eq;
use rcgen::CertifiedKey;
use rcgen::generate_simple_self_signed;
use rustls::ClientConfig;
use rustls::RootCertStore;
use rustls::ServerConfig;
use rustls::pki_types::PrivateKeyDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::Error as WebSocketError;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::http::header::COOKIE;
use tokio_tungstenite::tungstenite::http::header::SET_COOKIE;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::AsyncIo;
use crate::TcpNodelay;
use crate::WebSocketConnector;

#[tokio::test]
async fn websocket_handshakes_share_routing_cookies_and_respect_cookie_scope() {
    ensure_rustls_crypto_provider();
    let host = "websocket-cookie-test.chatgpt.com";
    let other_host = "other-websocket-cookie-test.chatgpt.com";
    let CertifiedKey { cert, signing_key } = generate_simple_self_signed(vec![
        host.to_string(),
        other_host.to_string(),
        "api.openai.com".to_string(),
    ])
    .unwrap();
    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der())),
        )
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let mut roots = RootCertStore::empty();
    roots.add(cert.der().clone()).unwrap();
    let tls_config = Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    );

    let parent_url = format!("wss://{host}/backend-api/codex/responses");
    let review_url = format!("wss://{host}/backend-api/codex/guardian");
    let requests = [
        (parent_url, None),
        (review_url.clone(), None),
        (review_url.clone(), Some("explicit=keep")),
        (format!("wss://{host}/outside"), None),
        (
            format!("wss://{other_host}/backend-api/codex/responses"),
            None,
        ),
        ("wss://api.openai.com/v1/responses".to_string(), None),
        ("wss://api.openai.com/v1/responses".to_string(), None),
        (format!("ws://{host}/backend-api/codex/responses"), None),
        (review_url.clone(), None),
        (review_url.clone(), None),
        (review_url, None),
    ];
    let responses: Vec<(StatusCode, Vec<&str>)> = vec![
        (
            StatusCode::SWITCHING_PROTOCOLS,
            vec![
                "__oailb=west; Path=/backend-api; Secure; HttpOnly",
                "chatgpt_session=never-store; Path=/; Secure",
            ],
        ),
        // Even a failed upgrade can refresh a cookie used by the next connection.
        (
            StatusCode::FORBIDDEN,
            vec!["__oailb=east; Path=/backend-api; Secure"],
        ),
        (StatusCode::SWITCHING_PROTOCOLS, vec![]),
        (StatusCode::SWITCHING_PROTOCOLS, vec![]),
        (StatusCode::SWITCHING_PROTOCOLS, vec![]),
        (
            StatusCode::SWITCHING_PROTOCOLS,
            vec!["__oailb=untrusted; Path=/; Secure"],
        ),
        (StatusCode::SWITCHING_PROTOCOLS, vec![]),
        (
            StatusCode::SWITCHING_PROTOCOLS,
            vec!["__oailb=insecure; Path=/backend-api"],
        ),
        (StatusCode::SWITCHING_PROTOCOLS, vec![]),
        (
            StatusCode::SWITCHING_PROTOCOLS,
            vec!["__oailb=; Path=/backend-api; Max-Age=0; Secure"],
        ),
        (StatusCode::SWITCHING_PROTOCOLS, vec![]),
    ];
    let expected_statuses = responses
        .iter()
        .map(|(status, _)| *status)
        .collect::<Vec<_>>();
    let secure = requests
        .iter()
        .map(|(url, _)| url.starts_with("wss:"))
        .collect::<Vec<_>>();

    // Terminate CONNECT locally so the real TLS/upgrade path can use ChatGPT hostnames without DNS
    // overrides, environment changes, or external traffic.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut received_cookies = Vec::new();
        for ((status, cookies), secure) in responses.into_iter().zip(secure) {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut connect = Vec::new();
            while !connect.ends_with(b"\r\n\r\n") {
                connect.push(stream.read_u8().await.unwrap());
            }
            stream
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
                .unwrap();
            let stream: Box<dyn AsyncIo> = if secure {
                Box::new(acceptor.accept(stream).await.unwrap())
            } else {
                Box::new(stream)
            };
            let result = accept_hdr_async(stream, |request: &Request, mut response: Response| {
                received_cookies.push(request.headers().get(COOKIE).cloned());
                for cookie in cookies {
                    response
                        .headers_mut()
                        .append(SET_COOKIE, HeaderValue::from_static(cookie));
                }
                if status == StatusCode::SWITCHING_PROTOCOLS {
                    Ok(response)
                } else {
                    *response.status_mut() = status;
                    Err(response.map(|()| None))
                }
            })
            .await;
            if status == StatusCode::SWITCHING_PROTOCOLS {
                drop(result.unwrap());
            } else {
                assert!(matches!(result, Err(WebSocketError::Http(_))));
            }
        }
        received_cookies
    });

    let mut actual_statuses = Vec::new();
    for (url, explicit_cookie) in requests {
        // Separate connector/factory instances must still see the shared routing cookie.
        let connector = WebSocketConnector {
            http_client_factory: HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            tls_config: Some(Arc::clone(&tls_config)),
            tcp_nodelay: TcpNodelay::Default,
        };
        let mut request = url.into_client_request().unwrap();
        if let Some(cookie) = explicit_cookie {
            request
                .headers_mut()
                .insert(COOKIE, HeaderValue::from_static(cookie));
        }
        let result = connector
            .connect_with_route(
                request,
                WebSocketConfig::default(),
                std::future::ready(Ok(OutboundProxyRoute::Proxy {
                    url: proxy_url.clone(),
                    no_proxy: None,
                })),
                /*loopback_direct*/ false,
            )
            .await;
        let status = match result {
            Ok((connection, response)) => {
                drop(connection);
                response.status()
            }
            Err(WebSocketError::Http(response)) => response.status(),
            Err(error) => panic!("unexpected handshake error: {error}"),
        };
        actual_statuses.push(status);
    }
    assert_eq!(actual_statuses, expected_statuses);
    assert_eq!(
        server.await.unwrap(),
        [
            None,
            Some("__oailb=west"),
            Some("explicit=keep"),
            None,
            None,
            None,
            None,
            None,
            Some("__oailb=east"),
            Some("__oailb=east"),
            None
        ]
        .map(|cookie| cookie.map(HeaderValue::from_static))
        .to_vec(),
    );
}
