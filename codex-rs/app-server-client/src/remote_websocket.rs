//! Negotiated WebSocket compression. Codec history lasts for the connection.

use std::io;

use codex_app_server_transport::CompressionConnection;
use codex_app_server_transport::WEBSOCKET_COMPRESSION_HEADER;
use codex_app_server_transport::WebSocketCompression;
use codex_app_server_transport::ZstdDecoder;
use codex_app_server_transport::ZstdEncoder;
use codex_app_server_transport::zstd_codec_pair;
use futures::SinkExt;
use futures::StreamExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Bytes;
use tokio_tungstenite::tungstenite::Error;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::client::Response;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::frame::Frame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::Data;
use tokio_tungstenite::tungstenite::protocol::frame::coding::OpCode;

// Match the listener's default incoming frame limit when no smaller cap is advertised.
const DEFAULT_MAX_FRAME_BYTES: usize = 16 << 20;
const MAX_UNFRAGMENTED_MESSAGE_BYTES_HEADER: &str =
    "x-codex-websocket-max-unfragmented-message-bytes";

pub(crate) type WebSocketHandshake<S> = (WebSocketStream<S>, Response);

pub(crate) struct RemoteWebSocket<S> {
    stream: WebSocketStream<S>,
    codec: Option<(ZstdEncoder, ZstdDecoder)>,
    max_frame_bytes: usize,
    write_failed: bool,
}

impl<S: AsyncRead + AsyncWrite + Unpin> RemoteWebSocket<S> {
    pub(crate) fn from_handshake(
        (stream, response): WebSocketHandshake<S>,
        max_message_size: usize,
    ) -> io::Result<Self> {
        let headers = response.headers();
        let header = headers
            .get(WEBSOCKET_COMPRESSION_HEADER)
            .map(HeaderValue::to_str)
            .transpose()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        let codec = match WebSocketCompression::from_response_header(header)? {
            WebSocketCompression::Plain => None,
            WebSocketCompression::Zstd => Some(zstd_codec_pair(
                max_message_size,
                CompressionConnection::NativeClient,
            )?),
        };
        let max_frame_bytes = headers
            .get(MAX_UNFRAGMENTED_MESSAGE_BYTES_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(DEFAULT_MAX_FRAME_BYTES)
            .min(DEFAULT_MAX_FRAME_BYTES);
        Ok(Self {
            stream,
            codec,
            max_frame_bytes,
            write_failed: false,
        })
    }

    pub(crate) async fn next(&mut self) -> Option<Result<Message, Error>> {
        if self.write_failed {
            return Some(Err(Error::Io(io::ErrorKind::BrokenPipe.into())));
        }
        let message = self.stream.next().await?;
        Some(match (&mut self.codec, message) {
            (Some((_, decoder)), Ok(Message::Binary(bytes))) => decoder
                .decode(&bytes)
                .map(|text| Message::Text(text.into()))
                .map_err(Error::Io),
            (Some(_), Ok(Message::Text(_))) => Err(Error::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected compressed binary app-server message",
            ))),
            (_, message) => message,
        })
    }

    pub(crate) async fn send_text(&mut self, text: String) -> Result<(), Error> {
        if self.write_failed {
            return Err(Error::Io(io::ErrorKind::BrokenPipe.into()));
        }
        let result = async {
            let Some((encoder, _)) = &mut self.codec else {
                return self.stream.send(Message::Text(text.into())).await;
            };
            let mut bytes = Bytes::from(encoder.encode(&text).map_err(Error::Io)?);
            if bytes.len() <= self.max_frame_bytes {
                return self.stream.send(Message::Binary(bytes)).await;
            }
            let mut opcode = OpCode::Data(Data::Binary);
            while !bytes.is_empty() {
                let chunk = bytes.split_to(bytes.len().min(self.max_frame_bytes));
                let frame = Frame::message(chunk, opcode, bytes.is_empty());
                self.stream.feed(Message::Frame(frame)).await?;
                opcode = OpCode::Data(Data::Continue);
            }
            self.stream.flush().await
        }
        .await;
        // A failed write may leave the compression history ahead of the receiver.
        self.write_failed = result.is_err();
        result
    }

    pub(crate) async fn close(&mut self) -> Result<(), Error> {
        self.stream.close(/*msg*/ None).await
    }
}
