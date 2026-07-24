use std::io;
use std::time::Duration;

use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::EncodedFrame;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::FramedWriter;
use codex_code_mode_protocol::host::HostToClient;
use codex_websocket_client::WebSocketConnection;
use futures::SinkExt;
use futures::StreamExt;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio_tungstenite::tungstenite::Message;

const WEBSOCKET_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) enum ConnectionReader {
    Stdio(FramedReader<ChildStdout>),
    WebSocket(SplitStream<WebSocketConnection>),
}

pub(super) enum ConnectionWriter {
    Stdio(FramedWriter<ChildStdin>),
    WebSocket(SplitSink<WebSocketConnection, Message>),
}

impl ConnectionReader {
    pub(super) async fn read(&mut self) -> io::Result<Option<HostToClient>> {
        match self {
            Self::Stdio(reader) => reader.read().await,
            Self::WebSocket(reader) => loop {
                match reader.next().await {
                    Some(Ok(Message::Binary(frame))) => {
                        return EncodedFrame::decode_framed(&frame).map(Some);
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | None => return Ok(None),
                    Some(Ok(Message::Text(_))) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "code-mode host websocket messages must be binary framed messages",
                        ));
                    }
                    Some(Ok(Message::Frame(_))) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "code-mode host websocket returned an unexpected raw frame",
                        ));
                    }
                    Some(Err(error)) => {
                        return Err(io::Error::other(format!(
                            "failed to read code-mode host websocket message: {error}"
                        )));
                    }
                }
            },
        }
    }
}

impl ConnectionWriter {
    pub(super) async fn write(&mut self, message: &ClientToHost) -> io::Result<()> {
        self.write_frame(EncodedFrame::encode(message)?).await
    }

    pub(super) async fn write_frame(&mut self, frame: EncodedFrame) -> io::Result<()> {
        match self {
            Self::Stdio(writer) => writer.write_frame(&frame).await,
            Self::WebSocket(writer) => writer
                .send(Message::Binary(frame.into_framed_bytes().into()))
                .await
                .map_err(|error| {
                    io::Error::other(format!(
                        "failed to write code-mode host websocket message: {error}"
                    ))
                }),
        }
    }

    pub(super) async fn close(&mut self) -> io::Result<()> {
        match self {
            Self::Stdio(_) => Ok(()),
            Self::WebSocket(writer) => {
                tokio::time::timeout(WEBSOCKET_CLOSE_TIMEOUT, writer.close())
                    .await
                    .map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::TimedOut,
                            "timed out closing code-mode host websocket connection",
                        )
                    })?
                    .map_err(|error| {
                        io::Error::other(format!(
                            "failed to close code-mode host websocket connection: {error}"
                        ))
                    })
            }
        }
    }
}
