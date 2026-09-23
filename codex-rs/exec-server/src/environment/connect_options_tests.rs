use std::collections::HashMap;
use std::time::Duration;

use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::SinkExt;
use futures::StreamExt;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;

use super::RemoteEnvironmentOptions;
use crate::EnvironmentManager;
use crate::InitializeParams;
use crate::InitializeResponse;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::JSONRPCMessage;
use crate::protocol::JSONRPCResponse;

#[test]
fn remote_environment_options_redact_header_values() {
    let options = RemoteEnvironmentOptions {
        exec_server_url: "wss://relay.example/environment".to_string(),
        connect_timeout: Some(Duration::from_secs(5)),
        http_headers: HashMap::from([(
            "authorization".to_string(),
            "Bearer secret-customer-token".to_string(),
        )]),
    };

    let debug = format!("{options:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-customer-token"));
}

#[test]
fn trusted_headers_require_tls_for_non_loopback_destinations() {
    let manager = EnvironmentManager::without_environments(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ));

    let error = manager
        .upsert_environment_with_options(
            "customer-environment".to_string(),
            RemoteEnvironmentOptions {
                exec_server_url: "ws://relay.example/environment".to_string(),
                connect_timeout: None,
                http_headers: HashMap::from([(
                    "x-session-id".to_string(),
                    "customer-session".to_string(),
                )]),
            },
        )
        .expect_err("trusted headers must not be sent over an insecure remote connection");

    assert_eq!(
        error.to_string(),
        "exec-server protocol error: exec-server WebSocket headers require wss:// or a loopback destination"
    );
    assert!(manager.get_environment("customer-environment").is_none());
}

#[test]
fn duplicate_case_insensitive_websocket_headers_fail_before_registration() {
    let manager = EnvironmentManager::without_environments(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ));

    let error = manager
        .upsert_environment_with_options(
            "customer-environment".to_string(),
            RemoteEnvironmentOptions {
                exec_server_url: "ws://127.0.0.1:8765".to_string(),
                connect_timeout: None,
                http_headers: HashMap::from([
                    ("X-Session-Id".to_string(), "first-session".to_string()),
                    ("x-session-id".to_string(), "second-session".to_string()),
                ]),
            },
        )
        .expect_err("duplicate header names must fail regardless of case");

    assert_eq!(
        error.to_string(),
        "exec-server protocol error: duplicate exec-server WebSocket header `x-session-id`"
    );
    assert!(manager.get_environment("customer-environment").is_none());
}

#[test]
fn invalid_websocket_header_names_fail_before_registration() {
    let manager = EnvironmentManager::without_environments(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ));

    let error = manager
        .upsert_environment_with_options(
            "customer-environment".to_string(),
            RemoteEnvironmentOptions {
                exec_server_url: "ws://127.0.0.1:8765".to_string(),
                connect_timeout: None,
                http_headers: HashMap::from([("bad header".to_string(), "value".to_string())]),
            },
        )
        .expect_err("invalid header name should fail");

    assert_eq!(
        error.to_string(),
        "exec-server protocol error: invalid exec-server WebSocket header name `bad header`"
    );
    assert!(manager.get_environment("customer-environment").is_none());
}

#[test]
fn invalid_websocket_header_values_fail_before_registration() {
    let manager = EnvironmentManager::without_environments(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ));

    let error = manager
        .upsert_environment_with_options(
            "customer-environment".to_string(),
            RemoteEnvironmentOptions {
                exec_server_url: "ws://127.0.0.1:8765".to_string(),
                connect_timeout: None,
                http_headers: HashMap::from([(
                    "x-session-id".to_string(),
                    "customer\nspoofed".to_string(),
                )]),
            },
        )
        .expect_err("invalid header value should fail");

    assert_eq!(
        error.to_string(),
        "exec-server protocol error: invalid value for exec-server WebSocket header `x-session-id`"
    );
    assert!(manager.get_environment("customer-environment").is_none());
}

#[test]
fn websocket_controlled_headers_fail_before_registration() {
    let manager = EnvironmentManager::without_environments(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ));

    for header in ["host", "connection", "upgrade", "sec-websocket-key"] {
        let error = manager
            .upsert_environment_with_options(
                "customer-environment".to_string(),
                RemoteEnvironmentOptions {
                    exec_server_url: "ws://127.0.0.1:8765".to_string(),
                    connect_timeout: None,
                    http_headers: HashMap::from([(header.to_string(), "overridden".to_string())]),
                },
            )
            .expect_err("connection-controlled header should fail");

        assert_eq!(
            error.to_string(),
            format!(
                "exec-server protocol error: exec-server WebSocket header `{header}` is controlled by the connection"
            )
        );
        assert!(manager.get_environment("customer-environment").is_none());
    }
}

#[tokio::test]
async fn trusted_headers_are_sent_on_initial_websocket_and_session_reconnect() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_url = format!(
        "ws://{}",
        listener.local_addr().expect("listener should have address")
    );
    let (resumed_tx, resumed_rx) = oneshot::channel();
    let (finish_tx, finish_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut first = accept_routing_headers_websocket(&listener).await;
        complete_websocket_initialize(&mut first, /*expected_resume_session_id*/ None).await;
        first
            .close(None)
            .await
            .expect("first websocket should close");

        let mut resumed = accept_routing_headers_websocket(&listener).await;
        complete_websocket_initialize(&mut resumed, Some("session-1")).await;
        resumed_tx.send(()).expect("resume should signal");
        finish_rx.await.expect("test should finish");
    });

    let manager = EnvironmentManager::without_environments(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ));
    manager
        .upsert_environment_with_options(
            "customer-environment".to_string(),
            RemoteEnvironmentOptions {
                exec_server_url: websocket_url,
                connect_timeout: Some(Duration::from_secs(1)),
                http_headers: HashMap::from([
                    (
                        "authorization".to_string(),
                        "Bearer reconnect-capability".to_string(),
                    ),
                    (
                        "x-account-id".to_string(),
                        "customer-account-456".to_string(),
                    ),
                    (
                        "x-session-id".to_string(),
                        "customer-session-123".to_string(),
                    ),
                    (
                        "x-environment-id".to_string(),
                        "customer-environment-789".to_string(),
                    ),
                ]),
            },
        )
        .expect("environment with routing headers should register");

    let environment = manager
        .get_environment("customer-environment")
        .expect("environment with routing headers should exist");
    environment
        .wait_until_ready()
        .await
        .expect("environment with routing headers should initialize");
    timeout(Duration::from_secs(3), resumed_rx)
        .await
        .expect("routing-header session resume should not time out")
        .expect("routing-header session resume should signal");

    finish_tx.send(()).expect("test should finish");
    server.await.expect("server task should finish");
}

async fn accept_routing_headers_websocket(listener: &TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = listener.accept().await.expect("listener should accept");
    accept_hdr_async(stream, |request: &Request, response: Response| {
        assert_eq!(
            request.headers().get(http::header::AUTHORIZATION),
            Some(&HeaderValue::from_static("Bearer reconnect-capability"))
        );
        assert_eq!(
            request.headers().get("x-account-id"),
            Some(&HeaderValue::from_static("customer-account-456"))
        );
        assert_eq!(
            request.headers().get("x-session-id"),
            Some(&HeaderValue::from_static("customer-session-123"))
        );
        assert_eq!(
            request.headers().get("x-environment-id"),
            Some(&HeaderValue::from_static("customer-environment-789"))
        );
        Ok(response)
    })
    .await
    .expect("routing-header websocket handshake should succeed")
}

async fn complete_websocket_initialize(
    websocket: &mut WebSocketStream<TcpStream>,
    expected_resume_session_id: Option<&str>,
) {
    let message = websocket
        .next()
        .await
        .expect("initialize request should arrive")
        .expect("initialize websocket message should succeed");
    let Message::Text(encoded) = message else {
        panic!("expected initialize text message");
    };
    let JSONRPCMessage::Request(request) =
        serde_json::from_str::<JSONRPCMessage>(&encoded).expect("initialize request should parse")
    else {
        panic!("expected initialize request");
    };
    assert_eq!(request.method, INITIALIZE_METHOD);
    let params: InitializeParams = serde_json::from_value(
        request
            .params
            .expect("initialize request should contain parameters"),
    )
    .expect("initialize parameters should parse");
    assert_eq!(
        params.resume_session_id.as_deref(),
        expected_resume_session_id
    );

    let response = JSONRPCMessage::Response(JSONRPCResponse {
        id: request.id,
        result: serde_json::to_value(InitializeResponse {
            session_id: "session-1".to_string(),
            environment_info: None,
        })
        .expect("initialize response should serialize"),
    });
    websocket
        .send(Message::Text(
            serde_json::to_string(&response)
                .expect("initialize response should encode")
                .into(),
        ))
        .await
        .expect("initialize response should send");

    let message = websocket
        .next()
        .await
        .expect("initialized notification should arrive")
        .expect("initialized websocket message should succeed");
    let Message::Text(encoded) = message else {
        panic!("expected initialized text message");
    };
    let JSONRPCMessage::Notification(notification) =
        serde_json::from_str::<JSONRPCMessage>(&encoded)
            .expect("initialized notification should parse")
    else {
        panic!("expected initialized notification");
    };
    assert_eq!(notification.method, INITIALIZED_METHOD);
}
