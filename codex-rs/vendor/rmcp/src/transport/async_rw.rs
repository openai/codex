use std::{marker::PhantomData, sync::Arc};

// use crate::schema::*;
use futures::{SinkExt, StreamExt};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::Mutex,
};
use tokio_util::{
    bytes::{Buf, BufMut, BytesMut},
    codec::{Decoder, Encoder, FramedRead, FramedWrite},
};

use super::{IntoTransport, Transport};
use crate::service::{RxJsonRpcMessage, ServiceRole, TxJsonRpcMessage};

pub enum TransportAdapterAsyncRW {}

impl<Role, R, W> IntoTransport<Role, std::io::Error, TransportAdapterAsyncRW> for (R, W)
where
    Role: ServiceRole,
    R: AsyncRead + Send + 'static + Unpin,
    W: AsyncWrite + Send + 'static + Unpin,
{
    fn into_transport(self) -> impl Transport<Role, Error = std::io::Error> + 'static {
        AsyncRwTransport::new(self.0, self.1)
    }
}

pub enum TransportAdapterAsyncCombinedRW {}
impl<Role, S> IntoTransport<Role, std::io::Error, TransportAdapterAsyncCombinedRW> for S
where
    Role: ServiceRole,
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    fn into_transport(self) -> impl Transport<Role, Error = std::io::Error> + 'static {
        IntoTransport::<Role, std::io::Error, TransportAdapterAsyncRW>::into_transport(
            tokio::io::split(self),
        )
    }
}

pub type TransportWriter<Role, W> = FramedWrite<W, JsonRpcMessageCodec<TxJsonRpcMessage<Role>>>;

pub struct AsyncRwTransport<Role: ServiceRole, R: AsyncRead, W: AsyncWrite> {
    read: FramedRead<R, JsonRpcMessageCodec<RxJsonRpcMessage<Role>>>,
    write: Arc<Mutex<Option<TransportWriter<Role, W>>>>,
}

impl<Role: ServiceRole, R, W> AsyncRwTransport<Role, R, W>
where
    R: Send + AsyncRead + Unpin,
    W: Send + AsyncWrite + Unpin + 'static,
{
    pub fn new(read: R, write: W) -> Self {
        let read = FramedRead::new(
            read,
            JsonRpcMessageCodec::<RxJsonRpcMessage<Role>>::default(),
        );
        let write = Arc::new(Mutex::new(Some(FramedWrite::new(
            write,
            JsonRpcMessageCodec::<TxJsonRpcMessage<Role>>::default(),
        ))));
        Self { read, write }
    }
}

#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
impl<R, W> AsyncRwTransport<crate::RoleClient, R, W>
where
    R: Send + AsyncRead + Unpin,
    W: Send + AsyncWrite + Unpin + 'static,
{
    pub fn new_client(read: R, write: W) -> Self {
        Self::new(read, write)
    }
}

#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
impl<R, W> AsyncRwTransport<crate::RoleServer, R, W>
where
    R: Send + AsyncRead + Unpin,
    W: Send + AsyncWrite + Unpin + 'static,
{
    pub fn new_server(read: R, write: W) -> Self {
        Self::new(read, write)
    }
}

impl<Role: ServiceRole, R, W> Transport<Role> for AsyncRwTransport<Role, R, W>
where
    R: Send + AsyncRead + Unpin,
    W: Send + AsyncWrite + Unpin + 'static,
{
    type Error = std::io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<Role>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let lock = self.write.clone();
        async move {
            let mut write = lock.lock().await;
            if let Some(ref mut write) = *write {
                write.send(item).await.map_err(Into::into)
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "Transport is closed",
                ))
            }
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<Role>>> {
        let next = self.read.next();
        async {
            next.await.and_then(|e| {
                e.inspect_err(|e| {
                    tracing::error!("Error reading from stream: {}", e);
                })
                .ok()
            })
        }
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        let mut write = self.write.lock().await;
        drop(write.take());
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct JsonRpcMessageCodec<T> {
    _marker: PhantomData<fn() -> T>,
    next_index: usize,
    max_length: usize,
    is_discarding: bool,
}

impl<T> Default for JsonRpcMessageCodec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> JsonRpcMessageCodec<T> {
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
            next_index: 0,
            max_length: usize::MAX,
            is_discarding: false,
        }
    }

    pub fn new_with_max_length(max_length: usize) -> Self {
        Self {
            max_length,
            ..Self::new()
        }
    }

    pub fn max_length(&self) -> usize {
        self.max_length
    }
}

fn without_carriage_return(s: &[u8]) -> &[u8] {
    if let Some(&b'\r') = s.last() {
        &s[..s.len() - 1]
    } else {
        s
    }
}

/// Check if a method is a standard MCP method (request, response, or notification).
/// This includes both requests and notifications defined in the MCP specification.
///
/// Based on MCP specification 2025-06-18: https://modelcontextprotocol.io/specification/2025-06-18
fn is_standard_method(method: &str) -> bool {
    matches!(
        method,
        "initialize"
            | "ping"
            | "prompts/get"
            | "prompts/list"
            | "resources/list"
            | "resources/read"
            | "resources/subscribe"
            | "resources/unsubscribe"
            | "resources/templates/list"
            | "tools/call"
            | "tools/list"
            | "completion/complete"
            | "logging/setLevel"
            | "roots/list"
            | "sampling/createMessage"
    ) || is_standard_notification(method)
}

fn is_standard_notification(method: &str) -> bool {
    matches!(
        method,
        "notifications/cancelled"
            | "notifications/initialized"
            | "notifications/message"
            | "notifications/progress"
            | "notifications/prompts/list_changed"
            | "notifications/resources/list_changed"
            | "notifications/resources/updated"
            | "notifications/roots/list_changed"
            | "notifications/tools/list_changed"
    )
}

/// Determines if a notification should be ignored for compatibility.
fn should_ignore_notification(json_value: &serde_json::Value, method: &str) -> bool {
    let is_notification = json_value.get("id").is_none();

    // Ignore non-MCP notifications (like LSP messages) for compatibility
    if is_notification && !is_standard_method(method) {
        tracing::trace!(
            "Ignoring non-MCP notification '{}' for compatibility",
            method
        );
        return true;
    }

    // Ignore non-standard MCP notifications
    matches!(
        (
            method.starts_with("notifications/"),
            is_standard_notification(method)
        ),
        (true, false)
    )
}

/// Try to parse a message with compatibility handling for non-standard notifications
fn try_parse_with_compatibility<T: serde::de::DeserializeOwned>(
    line: &[u8],
    context: &str,
) -> Result<Option<T>, JsonRpcMessageCodecError> {
    if let Ok(line_str) = std::str::from_utf8(line) {
        match serde_json::from_slice(line) {
            Ok(item) => Ok(Some(item)),
            Err(e) => {
                // Check if this is a notification that should be ignored for compatibility
                if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(line_str) {
                    if let Some(method) =
                        json_value.get("method").and_then(serde_json::Value::as_str)
                    {
                        if should_ignore_notification(&json_value, method) {
                            return Ok(None);
                        }
                    }
                }

                tracing::debug!(
                    "Failed to parse message {}: {} | Error: {}",
                    context,
                    line_str,
                    e
                );
                Err(JsonRpcMessageCodecError::Serde(e))
            }
        }
    } else {
        serde_json::from_slice(line)
            .map(Some)
            .map_err(JsonRpcMessageCodecError::Serde)
    }
}

#[derive(Debug, Error)]
pub enum JsonRpcMessageCodecError {
    #[error("max line length exceeded")]
    MaxLineLengthExceeded,
    #[error("serde error {0}")]
    Serde(#[from] serde_json::Error),
    #[error("io error {0}")]
    Io(#[from] std::io::Error),
}

impl From<JsonRpcMessageCodecError> for std::io::Error {
    fn from(value: JsonRpcMessageCodecError) -> Self {
        match value {
            JsonRpcMessageCodecError::MaxLineLengthExceeded => {
                std::io::Error::new(std::io::ErrorKind::InvalidData, value)
            }
            JsonRpcMessageCodecError::Serde(e) => e.into(),
            JsonRpcMessageCodecError::Io(e) => e,
        }
    }
}

impl<T: DeserializeOwned> Decoder for JsonRpcMessageCodec<T> {
    type Item = T;

    type Error = JsonRpcMessageCodecError;

    fn decode(
        &mut self,
        buf: &mut BytesMut,
    ) -> Result<Option<Self::Item>, JsonRpcMessageCodecError> {
        loop {
            // Determine how far into the buffer we'll search for a newline. If
            // there's no max_length set, we'll read to the end of the buffer.
            let read_to = std::cmp::min(self.max_length.saturating_add(1), buf.len());

            let newline_offset = buf[self.next_index..read_to]
                .iter()
                .position(|b| *b == b'\n');

            match (self.is_discarding, newline_offset) {
                (true, Some(offset)) => {
                    // If we found a newline, discard up to that offset and
                    // then stop discarding. On the next iteration, we'll try
                    // to read a line normally.
                    buf.advance(offset + self.next_index + 1);
                    self.is_discarding = false;
                    self.next_index = 0;
                }
                (true, None) => {
                    // Otherwise, we didn't find a newline, so we'll discard
                    // everything we read. On the next iteration, we'll continue
                    // discarding up to max_len bytes unless we find a newline.
                    buf.advance(read_to);
                    self.next_index = 0;
                    if buf.is_empty() {
                        return Ok(None);
                    }
                }
                (false, Some(offset)) => {
                    // Found a line!
                    let newline_index = offset + self.next_index;
                    self.next_index = 0;
                    let line = buf.split_to(newline_index + 1);
                    let line = &line[..line.len() - 1];
                    let line = without_carriage_return(line);

                    // Use compatibility handling function
                    let item = match try_parse_with_compatibility(line, "decode")? {
                        Some(item) => item,
                        None => return Ok(None), // Skip non-standard message
                    };
                    return Ok(Some(item));
                }
                (false, None) if buf.len() > self.max_length => {
                    // Reached the maximum length without finding a
                    // newline, return an error and start discarding on the
                    // next call.
                    self.is_discarding = true;
                    return Err(JsonRpcMessageCodecError::MaxLineLengthExceeded);
                }
                (false, None) => {
                    // We didn't find a line or reach the length limit, so the next
                    // call will resume searching at the current offset.
                    self.next_index = read_to;
                    return Ok(None);
                }
            }
        }
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<T>, JsonRpcMessageCodecError> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => {
                self.next_index = 0;
                // No terminating newline - return remaining data, if any
                if buf.is_empty() || buf == &b"\r"[..] {
                    None
                } else {
                    let line = buf.split_to(buf.len());
                    let line = without_carriage_return(&line);

                    // Use compatibility handling function
                    let item = match try_parse_with_compatibility(line, "decode_eof")? {
                        Some(item) => item,
                        None => return Ok(None), // Skip non-standard message
                    };
                    Some(item)
                }
            }
        })
    }
}

impl<T: Serialize> Encoder<T> for JsonRpcMessageCodec<T> {
    type Error = JsonRpcMessageCodecError;

    fn encode(&mut self, item: T, buf: &mut BytesMut) -> Result<(), JsonRpcMessageCodecError> {
        serde_json::to_writer(buf.writer(), &item)?;
        buf.put_u8(b'\n');
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use futures::{Sink, Stream};

    use super::*;
    fn from_async_read<T: DeserializeOwned, R: AsyncRead>(reader: R) -> impl Stream<Item = T> {
        FramedRead::new(reader, JsonRpcMessageCodec::<T>::default()).filter_map(|result| {
            if let Err(e) = &result {
                tracing::error!("Error reading from stream: {}", e);
            }
            futures::future::ready(result.ok())
        })
    }

    fn from_async_write<T: Serialize, W: AsyncWrite + Send>(
        writer: W,
    ) -> impl Sink<T, Error = std::io::Error> {
        FramedWrite::new(writer, JsonRpcMessageCodec::<T>::default()).sink_map_err(Into::into)
    }
    #[tokio::test]
    async fn test_decode() {
        use futures::StreamExt;
        use tokio::io::BufReader;

        let data = r#"{"jsonrpc":"2.0","method":"subtract","params":[42,23],"id":1}
    {"jsonrpc":"2.0","method":"subtract","params":[23,42],"id":2}
    {"jsonrpc":"2.0","method":"subtract","params":[42,23],"id":3}
    {"jsonrpc":"2.0","method":"subtract","params":[23,42],"id":4}
    {"jsonrpc":"2.0","method":"subtract","params":[42,23],"id":5}
    {"jsonrpc":"2.0","method":"subtract","params":[23,42],"id":6}
    {"jsonrpc":"2.0","method":"subtract","params":[42,23],"id":7}
    {"jsonrpc":"2.0","method":"subtract","params":[23,42],"id":8}
    {"jsonrpc":"2.0","method":"subtract","params":[42,23],"id":9}
    {"jsonrpc":"2.0","method":"subtract","params":[23,42],"id":10}

    "#;

        let mut cursor = BufReader::new(data.as_bytes());
        let mut stream = from_async_read::<serde_json::Value, _>(&mut cursor);

        for i in 1..=10 {
            let item = stream.next().await.unwrap();
            assert_eq!(
                item,
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "subtract",
                    "params": if i % 2 != 0 { [42, 23] } else { [23, 42] },
                    "id": i,
                })
            );
        }
    }

    #[tokio::test]
    async fn test_encode() {
        let test_messages = vec![
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "subtract",
                "params": [42, 23],
                "id": 1,
            }),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "subtract",
                "params": [23, 42],
                "id": 2,
            }),
        ];

        // Create a buffer to write to
        let mut buffer = Vec::new();
        let mut writer = from_async_write(&mut buffer);

        // Write the test messages
        for message in test_messages.iter() {
            writer.send(message.clone()).await.unwrap();
        }
        writer.close().await.unwrap();
        drop(writer);
        // Parse the buffer back into lines and check each one
        let output = String::from_utf8_lossy(&buffer);
        let mut lines = output.lines();

        for expected_message in test_messages {
            let line = lines.next().unwrap();
            let parsed_message: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(parsed_message, expected_message);
        }

        // Make sure there are no extra lines
        assert!(lines.next().is_none());
    }

    #[test]
    fn test_standard_notification_check() {
        // Test that all standard notifications are recognized
        assert!(is_standard_notification("notifications/cancelled"));
        assert!(is_standard_notification("notifications/initialized"));
        assert!(is_standard_notification("notifications/progress"));
        assert!(is_standard_notification(
            "notifications/resources/list_changed"
        ));
        assert!(is_standard_notification("notifications/resources/updated"));
        assert!(is_standard_notification(
            "notifications/prompts/list_changed"
        ));
        assert!(is_standard_notification("notifications/tools/list_changed"));
        assert!(is_standard_notification("notifications/message"));
        assert!(is_standard_notification("notifications/roots/list_changed"));

        // Test that non-standard notifications are not recognized
        assert!(!is_standard_notification("notifications/stderr"));
        assert!(!is_standard_notification("notifications/custom"));
        assert!(!is_standard_notification("notifications/debug"));
        assert!(!is_standard_notification("some/other/method"));
    }

    #[test]
    fn test_compatibility_function() {
        // Test the compatibility function directly
        let stderr_message =
            r#"{"method":"notifications/stderr","params":{"content":"stderr message"}}"#;
        let custom_message = r#"{"method":"notifications/custom","params":{"data":"custom"}}"#;
        let standard_message =
            r#"{"method":"notifications/message","params":{"level":"info","data":"standard"}}"#;
        let progress_message = r#"{"method":"notifications/progress","params":{"progressToken":"token","progress":50}}"#;

        // Test with valid JSON - all should parse successfully
        let result1 =
            try_parse_with_compatibility::<serde_json::Value>(stderr_message.as_bytes(), "test");
        let result2 =
            try_parse_with_compatibility::<serde_json::Value>(custom_message.as_bytes(), "test");
        let result3 =
            try_parse_with_compatibility::<serde_json::Value>(standard_message.as_bytes(), "test");
        let result4 =
            try_parse_with_compatibility::<serde_json::Value>(progress_message.as_bytes(), "test");

        // All should parse successfully since they're valid JSON
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());
        assert!(result4.is_ok());

        // Standard notifications should return Some(value)
        assert!(result3.unwrap().is_some());
        assert!(result4.unwrap().is_some());

        println!("Standard notifications are preserved, non-standard are handled gracefully");
    }
}
