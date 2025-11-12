use std::time::Duration;

use bytes::Bytes;
use codex_otel::otel_event_manager::OtelEventManager;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::error::Error;
use crate::error::Result;
// Legacy ResponseEvent-based SSE framer removed
use crate::stream::WireEvent;

// Legacy ResponseEvent-based SSE framer removed

/// Generic SSE framer for wire events: Byte stream -> framed JSON -> WireResponseDecoder.
#[allow(clippy::too_many_arguments)]
pub async fn process_sse_wire<S, D>(
    stream: S,
    tx_event: mpsc::Sender<Result<WireEvent>>,
    max_idle_duration: Duration,
    otel_event_manager: OtelEventManager,
    mut decoder: D,
) where
    S: Stream<Item = Result<Bytes>> + Send + 'static + Unpin,
    D: crate::client::WireResponseDecoder + Send,
{
    let mut stream = stream;
    let mut data_buffer = String::new();

    loop {
        let result = timeout(max_idle_duration, stream.next()).await;
        match result {
            Err(_) => {
                let _ = tx_event
                    .send(Err(Error::Stream(
                        "stream idle timeout fired before Completed event".to_string(),
                        None,
                    )))
                    .await;
                return;
            }
            Ok(Some(Err(err))) => {
                let _ = tx_event.send(Err(err)).await;
                return;
            }
            Ok(Some(Ok(chunk))) => {
                let chunk_str = match std::str::from_utf8(&chunk) {
                    Ok(s) => s,
                    Err(err) => {
                        let _ = tx_event
                            .send(Err(Error::Other(format!(
                                "Invalid UTF-8 in SSE chunk: {err}"
                            ))))
                            .await;
                        return;
                    }
                };

                for line in chunk_str.lines() {
                    if let Some(tail) = line.strip_prefix("data:") {
                        data_buffer.push_str(tail.trim_start());
                    } else if !line.is_empty() && !data_buffer.is_empty() {
                        data_buffer.push_str(line);
                    }

                    if line.is_empty() && !data_buffer.is_empty() {
                        let json = std::mem::take(&mut data_buffer);
                        if let Err(e) = decoder
                            .on_frame(&json, &tx_event, &otel_event_manager)
                            .await
                        {
                            let _ = tx_event.send(Err(e)).await;
                            return;
                        }
                    }
                }
            }
            Ok(None) => {
                // If the stream ended without a trailing blank line, flush any
                // buffered JSON frame to the decoder before returning.
                if !data_buffer.is_empty() {
                    let json = std::mem::take(&mut data_buffer);
                    if let Err(e) = decoder
                        .on_frame(&json, &tx_event, &otel_event_manager)
                        .await
                    {
                        let _ = tx_event.send(Err(e)).await;
                    }
                }
                return;
            }
        }
    }
}
