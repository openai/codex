use std::time::Duration;
use std::time::Instant;

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
    let mut buffer = String::new();

    loop {
        let start = Instant::now();
        let result = timeout(max_idle_duration, stream.next()).await;
        let duration = start.elapsed();
        match result {
            Err(_) => {
                otel_event_manager.sse_event_failed(
                    None,
                    duration,
                    &"idle timeout waiting for SSE",
                );
                let _ = tx_event
                    .send(Err(Error::Stream(
                        "stream idle timeout fired before Completed event".to_string(),
                        None,
                    )))
                    .await;
                return;
            }
            Ok(Some(Err(err))) => {
                otel_event_manager.sse_event_failed(None, duration, &err);
                let _ = tx_event.send(Err(err)).await;
                return;
            }
            Ok(Some(Ok(chunk))) => {
                let chunk_str = match std::str::from_utf8(&chunk) {
                    Ok(s) => s,
                    Err(err) => {
                        otel_event_manager.sse_event_failed(
                            None,
                            duration,
                            &format!("UTF8 error: {err}"),
                        );
                        let _ = tx_event
                            .send(Err(Error::Other(format!(
                                "Invalid UTF-8 in SSE chunk: {err}"
                            ))))
                            .await;
                        return;
                    }
                }
                .replace("\r\n", "\n")
                .replace('\r', "\n");

                buffer.push_str(&chunk_str);
                while let Some(frame) = next_frame(&mut buffer) {
                    if !handle_frame(
                        frame,
                        &mut decoder,
                        &tx_event,
                        &otel_event_manager,
                        duration,
                    )
                    .await
                    {
                        return;
                    }
                }
            }
            Ok(None) => {
                if !drain_buffer(
                    &mut buffer,
                    &mut decoder,
                    &tx_event,
                    &otel_event_manager,
                    duration,
                )
                .await
                {
                    return;
                }
                return;
            }
        }
    }
}

fn next_frame(buffer: &mut String) -> Option<String> {
    loop {
        let idx = buffer.find("\n\n")?;

        let frame = buffer[..idx].to_string();
        buffer.drain(..idx + 2);

        if frame.is_empty() {
            continue;
        }

        return Some(frame);
    }
}

async fn drain_buffer<D>(
    buffer: &mut String,
    decoder: &mut D,
    tx_event: &mpsc::Sender<Result<WireEvent>>,
    otel_event_manager: &OtelEventManager,
    duration: Duration,
) -> bool
where
    D: crate::client::WireResponseDecoder + Send,
{
    while let Some(frame) = next_frame(buffer) {
        if !handle_frame(frame, decoder, tx_event, otel_event_manager, duration).await {
            return false;
        }
    }

    if buffer.is_empty() {
        return true;
    }

    let remainder = std::mem::take(buffer);
    handle_frame(remainder, decoder, tx_event, otel_event_manager, duration).await
}

async fn handle_frame<D>(
    frame: String,
    decoder: &mut D,
    tx_event: &mpsc::Sender<Result<WireEvent>>,
    otel_event_manager: &OtelEventManager,
    duration: Duration,
) -> bool
where
    D: crate::client::WireResponseDecoder + Send,
{
    if let Some(frame) = parse_sse_frame(&frame) {
        if frame.data.trim() == "[DONE]" {
            otel_event_manager.sse_event_kind(&frame.event);
            return true;
        }

        match decoder
            .on_frame(&frame.data, tx_event, otel_event_manager)
            .await
        {
            Ok(_) => {
                otel_event_manager.sse_event_kind(&frame.event);
            }
            Err(e) => {
                otel_event_manager.sse_event_failed(Some(&frame.event), duration, &e);
                let _ = tx_event.send(Err(e)).await;
                return false;
            }
        };
    }

    true
}

fn parse_sse_frame(frame: &str) -> Option<SseFrame> {
    let mut data = String::new();
    let mut event: Option<String> = None;
    let mut saw_data_line = false;

    for raw_line in frame.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("event:") {
            let trimmed = rest.trim_start();
            if !trimmed.is_empty() {
                event = Some(trimmed.to_string());
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("data:") {
            let content = rest.strip_prefix(' ').unwrap_or(rest);
            if saw_data_line {
                data.push('\n');
            }
            data.push_str(content);
            saw_data_line = true;
            continue;
        }

        if saw_data_line {
            data.push('\n');
            data.push_str(line.trim_start());
        }
    }

    if data.is_empty() && event.is_none() && !saw_data_line {
        return None;
    }

    Some(SseFrame {
        event: event.unwrap_or_else(|| "message".to_string()),
        data,
    })
}

struct SseFrame {
    event: String,
    data: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::ConversationId;
    use futures::stream;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::fmt::Write as _;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn apply_patch_body_handles_coalesced_and_split_chunks() {
        let events = apply_patch_events();
        let chunk_variants = vec![
            vec![sse(events.clone())],
            vec![sse(events[..2].to_vec()), sse(events[2..].to_vec())],
        ];

        for chunks in chunk_variants {
            let events = collect_events(chunks).await;
            assert_eq!(
                events,
                vec![
                    "created",
                    "response.output_item.done",
                    "response.output_item.added",
                    "response.completed"
                ]
            );
        }
    }

    #[tokio::test]
    async fn multiple_events_in_single_chunk_emit_done() {
        let chunk = sse(vec![
            event_output_item_done("call-inline"),
            event_completed("resp-inline"),
        ]);
        let events = collect_events(vec![chunk]).await;
        assert_eq!(
            events,
            vec!["response.output_item.done", "response.completed",]
        );
    }

    async fn collect_events(chunks: Vec<String>) -> Vec<String> {
        let (tx_event, mut rx_event) = mpsc::channel::<Result<WireEvent>>(16);
        let stream = stream::iter(chunks.into_iter().map(|chunk| Ok(Bytes::from(chunk))));
        let otel_event_manager = OtelEventManager::new(
            ConversationId::new(),
            "test-model",
            "test-slug",
            None,
            None,
            None,
            false,
            "terminal".to_string(),
        );

        let handle = tokio::spawn(process_sse_wire(
            stream,
            tx_event,
            Duration::from_secs(5),
            otel_event_manager,
            crate::decode_wire::responses::WireResponsesSseDecoder,
        ));

        let mut out = Vec::new();
        while let Some(event) = rx_event.recv().await {
            let event = event.expect("event decoding should succeed");
            out.push(event_name(&event));
        }
        handle
            .await
            .expect("SSE framing task should complete without panicking");
        out
    }

    fn event_name(event: &WireEvent) -> String {
        match event {
            WireEvent::Created => "created",
            WireEvent::OutputItemDone(_) => "response.output_item.done",
            WireEvent::OutputItemAdded(_) => "response.output_item.added",
            WireEvent::Completed { .. } => "response.completed",
            WireEvent::OutputTextDelta(_) => "response.output_text.delta",
            WireEvent::ReasoningSummaryDelta(_) => "response.reasoning_summary_text.delta",
            WireEvent::ReasoningContentDelta(_) => "response.reasoning_text.delta",
            WireEvent::ReasoningSummaryPartAdded => "response.reasoning_summary_part.added",
            WireEvent::RateLimits(_) => "response.rate_limits",
        }
        .to_string()
    }

    fn apply_patch_events() -> Vec<serde_json::Value> {
        vec![
            json!({
                "type": "response.created",
                "response": { "id": "resp-apply-patch" }
            }),
            event_output_item_done("apply-patch-call"),
            json!({
                "type": "response.output_item.added",
                "item": {
                    "type": "message",
                    "role": "assistant",
                    "content": [{"type": "output_text", "text": "ok"}]
                }
            }),
            event_completed("resp-apply-patch"),
        ]
    }

    fn event_output_item_done(call_id: &str) -> serde_json::Value {
        json!({
            "type": "response.output_item.done",
            "item": {
                "type": "function_call",
                "name": "apply_patch",
                "arguments": "{\"input\":\"*** Begin Patch\\n*** End Patch\"}",
                "call_id": call_id
            }
        })
    }

    fn event_completed(id: &str) -> serde_json::Value {
        json!({
            "type": "response.completed",
            "response": {
                "id": id,
                "usage": {
                    "input_tokens": 0,
                    "input_tokens_details": null,
                    "output_tokens": 0,
                    "output_tokens_details": null,
                    "reasoning_output_tokens": 0,
                    "total_tokens": 0
                }
            }
        })
    }

    fn sse(events: Vec<serde_json::Value>) -> String {
        let mut out = String::new();
        for ev in events {
            let kind = ev.get("type").and_then(|v| v.as_str()).unwrap_or_default();
            writeln!(&mut out, "event: {kind}").unwrap();
            if !ev.as_object().map(|o| o.len() == 1).unwrap_or(false) {
                write!(&mut out, "data: {ev}\n\n").unwrap();
            } else {
                out.push('\n');
            }
        }
        out
    }
}
