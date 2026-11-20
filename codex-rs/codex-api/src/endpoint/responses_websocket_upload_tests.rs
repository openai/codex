#![cfg(test)]
//! Verify request framing and heartbeat delivery on idle WebSockets.

use super::FRAGMENT_SIZE;
use super::WsStream;
use super::request_frames;
use bytes::Bytes;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio::io::BufStream;
use tokio::io::DuplexStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::Utf8Bytes;
use tokio_tungstenite::tungstenite::protocol::Role;
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
