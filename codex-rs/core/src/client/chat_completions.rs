use std::time::Duration;

use crate::ModelProviderInfo;
use crate::client::ResponseEvent;
use crate::client::ResponseStream;
use crate::client::http::CodexHttpClient;
use crate::client_common::Prompt;
use crate::error::CodexErr;
use crate::error::ConnectionFailedError;
use crate::error::ResponseStreamFailed;
use crate::error::Result;
use crate::error::UnexpectedResponseError;
use crate::model_family::ModelFamily;
use crate::tools::spec::create_tools_json_for_chat_completions_api;
use crate::util::backoff;
use bytes::Bytes;
use codex_otel::otel_event_manager::OtelEventManager;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
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

/// Implementation for the classic Chat Completions API.
pub(crate) async fn stream_chat_completions(
    prompt: &Prompt,
    model_family: &ModelFamily,
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    otel_event_manager: &OtelEventManager,
    session_source: &SessionSource,
) -> Result<ResponseStream> {
    if prompt.output_schema.is_some() {
        return Err(CodexErr::UnsupportedOperation(
            "output_schema is not supported for Chat Completions API".to_string(),
        ));
    }

    // Build messages array
    let mut messages = Vec::<serde_json::Value>::new();

    let full_instructions = prompt.get_full_instructions(model_family);
    messages.push(json!({ "role": "system", "content": full_instructions }));

    let input = prompt.get_formatted_input();

    // Pre-scan: map Reasoning blocks to the adjacent assistant anchor after the last user.
    // - If the last emitted message is a user message, drop all reasoning.
    // - Otherwise, for each Reasoning item after the last user message, attach it
    //   to the immediate previous assistant message (stop turns) or the immediate
    //   next assistant anchor (tool-call turns: function/local shell call, or assistant message).
    let mut reasoning_by_anchor_index: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();

    // Determine the last role that would be emitted to Chat Completions.
    let mut last_emitted_role: Option<&str> = None;
    for item in &input {
        match item {
            ResponseItem::Message { role, .. } => last_emitted_role = Some(role.as_str()),
            ResponseItem::FunctionCall { .. } | ResponseItem::LocalShellCall { .. } => {
                last_emitted_role = Some("assistant")
            }
            ResponseItem::FunctionCallOutput { .. } => last_emitted_role = Some("tool"),
            ResponseItem::Reasoning { .. } | ResponseItem::Other => {}
            ResponseItem::CustomToolCall { .. } => {}
            ResponseItem::CustomToolCallOutput { .. } => {}
            ResponseItem::WebSearchCall { .. } => {}
            ResponseItem::GhostSnapshot { .. } => {}
        }
    }

    // Find the last user message index in the input.
    let mut last_user_index: Option<usize> = None;
    for (idx, item) in input.iter().enumerate() {
        if let ResponseItem::Message { role, .. } = item
            && role == "user"
        {
            last_user_index = Some(idx);
        }
    }

    // Attach reasoning only if the conversation does not end with a user message.
    if !matches!(last_emitted_role, Some("user")) {
        for (idx, item) in input.iter().enumerate() {
            // Only consider reasoning that appears after the last user message.
            if let Some(u_idx) = last_user_index
                && idx <= u_idx
            {
                continue;
            }

            if let ResponseItem::Reasoning {
                content: Some(items),
                ..
            } = item
            {
                let mut text = String::new();
                for entry in items {
                    match entry {
                        ReasoningItemContent::ReasoningText { text: segment }
                        | ReasoningItemContent::Text { text: segment } => text.push_str(segment),
                    }
                }
                if text.trim().is_empty() {
                    continue;
                }

                // Prefer immediate previous assistant message (stop turns)
                let mut attached = false;
                if idx > 0
                    && let ResponseItem::Message { role, .. } = &input[idx - 1]
                    && role == "assistant"
                {
                    reasoning_by_anchor_index
                        .entry(idx - 1)
                        .and_modify(|existing| existing.push_str(text.as_str()))
                        .or_insert(text.clone());
                    attached = true;
                }

                // Otherwise, attach to the first future assistant anchor.
                if !attached {
                    for anchor_idx in idx + 1..input.len() {
                        match &input[anchor_idx] {
                            ResponseItem::Message { role, .. } if role == "assistant" => {
                                reasoning_by_anchor_index
                                    .entry(anchor_idx)
                                    .and_modify(|existing| existing.push_str(text.as_str()))
                                    .or_insert(text.clone());
                                attached = true;
                                break;
                            }
                            ResponseItem::FunctionCall { .. }
                            | ResponseItem::LocalShellCall { .. }
                            | ResponseItem::FunctionCallOutput { .. } => {
                                continue;
                            }
                            _ => break,
                        }
                    }
                }

                // Either attached or dropped, move on.
            }
        }
    }

    for (index, item) in input.iter().enumerate() {
        match item {
            ResponseItem::Message { role, content, .. } => {
                let mut content_text = String::new();
                for item in content {
                    match item {
                        ContentItem::InputText { text } | ContentItem::OutputText { text } => {
                            content_text.push_str(text)
                        }
                        ContentItem::InputImage { .. } => {}
                    }
                }
                if content_text.trim().is_empty() {
                    continue;
                }

                // Append reasoning when mapped to this anchor.
                if let Some(reasoning) = reasoning_by_anchor_index.remove(&index) {
                    content_text.push_str(reasoning.as_str());
                }

                messages.push(json!({
                    "role": role,
                    "content": content_text,
                }));
            }

            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            } => {
                messages.push(json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments,
                        }
                    }],
                }));
            }

            ResponseItem::FunctionCallOutput { call_id, output } => {
                // Prefer structured content items when available (e.g., images)
                // otherwise fall back to the legacy plain-string content.
                let content_value = if let Some(items) = &output.content_items {
                    let mapped: Vec<serde_json::Value> = items
                        .iter()
                        .map(|it| match it {
                            FunctionCallOutputContentItem::InputText { text } => {
                                json!({"type":"text","text": text})
                            }
                            FunctionCallOutputContentItem::InputImage { image_url } => {
                                json!({"type":"image_url","image_url": {"url": image_url}})
                            }
                        })
                        .collect();
                    json!(mapped)
                } else {
                    json!(output.content)
                };

                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": content_value,
                }));

                if let Some(reasoning) = reasoning_by_anchor_index.remove(&index) {
                    messages.push(json!({
                        "role": "assistant",
                        "content": reasoning,
                    }));
                }
            }

            ResponseItem::LocalShellCall { call_id, .. } => {
                messages.push(json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": "shell",
                            "arguments": "{}", // arguments are defined via `input` only.
                        }
                    }],
                }));
            }

            ResponseItem::CustomToolCall { call_id, name, .. } => {
                messages.push(json!({
                    "role": "assistant",
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": "{}", // arguments are defined via `input` only.
                        }
                    }],
                }));
            }

            ResponseItem::CustomToolCallOutput { call_id, output } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": output,
                }));
            }

            ResponseItem::Reasoning { .. } => {
                // Reasoning is mapped onto adjacent assistant anchors above.
            }

            ResponseItem::WebSearchCall { .. } => {}

            ResponseItem::Other { .. } => {}

            ResponseItem::GhostSnapshot { .. } => {}
        }
    }

    // Attach any reasoning still not mapped (e.g., if the last input items are Reasoning).
    if messages.len() == 1 {
        if let Some(text) = reasoning_by_anchor_index.remove(&0) {
            messages.push(json!({
                "role": "assistant",
                "content": text,
            }));
        }
    }

    let tools_json = create_tools_json_for_chat_completions_api(&prompt.tools)?;

    let mut body = json!({
        "model": model_family.slug,
        "messages": messages,
        "stream": true,
        "stream_options": {
            "include_usage": true,
        },
    });

    if !tools_json.is_empty() {
        body["tools"] = json!(tools_json);
        body["tool_choice"] = json!("auto");
    }

    if let SessionSource::SubAgent(sub) = session_source {
        let subagent = if let SubAgentSource::Other(label) = sub {
            label.clone()
        } else {
            serde_json::to_value(sub)
                .ok()
                .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                .unwrap_or_else(|| "other".to_string())
        };
        body["metadata"] = json!({
            "x-openai-subagent": subagent,
        });
    }

    let max_attempts = provider.request_max_retries();
    let mut last_error = None;
    for attempt in 0..=max_attempts {
        match stream_single_chat_completion(
            attempt,
            client,
            provider,
            otel_event_manager,
            body.clone(),
        )
        .await
        {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                last_error = Some(e);
                if attempt != max_attempts {
                    tokio::time::sleep(backoff(attempt)).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or(CodexErr::InternalServerError))
}

async fn stream_single_chat_completion(
    attempt: u64,
    client: &CodexHttpClient,
    provider: &ModelProviderInfo,
    otel_event_manager: &OtelEventManager,
    body: serde_json::Value,
) -> Result<ResponseStream> {
    trace!(
        "POST to {}: {}",
        provider.get_full_url(&None),
        body.to_string()
    );

    let mut req_builder = provider.create_request_builder(client, &None).await?;
    req_builder = req_builder
        .header(reqwest::header::ACCEPT, "text/event-stream")
        .json(&body);

    let res = otel_event_manager
        .log_request(attempt, || req_builder.send())
        .await;

    let mut request_id = None;
    if let Ok(resp) = &res {
        request_id = resp
            .headers()
            .get("cf-ray")
            .map(|v| v.to_str().unwrap_or_default().to_string());
    }

    match res {
        Ok(resp) if resp.status().is_success() => {
            let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

            // spawn task to process SSE
            let stream = resp.bytes_stream().map_err(move |e| {
                CodexErr::ResponseStreamFailed(ResponseStreamFailed {
                    source: e,
                    request_id: request_id.clone(),
                })
            });
            tokio::spawn(process_chat_sse(
                stream,
                tx_event,
                provider.stream_idle_timeout(),
                otel_event_manager.clone(),
            ));

            Ok(ResponseStream { rx_event })
        }
        Ok(res) => {
            let status = res.status();

            if !(status == StatusCode::TOO_MANY_REQUESTS
                || status == StatusCode::UNAUTHORIZED
                || status.is_server_error())
            {
                // Surface the error body to callers. Use `unwrap_or_default` per Clippy.
                let body = res.text().await.unwrap_or_default();
                return Err(CodexErr::UnexpectedStatus(UnexpectedResponseError {
                    status,
                    body,
                    request_id: None,
                }));
            }

            Err(CodexErr::UnexpectedStatus(UnexpectedResponseError {
                status,
                body: String::new(),
                request_id,
            }))
        }
        Err(e) => Err(CodexErr::ConnectionFailed(ConnectionFailedError {
            source: e,
        })),
    }
}

async fn append_assistant_text(
    tx_event: &mpsc::Sender<Result<ResponseEvent>>,
    assistant_item: &mut Option<ResponseItem>,
    text: String,
) {
    if assistant_item.is_none() {
        let item = ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![],
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
    tx_event: &mpsc::Sender<Result<ResponseEvent>>,
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
        content.push(ReasoningItemContent::ReasoningText { text: text.clone() });

        let _ = tx_event
            .send(Ok(ResponseEvent::ReasoningContentDelta(text.clone())))
            .await;
    }
}

async fn process_chat_sse<S>(
    stream: S,
    tx_event: mpsc::Sender<Result<ResponseEvent>>,
    idle_timeout: Duration,
    otel_event_manager: OtelEventManager,
) where
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    let mut stream = stream.eventsource();

    // State to accumulate a function call across streaming chunks.
    // OpenAI may split the `arguments` string over multiple `delta` events
    // until the chunk whose `finish_reason` is `tool_calls` is emitted. We
    // keep collecting the pieces here and forward a single
    // `ResponseItem::FunctionCall` once the call is complete.
    #[derive(Default)]
    struct FunctionCallState {
        name: Option<String>,
        arguments: String,
        call_id: Option<String>,
        active: bool,
    }

    let mut fn_call_state = FunctionCallState::default();
    let mut assistant_item: Option<ResponseItem> = None;
    let mut reasoning_item: Option<ResponseItem> = None;

    loop {
        let start = std::time::Instant::now();
        let response = timeout(idle_timeout, stream.next()).await;
        let duration = start.elapsed();
        otel_event_manager.log_sse_event(&response, duration);

        let sse = match response {
            Ok(Some(Ok(ev))) => ev,
            Ok(Some(Err(e))) => {
                let _ = tx_event
                    .send(Err(CodexErr::Stream(e.to_string(), None)))
                    .await;
                return;
            }
            Ok(None) => {
                // Stream closed gracefully – emit Completed with dummy id.
                let _ = tx_event
                    .send(Ok(ResponseEvent::Completed {
                        response_id: String::new(),
                        token_usage: None,
                    }))
                    .await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(CodexErr::Stream(
                        "idle timeout waiting for SSE".into(),
                        None,
                    )))
                    .await;
                return;
            }
        };

        // OpenAI Chat streaming sends a literal string "[DONE]" when finished.
        if sse.data.trim() == "[DONE]" {
            // Emit any finalized items before closing so downstream consumers receive
            // terminal events for both assistant content and raw reasoning.
            if let Some(item) = assistant_item {
                let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
            }

            if let Some(item) = reasoning_item {
                let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
            }

            let _ = tx_event
                .send(Ok(ResponseEvent::Completed {
                    response_id: String::new(),
                    token_usage: None,
                }))
                .await;
            return;
        }

        // Parse JSON chunk
        let chunk: serde_json::Value = match serde_json::from_str(&sse.data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        trace!("chat_completions received SSE chunk: {chunk:?}");

        let choice_opt = chunk.get("choices").and_then(|c| c.get(0));

        if let Some(choice) = choice_opt {
            // Handle assistant content tokens as streaming deltas.
            if let Some(content) = choice
                .get("delta")
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
                && !content.is_empty()
            {
                append_assistant_text(&tx_event, &mut assistant_item, content.to_string()).await;
            }

            // Forward any reasoning/thinking deltas if present.
            // Some providers stream `reasoning` as a plain string while others
            // nest the text under an object (e.g. `{ "reasoning": { "text": "…" } }`).
            if let Some(reasoning_val) = choice.get("delta").and_then(|d| d.get("reasoning")) {
                let mut maybe_text = reasoning_val
                    .as_str()
                    .map(str::to_string)
                    .filter(|s| !s.is_empty());

                if maybe_text.is_none() && reasoning_val.is_object() {
                    if let Some(s) = reasoning_val
                        .get("text")
                        .and_then(|t| t.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        maybe_text = Some(s.to_string());
                    } else if let Some(s) = reasoning_val
                        .get("content")
                        .and_then(|t| t.as_str())
                        .filter(|s| !s.is_empty())
                    {
                        maybe_text = Some(s.to_string());
                    }
                }

                if let Some(reasoning) = maybe_text {
                    // Accumulate so we can emit a terminal Reasoning item at the end.
                    append_reasoning_text(&tx_event, &mut reasoning_item, reasoning).await;
                }
            }

            // Some providers only include reasoning on the final message object.
            if let Some(message_reasoning) = choice.get("message").and_then(|m| m.get("reasoning"))
            {
                // Accept either a plain string or an object with { text | content }
                if let Some(s) = message_reasoning.as_str() {
                    if !s.is_empty() {
                        append_reasoning_text(&tx_event, &mut reasoning_item, s.to_string()).await;
                    }
                } else if let Some(obj) = message_reasoning.as_object()
                    && let Some(s) = obj
                        .get("text")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("content").and_then(|v| v.as_str()))
                    && !s.is_empty()
                {
                    append_reasoning_text(&tx_event, &mut reasoning_item, s.to_string()).await;
                }
            }

            // Handle streaming function / tool calls.
            if let Some(tool_calls) = choice
                .get("delta")
                .and_then(|d| d.get("tool_calls"))
                .and_then(|tc| tc.as_array())
                && let Some(tool_call) = tool_calls.first()
            {
                // Mark that we have an active function call in progress.
                fn_call_state.active = true;

                // Extract call_id if present.
                if let Some(id) = tool_call.get("id").and_then(|v| v.as_str()) {
                    fn_call_state.call_id.get_or_insert_with(|| id.to_string());
                }

                // Extract function details if present.
                if let Some(function) = tool_call.get("function") {
                    if let Some(name) = function.get("name").and_then(|n| n.as_str()) {
                        fn_call_state.name.get_or_insert_with(|| name.to_string());
                    }

                    if let Some(args_fragment) = function.get("arguments").and_then(|a| a.as_str())
                    {
                        fn_call_state.arguments.push_str(args_fragment);
                    }
                }
            }

            // Emit end-of-turn when finish_reason signals completion.
            if let Some(finish_reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                match finish_reason {
                    "tool_calls" if fn_call_state.active => {
                        // First, flush the terminal raw reasoning so UIs can finalize
                        // the reasoning stream before any exec/tool events begin.
                        if let Some(item) = reasoning_item.take() {
                            let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                        }

                        // Then emit the FunctionCall response item.
                        let item = ResponseItem::FunctionCall {
                            id: None,
                            name: fn_call_state.name.clone().unwrap_or_else(|| "".to_string()),
                            arguments: fn_call_state.arguments.clone(),
                            call_id: fn_call_state.call_id.clone().unwrap_or_else(String::new),
                        };

                        let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                    }
                    "stop" => {
                        // Regular turn without tool-call. Emit the final assistant message
                        // as a single OutputItemDone so non-delta consumers see the result.
                        if let Some(item) = assistant_item.take() {
                            let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                        }
                        // Also emit a terminal Reasoning item so UIs can finalize raw reasoning.
                        if let Some(item) = reasoning_item.take() {
                            let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                        }
                    }
                    _ => {}
                }

                // Emit Completed regardless of reason so the agent can advance.
                let _ = tx_event
                    .send(Ok(ResponseEvent::Completed {
                        response_id: String::new(),
                        token_usage: None,
                    }))
                    .await;

                // Prepare for potential next turn (should not happen in same stream).
                // fn_call_state = FunctionCallState::default();

                return; // End processing for this SSE stream.
            }
        }
    }
}

/// Adapter that aggregates Chat Completions SSE output into the final assistant
/// message plus optional reasoning, mirroring the Responses API contract.
pub(crate) struct AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    inner: S,
    pending: std::collections::VecDeque<ResponseEvent>,
    finished: bool,
}

impl<S> AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    pub fn streaming_mode(inner: S) -> Self {
        Self {
            inner,
            pending: std::collections::VecDeque::new(),
            finished: false,
        }
    }
}

impl<S> Stream for AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    type Item = Result<ResponseEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(event) = this.pending.pop_front() {
            return Poll::Ready(Some(Ok(event)));
        }

        if this.finished {
            return Poll::Ready(None);
        }

        loop {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item)))) => {
                    this.pending.push_back(ResponseEvent::OutputItemDone(item));
                    continue;
                }
                Poll::Ready(Some(Ok(ResponseEvent::RateLimits(snapshot)))) => {
                    this.pending.push_back(ResponseEvent::RateLimits(snapshot));
                    continue;
                }
                Poll::Ready(Some(Ok(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                }))) => {
                    this.pending.push_back(ResponseEvent::Completed {
                        response_id,
                        token_usage,
                    });
                    this.finished = true;
                    return Poll::Ready(this.pending.pop_front().map(Ok));
                }
                Poll::Ready(Some(Ok(other))) => {
                    this.pending.push_back(other);
                    continue;
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(None) => {
                    this.finished = true;
                    return Poll::Ready(None);
                }
            }
        }
    }
}

/// Extension trait that activates aggregation on any stream of [`ResponseEvent`].
pub(crate) trait AggregateStreamExt:
    Stream<Item = Result<ResponseEvent>> + Sized + Unpin
{
    fn aggregate(self) -> AggregatedChatStream<Self> {
        AggregatedChatStream::streaming_mode(self)
    }
}

impl<T> AggregateStreamExt for T where T: Stream<Item = Result<ResponseEvent>> + Sized + Unpin {}
