#![cfg(test)]
//! Verify heartbeat delivery and peer responses on backpressured WebSockets.

use super::FRAGMENT_SIZE;
use super::ResponsesWebsocketTimingLogContext;
use super::UPLOAD_READ_AHEAD_BYTES;
use super::WsPump;
use super::WsRequestText;
use super::WsSendOutcome;
use super::WsStream;
use super::request_frames;
use super::run_websocket_response_stream;
use crate::common::ResponseEvent;
use crate::common::ResponseInterrupt;
use crate::error::ApiError;
use crate::telemetry::WebsocketTelemetry;
use assert_matches::assert_matches;
use bytes::Bytes;
use codex_client::TransportError;
use codex_http_client::NetworkPolicyDenied;
use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use http::StatusCode;
use pretty_assertions::assert_eq;
use std::io;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufStream;
use tokio::io::DuplexStream;
use tokio::sync::mpsc;
use tokio::task::coop;
use tokio::time::Instant;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::Utf8Bytes;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::tungstenite::protocol::frame::Frame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::frame::coding::Data;
use tokio_tungstenite::tungstenite::protocol::frame::coding::OpCode;

const TIMEOUT: Duration = Duration::from_secs(3);
const RATE_LIMIT: &str = r#"{"type":"error","status":429,"error":{"code":"rate_limit_exceeded"}}"#;
const CREATED: &str = r#"{"type":"response.created","response":{"id":"resp_test"}}"#;
const COMPLETED_WITH_USAGE: &str = r#"{"type":"response.completed","response":{"id":"resp_test","usage":{"input_tokens":1,"output_tokens":2,"total_tokens":3}}}"#;

async fn backpressured_websockets(
    capacity: usize,
) -> (WsStream, WebSocketStream<BufStream<DuplexStream>>) {
    let (client_io, server_io) = tokio::io::duplex(capacity);
    let socket = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
    let client = WsStream::with_socket(
        socket,
        /*max_message_bytes*/ Some(16 * 1024 * 1024),
        /*routing_affinity*/ None,
    );
    let server =
        WebSocketStream::from_raw_socket(BufStream::new(server_io), Role::Server, None).await;
    (client, server)
}

async fn scripted_client(io: tokio_test::io::Mock) -> WsStream {
    let socket = WebSocketStream::from_raw_socket(io, Role::Client, None).await;
    WsStream::with_socket(
        socket, /*max_message_bytes*/ None, /*routing_affinity*/ None,
    )
}

fn encoded_text(message: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    Frame::message(
        message.to_owned(),
        OpCode::Data(Data::Text),
        /*is_final*/ true,
    )
    .format(&mut bytes)
    .unwrap();
    bytes
}

fn large_request_text() -> Utf8Bytes {
    Utf8Bytes::from(
        serde_json::json!({"type": "response.create", "input": "🌱".repeat(300_000)}).to_string(),
    )
}

fn size_close() -> Message {
    Message::Close(Some(CloseFrame {
        code: CloseCode::Size,
        reason: "message too big".into(),
    }))
}

async fn upload(
    client: WsStream,
    telemetry: Option<Arc<dyn WebsocketTelemetry>>,
) -> Result<ResponseEvent, ApiError> {
    upload_with_interrupt(client, telemetry, std::future::pending()).await
}

async fn upload_with_interrupt(
    mut client: WsStream,
    telemetry: Option<Arc<dyn WebsocketTelemetry>>,
    interrupt: impl Future<Output = ResponseInterrupt> + Clone + Unpin,
) -> Result<ResponseEvent, ApiError> {
    let (tx, mut events) = mpsc::channel(1);
    let mut completion = None;
    let result = run_websocket_response_stream(
        &mut client,
        tx,
        WsRequestText {
            text: large_request_text(),
            private: false,
        },
        TIMEOUT,
        telemetry,
        /*turn_state*/ None,
        &ResponsesWebsocketTimingLogContext::default(),
        interrupt,
        &mut completion,
    )
    .await;
    // The connection owner retires a failed socket and still delivers its saved completion.
    if let Some(completed) = completion {
        return Ok(completed);
    }
    result?;
    events.try_recv().expect("expected a response event")
}

#[test]
fn large_turn_is_one_text_message_in_bounded_frames() {
    let at_limit = Utf8Bytes::from("a".repeat(FRAGMENT_SIZE));
    assert_eq!(
        request_frames(at_limit.clone()).collect::<Vec<_>>(),
        [Message::Text(at_limit)]
    );

    let mut text = "a".repeat(FRAGMENT_SIZE - 1);
    text.push('🌱');
    text.push_str(&"b".repeat(FRAGMENT_SIZE));

    let mut frames = request_frames(text.clone().into());
    for (index, expected) in text.as_bytes().chunks(FRAGMENT_SIZE).enumerate() {
        let Some(Message::Frame(frame)) = frames.next() else {
            panic!("expected a data frame");
        };
        let opcode = if index == 0 {
            Data::Text
        } else {
            Data::Continue
        };
        assert_eq!(frame.header().opcode, OpCode::Data(opcode));
        assert_eq!(
            frame.header().is_final,
            (index + 1) * FRAGMENT_SIZE >= text.len()
        );
        assert_eq!(frame.payload(), expected);
    }
    assert!(frames.next().is_none());
}

#[tokio::test]
async fn ready_messages_do_not_starve_a_pending_write() {
    // Empty text frames consume the whole cooperative budget in a batch.
    // Cheap Pings exercise the batch limit independently of that budget.
    for (frame, read_cost) in [([0x81, 0], 4), ([0x89, 0], 0)] {
        let burst = frame.repeat(128);
        let io = tokio::io::join(burst.as_slice(), tokio::io::sink());
        let socket = WebSocketStream::from_raw_socket(io, Role::Client, None).await;
        let mut socket = Box::pin(socket.then(|message| async move {
            for _ in 0..read_cost {
                coop::consume_budget().await;
            }
            message
        }));
        let (tx, _rx) = mpsc::unbounded_channel();

        let result = WsPump::new(&mut socket, tx)
            .send_frames("request".into(), Instant::now() + TIMEOUT)
            .await;
        assert!(matches!(result, WsSendOutcome::Flushed));
        assert!(
            matches!(socket.next().await, Some(Ok(_))),
            "the receive burst drained before the send"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn backpressured_upload_bounds_read_ahead_across_frames() {
    let payload = "x".repeat(16 * 1024);
    let limit = UPLOAD_READ_AHEAD_BYTES.div_ceil(payload.len());
    let burst = encoded_text(&payload).repeat(2 * limit);
    let (io, peer) = tokio::io::duplex(1024);
    let mut socket = WebSocketStream::from_partially_read(io, burst, Role::Client, None).await;
    let mut peer = WebSocketStream::from_raw_socket(peer, Role::Server, None).await;
    let (tx, rx) = mpsc::unbounded_channel();

    // Reads are always ready while the small transport requires many polls per outgoing frame.
    let mut pump = WsPump::new(&mut socket, tx);
    let send = pump.send_frames(large_request_text(), Instant::now() + TIMEOUT);
    let (result, received) =
        tokio::time::timeout(TIMEOUT, async { tokio::join!(send, peer.next()) })
            .await
            .expect("the write should continue once read-ahead stops");
    assert!(matches!(result, WsSendOutcome::Flushed));
    assert_matches!(received, Some(Ok(Message::Text(_))));
    assert!(rx.len() <= limit, "queued {} messages", rx.len());

    drop(peer);
    for _ in rx.len()..2 * limit {
        assert_eq!(socket.next().await.unwrap().unwrap().len(), payload.len());
    }
}

#[tokio::test(start_paused = true)]
async fn large_request_answers_ping_before_upload_completes() {
    let (client, mut server) = backpressured_websockets(/*capacity*/ 1024).await;
    let completed = Message::text(r#"{"type":"response.completed","response":{"id":"resp_test"}}"#);
    let serve_request = async {
        // Wait until the client is writing, keeping the prefix for Tungstenite to decode.
        // The small duplex buffer keeps the full message from being written before the Ping.
        assert!(!server.get_mut().fill_buf().await.unwrap().is_empty());
        let ping = Bytes::from_static(b"upload heartbeat");
        server.send(Message::Ping(ping.clone())).await.unwrap();

        let Some(Ok(Message::Pong(pong))) = server.next().await else {
            panic!("expected the Pong before receiving the full request");
        };
        assert_eq!(pong, ping);
        let Some(Ok(Message::Text(received))) = server.next().await else {
            panic!("expected a complete text message after the Pong");
        };
        assert!(
            received == large_request_text(),
            "request text changed in transit"
        );
        server.send(completed).await.unwrap();
    };

    let (result, ()) = tokio::time::timeout(TIMEOUT, async {
        tokio::join!(upload(client, /*telemetry*/ None), serve_request)
    })
    .await
    .expect("peer should receive the Pong and complete the upload before the deadline");
    assert_matches!(result, Ok(ResponseEvent::Completed { response_id, .. }) if response_id == "resp_test");
}

#[tokio::test(start_paused = true)]
async fn idle_size_close_is_not_an_upload_rejection() {
    let (mut client, mut server) = backpressured_websockets(/*capacity*/ 1024).await;
    server.send(size_close()).await.unwrap();
    tokio::time::timeout(TIMEOUT, &mut client.pump_task)
        .await
        .expect("idle pump should observe the close")
        .unwrap();

    let result = upload(client, /*telemetry*/ None).await;
    assert_matches!(result, Err(ApiError::Stream(message)) => {
        assert_eq!(message, WsError::ConnectionClosed.to_string());
    });
    assert!(server.get_mut().fill_buf().await.unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn size_close_before_first_frame_is_not_an_upload_rejection() {
    let io = tokio_test::io::Builder::new()
        .read(&[0x88, 0x02, 0x03, 0xf1]) // Close(1009)
        .build();
    let mut socket = WebSocketStream::from_raw_socket(io, Role::Client, None).await;
    let (tx, _rx) = mpsc::unbounded_channel();

    // The pump has dequeued a command and is about to write its first frame.
    assert!(matches!(
        WsPump::new(&mut socket, tx)
            .send_frames("request".into(), Instant::now() + TIMEOUT)
            .await,
        WsSendOutcome::NotStarted(_)
    ));
}

#[tokio::test(start_paused = true)]
async fn size_close_after_partial_first_frame_write_error_rejects_upload() {
    // The peer learns the length of the masked text fragment before its payload arrives.
    let mut header = vec![0x01, 0xff];
    header.extend_from_slice(&(FRAGMENT_SIZE as u64).to_be_bytes());
    let mut pings_and_close = [0x89, 0x00].repeat(64);
    pings_and_close.extend_from_slice(&[0x88, 0x02, 0x03, 0xf1]); // Close(1009)
    let io = tokio_test::io::Builder::new()
        .write(&header)
        .write_error(io::ErrorKind::BrokenPipe.into())
        .read(&[0x8a, 0x00].repeat(64)) // More than one batch of Pongs.
        .read(&pings_and_close)
        .write_error(io::ErrorKind::BrokenPipe.into())
        .wait(TIMEOUT) // Subsequent Pong writes stay blocked during recovery.
        .build();
    let result = upload(scripted_client(io).await, /*telemetry*/ None).await;
    assert_matches!(result, Err(ApiError::WebsocketMessageTooLarge));
}

async fn reject_upload(messages: impl IntoIterator<Item = Message>) -> ApiError {
    let (client, mut server) = backpressured_websockets(/*capacity*/ 1024).await;
    let reject_request = async {
        // Wait for the first bytes, then reject the request without draining the upload.
        assert!(!server.get_mut().fill_buf().await.unwrap().is_empty());
        for message in messages {
            server.send(message).await.unwrap();
        }
    };

    let (result, ()) = tokio::time::timeout(TIMEOUT, async {
        tokio::join!(upload(client, /*telemetry*/ None), reject_request)
    })
    .await
    .expect("client should see the close while the upload is blocked");
    result.unwrap_err()
}

#[tokio::test(start_paused = true)]
async fn upload_reports_size_close_without_waiting_for_remaining_bytes() {
    let error = reject_upload([size_close()]).await;
    assert_matches!(error, ApiError::WebsocketMessageTooLarge);
}

fn assert_rate_limit(error: ApiError) {
    assert_matches!(
        error,
        ApiError::Transport(TransportError::Http {
            status: StatusCode::TOO_MANY_REQUESTS,
            body: Some(body),
            ..
        }) if body == RATE_LIMIT
    );
}

#[tokio::test(start_paused = true)]
async fn upload_preserves_peer_error_before_size_close() {
    assert_rate_limit(reject_upload([Message::text(RATE_LIMIT), size_close()]).await);
}

#[derive(Default)]
struct UploadTelemetry {
    send_error: OnceLock<String>,
}

impl WebsocketTelemetry for UploadTelemetry {
    fn on_ws_request(&self, _: Duration, error: Option<&ApiError>, _: bool) {
        let Some(ApiError::Stream(message)) = error else {
            panic!("expected an upload error, got {error:?}");
        };
        self.send_error.set(message.clone()).unwrap();
    }

    fn on_ws_receive_failure(&self, _: Duration) {
        panic!("upload failure counted again as a receive failure");
    }

    fn on_ws_text_event(&self, _: &str, _: Option<&str>, _: Duration) {}
}

fn partial_upload(message: &str) -> tokio_test::io::Builder {
    let mut io = tokio_test::io::Builder::new();
    io.write(&[0x01]) // Start the first text fragment.
        .read(&encoded_text(message))
        // Wake the pending flush after the pump has read the peer response.
        .wait(Duration::from_millis(1));
    io
}

fn failed_upload(message: &str) -> tokio_test::io::Builder {
    let mut io = partial_upload(message);
    io.write_error(io::ErrorKind::BrokenPipe.into());
    io
}

fn revoked() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        NetworkPolicyDenied::Revoked,
    )
}

#[tokio::test(start_paused = true)]
async fn upload_reports_policy_revocation_before_buffered_peer_error() {
    let scripts = [
        partial_upload(RATE_LIMIT).write_error(revoked()).build(),
        failed_upload(RATE_LIMIT).read_error(revoked()).build(),
        failed_upload(RATE_LIMIT)
            .read(&[0x89, 0x00]) // Ping during the final read.
            .write_error(revoked())
            .build(),
    ];
    for io in scripts {
        let result = upload(scripted_client(io).await, /*telemetry*/ None).await;
        assert_matches!(
            result,
            Err(ApiError::Transport(TransportError::Policy(
                NetworkPolicyDenied::Revoked
            )))
        );
    }
}

fn broken_pipe_message() -> String {
    WsError::Io(io::ErrorKind::BrokenPipe.into()).to_string()
}

async fn upload_with_send_error(io: tokio_test::io::Mock, send_error: &str) -> ApiError {
    let client = scripted_client(io).await;
    let telemetry = Arc::new(UploadTelemetry::default());
    let error = upload(client, Some(telemetry.clone())).await.unwrap_err();
    assert_eq!(telemetry.send_error.get().unwrap(), send_error);
    error
}

#[tokio::test(start_paused = true)]
async fn upload_preserves_peer_error_when_send_times_out() {
    let io = partial_upload(RATE_LIMIT).wait(TIMEOUT * 2).build();
    assert_rate_limit(upload_with_send_error(io, "idle timeout sending websocket request").await);
}

#[tokio::test(start_paused = true)]
async fn upload_delivers_buffered_peer_error_before_local_write_error() {
    let io = failed_upload(RATE_LIMIT).build();
    assert_rate_limit(upload_with_send_error(io, &broken_pipe_message()).await);
}

#[tokio::test(start_paused = true)]
async fn upload_reports_write_error_after_incomplete_response() {
    let io = failed_upload(CREATED).build();
    let error = upload_with_send_error(io, &broken_pipe_message()).await;
    assert_matches!(error, ApiError::Stream(message) if message == broken_pipe_message());
}

#[tokio::test(start_paused = true)]
async fn close_before_sending_interrupt_preserves_completion_usage() {
    let (client, mut server) = backpressured_websockets(/*capacity*/ 1024).await;
    let created = tokio::spawn(async move {
        server.next().await.unwrap().unwrap();
        server.send(Message::text(CREATED)).await.unwrap();
        server
    });
    let cancel = async {
        let mut server = created.await.unwrap();
        // Make both frames readable just as the response loop decides to interrupt.
        server
            .feed(Message::text(COMPLETED_WITH_USAGE))
            .await
            .unwrap();
        server.send(Message::Close(None)).await.unwrap();
        ResponseInterrupt::Cancel {
            deadline: Instant::now() + TIMEOUT,
        }
    }
    .shared();
    let event = upload_with_interrupt(client, /*telemetry*/ None, cancel)
        .await
        .unwrap();
    assert_matches!(event, ResponseEvent::Completed { token_usage: Some(usage), .. } if usage.total_tokens == 3);
}

#[tokio::test(start_paused = true)]
async fn idle_ping_finishes_pong_after_write_backpressure() {
    // The Pong cannot fit in the buffer and the peer sends no more messages or commands.
    let (_client, mut server) = backpressured_websockets(/*capacity*/ 8).await;
    let payload = Bytes::from_static(b"idle heartbeat under write pressure");

    let reply = tokio::time::timeout(TIMEOUT, async {
        server.send(Message::Ping(payload.clone())).await.unwrap();
        server.next().await
    })
    .await
    .expect("the idle pump should finish flushing the Pong when the peer drains the socket");
    assert_eq!(reply.unwrap().unwrap(), Message::Pong(payload));
}
