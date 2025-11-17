use std::time::Duration;

use crate::ModelProviderInfo;
use crate::client::ResponseEvent;
use crate::client::ResponseStream;
use crate::client::http::CodexHttpClient;
use crate::client::retry::RetryableStreamError;
use crate::client::retry::retry_stream;
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
use eventsource_stream::Eventsource;
use futures::Stream;
use futures::TryStreamExt;
use reqwest::StatusCode;
use serde_json::json;
use tokio::sync::mpsc;
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
                        .and_modify(|v| v.push_str(text.as_str()))
                        .or_insert(text.clone());
                    attached = true;
                }

                // Otherwise, attach to immediate next assistant anchor (tool-calls or assistant message)
                if !attached && idx + 1 < input.len() {
                    match &input[idx + 1] {
                        ResponseItem::FunctionCall { .. } | ResponseItem::LocalShellCall { .. } => {
                            reasoning_by_anchor_index
                                .entry(idx + 1)
                                .and_modify(|v| v.push_str(text.as_str()))
                                .or_insert(text.clone());
                        }
                        ResponseItem::Message { role, .. } if role == "assistant" => {
                            reasoning_by_anchor_index
                                .entry(idx + 1)
                                .and_modify(|v| v.push_str(text.as_str()))
                                .or_insert(text.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Track last assistant text we emitted to avoid duplicate assistant messages
    // in the outbound Chat Completions payload (can happen if a final
    // aggregated assistant message was recorded alongside an earlier partial).
    let mut last_assistant_text: Option<String> = None;

    for (idx, item) in input.iter().enumerate() {
        match item {
            ResponseItem::Message { role, content, .. } => {
                // Build content either as a plain string (typical for assistant text)
                // or as an array of content items when images are present (user/tool multimodal).
                let mut text = String::new();
                let mut items: Vec<serde_json::Value> = Vec::new();
                let mut saw_image = false;

                for c in content {
                    match c {
                        ContentItem::InputText { text: t }
                        | ContentItem::OutputText { text: t } => {
                            text.push_str(t);
                            items.push(json!({"type":"text","text": t}));
                        }
                        ContentItem::InputImage { image_url } => {
                            saw_image = true;
                            items.push(json!({"type":"image_url","image_url": {"url": image_url}}));
                        }
                    }
                }

                // Skip exact-duplicate assistant messages.
                if role == "assistant" {
                    if let Some(prev) = &last_assistant_text
                        && prev == &text
                    {
                        continue;
                    }
                    last_assistant_text = Some(text.clone());
                }

                // For assistant messages, always send a plain string for compatibility.
                // For user messages, if an image is present, send an array of content items.
                let content_value = if role == "assistant" {
                    json!(text)
                } else if saw_image {
                    json!(items)
                } else {
                    json!(text)
                };

                let mut msg = json!({
                    "role": role,
                    "content": content_value
                });

                if role == "assistant"
                    && let Some(reasoning) = reasoning_by_anchor_index.get(&idx)
                    && let Some(obj) = msg.as_object_mut()
                {
                    obj.insert("reasoning".to_string(), json!(reasoning));
                }

                messages.push(msg);
            }
            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
                ..
            } => {
                let mut msg = json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments,
                        }
                    }],
                });

                if let Some(reasoning) = reasoning_by_anchor_index.get(&idx)
                    && let Some(obj) = msg.as_object_mut()
                {
                    obj.insert("reasoning".to_string(), json!(reasoning));
                }

                messages.push(msg);
            }

            ResponseItem::LocalShellCall {
                id,
                call_id: _,
                status,
                action,
            } => {
                let mut msg = json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": id.clone().unwrap_or_else(|| "".to_string()),
                        "type": "local_shell_call",
                        "status": status,
                        "action": action,
                    }]
                });

                if let Some(reasoning) = reasoning_by_anchor_index.get(&idx)
                    && let Some(obj) = msg.as_object_mut()
                {
                    obj.insert("reasoning".to_string(), json!(reasoning));
                }

                messages.push(msg);
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
            }

            ResponseItem::CustomToolCall {
                id,
                call_id: _,
                name,
                input,
                status: _,
            } => {
                messages.push(json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": id,
                        "type": "custom",
                        "custom": {
                            "name": name,
                            "input": input,
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
                // Omit from conversation history; reasoning is attached to anchors above.
                continue;
            }

            ResponseItem::WebSearchCall { .. } | ResponseItem::Other => {
                continue;
            }

            ResponseItem::GhostSnapshot { .. } => {
                // Ghost snapshots annotate history but are not sent to the model.
                continue;
            }
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
        let subagent = crate::client::types::subagent_label(sub);
        body["metadata"] = json!({
            "x-openai-subagent": subagent,
        });
    }

    let max_attempts = provider.request_max_retries();
    retry_stream(max_attempts, |attempt| {
        let body = body.clone();
        async move {
            stream_single_chat_completion(attempt, client, provider, otel_event_manager, body)
                .await
                .map_err(ChatStreamError::Retryable)
        }
    })
    .await
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

enum ChatStreamError {
    Retryable(CodexErr),
}

impl RetryableStreamError for ChatStreamError {
    fn delay(&self, attempt: u64) -> Option<Duration> {
        Some(backoff(attempt))
    }

    fn into_error(self) -> CodexErr {
        match self {
            ChatStreamError::Retryable(e) => e,
        }
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
        let sse = match crate::client::sse::next_sse_event(
            &mut stream,
            idle_timeout,
            &otel_event_manager,
        )
        .await
        {
            crate::client::sse::SseNext::Event(ev) => ev,
            crate::client::sse::SseNext::Eof => {
                let _ = tx_event
                    .send(Ok(ResponseEvent::Completed {
                        response_id: String::new(),
                        token_usage: None,
                    }))
                    .await;
                return;
            }
            crate::client::sse::SseNext::StreamError(message) => {
                let _ = tx_event.send(Err(CodexErr::Stream(message, None))).await;
                return;
            }
            crate::client::sse::SseNext::Timeout => {
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
            if let Some(finish_reason) = choice.get("finish_reason").and_then(|v| v.as_str())
                && !finish_reason.is_empty()
            {
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
