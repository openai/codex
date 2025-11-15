use std::time::Duration;

use codex_otel::otel_event_manager::OtelEventManager;
use eventsource_stream::Event;
use eventsource_stream::EventStreamError as StreamError;
use futures::Stream;
use futures::StreamExt;
use tokio::time::timeout;

/// Result of polling the next SSE event with timeout and logging applied.
pub(crate) enum SseNext {
    Event(Event),
    Eof,
    StreamError(String),
    Timeout,
}

/// Read the next SSE event from `stream`, applying an idle timeout and recording
/// telemetry via `otel_event_manager`.
///
/// This helper centralizes the boilerplate for:
/// - `tokio::time::timeout`
/// - calling `log_sse_event`
/// - mapping the different outcomes into a small enum that callers can
///   interpret according to their own protocol semantics.
pub(crate) async fn next_sse_event<S, E>(
    stream: &mut S,
    idle_timeout: Duration,
    otel_event_manager: &OtelEventManager,
) -> SseNext
where
    S: Stream<Item = Result<Event, StreamError<E>>> + Unpin,
    E: std::fmt::Display,
{
    let start = tokio::time::Instant::now();
    let next_event = timeout(idle_timeout, stream.next()).await;
    let duration = start.elapsed();
    otel_event_manager.log_sse_event(&next_event, duration);

    match next_event {
        Ok(Some(Ok(ev))) => SseNext::Event(ev),
        Ok(Some(Err(e))) => SseNext::StreamError(e.to_string()),
        Ok(None) => SseNext::Eof,
        Err(_) => SseNext::Timeout,
    }
}
