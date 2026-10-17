//! Negotiated WebSocket payload compression with one persistent zstd stream per direction.
//!
//! The upgrade request and response must both carry
//! `x-codex-app-server-compression: zstd-stream-v1` to enable compression.
//! Without an acknowledgment, messages remain JSON text.
//! Each binary message contains one UTF-8 JSON message flushed with `ZSTD_e_flush`.
//! Keep separate streams per direction and start fresh on reconnect. Never finish
//! the zstd frame between messages.

use std::io;
use zstd::stream::raw::CParameter;
use zstd::stream::raw::DParameter;
use zstd::stream::raw::Decoder;
use zstd::stream::raw::Encoder;
use zstd::stream::raw::Operation;
use zstd::stream::raw::OutBuffer;

pub const WEBSOCKET_COMPRESSION_HEADER: &str = "x-codex-app-server-compression";
pub const ZSTD_STREAM_PROTOCOL: &str = "zstd-stream-v1";
const COMPRESSION_LEVEL: i32 = -7;
const WINDOW_LOG: u32 = 20;
const BUFFER_SIZE: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WebSocketCompression {
    Plain,
    Zstd,
}

impl WebSocketCompression {
    /// A missing echo selects the existing text protocol for older peers.
    pub fn from_response_header(value: Option<&str>) -> io::Result<Self> {
        match value {
            None => Ok(Self::Plain),
            Some(ZSTD_STREAM_PROTOCOL) => Ok(Self::Zstd),
            Some(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported app-server WebSocket compression protocol",
            )),
        }
    }
}

pub struct ZstdEncoder {
    encoder: Encoder<'static>,
}

impl ZstdEncoder {
    pub fn new() -> io::Result<Self> {
        let mut encoder = Encoder::new(COMPRESSION_LEVEL)?;
        encoder.set_parameter(CParameter::WindowLog(WINDOW_LOG))?;
        Ok(Self { encoder })
    }

    /// Flush one message without ending the frame or resetting its history.
    pub fn encode(&mut self, text: &str) -> io::Result<Vec<u8>> {
        let mut input = text.as_bytes();
        let mut encoded = Vec::new();
        let mut buffer = [0; BUFFER_SIZE];
        while !input.is_empty() {
            let status = self.encoder.run_on_buffers(input, &mut buffer)?;
            encoded.extend_from_slice(&buffer[..status.bytes_written]);
            input = &input[status.bytes_read..];
        }
        loop {
            let mut output = OutBuffer::around(&mut buffer[..]);
            let remaining = self.encoder.flush(&mut output)?;
            encoded.extend_from_slice(output.as_slice());
            if remaining == 0 {
                return Ok(encoded);
            }
        }
    }
}

pub struct ZstdDecoder {
    decoder: Decoder<'static>,
    max_message_size: usize,
}

impl ZstdDecoder {
    pub fn new(max_message_size: usize) -> io::Result<Self> {
        let mut decoder = Decoder::new()?;
        decoder.set_parameter(DParameter::WindowLogMax(WINDOW_LOG))?;
        Ok(Self {
            decoder,
            max_message_size,
        })
    }

    /// Decode one flushed message, retaining history and bounding its decoded size.
    /// Any error invalidates the stream; the caller must close the connection.
    pub fn decode(&mut self, mut input: &[u8]) -> io::Result<String> {
        let mut decoded = Vec::new();
        let mut buffer = [0; BUFFER_SIZE];
        loop {
            let status = self.decoder.run_on_buffers(input, &mut buffer)?;
            if status.bytes_written > self.max_message_size.saturating_sub(decoded.len()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "decompressed app-server WebSocket message exceeds size limit",
                ));
            }
            decoded.extend_from_slice(&buffer[..status.bytes_written]);
            input = &input[status.bytes_read..];
            if status.bytes_read == 0 && status.bytes_written == 0 {
                if !input.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid app-server WebSocket compressed message",
                    ));
                }
                break;
            }
        }
        String::from_utf8(decoded)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

#[cfg(test)]
#[path = "websocket_compression_tests.rs"]
mod tests;
