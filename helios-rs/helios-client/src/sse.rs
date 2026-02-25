use crate::error::StreamError;
use crate::transport::ByteStream;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::timeout;

/// Minimal SSE helper that forwards raw `data:` frames as UTF-8 strings.
///
/// Errors and idle timeouts are sent as `Err(StreamError)` before the task exits.
pub fn sse_stream(
    stream: ByteStream,
    idle_timeout: Duration,
    tx: mpsc::Sender<Result<String, StreamError>>,
) {
    tokio::spawn(async move {
        let mut stream = stream
            .map(|res| res.map_err(|e| StreamError::Stream(e.to_string())))
            .eventsource();

        loop {
            match timeout(idle_timeout, stream.next()).await {
                Ok(Some(Ok(ev))) => {
                    if tx.send(Ok(ev.data.clone())).await.is_err() {
                        return;
                    }
                }
                Ok(Some(Err(e))) => {
                    let _ = tx.send(Err(StreamError::Stream(e.to_string()))).await;
                    return;
                }
                Ok(None) => {
                    let _ = tx
                        .send(Err(StreamError::Stream(
                            "stream closed before completion".into(),
                        )))
                        .await;
                    return;
                }
                Err(_) => {
                    let _ = tx.send(Err(StreamError::Timeout)).await;
                    return;
                }
            }
        }
    });
}
