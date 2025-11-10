use std::time::Duration;

use bytes::Bytes;
use codex_otel::otel_event_manager::OtelEventManager;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time::timeout;

use crate::client::ResponseDecoder;
use crate::error::Error;
use crate::error::Result;
use crate::stream::ResponseEvent;

/// Generic SSE framer: turns a Byte stream into framed JSON and delegates to a ResponseDecoder.
#[allow(clippy::too_many_arguments)]
pub async fn process_sse<S, D>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    max_idle_duration: Duration,
    otel_event_manager: OtelEventManager,
    mut decoder: D,
) where
    S: Stream<Item = Result<Bytes>> + Send + 'static + Unpin,
    D: ResponseDecoder + Send,
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
                        // Continuation of a long data: line split across chunks; append raw.
                        data_buffer.push_str(line);
                    }

                    if line.is_empty() && !data_buffer.is_empty() {
                        // One full JSON frame ready – delegate to decoder
                        if let Err(err) = decoder
                            .on_frame(&data_buffer, &tx_event, &otel_event_manager)
                            .await
                        {
                            let _ = tx_event.send(Err(err)).await;
                            return;
                        }
                        data_buffer.clear();
                    }
                }
            }
            Ok(None) => return,
        }
    }
}
