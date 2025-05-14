use std::time::Duration;

use bytes::Bytes;
use eventsource_stream::Eventsource;
use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use reqwest::StatusCode;
use serde_json::json;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;

use crate::ModelProviderInfo;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::client_common::ResponseStream;
use crate::error::CodexErr;
use crate::error::Result;
use crate::flags::OPENAI_REQUEST_MAX_RETRIES;
use crate::flags::OPENAI_STREAM_IDLE_TIMEOUT_MS;
use crate::models::ContentItem;
use crate::models::ResponseItem;
use crate::util::backoff;

/// Implementation for the classic Chat Completions API. This is intentionally
/// minimal: we only stream back plain assistant text.
pub(crate) async fn stream_chat_completions(
    prompt: &Prompt,
    model: &str,
    client: &reqwest::Client,
    provider: &ModelProviderInfo,
) -> Result<ResponseStream> {
    // Build messages array
    let mut messages = Vec::<serde_json::Value>::new();

    let full_instructions = prompt.get_full_instructions();
    messages.push(json!({"role": "system", "content": full_instructions}));

    for item in &prompt.input {
        if let ResponseItem::Message { role, content } = item {
            let mut text = String::new();
            for c in content {
                match c {
                    ContentItem::InputText { text: t } | ContentItem::OutputText { text: t } => {
                        text.push_str(t);
                    }
                    _ => {}
                }
            }
            messages.push(json!({"role": role, "content": text}));
        }
    }

    let payload = json!({
        "model": model,
        "messages": messages,
        "stream": true
    });

    let base_url = provider.base_url.trim_end_matches('/');
    let url = format!("{}/chat/completions", base_url);

    debug!(url, "POST (chat)");
    trace!("request payload: {}", payload);

    let api_key = provider.api_key()?;
    let mut attempt = 0;
    loop {
        attempt += 1;

        let mut req_builder = client.post(&url);
        if let Some(api_key) = &api_key {
            req_builder = req_builder.bearer_auth(api_key.clone());
        }
        let res = req_builder
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&payload)
            .send()
            .await;

        match res {
            Ok(resp) if resp.status().is_success() => {
                let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(16);
                let stream = resp.bytes_stream().map_err(CodexErr::Reqwest);
                tokio::spawn(process_chat_sse(stream, tx_event));
                return Ok(ResponseStream { rx_event });
            }
            Ok(res) => {
                let status = res.status();
                if !(status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) {
                    let body = (res.text().await).unwrap_or_default();
                    return Err(CodexErr::UnexpectedStatus(status, body));
                }

                if attempt > *OPENAI_REQUEST_MAX_RETRIES {
                    return Err(CodexErr::RetryLimit(status));
                }

                let retry_after_secs = res
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());

                let delay = retry_after_secs
                    .map(|s| Duration::from_millis(s * 1_000))
                    .unwrap_or_else(|| backoff(attempt));
                tokio::time::sleep(delay).await;
            }
            Err(e) => {
                if attempt > *OPENAI_REQUEST_MAX_RETRIES {
                    return Err(e.into());
                }
                let delay = backoff(attempt);
                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Lightweight SSE processor for the Chat Completions streaming format. The
/// output is mapped onto Codex's internal [`ResponseEvent`] so that the rest
/// of the pipeline can stay agnostic of the underlying wire format.
async fn process_chat_sse<S>(stream: S, tx_event: mpsc::Sender<Result<ResponseEvent>>)
where
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    let mut stream = stream.eventsource();

    let idle_timeout = *OPENAI_STREAM_IDLE_TIMEOUT_MS;

    loop {
        let sse = match timeout(idle_timeout, stream.next()).await {
            Ok(Some(Ok(ev))) => ev,
            Ok(Some(Err(e))) => {
                let _ = tx_event.send(Err(CodexErr::Stream(e.to_string()))).await;
                return;
            }
            Ok(None) => {
                // Stream closed gracefully – emit Completed with dummy id.
                let _ = tx_event
                    .send(Ok(ResponseEvent::Completed {
                        response_id: String::new(),
                    }))
                    .await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(CodexErr::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        // OpenAI Chat streaming sends a literal string "[DONE]" when finished.
        if sse.data.trim() == "[DONE]" {
            let _ = tx_event
                .send(Ok(ResponseEvent::Completed {
                    response_id: String::new(),
                }))
                .await;
            return;
        }

        // Parse JSON chunk
        let chunk: serde_json::Value = match serde_json::from_str(&sse.data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let content_opt = chunk
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("content"))
            .and_then(|c| c.as_str());

        if let Some(content) = content_opt {
            let item = ResponseItem::Message {
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: content.to_string(),
                }],
            };

            let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
        }
    }
}

/// Optional client-side aggregation helper
///
/// Stream adapter that merges the incremental `OutputItemDone` chunks coming from
/// [`process_chat_sse`] into a *running* assistant message, **suppressing the
/// per-token deltas**.  The stream stays silent while the model is thinking
/// and only emits two events per turn:
///
///   1. `ResponseEvent::OutputItemDone` with the *complete* assistant message
///      (fully concatenated).
///   2. The original `ResponseEvent::Completed` right after it.
///
/// This mirrors the behaviour the TypeScript CLI exposes to its higher layers.
///
/// The adapter is intentionally *lossless*: callers who do **not** opt in via
/// [`AggregateStreamExt::aggregate()`] keep receiving the original unmodified
/// events.
pub(crate) struct AggregatedChatStream<S> {
    inner: S,
    cumulative: String,
    pending_completed: Option<ResponseEvent>,
}

impl<S> Stream for AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    type Item = Result<ResponseEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // First, flush any buffered Completed event from the previous call.
        if let Some(ev) = this.pending_completed.take() {
            return Poll::Ready(Some(Ok(ev)));
        }

        loop {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item)))) => {
                    // Accumulate *assistant* text but do not emit yet.
                    if let crate::models::ResponseItem::Message { role, content } = &item {
                        if role == "assistant" {
                            if let Some(text) = content.iter().find_map(|c| match c {
                                crate::models::ContentItem::OutputText { text } => Some(text),
                                _ => None,
                            }) {
                                this.cumulative.push_str(text);
                            }
                        }
                    }

                    // Swallow partial event; keep polling.
                    continue;
                }
                Poll::Ready(Some(Ok(ResponseEvent::Completed { response_id }))) => {
                    if !this.cumulative.is_empty() {
                        let aggregated_item = crate::models::ResponseItem::Message {
                            role: "assistant".to_string(),
                            content: vec![crate::models::ContentItem::OutputText {
                                text: std::mem::take(&mut this.cumulative),
                            }],
                        };

                        // Buffer Completed so it is returned *after* the aggregated message.
                        this.pending_completed = Some(ResponseEvent::Completed { response_id });

                        return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(
                            aggregated_item,
                        ))));
                    }

                    // Nothing aggregated – forward Completed directly.
                    return Poll::Ready(Some(Ok(ResponseEvent::Completed { response_id })));
                } // No other `Ok` variants exist at the moment, continue polling.
            }
        }
    }
}

/// Extension trait that activates aggregation on any stream of [`ResponseEvent`].
pub(crate) trait AggregateStreamExt: Stream<Item = Result<ResponseEvent>> + Sized {
    /// Returns a new stream that emits **only** the final assistant message
    /// per turn instead of every incremental delta.  The produced
    /// `ResponseEvent` sequence for a typical text turn looks like:
    ///
    /// ```ignore
    ///     OutputItemDone(<full message>)
    ///     Completed { .. }
    /// ```
    ///
    /// No other `OutputItemDone` events will be seen by the caller.
    ///
    /// Usage:
    ///
    /// ```ignore
    /// let agg_stream = client.stream(&prompt).await?.aggregate();
    /// while let Some(event) = agg_stream.next().await {
    ///     // event now contains cumulative text
    /// }
    /// ```
    fn aggregate(self) -> AggregatedChatStream<Self> {
        AggregatedChatStream {
            inner: self,
            cumulative: String::new(),
            pending_completed: None,
        }
    }
}

impl<T> AggregateStreamExt for T where T: Stream<Item = Result<ResponseEvent>> + Sized {}
