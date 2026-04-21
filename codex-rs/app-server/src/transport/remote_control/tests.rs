use super::enroll::REMOTE_CONTROL_ACCOUNT_ID_HEADER;
use super::enroll::RemoteControlEnrollment;
use super::enroll::load_persisted_remote_control_enrollment;
use super::enroll::update_persisted_remote_control_enrollment;
use super::protocol::ClientEnvelope;
use super::protocol::ClientEvent;
use super::protocol::ClientId;
use super::protocol::StreamId;
use super::protocol::normalize_remote_control_url;
use super::websocket::REMOTE_CONTROL_PROTOCOL_VERSION;
use super::*;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::QueuedOutgoingMessage;
use crate::transport::CHANNEL_CAPACITY;
use crate::transport::TransportEvent;
use base64::Engine;
use codex_app_server_protocol::AuthMode;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ServerNotification;
use codex_config::types::AuthCredentialsStoreMode;
use codex_core::test_support::auth_manager_from_auth;
use codex_core::test_support::auth_manager_from_auth_with_home;
use codex_login::AuthDotJson;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::save_auth;
use codex_login::token_data::TokenData;
use codex_login::token_data::parse_chatgpt_jwt_claims;
use codex_state::StateRuntime;
use futures::SinkExt;
use futures::StreamExt;
use gethostname::gethostname;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite;
use tokio_util::sync::CancellationToken;

fn remote_control_auth_manager() -> Arc<AuthManager> {
    auth_manager_from_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
}

fn remote_control_auth_manager_with_home(codex_home: &TempDir) -> Arc<AuthManager> {
    auth_manager_from_auth_with_home(
        CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        codex_home.path().to_path_buf(),
    )
}

fn remote_control_auth_dot_json(account_id: Option<&str>) -> AuthDotJson {
    #[derive(serde::Serialize)]
    struct Header {
        alg: &'static str,
        typ: &'static str,
    }

    let header = Header {
        alg: "none",
        typ: "JWT",
    };
    let payload = serde_json::json!({
        "email": "user@example.com",
        "https://api.openai.com/auth": {
            "chatgpt_user_id": "user-12345",
            "user_id": "user-12345",
            "chatgpt_account_id": "account_id"
        }
    });
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let header_b64 = b64(&serde_json::to_vec(&header).expect("header should serialize"));
    let payload_b64 = b64(&serde_json::to_vec(&payload).expect("payload should serialize"));
    let fake_jwt = format!("{header_b64}.{payload_b64}.sig");

    AuthDotJson {
        auth_mode: Some(AuthMode::Chatgpt),
        openai_api_key: None,
        tokens: Some(TokenData {
            id_token: parse_chatgpt_jwt_claims(&fake_jwt).expect("fake jwt should parse"),
            access_token: "Access Token".to_string(),
            refresh_token: "refresh-token".to_string(),
            account_id: account_id.map(str::to_string),
        }),
        last_refresh: Some(chrono::Utc::now()),
    }
}

async fn remote_control_state_runtime(codex_home: &TempDir) -> Arc<StateRuntime> {
    StateRuntime::init(codex_home.path().to_path_buf(), "test-provider".to_string())
        .await
        .expect("state runtime should initialize")
}

fn remote_control_url_for_listener(listener: &TcpListener) -> String {
    let addr = listener
        .local_addr()
        .expect("listener should have a local addr");
    format!("http://{addr}/backend-api/")
}

#[tokio::test]
async fn remote_control_transport_manages_virtual_clients_and_routes_messages() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, mut transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        Some(remote_control_state_runtime(&codex_home).await),
        remote_control_auth_manager(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");
    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_json(
        enroll_request.stream,
        json!({ "server_id": "srv_e_test", "environment_id": "env_test" }),
    )
    .await;
    let mut websocket = accept_remote_control_connection(&listener).await;

    let client_id = ClientId("client-1".to_string());
    send_client_event(
        &mut websocket,
        ClientEnvelope {
            event: ClientEvent::Ping,
            client_id: client_id.clone(),
            stream_id: None,
            seq_id: None,
            cursor: None,
        },
    )
    .await;
    assert_eq!(
        read_server_event(&mut websocket).await,
        json!({
            "type": "pong",
            "client_id": "client-1",
            "seq_id": 1,
            "status": "unknown",
        })
    );

    send_client_event(
        &mut websocket,
        ClientEnvelope {
            event: ClientEvent::ClientMessage {
                message: JSONRPCMessage::Notification(
                    codex_app_server_protocol::JSONRPCNotification {
                        method: "initialized".to_string(),
                        params: None,
                    },
                ),
            },
            client_id: client_id.clone(),
            stream_id: None,
            seq_id: Some(0),
            cursor: None,
        },
    )
    .await;
    assert!(
        timeout(Duration::from_millis(100), transport_event_rx.recv())
            .await
            .is_err(),
        "non-initialize client messages should be ignored before connection creation"
    );

    let initialize_message = JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
        id: codex_app_server_protocol::RequestId::Integer(1),
        method: "initialize".to_string(),
        params: Some(json!({
            "clientInfo": {
                "name": "remote-test-client",
                "version": "0.1.0"
            }
        })),
        trace: None,
    });
    send_client_event(
        &mut websocket,
        ClientEnvelope {
            event: ClientEvent::ClientMessage {
                message: initialize_message.clone(),
            },
            client_id: client_id.clone(),
            stream_id: None,
            seq_id: Some(1),
            cursor: None,
        },
    )
    .await;

    let (connection_id, writer) = match timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .expect("connection open should arrive in time")
        .expect("connection open should exist")
    {
        TransportEvent::ConnectionOpened {
            connection_id,
            writer,
            ..
        } => (connection_id, writer),
        other => panic!("expected connection open event, got {other:?}"),
    };

    match timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .expect("initialize message should arrive in time")
        .expect("initialize message should exist")
    {
        TransportEvent::IncomingMessage {
            connection_id: incoming_connection_id,
            message,
        } => {
            assert_eq!(incoming_connection_id, connection_id);
            assert_eq!(message, initialize_message);
        }
        other => panic!("expected initialize incoming message, got {other:?}"),
    }

    let followup_message =
        JSONRPCMessage::Notification(codex_app_server_protocol::JSONRPCNotification {
            method: "initialized".to_string(),
            params: None,
        });
    send_client_event(
        &mut websocket,
        ClientEnvelope {
            event: ClientEvent::ClientMessage {
                message: followup_message.clone(),
            },
            client_id: client_id.clone(),
            stream_id: None,
            seq_id: Some(2),
            cursor: None,
        },
    )
    .await;
    match timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .expect("followup message should arrive in time")
        .expect("followup message should exist")
    {
        TransportEvent::IncomingMessage {
            connection_id: incoming_connection_id,
            message,
        } => {
            assert_eq!(incoming_connection_id, connection_id);
            assert_eq!(message, followup_message);
        }
        other => panic!("expected followup incoming message, got {other:?}"),
    }

    send_client_event(
        &mut websocket,
        ClientEnvelope {
            event: ClientEvent::Ping,
            client_id: client_id.clone(),
            stream_id: None,
            seq_id: None,
            cursor: None,
        },
    )
    .await;
    assert_eq!(
        read_server_event(&mut websocket).await,
        json!({
            "type": "pong",
            "client_id": "client-1",
            "seq_id": 1,
            "status": "active",
        })
    );

    writer
        .send(QueuedOutgoingMessage::new(
            OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                ConfigWarningNotification {
                    summary: "test".to_string(),
                    details: None,
                    path: None,
                    range: None,
                },
            )),
        ))
        .await
        .expect("remote writer should accept outgoing message");
    assert_eq!(
        read_server_event(&mut websocket).await,
        json!({
            "type": "server_message",
            "client_id": "client-1",
            "seq_id": 2,
            "message": {
                "method": "configWarning",
                "params": {
                    "summary": "test",
                    "details": null,
                }
            }
        })
    );

    send_client_event(
        &mut websocket,
        ClientEnvelope {
            event: ClientEvent::ClientClosed,
            client_id: client_id.clone(),
            stream_id: None,
            seq_id: None,
            cursor: None,
        },
    )
    .await;
    match timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .expect("connection close should arrive in time")
        .expect("connection close should exist")
    {
        TransportEvent::ConnectionClosed {
            connection_id: closed_connection_id,
        } => {
            assert_eq!(closed_connection_id, connection_id);
        }
        other => panic!("expected connection close event, got {other:?}"),
    }

    send_client_event(
        &mut websocket,
        ClientEnvelope {
            event: ClientEvent::Ping,
            client_id,
            stream_id: None,
            seq_id: None,
            cursor: None,
        },
    )
    .await;
    assert_eq!(
        read_server_event(&mut websocket).await,
        json!({
            "type": "pong",
            "client_id": "client-1",
            "seq_id": 1,
            "status": "unknown",
        })
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_transport_reconnects_after_disconnect() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, mut transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        Some(remote_control_state_runtime(&codex_home).await),
        remote_control_auth_manager(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");

    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_json(
        enroll_request.stream,
        json!({ "server_id": "srv_e_test", "environment_id": "env_test" }),
    )
    .await;
    let mut first_websocket = accept_remote_control_connection(&listener).await;
    first_websocket
        .close(None)
        .await
        .expect("first websocket should close");
    drop(first_websocket);

    let mut second_websocket = accept_remote_control_connection(&listener).await;
    send_client_event(
        &mut second_websocket,
        ClientEnvelope {
            event: ClientEvent::ClientMessage {
                message: JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
                    id: codex_app_server_protocol::RequestId::Integer(2),
                    method: "initialize".to_string(),
                    params: Some(json!({
                        "clientInfo": {
                            "name": "remote-test-client",
                            "version": "0.1.0"
                        }
                    })),
                    trace: None,
                }),
            },
            client_id: ClientId("client-2".to_string()),
            stream_id: None,
            seq_id: Some(0),
            cursor: None,
        },
    )
    .await;

    match timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .expect("reconnected initialize should arrive in time")
        .expect("reconnected initialize should exist")
    {
        TransportEvent::ConnectionOpened { .. } => {}
        other => panic!("expected connection open after reconnect, got {other:?}"),
    }

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_start_allows_remote_control_invalid_url_when_disabled() {
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        "https://internal.example.com/backend-api/".to_string(),
        /*state_db*/ None,
        remote_control_auth_manager(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ false,
    )
    .await
    .expect("disabled remote control should not validate the URL at startup");

    shutdown_token.cancel();
    timeout(Duration::from_secs(1), remote_task)
        .await
        .expect("remote control task should stop")
        .expect("remote control task should join");
}

#[tokio::test]
async fn remote_control_start_allows_missing_auth_when_enabled() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
    );
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        /*state_db*/ None,
        auth_manager,
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start before ChatGPT auth is available");

    timeout(Duration::from_millis(100), listener.accept())
        .await
        .expect_err("remote control should wait for auth before connecting");

    shutdown_token.cancel();
    timeout(Duration::from_secs(1), remote_task)
        .await
        .expect("remote control task should stop")
        .expect("remote control task should join");
}

#[tokio::test]
async fn remote_control_handle_set_enabled_stops_and_restarts_connections() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, remote_handle) = start_remote_control(
        remote_control_url,
        Some(remote_control_state_runtime(&codex_home).await),
        remote_control_auth_manager(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");

    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_json(
        enroll_request.stream,
        json!({ "server_id": "srv_e_test", "environment_id": "env_test" }),
    )
    .await;
    let mut first_websocket = accept_remote_control_connection(&listener).await;

    remote_handle.set_enabled(/*enabled*/ false);
    timeout(Duration::from_secs(1), first_websocket.next())
        .await
        .expect("disabling remote control should close the websocket");
    timeout(Duration::from_millis(100), listener.accept())
        .await
        .expect_err("disabled remote control should not reconnect");

    remote_handle.set_enabled(/*enabled*/ true);
    let mut second_websocket = accept_remote_control_connection(&listener).await;
    second_websocket
        .close(None)
        .await
        .expect("second websocket should close");

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_transport_clears_outgoing_buffer_when_backend_acks() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, mut transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        Some(remote_control_state_runtime(&codex_home).await),
        remote_control_auth_manager(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");

    let enroll_request = accept_http_request(&listener).await;
    respond_with_json(
        enroll_request.stream,
        json!({ "server_id": "srv_e_test", "environment_id": "env_test" }),
    )
    .await;
    let mut first_websocket = accept_remote_control_connection(&listener).await;

    let client_id = ClientId("client-1".to_string());
    let initialize_message = JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
        id: codex_app_server_protocol::RequestId::Integer(1),
        method: "initialize".to_string(),
        params: Some(json!({
            "clientInfo": {
                "name": "remote-test-client",
                "version": "0.1.0"
            }
        })),
        trace: None,
    });
    send_client_event(
        &mut first_websocket,
        ClientEnvelope {
            event: ClientEvent::ClientMessage {
                message: initialize_message,
            },
            client_id: client_id.clone(),
            stream_id: None,
            seq_id: Some(0),
            cursor: None,
        },
    )
    .await;

    let writer = match timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .expect("connection open should arrive in time")
        .expect("connection open should exist")
    {
        TransportEvent::ConnectionOpened { writer, .. } => writer,
        other => panic!("expected connection open event, got {other:?}"),
    };
    match timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .expect("initialize message should arrive in time")
        .expect("initialize message should exist")
    {
        TransportEvent::IncomingMessage { .. } => {}
        other => panic!("expected initialize incoming message, got {other:?}"),
    }

    writer
        .send(QueuedOutgoingMessage::new(
            OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                ConfigWarningNotification {
                    summary: "stale".to_string(),
                    details: None,
                    path: None,
                    range: None,
                },
            )),
        ))
        .await
        .expect("remote writer should accept outgoing message");
    let (server_event, stream_id) = read_server_event_with_stream_id(&mut first_websocket).await;
    assert_eq!(
        server_event,
        json!({
            "type": "server_message",
            "client_id": "client-1",
            "seq_id": 1,
            "message": {
                "method": "configWarning",
                "params": {
                    "summary": "stale",
                    "details": null,
                }
            }
        })
    );

    send_client_event(
        &mut first_websocket,
        ClientEnvelope {
            event: ClientEvent::Ack,
            client_id: client_id.clone(),
            stream_id: Some(stream_id),
            seq_id: Some(1),
            cursor: None,
        },
    )
    .await;

    send_client_event(
        &mut first_websocket,
        ClientEnvelope {
            event: ClientEvent::ClientClosed,
            client_id: client_id.clone(),
            stream_id: None,
            seq_id: None,
            cursor: None,
        },
    )
    .await;
    match timeout(Duration::from_secs(5), transport_event_rx.recv())
        .await
        .expect("connection close should arrive in time")
        .expect("connection close should exist")
    {
        TransportEvent::ConnectionClosed { .. } => {}
        other => panic!("expected connection close event, got {other:?}"),
    }

    first_websocket
        .close(None)
        .await
        .expect("first websocket should close");
    drop(first_websocket);

    let mut second_websocket = accept_remote_control_connection(&listener).await;
    send_client_event(
        &mut second_websocket,
        ClientEnvelope {
            event: ClientEvent::Ping,
            client_id,
            stream_id: None,
            seq_id: None,
            cursor: None,
        },
    )
    .await;
    assert_eq!(
        read_server_event(&mut second_websocket).await,
        json!({
            "type": "pong",
            "client_id": "client-1",
            "seq_id": 1,
            "status": "unknown",
        })
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_http_mode_enrolls_before_connecting() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let (transport_event_tx, mut transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let expected_server_name = gethostname().to_string_lossy().trim().to_string();
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        Some(remote_control_state_runtime(&codex_home).await),
        remote_control_auth_manager(),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");

    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    assert_eq!(
        enroll_request.headers.get("authorization"),
        Some(&"Bearer Access Token".to_string())
    );
    assert_eq!(
        enroll_request.headers.get(REMOTE_CONTROL_ACCOUNT_ID_HEADER),
        Some(&"account_id".to_string())
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&enroll_request.body)
            .expect("enroll body should deserialize"),
        json!({
            "name": expected_server_name,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "app_server_version": env!("CARGO_PKG_VERSION"),
        })
    );
    respond_with_json(
        enroll_request.stream,
        json!({ "server_id": "srv_e_test", "environment_id": "env_test" }),
    )
    .await;

    let (handshake_request, mut websocket) =
        accept_remote_control_backend_connection(&listener).await;
    assert_eq!(
        handshake_request.path,
        "/backend-api/wham/remote/control/server"
    );
    assert_eq!(
        handshake_request.headers.get("authorization"),
        Some(&"Bearer Access Token".to_string())
    );
    assert_eq!(
        handshake_request
            .headers
            .get(REMOTE_CONTROL_ACCOUNT_ID_HEADER),
        Some(&"account_id".to_string())
    );
    assert_eq!(
        handshake_request.headers.get("x-codex-server-id"),
        Some(&"srv_e_test".to_string())
    );
    assert_eq!(
        handshake_request.headers.get("x-codex-name"),
        Some(&base64::engine::general_purpose::STANDARD.encode(&expected_server_name))
    );
    assert_eq!(
        handshake_request.headers.get("x-codex-protocol-version"),
        Some(&REMOTE_CONTROL_PROTOCOL_VERSION.to_string())
    );

    let backend_client_id = ClientId("backend-test-client".to_string());
    let writer = {
        let initialize_message =
            JSONRPCMessage::Request(codex_app_server_protocol::JSONRPCRequest {
                id: codex_app_server_protocol::RequestId::Integer(11),
                method: "initialize".to_string(),
                params: Some(json!({
                    "clientInfo": {
                        "name": "remote-backend-client",
                        "version": "0.1.0"
                    }
                })),
                trace: None,
            });
        send_client_event(
            &mut websocket,
            ClientEnvelope {
                event: ClientEvent::ClientMessage {
                    message: initialize_message.clone(),
                },
                client_id: backend_client_id.clone(),
                stream_id: None,
                seq_id: Some(0),
                cursor: None,
            },
        )
        .await;

        let (connection_id, writer) =
            match timeout(Duration::from_secs(5), transport_event_rx.recv())
                .await
                .expect("connection open should arrive in time")
                .expect("connection open should exist")
            {
                TransportEvent::ConnectionOpened {
                    connection_id,
                    writer,
                    ..
                } => (connection_id, writer),
                other => panic!("expected connection open event, got {other:?}"),
            };

        match timeout(Duration::from_secs(5), transport_event_rx.recv())
            .await
            .expect("initialize message should arrive in time")
            .expect("initialize message should exist")
        {
            TransportEvent::IncomingMessage {
                connection_id: incoming_connection_id,
                message,
            } => {
                assert_eq!(incoming_connection_id, connection_id);
                assert_eq!(message, initialize_message);
            }
            other => panic!("expected initialize incoming message, got {other:?}"),
        }
        writer
    };

    writer
        .send(QueuedOutgoingMessage::new(OutgoingMessage::Response(
            crate::outgoing_message::OutgoingResponse {
                id: codex_app_server_protocol::RequestId::Integer(11),
                result: json!({
                    "userAgent": "codex-test-agent"
                }),
            },
        )))
        .await
        .expect("remote writer should accept initialize response");
    assert_eq!(
        read_server_event(&mut websocket).await,
        json!({
            "type": "server_message",
            "client_id": backend_client_id.0.clone(),
            "seq_id": 1,
            "message": {
                "id": 11,
                "result": {
                    "userAgent": "codex-test-agent",
                }
            }
        })
    );

    writer
        .send(QueuedOutgoingMessage::new(
            OutgoingMessage::AppServerNotification(ServerNotification::ConfigWarning(
                ConfigWarningNotification {
                    summary: "backend".to_string(),
                    details: None,
                    path: None,
                    range: None,
                },
            )),
        ))
        .await
        .expect("remote writer should accept outgoing message");
    assert_eq!(
        read_server_event(&mut websocket).await,
        json!({
            "type": "server_message",
            "client_id": backend_client_id.0.clone(),
            "seq_id": 2,
            "message": {
                "method": "configWarning",
                "params": {
                    "summary": "backend",
                    "details": null,
                }
            }
        })
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_http_mode_reuses_persisted_enrollment_before_reenrolling() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let persisted_enrollment = RemoteControlEnrollment {
        account_id: "account_id".to_string(),
        environment_id: "env_persisted".to_string(),
        server_id: "srv_e_persisted".to_string(),
        server_name: "persisted-server".to_string(),
    };
    update_persisted_remote_control_enrollment(
        Some(state_db.as_ref()),
        &remote_control_target,
        "account_id",
        /*app_server_client_name*/ None,
        Some(&persisted_enrollment),
    )
    .await
    .expect("persisted enrollment should save");

    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        Some(state_db.clone()),
        remote_control_auth_manager_with_home(&codex_home),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");

    let (handshake_request, _websocket) = accept_remote_control_backend_connection(&listener).await;
    assert_eq!(
        handshake_request.path,
        "/backend-api/wham/remote/control/server"
    );
    assert_eq!(
        handshake_request.headers.get("x-codex-server-id"),
        Some(&persisted_enrollment.server_id)
    );
    assert_eq!(
        load_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &remote_control_target,
            "account_id",
            /*app_server_client_name*/ None,
        )
        .await,
        Some(persisted_enrollment)
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_stdio_mode_waits_for_client_name_before_connecting() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let app_server_client_name = "stdio-client";
    let persisted_enrollment = RemoteControlEnrollment {
        account_id: "account_id".to_string(),
        environment_id: "env_persisted".to_string(),
        server_id: "srv_e_persisted".to_string(),
        server_name: "persisted-server".to_string(),
    };
    update_persisted_remote_control_enrollment(
        Some(state_db.as_ref()),
        &remote_control_target,
        "account_id",
        Some(app_server_client_name),
        Some(&persisted_enrollment),
    )
    .await
    .expect("persisted enrollment should save");

    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let (app_server_client_name_tx, app_server_client_name_rx) = oneshot::channel::<String>();
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        Some(state_db.clone()),
        remote_control_auth_manager_with_home(&codex_home),
        transport_event_tx,
        shutdown_token.clone(),
        Some(app_server_client_name_rx),
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");

    timeout(Duration::from_millis(100), listener.accept())
        .await
        .expect_err("remote control should wait for the stdio client name");

    let _ = app_server_client_name_tx.send(app_server_client_name.to_string());
    let (handshake_request, _websocket) = accept_remote_control_backend_connection(&listener).await;
    assert_eq!(
        handshake_request.headers.get("x-codex-server-id"),
        Some(&persisted_enrollment.server_id)
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_waits_for_account_id_before_enrolling() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    save_auth(
        codex_home.path(),
        &remote_control_auth_dot_json(/*account_id*/ None),
        AuthCredentialsStoreMode::File,
    )
    .expect("auth without account id should save");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let auth_manager = AuthManager::shared(
        codex_home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
    );
    let expected_server_name = gethostname().to_string_lossy().trim().to_string();
    let expected_enrollment = RemoteControlEnrollment {
        account_id: "account_id".to_string(),
        environment_id: "env_ready".to_string(),
        server_id: "srv_e_ready".to_string(),
        server_name: expected_server_name,
    };

    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        Some(state_db.clone()),
        auth_manager,
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start before account id is available");

    timeout(Duration::from_millis(100), listener.accept())
        .await
        .expect_err("remote control should wait for account id before enrolling");

    save_auth(
        codex_home.path(),
        &remote_control_auth_dot_json(Some("account_id")),
        AuthCredentialsStoreMode::File,
    )
    .expect("auth with account id should save");

    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_json(
        enroll_request.stream,
        json!({
            "server_id": expected_enrollment.server_id,
            "environment_id": expected_enrollment.environment_id,
        }),
    )
    .await;

    let (handshake_request, _websocket) = accept_remote_control_backend_connection(&listener).await;
    assert_eq!(
        handshake_request.headers.get("x-codex-server-id"),
        Some(&expected_enrollment.server_id)
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[tokio::test]
async fn remote_control_http_mode_clears_stale_persisted_enrollment_after_404() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let remote_control_url = remote_control_url_for_listener(&listener);
    let codex_home = TempDir::new().expect("temp dir should create");
    let state_db = remote_control_state_runtime(&codex_home).await;
    let remote_control_target =
        normalize_remote_control_url(&remote_control_url).expect("target should parse");
    let expected_server_name = gethostname().to_string_lossy().trim().to_string();
    let stale_enrollment = RemoteControlEnrollment {
        account_id: "account_id".to_string(),
        environment_id: "env_stale".to_string(),
        server_id: "srv_e_stale".to_string(),
        server_name: "stale-server".to_string(),
    };
    let refreshed_enrollment = RemoteControlEnrollment {
        account_id: "account_id".to_string(),
        environment_id: "env_refreshed".to_string(),
        server_id: "srv_e_refreshed".to_string(),
        server_name: expected_server_name,
    };
    update_persisted_remote_control_enrollment(
        Some(state_db.as_ref()),
        &remote_control_target,
        "account_id",
        /*app_server_client_name*/ None,
        Some(&stale_enrollment),
    )
    .await
    .expect("stale enrollment should save");

    let (transport_event_tx, _transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let shutdown_token = CancellationToken::new();
    let (remote_task, _remote_handle) = start_remote_control(
        remote_control_url,
        Some(state_db.clone()),
        remote_control_auth_manager_with_home(&codex_home),
        transport_event_tx,
        shutdown_token.clone(),
        /*app_server_client_name_rx*/ None,
        /*initial_enabled*/ true,
    )
    .await
    .expect("remote control should start");

    let websocket_request = accept_http_request(&listener).await;
    assert_eq!(
        websocket_request.request_line,
        "GET /backend-api/wham/remote/control/server HTTP/1.1"
    );
    assert_eq!(
        websocket_request.headers.get("x-codex-server-id"),
        Some(&stale_enrollment.server_id)
    );
    respond_with_status(websocket_request.stream, "404 Not Found", "").await;

    let enroll_request = accept_http_request(&listener).await;
    assert_eq!(
        enroll_request.request_line,
        "POST /backend-api/wham/remote/control/server/enroll HTTP/1.1"
    );
    respond_with_json(
        enroll_request.stream,
        json!({
            "server_id": refreshed_enrollment.server_id,
            "environment_id": refreshed_enrollment.environment_id,
        }),
    )
    .await;

    let (handshake_request, _websocket) = accept_remote_control_backend_connection(&listener).await;
    assert_eq!(
        handshake_request.headers.get("x-codex-server-id"),
        Some(&refreshed_enrollment.server_id)
    );
    assert_eq!(
        load_persisted_remote_control_enrollment(
            Some(state_db.as_ref()),
            &remote_control_target,
            "account_id",
            /*app_server_client_name*/ None,
        )
        .await,
        Some(refreshed_enrollment)
    );

    shutdown_token.cancel();
    let _ = remote_task.await;
}

#[derive(Debug)]
struct CapturedHttpRequest {
    stream: TcpStream,
    request_line: String,
    headers: BTreeMap<String, String>,
    body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CapturedWebSocketRequest {
    path: String,
    headers: BTreeMap<String, String>,
}

async fn accept_remote_control_connection(listener: &TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("remote control should connect in time")
        .expect("listener accept should succeed");
    accept_async(stream)
        .await
        .expect("websocket handshake should succeed")
}

async fn accept_http_request(listener: &TcpListener) -> CapturedHttpRequest {
    let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("HTTP request should arrive in time")
        .expect("listener accept should succeed");
    let mut reader = BufReader::new(stream);

    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .await
        .expect("request line should read");
    let request_line = request_line.trim_end_matches("\r\n").to_string();

    let mut headers = BTreeMap::new();
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .expect("header line should read");
        if line == "\r\n" {
            break;
        }
        let line = line.trim_end_matches("\r\n");
        let (name, value) = line.split_once(':').expect("header should contain colon");
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .await
        .expect("request body should read");

    CapturedHttpRequest {
        stream: reader.into_inner(),
        request_line,
        headers,
        body: String::from_utf8(body).expect("body should be utf-8"),
    }
}

async fn respond_with_json(mut stream: TcpStream, body: serde_json::Value) {
    let body = body.to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("response should write");
    stream.flush().await.expect("response should flush");
}

async fn respond_with_status(stream: TcpStream, status: &str, body: &str) {
    respond_with_status_and_headers(stream, status, &[], body).await;
}

async fn respond_with_status_and_headers(
    mut stream: TcpStream,
    status: &str,
    headers: &[(&str, &str)],
    body: &str,
) {
    let extra_headers = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect::<String>();
    let response = format!(
        "HTTP/1.1 {status}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n{extra_headers}\r\n{body}",
        body.len(),
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("response should write");
    stream.flush().await.expect("response should flush");
}

async fn accept_remote_control_backend_connection(
    listener: &TcpListener,
) -> (CapturedWebSocketRequest, WebSocketStream<TcpStream>) {
    let (stream, _) = timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("websocket request should arrive in time")
        .expect("listener accept should succeed");
    let captured_request = Arc::new(std::sync::Mutex::new(None::<CapturedWebSocketRequest>));
    let captured_request_for_callback = captured_request.clone();
    let websocket = accept_hdr_async(
        stream,
        move |request: &tungstenite::handshake::server::Request,
              response: tungstenite::handshake::server::Response| {
            let headers = request
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_str().to_ascii_lowercase(),
                        value
                            .to_str()
                            .expect("header should be valid utf-8")
                            .to_string(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            *captured_request_for_callback
                .lock()
                .expect("capture lock should acquire") = Some(CapturedWebSocketRequest {
                path: request.uri().path().to_string(),
                headers,
            });
            Ok(response)
        },
    )
    .await
    .expect("websocket handshake should succeed");
    let captured_request = captured_request
        .lock()
        .expect("capture lock should acquire")
        .clone()
        .expect("websocket request should be captured");
    (captured_request, websocket)
}

async fn send_client_event(
    websocket: &mut WebSocketStream<TcpStream>,
    client_envelope: ClientEnvelope,
) {
    let payload = serde_json::to_string(&client_envelope).expect("client event should serialize");
    websocket
        .send(tungstenite::Message::Text(payload.into()))
        .await
        .expect("client event should send");
}

async fn read_server_event(websocket: &mut WebSocketStream<TcpStream>) -> serde_json::Value {
    read_server_event_with_stream_id(websocket).await.0
}

async fn read_server_event_with_stream_id(
    websocket: &mut WebSocketStream<TcpStream>,
) -> (serde_json::Value, StreamId) {
    loop {
        let frame = timeout(Duration::from_secs(5), websocket.next())
            .await
            .expect("server event should arrive in time")
            .expect("websocket should stay open")
            .expect("websocket frame should be readable");
        match frame {
            tungstenite::Message::Text(text) => {
                let mut event: serde_json::Value =
                    serde_json::from_str(text.as_ref()).expect("server event should deserialize");
                let stream_id = event
                    .as_object_mut()
                    .and_then(|event| event.remove("stream_id"))
                    .expect("stream_id should be present");
                let stream_id = stream_id
                    .as_str()
                    .expect("stream_id should be a string")
                    .to_string();
                return (event, StreamId(stream_id));
            }
            tungstenite::Message::Ping(payload) => {
                websocket
                    .send(tungstenite::Message::Pong(payload))
                    .await
                    .expect("websocket pong should send");
            }
            tungstenite::Message::Pong(_) => {}
            tungstenite::Message::Close(frame) => {
                panic!("unexpected websocket close frame: {frame:?}");
            }
            tungstenite::Message::Binary(_) => {
                panic!("unexpected binary websocket frame");
            }
            tungstenite::Message::Frame(_) => {}
        }
    }
}
