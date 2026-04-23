#![allow(dead_code)]
use std::{convert::Infallible, fmt::Display, sync::Arc, time::Duration};

use bytes::{Buf, Bytes};
use http::Response;
use http_body::Body;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use sse_stream::{KeepAlive, Sse, SseBody};
use tokio_util::sync::CancellationToken;

use super::http_header::EVENT_STREAM_MIME_TYPE;
use crate::model::{ClientJsonRpcMessage, ServerJsonRpcMessage};

pub type SessionId = Arc<str>;

pub fn session_id() -> SessionId {
    uuid::Uuid::new_v4().to_string().into()
}

pub const DEFAULT_AUTO_PING_INTERVAL: Duration = Duration::from_secs(15);

pub(crate) type BoxResponse = Response<BoxBody<Bytes, Infallible>>;

pub(crate) fn accepted_response() -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(http::StatusCode::ACCEPTED)
        .body(Empty::new().boxed())
        .expect("valid response")
}
pin_project_lite::pin_project! {
    struct TokioTimer {
        #[pin]
        sleep: tokio::time::Sleep,
    }
}
impl Future for TokioTimer {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.project();
        this.sleep.poll(cx)
    }
}
impl sse_stream::Timer for TokioTimer {
    fn from_duration(duration: Duration) -> Self {
        Self {
            sleep: tokio::time::sleep(duration),
        }
    }

    fn reset(self: std::pin::Pin<&mut Self>, when: std::time::Instant) {
        let this = self.project();
        this.sleep.reset(tokio::time::Instant::from_std(when));
    }
}

#[derive(Debug, Clone)]
pub struct ServerSseMessage {
    /// The event ID for this message. When set, clients can use this ID
    /// with the `Last-Event-ID` header to resume the stream from this point.
    pub event_id: Option<String>,
    /// The JSON-RPC message content. Set to `None` for priming events.
    /// See [SEP-1699](https://github.com/modelcontextprotocol/modelcontextprotocol/issues/1699)
    pub message: Option<Arc<ServerJsonRpcMessage>>,
    /// The retry interval hint for clients. Clients should wait this duration
    /// before attempting to reconnect. This maps to the SSE `retry:` field.
    pub retry: Option<Duration>,
}

pub(crate) fn sse_stream_response(
    stream: impl futures::Stream<Item = ServerSseMessage> + Send + Sync + 'static,
    keep_alive: Option<Duration>,
    ct: CancellationToken,
) -> Response<BoxBody<Bytes, Infallible>> {
    use futures::StreamExt;
    let stream = stream
        .map(|message| {
            let mut sse = if let Some(ref msg) = message.message {
                let data = serde_json::to_string(msg.as_ref()).expect("valid message");
                Sse::default().data(data)
            } else {
                // Priming event: empty data per SEP-1699 (just "data:\n")
                Sse::default().data("")
            };

            sse.id = message.event_id;

            if let Some(retry) = message.retry {
                sse.retry = Some(retry.as_millis() as u64);
            }

            Result::<Sse, Infallible>::Ok(sse)
        })
        .take_until(async move { ct.cancelled().await });
    let stream = SseBody::new(stream);

    let stream = match keep_alive {
        Some(duration) => stream
            .with_keep_alive::<TokioTimer>(KeepAlive::new().interval(duration))
            .boxed(),
        None => stream.boxed(),
    };

    Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, EVENT_STREAM_MIME_TYPE)
        .header(http::header::CACHE_CONTROL, "no-cache")
        .body(stream)
        .expect("valid response")
}

pub(crate) const fn internal_error_response<E: Display>(
    context: &str,
) -> impl FnOnce(E) -> Response<BoxBody<Bytes, Infallible>> {
    move |error| {
        tracing::error!("Internal server error when {context}: {error}");
        Response::builder()
            .status(http::StatusCode::INTERNAL_SERVER_ERROR)
            .body(
                Full::new(Bytes::from(format!(
                    "Encounter an error when {context}: {error}"
                )))
                .boxed(),
            )
            .expect("valid response")
    }
}

pub(crate) fn unexpected_message_response(expect: &str) -> Response<BoxBody<Bytes, Infallible>> {
    Response::builder()
        .status(http::StatusCode::UNPROCESSABLE_ENTITY)
        .body(Full::new(Bytes::from(format!("Unexpected message, expect {expect}"))).boxed())
        .expect("valid response")
}

pub(crate) async fn expect_json<B>(
    body: B,
) -> Result<ClientJsonRpcMessage, Response<BoxBody<Bytes, Infallible>>>
where
    B: Body + Send + 'static,
    B::Error: Display,
{
    match body.collect().await {
        Ok(bytes) => {
            match serde_json::from_reader::<_, ClientJsonRpcMessage>(bytes.aggregate().reader()) {
                Ok(message) => Ok(message),
                Err(e) => {
                    let response = Response::builder()
                        .status(http::StatusCode::UNSUPPORTED_MEDIA_TYPE)
                        .body(
                            Full::new(Bytes::from(format!("fail to deserialize request body {e}")))
                                .boxed(),
                        )
                        .expect("valid response");
                    Err(response)
                }
            }
        }
        Err(e) => {
            let response = Response::builder()
                .status(http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from(format!("Failed to read request body: {e}"))).boxed())
                .expect("valid response");
            Err(response)
        }
    }
}
