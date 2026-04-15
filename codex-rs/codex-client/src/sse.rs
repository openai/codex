use crate::error::StreamError;
use crate::transport::ByteStream;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::timeout;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

/// Minimal SSE helper that forwards raw `data:` frames as UTF-8 strings.
///
/// Errors and idle timeouts are sent as `Err(StreamError)` before the task exits.
pub fn sse_stream(
    stream: ByteStream,
    idle_timeout: Duration,
    tx: mpsc::Sender<Result<String, StreamError>>,
) {
    spawn_sse_task(async move {
        let mut stream = stream
            .map(|res| res.map_err(|e| StreamError::Stream(e.to_string())))
            .eventsource();

        loop {
            match next_sse_event(&mut stream, idle_timeout).await {
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

#[cfg(not(target_arch = "wasm32"))]
async fn next_sse_event<S, E>(
    stream: &mut S,
    idle_timeout: Duration,
) -> Result<
    Option<Result<eventsource_stream::Event, eventsource_stream::EventStreamError<E>>>,
    tokio::time::error::Elapsed,
>
where
    S: futures::Stream<
            Item = Result<eventsource_stream::Event, eventsource_stream::EventStreamError<E>>,
        > + Unpin,
{
    timeout(idle_timeout, stream.next()).await
}

#[cfg(target_arch = "wasm32")]
async fn next_sse_event<S, E>(
    stream: &mut S,
    idle_timeout: Duration,
) -> Result<
    Option<Result<eventsource_stream::Event, eventsource_stream::EventStreamError<E>>>,
    tokio::time::error::Elapsed,
>
where
    S: futures::Stream<
            Item = Result<eventsource_stream::Event, eventsource_stream::EventStreamError<E>>,
        > + Unpin,
{
    let _ = idle_timeout;
    Ok(stream.next().await)
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_sse_task(task: impl std::future::Future<Output = ()> + Send + 'static) {
    tokio::spawn(task);
}

#[cfg(target_arch = "wasm32")]
fn spawn_sse_task(task: impl std::future::Future<Output = ()> + 'static) {
    spawn_local(task);
}
