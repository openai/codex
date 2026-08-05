use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::host::Capability;
use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientHello;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::DUAL_WEBSOCKET_CAPABILITY;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::FramedWriter;
use codex_code_mode_protocol::host::HandshakeRejectReason;
use codex_code_mode_protocol::host::HostHello;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::HostResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SESSION_RESOURCE_LIMITS_CAPABILITY;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::SupportedProtocolVersions;
use codex_code_mode_protocol::host::WireExecuteRequest;
use codex_code_mode_protocol::host::WireResult;
use pretty_assertions::assert_eq;
use tokio::io::AsyncWrite;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use uuid::Uuid;

use super::HostState;
use super::MAX_ACTIVE_CELLS;
use super::MAX_IN_FLIGHT_REQUESTS;
use super::MAX_RECENT_REQUEST_IDS;
use super::RequestKind;
use super::RequestRegistry;
use super::SeenSessionIds;
use super::negotiate;
use super::peer::HostPeer;
use super::run;
use super::transport::BulkConnectionRegistry;
use super::transport::ConnectionReader;
use super::transport::ConnectionWriter;

fn client_hello(
    versions: impl IntoIterator<Item = ProtocolVersion>,
    required_capabilities: CapabilitySet,
) -> ClientToHost {
    ClientToHost::ClientHello(
        ClientHello::new(
            SupportedProtocolVersions::try_new(versions).expect("supported versions"),
            required_capabilities,
            CapabilitySet::empty(),
        )
        .expect("client hello"),
    )
}

fn session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("session ID")
}

fn request_id(value: i64) -> RequestId {
    RequestId::new(value)
}

async fn decode_frame(frame: EncodedFrame) -> HostToClient {
    let (reader, writer) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let writer = tokio::spawn(async move {
        FramedWriter::new(writer)
            .write_frame(&frame)
            .await
            .expect("write encoded frame");
    });
    let message = FramedReader::new(reader)
        .read()
        .await
        .expect("read encoded frame")
        .expect("encoded frame message");
    writer.await.expect("frame writer task");
    message
}

fn execute_request(source: &str) -> WireExecuteRequest {
    WireExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
        yield_time_ms: Some(60_000),
        max_output_tokens: Some(1_000),
    }
}

#[tokio::test]
async fn handshake_and_multiple_session_lifecycles_are_ordered() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);

    writer
        .write(&client_hello([ProtocolVersion::V1], CapabilitySet::empty()))
        .await
        .expect("write hello");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("read hello"),
        Some(HostToClient::HostHello(HostHello::new(
            ProtocolVersion::V1,
            CapabilitySet::empty(),
        )))
    );

    for (request_id, id) in [
        (request_id(/*value*/ 1), "session-1"),
        (request_id(/*value*/ 2), "session-2"),
    ] {
        writer
            .write(&ClientToHost::Request {
                id: request_id,
                request: HostRequest::OpenSession {
                    session_id: session_id(id),
                    cell_execution_limits: None,
                },
            })
            .await
            .expect("open session");
        assert_eq!(
            reader.read::<HostToClient>().await.expect("session ready"),
            Some(HostToClient::Response {
                id: request_id,
                result: WireResult::Ok {
                    value: HostResponse::SessionReady {
                        session_id: session_id(id),
                    },
                },
            })
        );
    }

    for (request_id, id) in [
        (request_id(/*value*/ 3), "session-1"),
        (request_id(/*value*/ 4), "session-2"),
    ] {
        writer
            .write(&ClientToHost::Request {
                id: request_id,
                request: HostRequest::ShutdownSession {
                    session_id: session_id(id),
                },
            })
            .await
            .expect("shutdown session");
        assert_eq!(
            reader.read::<HostToClient>().await.expect("session closed"),
            Some(HostToClient::Response {
                id: request_id,
                result: WireResult::Ok {
                    value: HostResponse::SessionClosed {
                        session_id: session_id(id),
                    },
                },
            })
        );
    }

    drop(writer);
    drop(reader);
    host.await.expect("host task").expect("host connection");
}

#[tokio::test]
async fn disconnect_cancels_a_backpressured_host_writer() {
    let (host_reader, client_writer) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let (blocked_tx, blocked_rx) = oneshot::channel();
    let host = tokio::spawn(run(
        host_reader,
        BlockingWriter {
            blocked_tx: Some(blocked_tx),
            handshake_flushed: false,
        },
    ));
    let mut writer = FramedWriter::new(client_writer);

    writer
        .write(&client_hello([ProtocolVersion::V1], CapabilitySet::empty()))
        .await
        .expect("write client hello");
    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 1),
            request: HostRequest::OpenSession {
                session_id: session_id("backpressured-session"),
                cell_execution_limits: None,
            },
        })
        .await
        .expect("write session-open request");

    tokio::time::timeout(Duration::from_secs(1), blocked_rx)
        .await
        .expect("host writer should reach backpressure")
        .expect("host writer should report backpressure");

    writer
        .write(&client_hello([ProtocolVersion::V1], CapabilitySet::empty()))
        .await
        .expect("write invalid second client hello");

    let error = tokio::time::timeout(Duration::from_secs(1), host)
        .await
        .expect("disconnect should cancel the backpressured writer")
        .expect("host task should finish")
        .expect_err("a second client hello should fail the connection");
    assert_eq!(
        error.to_string(),
        "received a second code-mode client hello"
    );
}

struct BlockingWriter {
    blocked_tx: Option<oneshot::Sender<()>>,
    handshake_flushed: bool,
}

impl AsyncWrite for BlockingWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.handshake_flushed {
            if let Some(blocked_tx) = self.blocked_tx.take() {
                let _ = blocked_tx.send(());
            }
            Poll::Pending
        } else {
            Poll::Ready(Ok(bytes.len()))
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.handshake_flushed = true;
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn failed_host_hello_removes_the_bulk_pairing_reservation() {
    let (host_reader, client_writer) = tokio::io::duplex(/*max_buf_size*/ 4096);
    let capability = Capability::new(DUAL_WEBSOCKET_CAPABILITY).expect("dual websocket capability");
    let hello = ClientToHost::ClientHello(
        ClientHello::new(
            SupportedProtocolVersions::try_new([ProtocolVersion::V1]).expect("supported versions"),
            CapabilitySet::empty(),
            CapabilitySet::try_new([capability]).expect("optional capabilities"),
        )
        .expect("client hello"),
    );
    FramedWriter::new(client_writer)
        .write(&hello)
        .await
        .expect("write client hello");

    let bytes = Arc::new(Mutex::new(Vec::new()));
    let registry = BulkConnectionRegistry::default();
    let mut reader = ConnectionReader::from_reader(host_reader);
    let mut writer = ConnectionWriter::from_writer(FailingHandshakeWriter {
        bytes: Arc::clone(&bytes),
    });
    let result = negotiate(&mut reader, &mut writer, Some(&registry)).await;
    assert!(result.is_err());

    let message = EncodedFrame::decode_framed::<HostToClient>(
        &bytes.lock().unwrap_or_else(PoisonError::into_inner),
    )
    .expect("decode failed host hello");
    let HostToClient::HostHello(hello) = message else {
        panic!("expected a host hello");
    };
    let token = hello
        .bulk_connection_token()
        .expect("dual websocket pairing token");
    let token = Uuid::parse_str(token).expect("UUID pairing token");
    assert!(registry.remove(token).is_none());
}

struct FailingHandshakeWriter {
    bytes: Arc<Mutex<Vec<u8>>>,
}

impl AsyncWrite for FailingHandshakeWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.bytes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
        Poll::Ready(Ok(bytes.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Err(std::io::Error::other("host hello write failed")))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn optional_dual_websocket_capability_falls_back_to_a_single_connection() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    let capability = Capability::new(DUAL_WEBSOCKET_CAPABILITY).expect("dual websocket capability");

    writer
        .write(&ClientToHost::ClientHello(
            ClientHello::new(
                SupportedProtocolVersions::try_new([ProtocolVersion::V1])
                    .expect("supported versions"),
                CapabilitySet::empty(),
                CapabilitySet::try_new([capability]).expect("optional capabilities"),
            )
            .expect("client hello"),
        ))
        .await
        .expect("write hello");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("host hello"),
        Some(HostToClient::HostHello(HostHello::new(
            ProtocolVersion::V1,
            CapabilitySet::empty(),
        )))
    );

    drop(writer);
    drop(reader);
    host.await.expect("host task").expect("host connection");
}

#[tokio::test]
async fn session_resource_limits_are_negotiated_when_optional_or_required() {
    let dual_capability =
        Capability::new(DUAL_WEBSOCKET_CAPABILITY).expect("dual websocket capability");
    let resource_limits_capability =
        Capability::new(SESSION_RESOURCE_LIMITS_CAPABILITY).expect("resource limits capability");
    let resource_limits =
        CapabilitySet::try_new([resource_limits_capability.clone()]).expect("host capabilities");

    for (required, optional) in [
        (
            CapabilitySet::empty(),
            CapabilitySet::try_new([dual_capability, resource_limits_capability])
                .expect("optional capabilities"),
        ),
        (resource_limits.clone(), CapabilitySet::empty()),
    ] {
        let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
        let (host_reader, host_writer) = tokio::io::split(host_stream);
        let (client_reader, client_writer) = tokio::io::split(client_stream);
        let host = tokio::spawn(run(host_reader, host_writer));
        let mut reader = FramedReader::new(client_reader);
        let mut writer = FramedWriter::new(client_writer);

        writer
            .write(&ClientToHost::ClientHello(
                ClientHello::new(
                    SupportedProtocolVersions::try_new([ProtocolVersion::V1])
                        .expect("supported versions"),
                    required,
                    optional,
                )
                .expect("client hello"),
            ))
            .await
            .expect("write hello");
        assert_eq!(
            reader.read::<HostToClient>().await.expect("host hello"),
            Some(HostToClient::HostHello(HostHello::new(
                ProtocolVersion::V1,
                resource_limits.clone(),
            )))
        );

        drop(writer);
        drop(reader);
        host.await.expect("host task").expect("host connection");
    }
}

#[tokio::test]
async fn incompatible_or_invalid_handshake_is_rejected() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    let version_two = ProtocolVersion::new(/*value*/ 2).expect("protocol version");

    writer
        .write(&client_hello([version_two], CapabilitySet::empty()))
        .await
        .expect("write hello");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("rejection"),
        Some(HostToClient::HandshakeRejected {
            reason: HandshakeRejectReason::NoCompatibleVersion {
                supported_versions: SupportedProtocolVersions::try_new([ProtocolVersion::V1])
                    .expect("host versions"),
            },
        })
    );
    host.await.expect("host task").expect("host connection");

    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 1),
            request: HostRequest::OpenSession {
                session_id: session_id("session-1"),
                cell_execution_limits: None,
            },
        })
        .await
        .expect("write invalid first message");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("rejection"),
        Some(HostToClient::HandshakeRejected {
            reason: HandshakeRejectReason::InvalidHello {
                message: "first message must be connection/hello".to_string(),
            },
        })
    );
    host.await.expect("host task").expect("host connection");
}

#[tokio::test]
async fn unsupported_required_capability_is_rejected() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 1024);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    let capability = Capability::new("required").expect("capability");

    writer
        .write(&client_hello(
            [ProtocolVersion::V1],
            CapabilitySet::try_new([capability.clone()]).expect("capabilities"),
        ))
        .await
        .expect("write hello");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("rejection"),
        Some(HostToClient::HandshakeRejected {
            reason: HandshakeRejectReason::MissingRequiredCapability { capability },
        })
    );
    host.await.expect("host task").expect("host connection");
}

#[tokio::test]
async fn session_id_cannot_be_reused_after_shutdown() {
    let (host_stream, client_stream) = tokio::io::duplex(/*max_buf_size*/ 2048);
    let (host_reader, host_writer) = tokio::io::split(host_stream);
    let (client_reader, client_writer) = tokio::io::split(client_stream);
    let host = tokio::spawn(run(host_reader, host_writer));
    let mut reader = FramedReader::new(client_reader);
    let mut writer = FramedWriter::new(client_writer);
    writer
        .write(&client_hello([ProtocolVersion::V1], CapabilitySet::empty()))
        .await
        .expect("write hello");
    reader
        .read::<HostToClient>()
        .await
        .expect("read hello")
        .expect("host hello");

    let id = session_id("session-1");
    for (request_id, request) in [
        (
            request_id(/*value*/ 1),
            HostRequest::OpenSession {
                session_id: id.clone(),
                cell_execution_limits: None,
            },
        ),
        (
            request_id(/*value*/ 2),
            HostRequest::ShutdownSession {
                session_id: id.clone(),
            },
        ),
    ] {
        writer
            .write(&ClientToHost::Request {
                id: request_id,
                request,
            })
            .await
            .expect("session request");
        reader
            .read::<HostToClient>()
            .await
            .expect("session response")
            .expect("session response message");
    }
    writer
        .write(&ClientToHost::Request {
            id: request_id(/*value*/ 3),
            request: HostRequest::OpenSession {
                session_id: id,
                cell_execution_limits: None,
            },
        })
        .await
        .expect("reuse session ID");
    assert_eq!(
        reader.read::<HostToClient>().await.expect("reuse response"),
        Some(HostToClient::Response {
            id: request_id(/*value*/ 3),
            result: WireResult::Err {
                message: "code-mode session ID `session-1` was reused".to_string(),
            },
        })
    );
    drop(writer);
    drop(reader);
    host.await.expect("host task").expect("host connection");
}

#[test]
fn request_history_is_bounded() {
    let mut requests = RequestRegistry::default();
    let duplicate = request_id(/*value*/ -1);
    requests
        .start(duplicate, RequestKind::OpenSession)
        .expect("start duplicate probe");
    assert!(requests.start(duplicate, RequestKind::OpenSession).is_err());
    requests.finish(duplicate);
    for value in 1..=MAX_RECENT_REQUEST_IDS as i64 + 100 {
        let id = request_id(value);
        requests
            .start(id, RequestKind::Wait)
            .expect("start request");
        requests.cancel(id);
        requests.finish(id);
    }
    assert!(requests.active.is_empty());
    assert_eq!(requests.recent.len(), MAX_RECENT_REQUEST_IDS);
    assert_eq!(requests.recent_order.len(), MAX_RECENT_REQUEST_IDS);
}

#[tokio::test]
async fn request_task_panic_disconnects_host() {
    let (outgoing_tx, _outgoing_rx) = mpsc::channel(/*max_capacity*/ 1);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let state = HostState {
        sessions: Mutex::new(HashMap::new()),
        seen_session_ids: Mutex::new(SeenSessionIds::default()),
        requests: Mutex::new(RequestRegistry::default()),
        request_tasks: TaskTracker::new(),
        request_permits: Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS)),
        active_cell_permits: Arc::new(Semaphore::new(MAX_ACTIVE_CELLS)),
        closing: AtomicBool::new(false),
        peer: Arc::clone(&peer),
    };
    let task = state.request_tasks.spawn(async {
        panic!("request panic probe");
    });
    state.supervise_request_task(task);

    tokio::time::timeout(Duration::from_secs(1), peer.disconnected())
        .await
        .expect("request panic should disconnect host");
    assert!(
        peer.failure()
            .expect("request failure")
            .contains("request task failed")
    );
}

#[tokio::test]
async fn execute_request_id_remains_active_until_initial_response() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*max_capacity*/ 4);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let state = Arc::new(HostState {
        sessions: Mutex::new(HashMap::new()),
        seen_session_ids: Mutex::new(SeenSessionIds::default()),
        requests: Mutex::new(RequestRegistry::default()),
        request_tasks: TaskTracker::new(),
        request_permits: Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS)),
        active_cell_permits: Arc::new(Semaphore::new(MAX_ACTIVE_CELLS)),
        closing: AtomicBool::new(false),
        peer,
    });
    let session_id = session_id("session-1");
    state
        .open_session(
            session_id.clone(),
            CodeModeSessionCellExecutionLimits::default(),
        )
        .expect("open session");
    let request_id = request_id(/*value*/ 1);

    state
        .spawn_request(
            request_id,
            HostRequest::Execute {
                session_id: session_id.clone(),
                request: execute_request("await new Promise(() => {});"),
            },
        )
        .expect("spawn execute request");
    let started = decode_frame(outgoing_rx.recv().await.expect("execution started frame")).await;
    let HostToClient::Response {
        id,
        result:
            WireResult::Ok {
                value: HostResponse::ExecutionStarted { cell_id },
            },
    } = started
    else {
        panic!("expected execution started response");
    };
    assert_eq!(id, request_id);
    assert!(
        state
            .requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .active
            .contains_key(&request_id)
    );

    state
        .session(&session_id)
        .expect("session")
        .terminate(cell_id.into())
        .await
        .expect("terminate cell");
    state.disconnect().await;
}

#[tokio::test]
async fn active_cell_limit_rejects_execute_without_disconnecting() {
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel(/*max_capacity*/ 1);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let state = HostState {
        sessions: Mutex::new(HashMap::new()),
        seen_session_ids: Mutex::new(SeenSessionIds::default()),
        requests: Mutex::new(RequestRegistry::default()),
        request_tasks: TaskTracker::new(),
        request_permits: Arc::new(Semaphore::new(MAX_IN_FLIGHT_REQUESTS)),
        active_cell_permits: Arc::new(Semaphore::new(/*permits*/ 0)),
        closing: AtomicBool::new(false),
        peer: Arc::clone(&peer),
    };
    let session_id = session_id("session-1");
    state
        .open_session(
            session_id.clone(),
            CodeModeSessionCellExecutionLimits::default(),
        )
        .expect("open session");
    let request_id = request_id(/*value*/ 1);

    state
        .handle_request(
            request_id,
            HostRequest::Execute {
                session_id,
                request: execute_request("text(\"hello\");"),
            },
            CancellationToken::new(),
        )
        .await;

    assert_eq!(
        decode_frame(outgoing_rx.recv().await.expect("execute response frame")).await,
        HostToClient::Response {
            id: request_id,
            result: WireResult::Err {
                message: "code-mode host has too many active cells".to_string(),
            },
        }
    );
    assert!(!peer.is_disconnected());
    state.disconnect().await;
}

#[tokio::test]
async fn cell_forwarding_panic_disconnects_host() {
    let (outgoing_tx, _outgoing_rx) = mpsc::channel(/*max_capacity*/ 1);
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    peer.spawn_critical("cell forwarding", async {
        panic!("cell forwarding panic probe");
    });

    tokio::time::timeout(Duration::from_secs(1), peer.disconnected())
        .await
        .expect("cell panic should disconnect host");
    assert!(
        peer.failure()
            .expect("cell failure")
            .contains("cell forwarding task failed")
    );
}
