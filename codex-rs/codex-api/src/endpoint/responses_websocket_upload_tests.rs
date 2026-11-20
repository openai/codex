#![cfg(test)]
//! Verify heartbeat delivery and peer responses on backpressured WebSockets.

use super::FRAGMENT_SIZE;
use super::ResponsesWebsocketTimingLogContext;
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
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufStream;
use tokio::io::DuplexStream;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::Utf8Bytes;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::frame::coding::Data;
use tokio_tungstenite::tungstenite::protocol::frame::coding::OpCode;

const TIMEOUT: Duration = Duration::from_secs(3);

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
