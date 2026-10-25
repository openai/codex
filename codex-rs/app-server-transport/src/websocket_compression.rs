//! Negotiated WebSocket payload compression with one persistent zstd stream per direction.
//!
//! The upgrade request and response must both carry
//! `x-codex-app-server-compression: zstd-stream-v1` to enable compression.
//! Without an acknowledgment, messages remain JSON text.
//! Each binary message contains one UTF-8 JSON message flushed with `ZSTD_e_flush`.
//! Keep separate streams per direction and start fresh on reconnect. Never finish
//! the zstd frame between messages.
//! Paired codecs report cumulative payload sizes per connection and direction.

use std::io;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use zstd::stream::raw::CParameter;
use zstd::stream::raw::DParameter;
use zstd::stream::raw::Decoder;
use zstd::stream::raw::Encoder;
use zstd::stream::raw::Operation;
use zstd::stream::raw::OutBuffer;

use crate::ConnectionId;

pub const WEBSOCKET_COMPRESSION_HEADER: &str = "x-codex-app-server-compression";
pub const ZSTD_STREAM_PROTOCOL: &str = "zstd-stream-v1";
const COMPRESSION_LEVEL: i32 = -7;
const WINDOW_LOG: u32 = 20;
const BUFFER_SIZE: usize = 32 * 1024;
const STATS_INTERVAL: Duration = Duration::from_secs(60);
static NEXT_NATIVE_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

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
    stats: Option<CompressionStats>,
}

impl ZstdEncoder {
    pub fn new() -> io::Result<Self> {
        let mut encoder = Encoder::new(COMPRESSION_LEVEL)?;
        encoder.set_parameter(CParameter::WindowLog(WINDOW_LOG))?;
        Ok(Self {
            encoder,
            stats: None,
        })
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
                if let Some(stats) = &mut self.stats {
                    stats.record(text.len(), encoded.len());
                }
                return Ok(encoded);
            }
        }
    }
}

pub struct ZstdDecoder {
    decoder: Decoder<'static>,
    max_message_size: usize,
    stats: Option<CompressionStats>,
}

impl ZstdDecoder {
    pub fn new(max_message_size: usize) -> io::Result<Self> {
        let mut decoder = Decoder::new()?;
        decoder.set_parameter(DParameter::WindowLogMax(WINDOW_LOG))?;
        Ok(Self {
            decoder,
            max_message_size,
            stats: None,
        })
    }

    /// Decode one flushed message, retaining history and bounding its decoded size.
    /// Any error invalidates the stream; the caller must close the connection.
    pub fn decode(&mut self, mut input: &[u8]) -> io::Result<String> {
        let compressed_bytes = input.len();
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
        let text = String::from_utf8(decoded)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if let Some(stats) = &mut self.stats {
            stats.record(text.len(), compressed_bytes);
        }
        Ok(text)
    }
}

/// Identifies the connection whose compression statistics are reported.
pub enum CompressionConnection {
    Listener(ConnectionId),
    NativeClient,
}

/// Create codecs whose payload statistics share the transport connection ID.
/// TX counts encoded payloads, including those whose subsequent socket write fails.
pub fn zstd_codec_pair(
    max_message_size: usize,
    connection: CompressionConnection,
) -> io::Result<(ZstdEncoder, ZstdDecoder)> {
    let mut encoder = ZstdEncoder::new()?;
    let mut decoder = ZstdDecoder::new(max_message_size)?;
    let (connection_id, connection_kind) = match connection {
        CompressionConnection::Listener(connection_id) => (connection_id.0, "listener"),
        CompressionConnection::NativeClient => (
            NEXT_NATIVE_CONNECTION_ID.fetch_add(1, Ordering::Relaxed),
            "native_client",
        ),
    };
    encoder.stats = Some(CompressionStats::new(connection_id, connection_kind, "tx"));
    decoder.stats = Some(CompressionStats::new(connection_id, connection_kind, "rx"));
    Ok((encoder, decoder))
}

struct CompressionStats {
    connection_id: u64,
    connection_kind: &'static str,
    direction: &'static str,
    uncompressed_bytes: u64,
    compressed_bytes: u64,
    messages: u64,
    last_report: Instant,
}

impl CompressionStats {
    fn new(connection_id: u64, connection_kind: &'static str, direction: &'static str) -> Self {
        Self {
            connection_id,
            connection_kind,
            direction,
            uncompressed_bytes: 0,
            compressed_bytes: 0,
            messages: 0,
            last_report: Instant::now(),
        }
    }

    fn record(&mut self, uncompressed_bytes: usize, compressed_bytes: usize) {
        self.uncompressed_bytes += uncompressed_bytes as u64;
        self.compressed_bytes += compressed_bytes as u64;
        self.messages += 1;
        if self.last_report.elapsed() >= STATS_INTERVAL {
            self.report("periodic");
            self.last_report = Instant::now();
        }
    }

    fn saved_percent(&self) -> Option<f64> {
        (self.uncompressed_bytes != 0)
            .then(|| 100.0 * (1.0 - self.compressed_bytes as f64 / self.uncompressed_bytes as f64))
    }

    fn report(&self, report: &'static str) {
        tracing::info!(
            connection_id = self.connection_id,
            connection_kind = self.connection_kind,
            direction = self.direction,
            report,
            uncompressed_bytes = self.uncompressed_bytes,
            compressed_bytes = self.compressed_bytes,
            messages = self.messages,
            saved_percent = self.saved_percent(),
            "app-server WebSocket compression payload totals"
        );
    }
}

impl Drop for CompressionStats {
    fn drop(&mut self) {
        self.report("final");
    }
}

#[cfg(test)]
#[path = "websocket_compression_tests.rs"]
mod tests;
