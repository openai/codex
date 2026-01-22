use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::telemetry::SseTelemetry;
use codex_client::StreamResponse;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use eventsource_stream::Eventsource;
use futures::Stream;
use futures::StreamExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;

pub(crate) fn spawn_chat_stream(
    stream_response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
    _turn_state: Option<Arc<OnceLock<String>>>,
) -> ResponseStream {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);
    tokio::spawn(async move {
        process_chat_sse(stream_response.bytes, tx_event, idle_timeout, telemetry).await;
    });
    ResponseStream { rx_event }
}

/// Processes Server-Sent Events from the legacy Chat Completions streaming API.
///
/// The upstream protocol terminates a streaming response with a final sentinel event
/// (`data: [DONE]`). Historically, some of our test stubs have emitted `data: DONE`
/// (without brackets) instead.
///
/// `eventsource_stream` delivers these sentinels as regular events rather than signaling
/// end-of-stream. If we try to parse them as JSON, we log and skip them, then keep
/// polling for more events.
///
/// On servers that keep the HTTP connection open after emitting the sentinel (notably
/// wiremock on Windows), skipping the sentinel means we never emit `ResponseEvent::Completed`.
/// Higher-level workflows/tests that wait for completion before issuing subsequent model
/// calls will then stall, which shows up as "expected N requests, got 1" verification
/// failures in the mock server.
pub async fn process_chat_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<std::sync::Arc<dyn SseTelemetry>>,
) where
    S: Stream<Item = Result<bytes::Bytes, codex_client::TransportError>> + Unpin,
{
    let mut stream = stream.eventsource();

    #[derive(Default, Debug)]
    struct ToolCallState {
        id: Option<String>,
        name: Option<String>,
        arguments: String,
    }

    let mut tool_calls: HashMap<usize, ToolCallState> = HashMap::new();
    let mut tool_call_order: Vec<usize> = Vec::new();
    let mut tool_call_order_seen: HashSet<usize> = HashSet::new();
    let mut tool_call_index_by_id: HashMap<String, usize> = HashMap::new();
    let mut next_tool_call_index = 0usize;
    let mut last_tool_call_index: Option<usize> = None;
    let mut assistant_item: Option<ResponseItem> = None;
    let mut reasoning_item: Option<ResponseItem> = None;
    let mut completed_sent = false;

    async fn flush_and_complete(
        tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
        reasoning_item: &mut Option<ResponseItem>,
        assistant_item: &mut Option<ResponseItem>,
    ) {
        if let Some(reasoning) = reasoning_item.take() {
            let _ = tx_event
                .send(Ok(ResponseEvent::OutputItemDone(reasoning)))
                .await;
        }

        if let Some(assistant) = assistant_item.take() {
            let _ = tx_event
                .send(Ok(ResponseEvent::OutputItemDone(assistant)))
                .await;
        }

        let _ = tx_event
            .send(Ok(ResponseEvent::Completed {
                response_id: String::new(),
                token_usage: None,
            }))
            .await;
    }

    loop {
        let start = Instant::now();
        let response = timeout(idle_timeout, stream.next()).await;
        if let Some(t) = telemetry.as_ref() {
            t.on_sse_poll(&response, start.elapsed());
        }
        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                let _ = tx_event.send(Err(ApiError::Stream(e.to_string()))).await;
                return;
            }
            Ok(None) => {
                if !completed_sent {
                    flush_and_complete(&tx_event, &mut reasoning_item, &mut assistant_item).await;
                }
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        trace!("SSE event: {}", sse.data);

        let data = sse.data.trim();

        if data.is_empty() {
            continue;
        }

        if data == "[DONE]" || data == "DONE" {
            if !completed_sent {
                flush_and_complete(&tx_event, &mut reasoning_item, &mut assistant_item).await;
            }
            return;
        }

        let value: serde_json::Value = match serde_json::from_str(data) {
            Ok(val) => val,
            Err(err) => {
                debug!(
                    "Failed to parse ChatCompletions SSE event: {err}, data: {}",
                    data
                );
                continue;
            }
        };

        let Some(choices) = value.get("choices").and_then(|c| c.as_array()) else {
            continue;
        };

        for choice in choices {
            if let Some(delta) = choice.get("delta") {
                if let Some(reasoning) = delta.get("reasoning") {
                    if let Some(text) = reasoning.as_str() {
                        append_reasoning_text(&tx_event, &mut reasoning_item, text.to_string())
                            .await;
                    } else if let Some(text) = reasoning.get("text").and_then(|v| v.as_str()) {
                        append_reasoning_text(&tx_event, &mut reasoning_item, text.to_string())
                            .await;
                    } else if let Some(text) = reasoning.get("content").and_then(|v| v.as_str()) {
                        append_reasoning_text(&tx_event, &mut reasoning_item, text.to_string())
                            .await;
                    }
                }

                if let Some(content) = delta.get("content") {
                    if content.is_array() {
                        for item in content.as_array().unwrap_or(&vec![]) {
                            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                                append_assistant_text(
                                    &tx_event,
                                    &mut assistant_item,
                                    text.to_string(),
                                )
                                .await;
                            }
                        }
                    } else if let Some(text) = content.as_str() {
                        append_assistant_text(&tx_event, &mut assistant_item, text.to_string())
                            .await;
                    }
                }

                if let Some(tool_call_values) = delta.get("tool_calls").and_then(|c| c.as_array()) {
                    for tool_call in tool_call_values {
                        let mut index = tool_call
                            .get("index")
                            .and_then(serde_json::Value::as_u64)
                            .map(|i| i as usize);

                        let mut call_id_for_lookup = None;
                        if let Some(call_id) = tool_call.get("id").and_then(|i| i.as_str()) {
                            call_id_for_lookup = Some(call_id.to_string());
                            if let Some(existing) = tool_call_index_by_id.get(call_id) {
                                index = Some(*existing);
                            }
                        }

                        if index.is_none() && call_id_for_lookup.is_none() {
                            index = last_tool_call_index;
                        }

                        let index = index.unwrap_or_else(|| {
                            while tool_calls.contains_key(&next_tool_call_index) {
                                next_tool_call_index += 1;
                            }
                            let idx = next_tool_call_index;
                            next_tool_call_index += 1;
                            idx
                        });

                        let call_state = tool_calls.entry(index).or_default();
                        if tool_call_order_seen.insert(index) {
                            tool_call_order.push(index);
                        }

                        if let Some(id) = tool_call.get("id").and_then(|i| i.as_str()) {
                            call_state.id.get_or_insert_with(|| id.to_string());
                            tool_call_index_by_id.entry(id.to_string()).or_insert(index);
                        }

                        if let Some(func) = tool_call.get("function") {
                            if let Some(fname) = func.get("name").and_then(|n| n.as_str())
                                && !fname.is_empty()
                            {
                                call_state.name.get_or_insert_with(|| fname.to_string());
                            }
                            if let Some(arguments) = func.get("arguments").and_then(|a| a.as_str())
                            {
                                call_state.arguments.push_str(arguments);
                            }
                        }

                        last_tool_call_index = Some(index);
                    }
                }
            }

            if let Some(message) = choice.get("message")
                && let Some(reasoning) = message.get("reasoning")
            {
                if let Some(text) = reasoning.as_str() {
                    append_reasoning_text(&tx_event, &mut reasoning_item, text.to_string()).await;
                } else if let Some(text) = reasoning.get("text").and_then(|v| v.as_str()) {
                    append_reasoning_text(&tx_event, &mut reasoning_item, text.to_string()).await;
                } else if let Some(text) = reasoning.get("content").and_then(|v| v.as_str()) {
                    append_reasoning_text(&tx_event, &mut reasoning_item, text.to_string()).await;
                }
            }

            let finish_reason = choice.get("finish_reason").and_then(|r| r.as_str());
            if finish_reason == Some("stop") {
                if let Some(reasoning) = reasoning_item.take() {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(reasoning)))
                        .await;
                }

                if let Some(assistant) = assistant_item.take() {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(assistant)))
                        .await;
                }
                if !completed_sent {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::Completed {
                            response_id: String::new(),
                            token_usage: None,
                        }))
                        .await;
                    completed_sent = true;
                }
                continue;
            }

            if finish_reason == Some("length") {
                let _ = tx_event.send(Err(ApiError::ContextWindowExceeded)).await;
                return;
            }

            if finish_reason == Some("tool_calls") {
                if let Some(reasoning) = reasoning_item.take() {
                    let _ = tx_event
                        .send(Ok(ResponseEvent::OutputItemDone(reasoning)))
                        .await;
                }

                for index in tool_call_order.drain(..) {
                    let Some(state) = tool_calls.remove(&index) else {
                        continue;
                    };
                    tool_call_order_seen.remove(&index);
                    let ToolCallState {
                        id,
                        name,
                        arguments,
                    } = state;
                    let Some(name) = name else {
                        debug!("Skipping tool call at index {index} because name is missing");
                        continue;
                    };
                    let item = ResponseItem::FunctionCall {
                        id: None,
                        name,
                        arguments,
                        call_id: id.unwrap_or_else(|| format!("tool-call-{index}")),
                    };
                    let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                }
            }
        }
    }
}

async fn append_assistant_text(
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    assistant_item: &mut Option<ResponseItem>,
    text: String,
) {
    if assistant_item.is_none() {
        let item = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![],
            end_turn: None,
        };
        *assistant_item = Some(item.clone());
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputItemAdded(item)))
            .await;
    }

    if let Some(ResponseItem::Message { content, .. }) = assistant_item {
        content.push(ContentItem::OutputText { text: text.clone() });
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputTextDelta(text.clone())))
            .await;
    }
}

async fn append_reasoning_text(
    tx_event: &mpsc::Sender<Result<ResponseEvent, ApiError>>,
    reasoning_item: &mut Option<ResponseItem>,
    text: String,
) {
    if reasoning_item.is_none() {
        let item = ResponseItem::Reasoning {
            id: String::new(),
            summary: Vec::new(),
            content: Some(vec![]),
            encrypted_content: None,
        };
        *reasoning_item = Some(item.clone());
        let _ = tx_event
            .send(Ok(ResponseEvent::OutputItemAdded(item)))
            .await;
    }

    if let Some(ResponseItem::Reasoning {
        content: Some(content),
        ..
    }) = reasoning_item
    {
        let content_index = content.len() as i64;
        content.push(ReasoningItemContent::ReasoningText { text: text.clone() });

        let _ = tx_event
            .send(Ok(ResponseEvent::ReasoningContentDelta {
                delta: text.clone(),
                content_index,
            }))
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use codex_protocol::models::ResponseItem;
    use futures::TryStreamExt;
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_util::io::ReaderStream;

    fn build_body(events: &[serde_json::Value]) -> String {
        let mut body = String::new();
        for e in events {
            body.push_str(&format!("event: message\ndata: {e}\n\n"));
        }
        body
    }

    /// Regression test: the stream should complete when we see a `[DONE]` sentinel.
    ///
    /// This is important for tests/mocks that don't immediately close the underlying
    /// connection after emitting the sentinel.
    #[tokio::test]
    async fn completes_on_done_sentinel_without_json() {
        let events = collect_events("event: message\ndata: [DONE]\n\n").await;
        assert_matches!(&events[..], [ResponseEvent::Completed { .. }]);
    }

    async fn collect_events(body: &str) -> Vec<ResponseEvent> {
        let reader = ReaderStream::new(std::io::Cursor::new(body.to_string()))
            .map_err(|err| codex_client::TransportError::Network(err.to_string()));
        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent, ApiError>>(16);
        tokio::spawn(process_chat_sse(
            reader,
            tx,
            Duration::from_millis(1000),
            None,
        ));

        let mut out = Vec::new();
        while let Some(ev) = rx.recv().await {
            out.push(ev.expect("stream error"));
        }
        out
    }

    #[tokio::test]
    async fn concatenates_tool_call_arguments_across_deltas() {
        let delta_name = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": "call_a",
                        "index": 0,
                        "function": { "name": "do_a" }
                    }]
                }
            }]
        });

        let delta_args_1 = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": { "arguments": "{ \"foo\":" }
                    }]
                }
            }]
        });

        let delta_args_2 = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": { "arguments": "1}" }
                    }]
                }
            }]
        });

        let finish = json!({
            "choices": [{
                "finish_reason": "tool_calls"
            }]
        });

        let body = build_body(&[delta_name, delta_args_1, delta_args_2, finish]);
        let events = collect_events(&body).await;
        assert_matches!(
            &events[..],
            [
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id, name, arguments, .. }),
                ResponseEvent::Completed { .. }
            ] if call_id == "call_a" && name == "do_a" && arguments == "{ \"foo\":1}"
        );
    }

    #[tokio::test]
    async fn emits_multiple_tool_calls() {
        let delta_a = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": "call_a",
                        "function": { "name": "do_a", "arguments": "{\"foo\":1}" }
                    }]
                }
            }]
        });

        let delta_b = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": "call_b",
                        "function": { "name": "do_b", "arguments": "{\"bar\":2}" }
                    }]
                }
            }]
        });

        let finish = json!({
            "choices": [{
                "finish_reason": "tool_calls"
            }]
        });

        let body = build_body(&[delta_a, delta_b, finish]);
        let events = collect_events(&body).await;
        assert_matches!(
            &events[..],
            [
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id: call_a, name: name_a, arguments: args_a, .. }),
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id: call_b, name: name_b, arguments: args_b, .. }),
                ResponseEvent::Completed { .. }
            ] if call_a == "call_a" && name_a == "do_a" && args_a == "{\"foo\":1}" && call_b == "call_b" && name_b == "do_b" && args_b == "{\"bar\":2}"
        );
    }

    #[tokio::test]
    async fn emits_tool_calls_for_multiple_choices() {
        let payload = json!({
            "choices": [
                {
                    "delta": {
                        "tool_calls": [{
                            "id": "call_a",
                            "index": 0,
                            "function": { "name": "do_a", "arguments": "{}" }
                        }]
                    },
                    "finish_reason": "tool_calls"
                },
                {
                    "delta": {
                        "tool_calls": [{
                            "id": "call_b",
                            "index": 0,
                            "function": { "name": "do_b", "arguments": "{}" }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }
            ]
        });

        let body = build_body(&[payload]);
        let events = collect_events(&body).await;
        assert_matches!(
            &events[..],
            [
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id: call_a, name: name_a, arguments: args_a, .. }),
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id: call_b, name: name_b, arguments: args_b, .. }),
                ResponseEvent::Completed { .. }
            ] if call_a == "call_a" && name_a == "do_a" && args_a == "{}" && call_b == "call_b" && name_b == "do_b" && args_b == "{}"
        );
    }

    #[tokio::test]
    async fn merges_tool_calls_by_index_when_id_missing_on_subsequent_deltas() {
        let delta_with_id = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_a",
                        "function": { "name": "do_a", "arguments": "{ \"foo\":" }
                    }]
                }
            }]
        });

        let delta_without_id = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "function": { "arguments": "1}" }
                    }]
                }
            }]
        });

        let finish = json!({
            "choices": [{
                "finish_reason": "tool_calls"
            }]
        });

        let body = build_body(&[delta_with_id, delta_without_id, finish]);
        let events = collect_events(&body).await;
        assert_matches!(
            &events[..],
            [
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id, name, arguments, .. }),
                ResponseEvent::Completed { .. }
            ] if call_id == "call_a" && name == "do_a" && arguments == "{ \"foo\":1}"
        );
    }

    #[tokio::test]
    async fn preserves_tool_call_name_when_empty_deltas_arrive() {
        let delta_with_name = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": "call_a",
                        "function": { "name": "do_a" }
                    }]
                }
            }]
        });

        let delta_with_empty_name = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": "call_a",
                        "function": { "name": "", "arguments": "{}" }
                    }]
                }
            }]
        });

        let finish = json!({
            "choices": [{
                "finish_reason": "tool_calls"
            }]
        });

        let body = build_body(&[delta_with_name, delta_with_empty_name, finish]);
        let events = collect_events(&body).await;
        assert_matches!(
            &events[..],
            [
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { name, arguments, .. }),
                ResponseEvent::Completed { .. }
            ] if name == "do_a" && arguments == "{}"
        );
    }

    #[tokio::test]
    async fn emits_tool_calls_even_when_content_and_reasoning_present() {
        let delta_content_and_tools = json!({
            "choices": [{
                "delta": {
                    "content": [{"text": "hi"}],
                    "reasoning": "because",
                    "tool_calls": [{
                        "id": "call_a",
                        "function": { "name": "do_a", "arguments": "{}" }
                    }]
                }
            }]
        });

        let finish = json!({
            "choices": [{
                "finish_reason": "tool_calls"
            }]
        });

        let body = build_body(&[delta_content_and_tools, finish]);
        let events = collect_events(&body).await;

        assert_matches!(
            &events[..],
            [
                ResponseEvent::OutputItemAdded(ResponseItem::Reasoning { .. }),
                ResponseEvent::ReasoningContentDelta { .. },
                ResponseEvent::OutputItemAdded(ResponseItem::Message { .. }),
                ResponseEvent::OutputTextDelta(delta),
                ResponseEvent::OutputItemDone(ResponseItem::Reasoning { .. }),
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id, name, .. }),
                ResponseEvent::OutputItemDone(ResponseItem::Message { .. }),
                ResponseEvent::Completed { .. }
            ] if delta == "hi" && call_id == "call_a" && name == "do_a"
        );
    }

    #[tokio::test]
    async fn drops_partial_tool_calls_on_stop_finish_reason() {
        let delta_tool = json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "id": "call_a",
                        "function": { "name": "do_a", "arguments": "{}" }
                    }]
                }
            }]
        });

        let finish_stop = json!({
            "choices": [{
                "finish_reason": "stop"
            }]
        });

        let body = build_body(&[delta_tool, finish_stop]);
        let events = collect_events(&body).await;

        assert!(!events.iter().any(|ev| {
            matches!(
                ev,
                ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { .. })
            )
        }));
        assert_matches!(events.last(), Some(ResponseEvent::Completed { .. }));
    }
}
