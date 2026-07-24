use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_protocol::CodeModeSessionProvider;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use codex_code_mode_protocol::RuntimeResponse;
use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::HostHello;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::HostResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::WireCellId;
use codex_code_mode_protocol::host::WireContentItem;
use codex_code_mode_protocol::host::WireResult;
use codex_code_mode_protocol::host::WireRuntimeResponse;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::ProcessOwnedCodeModeSession;
use super::ProcessOwnedCodeModeSessionProvider;
use super::WebSocketCodeModeSessionProvider;
use super::resolve_host_program;
use crate::NoopCodeModeSessionDelegate;

#[test]
fn provider_reuses_its_live_process_host() {
    let provider = ProcessOwnedCodeModeSessionProvider::default();

    let first = provider.process_host().expect("owned process host");
    let second = provider.process_host().expect("owned process host");

    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn host_program_override_takes_precedence() {
    assert_eq!(
        resolve_host_program(
            Some("custom-code-mode-host".into()),
            Ok(PathBuf::from("/opt/codex/bin/codex")),
        ),
        PathBuf::from("custom-code-mode-host")
    );
}

#[test]
fn host_program_is_next_to_the_main_executable_even_when_missing() {
    let executable_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };

    assert_eq!(
        resolve_host_program(
            /*override_path*/ None,
            Ok(PathBuf::from("/opt/codex/bin/codex")),
        ),
        PathBuf::from("/opt/codex/bin").join(executable_name)
    );
}

#[test]
fn host_program_falls_back_to_its_name_when_main_executable_is_unknown() {
    let executable_name = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };

    assert_eq!(
        resolve_host_program(
            /*override_path*/ None,
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "missing executable"
            )),
        ),
        PathBuf::from(executable_name)
    );
}

#[tokio::test]
async fn provider_falls_back_to_in_process_session_when_host_is_missing() {
    let provider = ProcessOwnedCodeModeSessionProvider::with_host_program(
        "codex-code-mode-host-does-not-exist".into(),
    );

    let session = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .expect("missing host should fall back to an in-process session");
    let response = session
        .execute(ExecuteRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: "text('fallback')".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
        .await
        .expect("execute fallback session")
        .initial_response()
        .await
        .expect("read fallback response");

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: codex_code_mode_protocol::CellId::new("1".to_string()),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "fallback".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn websocket_provider_executes_over_shared_connector() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("websocket test listener should bind");
    let websocket_url = format!(
        "ws://{}",
        listener
            .local_addr()
            .expect("websocket test listener should have an address")
    );
    let server = tokio::spawn(async move {
        let (stream, _) = listener
            .accept()
            .await
            .expect("websocket test host should accept a connection");
        let mut websocket = accept_async(stream)
            .await
            .expect("websocket test host should complete the HTTP handshake");

        while let Some(message) = websocket.next().await {
            let message = message.expect("websocket test host should receive a valid message");
            let frame = match message {
                Message::Binary(frame) => frame,
                Message::Ping(_) | Message::Pong(_) => continue,
                Message::Close(_) => break,
                Message::Text(_) | Message::Frame(_) => {
                    panic!("websocket test host received an unexpected message: {message:?}");
                }
            };
            let request = EncodedFrame::decode_framed::<ClientToHost>(&frame)
                .expect("websocket test host should decode a framed protocol message");
            let responses = match request {
                ClientToHost::ClientHello(_) => vec![HostToClient::HostHello(HostHello::new(
                    ProtocolVersion::V1,
                    CapabilitySet::empty(),
                ))],
                ClientToHost::Request {
                    id,
                    request: HostRequest::OpenSession { session_id },
                } => vec![HostToClient::Response {
                    id,
                    result: WireResult::Ok {
                        value: HostResponse::SessionReady { session_id },
                    },
                }],
                ClientToHost::Request {
                    id,
                    request: HostRequest::Execute { request, .. },
                } => {
                    assert_eq!(request.source, "text('shared connector')");
                    let cell_id = WireCellId::new("1");
                    vec![
                        HostToClient::Response {
                            id,
                            result: WireResult::Ok {
                                value: HostResponse::ExecutionStarted {
                                    cell_id: cell_id.clone(),
                                },
                            },
                        },
                        HostToClient::InitialResponse {
                            id,
                            result: WireResult::Ok {
                                value: WireRuntimeResponse::Result {
                                    cell_id,
                                    content_items: vec![WireContentItem::InputText {
                                        text: "shared connector".to_string(),
                                    }],
                                    error_text: None,
                                },
                            },
                        },
                    ]
                }
                ClientToHost::Request {
                    id,
                    request: HostRequest::ShutdownSession { session_id },
                } => vec![HostToClient::Response {
                    id,
                    result: WireResult::Ok {
                        value: HostResponse::SessionClosed { session_id },
                    },
                }],
                request => {
                    panic!("websocket test host received an unexpected request: {request:?}")
                }
            };

            for response in responses {
                let frame = EncodedFrame::encode(&response)
                    .expect("websocket test host should encode a framed response");
                websocket
                    .send(Message::Binary(frame.into_framed_bytes().into()))
                    .await
                    .expect("websocket test host should send its response");
            }
        }
    });

    let provider = WebSocketCodeModeSessionProvider::with_http_client_factory(
        websocket_url,
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
    );
    let session = provider
        .create_session(Arc::new(NoopCodeModeSessionDelegate))
        .await
        .expect("shared websocket connector should open a code-mode session");
    let response = session
        .execute(ExecuteRequest {
            tool_call_id: "shared-websocket".to_string(),
            enabled_tools: Vec::new(),
            source: "text('shared connector')".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
        .await
        .expect("shared websocket connector should start a cell")
        .initial_response()
        .await
        .expect("shared websocket connector should return a cell result");

    assert_eq!(
        response,
        RuntimeResponse::Result {
            cell_id: codex_code_mode_protocol::CellId::new("1".to_string()),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "shared connector".to_string(),
            }],
            error_text: None,
        }
    );
    session
        .shutdown()
        .await
        .expect("shared websocket connector should shut down its session");
    drop(session);
    drop(provider);
    timeout(Duration::from_secs(5), server)
        .await
        .expect("websocket test host should disconnect promptly")
        .expect("websocket test host task should succeed");
}

#[tokio::test]
async fn shutdown_before_open_does_not_spawn_the_host() {
    let session = ProcessOwnedCodeModeSession::new();

    session.shutdown().await.expect("shutdown session");
    let error = session
        .execute(codex_code_mode_protocol::ExecuteRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: "text('unreachable')".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        })
        .await
        .err()
        .expect("shutdown session should reject execution");

    assert_eq!(error, "code mode session is shutting down");
}
