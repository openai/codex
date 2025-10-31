use std::collections::VecDeque;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use async_trait::async_trait;
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
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::debug;
use tracing::trace;

use crate::api::ApiClient;
use crate::error::Error;
use crate::model_provider::ModelProviderInfo;
use crate::prompt::Prompt;
use crate::stream::ResponseEvent;
use crate::stream::ResponseStream;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug)]
pub enum ChatAggregationMode {
    AggregatedOnly,
    Streaming,
}

#[derive(Clone)]
pub struct ChatCompletionsApiClientConfig {
    pub http_client: reqwest::Client,
    pub provider: ModelProviderInfo,
    pub model: String,
    pub otel_event_manager: OtelEventManager,
    pub session_source: SessionSource,
    pub aggregation_mode: ChatAggregationMode,
}

#[derive(Clone)]
pub struct ChatCompletionsApiClient {
    config: ChatCompletionsApiClientConfig,
}

#[async_trait]
impl ApiClient for ChatCompletionsApiClient {
    type Config = ChatCompletionsApiClientConfig;

    async fn new(config: Self::Config) -> Result<Self> {
        Ok(Self { config })
    }

    async fn stream(&self, prompt: Prompt) -> Result<ResponseStream> {
        Self::validate_prompt(&prompt)?;

        let payload = self.build_payload(&prompt)?;
        let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(1600);

        let mut attempt = 0u64;
        let max_retries = self.config.provider.request_max_retries();

        loop {
            attempt += 1;

            let mut req_builder = self
                .config
                .provider
                .create_request_builder(&self.config.http_client, &None)
                .await?;

            if let SessionSource::SubAgent(sub) = &self.config.session_source {
                let subagent = if let SubAgentSource::Other(label) = sub {
                    label.clone()
                } else {
                    serde_json::to_value(sub)
                        .ok()
                        .and_then(|v| v.as_str().map(std::string::ToString::to_string))
                        .unwrap_or_else(|| "other".to_string())
                };
                req_builder = req_builder.header("x-openai-subagent", subagent);
            }

            let res = self
                .config
                .otel_event_manager
                .log_request(attempt, || {
                    req_builder
                        .header(reqwest::header::ACCEPT, "text/event-stream")
                        .json(&payload)
                        .send()
                })
                .await;

            match res {
                Ok(resp) if resp.status().is_success() => {
                    let stream = resp
                        .bytes_stream()
                        .map_err(|err| Error::ResponseStreamFailed {
                            source: err,
                            request_id: None,
                        });
                    let idle_timeout = self.config.provider.stream_idle_timeout();
                    let otel = self.config.otel_event_manager.clone();
                    let mode = self.config.aggregation_mode;

                    tokio::spawn(process_chat_sse(
                        stream,
                        tx_event.clone(),
                        idle_timeout,
                        otel,
                        mode,
                    ));

                    return Ok(ResponseStream { rx_event });
                }
                Ok(resp) => {
                    if attempt >= max_retries {
                        let status = resp.status();
                        let body = resp
                            .text()
                            .await
                            .unwrap_or_else(|_| "<failed to read response>".to_string());
                        return Err(Error::UnexpectedStatus { status, body });
                    }

                    let retry_after = resp
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs);
                    tokio::time::sleep(retry_after.unwrap_or_else(|| backoff(attempt))).await;
                }
                Err(error) => {
                    if attempt >= max_retries {
                        return Err(Error::Http(error));
                    }
                    tokio::time::sleep(backoff(attempt)).await;
                }
            }
        }
    }
}

impl ChatCompletionsApiClient {
    fn validate_prompt(prompt: &Prompt) -> Result<()> {
        if prompt.output_schema.is_some() {
            return Err(Error::UnsupportedOperation(
                "output_schema is not supported for Chat Completions API".to_string(),
            ));
        }
        Ok(())
    }

    fn build_payload(&self, prompt: &Prompt) -> Result<serde_json::Value> {
        let mut messages = Vec::<serde_json::Value>::new();
        messages.push(json!({ "role": "system", "content": prompt.instructions }));

        let mut reasoning_by_anchor_index: std::collections::HashMap<usize, String> =
            std::collections::HashMap::new();

        let mut last_emitted_role: Option<&str> = None;
        for item in &prompt.input {
            match item {
                ResponseItem::Message { role, .. } => last_emitted_role = Some(role.as_str()),
                ResponseItem::FunctionCall { .. } | ResponseItem::LocalShellCall { .. } => {
                    last_emitted_role = Some("assistant");
                }
                ResponseItem::FunctionCallOutput { .. } => last_emitted_role = Some("tool"),
                ResponseItem::Reasoning { .. }
                | ResponseItem::Other
                | ResponseItem::CustomToolCall { .. }
                | ResponseItem::CustomToolCallOutput { .. }
                | ResponseItem::WebSearchCall { .. }
                | ResponseItem::GhostSnapshot { .. } => {}
            }
        }

        let mut last_user_index: Option<usize> = None;
        for (idx, item) in prompt.input.iter().enumerate() {
            if let ResponseItem::Message { role, .. } = item
                && role == "user"
            {
                last_user_index = Some(idx);
            }
        }

        if !matches!(last_emitted_role, Some("user")) {
            for (idx, item) in prompt.input.iter().enumerate() {
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
                            | ReasoningItemContent::Text { text: segment } => {
                                text.push_str(segment);
                            }
                        }
                    }
                    if text.trim().is_empty() {
                        continue;
                    }

                    let mut attached = false;
                    if idx > 0
                        && let ResponseItem::Message { role, .. } = &prompt.input[idx - 1]
                        && role == "assistant"
                    {
                        reasoning_by_anchor_index
                            .entry(idx - 1)
                            .and_modify(|v| v.push_str(&text))
                            .or_insert(text.clone());
                        attached = true;
                    }

                    if !attached && idx + 1 < prompt.input.len() {
                        match &prompt.input[idx + 1] {
                            ResponseItem::FunctionCall { .. }
                            | ResponseItem::LocalShellCall { .. } => {
                                reasoning_by_anchor_index
                                    .entry(idx + 1)
                                    .and_modify(|v| v.push_str(&text))
                                    .or_insert(text.clone());
                            }
                            ResponseItem::Message { role, .. } if role == "assistant" => {
                                reasoning_by_anchor_index
                                    .entry(idx + 1)
                                    .and_modify(|v| v.push_str(&text))
                                    .or_insert(text.clone());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let mut last_assistant_text: Option<String> = None;

        for (idx, item) in prompt.input.iter().enumerate() {
            match item {
                ResponseItem::Message { role, content, .. } => {
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
                                items.push(
                                    json!({"type":"image_url","image_url": {"url": image_url}}),
                                );
                            }
                        }
                    }

                    if role == "assistant" {
                        if let Some(prev) = &last_assistant_text
                            && prev == &text
                        {
                            continue;
                        }
                        last_assistant_text = Some(text.clone());
                    }

                    let content_value = if role == "assistant" {
                        json!(text)
                    } else if saw_image {
                        json!(items)
                    } else {
                        json!(text)
                    };

                    let mut message = json!({
                        "role": role,
                        "content": content_value,
                    });

                    if let Some(reasoning) = reasoning_by_anchor_index.get(&idx) {
                        message
                            .as_object_mut()
                            .expect("message")
                            .insert("reasoning".to_string(), json!({"text": reasoning}));
                    }

                    messages.push(message);
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
                            },
                        }],
                    }));
                }
                ResponseItem::FunctionCallOutput { call_id, output } => {
                    let content_value = if let Some(items) = &output.content_items {
                        let mapped: Vec<serde_json::Value> = items
                            .iter()
                            .map(|item| match item {
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
                ResponseItem::LocalShellCall {
                    id,
                    call_id,
                    action,
                    ..
                } => {
                    let tool_id = call_id
                        .clone()
                        .filter(|value| !value.is_empty())
                        .or_else(|| id.clone())
                        .unwrap_or_default();
                    messages.push(json!({
                        "role": "assistant",
                        "tool_calls": [{
                            "id": tool_id,
                            "type": "function",
                            "function": {
                                "name": "shell",
                                "arguments": serde_json::to_string(action).unwrap_or_default(),
                            },
                        }],
                    }));
                }
                ResponseItem::CustomToolCall {
                    call_id,
                    name,
                    input,
                    ..
                } => {
                    messages.push(json!({
                        "role": "assistant",
                        "tool_calls": [{
                            "id": call_id.clone(),
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": input,
                            },
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
                ResponseItem::WebSearchCall { .. }
                | ResponseItem::Reasoning { .. }
                | ResponseItem::Other
                | ResponseItem::GhostSnapshot { .. } => {}
            }
        }

        let tools_json = create_tools_json_for_chat_completions_api(&prompt.tools)?;
        let payload = json!({
            "model": self.config.model,
            "messages": messages,
            "stream": true,
            "tools": tools_json,
        });

        trace!("chat completions payload: {}", payload);
        Ok(payload)
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
    _aggregation_mode: ChatAggregationMode,
) where
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    let mut stream = stream.eventsource();

    #[derive(Default)]
    struct FunctionCallState {
        name: Option<String>,
        arguments: String,
        call_id: Option<String>,
    }

    let mut function_call_state = FunctionCallState::default();
    let mut assistant_item: Option<ResponseItem> = None;
    let mut reasoning_item: Option<ResponseItem> = None;

    loop {
        let response = timeout(idle_timeout, stream.next()).await;
        otel_event_manager.log_sse_event(&response, idle_timeout);

        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                let event = Error::Stream(e.to_string(), None);
                let _ = tx_event.send(Err(event)).await;
                return;
            }
            Ok(None) => {
                if let Some(item) = assistant_item.take() {
                    let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                }
                if let Some(item) = reasoning_item.take() {
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
            Err(_) => {
                let _ = tx_event
                    .send(Err(Error::Stream(
                        "idle timeout waiting for SSE".into(),
                        None,
                    )))
                    .await;
                return;
            }
        };

        trace!("chat_completions received SSE chunk: {}", sse.data);

        if sse.data.trim() == "[DONE]" {
            if let Some(item) = assistant_item.take() {
                let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
            }
            if let Some(item) = reasoning_item.take() {
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

        let chunk: serde_json::Value = match serde_json::from_str(&sse.data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let choice_opt = chunk.get("choices").and_then(|c| c.get(0));

        if let Some(choice) = choice_opt {
            if let Some(content) = choice
                .get("delta")
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
                && !content.is_empty()
            {
                append_assistant_text(&tx_event, &mut assistant_item, content.to_string()).await;
            }

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
                    append_reasoning_text(&tx_event, &mut reasoning_item, reasoning).await;
                }
            }

            if let Some(message_reasoning) = choice.get("message").and_then(|m| m.get("reasoning"))
            {
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

            if let Some(tool_calls) = choice
                .get("delta")
                .and_then(|d| d.get("tool_calls"))
                .and_then(|v| v.as_array())
            {
                for call in tool_calls {
                    if let Some(index) = call.get("index").and_then(serde_json::Value::as_u64)
                        && index == 0
                        && let Some(function) = call.get("function")
                    {
                        if let Some(name) = function.get("name").and_then(|n| n.as_str()) {
                            function_call_state.name = Some(name.to_string());
                        }
                        if let Some(arguments) = function.get("arguments").and_then(|a| a.as_str())
                        {
                            function_call_state.arguments.push_str(arguments);
                        }
                        if let Some(id) = call.get("id").and_then(|i| i.as_str()) {
                            function_call_state.call_id = Some(id.to_string());
                        }

                        if let Some(finish) = choice.get("finish_reason").and_then(|f| f.as_str())
                            && finish == "tool_calls"
                            && let Some(name) = function_call_state.name.take() {
                                let call_id =
                                    function_call_state.call_id.take().unwrap_or_default();
                                let arguments = std::mem::take(&mut function_call_state.arguments);
                                let item = ResponseItem::FunctionCall {
                                    id: None,
                                    name,
                                    arguments,
                                    call_id,
                                };
                                let _ =
                                    tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                            }
                    }
                }
            }
        }
    }
}

pub trait AggregateStreamExt: Stream<Item = Result<ResponseEvent>> + Sized {
    fn aggregate(self) -> AggregatedChatStream<Self>
    where
        Self: Unpin,
    {
        AggregatedChatStream::new(self, AggregateMode::AggregatedOnly)
    }

    fn streaming_mode(self) -> AggregatedChatStream<Self>
    where
        Self: Unpin,
    {
        AggregatedChatStream::new(self, AggregateMode::Streaming)
    }
}

impl<S> AggregateStreamExt for S where S: Stream<Item = Result<ResponseEvent>> + Sized + Unpin {}

enum AggregateMode {
    AggregatedOnly,
    Streaming,
}

pub struct AggregatedChatStream<S> {
    inner: S,
    cumulative: String,
    cumulative_reasoning: String,
    pending: VecDeque<ResponseEvent>,
    mode: AggregateMode,
}

impl<S> AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    fn new(inner: S, mode: AggregateMode) -> Self {
        Self {
            inner,
            cumulative: String::new(),
            cumulative_reasoning: String::new(),
            pending: VecDeque::new(),
            mode,
        }
    }
}

impl<S> Stream for AggregatedChatStream<S>
where
    S: Stream<Item = Result<ResponseEvent>> + Unpin,
{
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(ev) = self.pending.pop_front() {
            return Poll::Ready(Some(Ok(ev)));
        }

        loop {
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item)))) => {
                    let is_assistant_message = matches!(
                        &item,
                        ResponseItem::Message { role, .. } if role == "assistant"
                    );

                    if is_assistant_message {
                        match self.mode {
                            AggregateMode::AggregatedOnly => {
                                if self.cumulative.is_empty()
                                    && let ResponseItem::Message { content, .. } = &item
                                    && let Some(text) = content.iter().find_map(|c| match c {
                                        ContentItem::OutputText { text } => Some(text),
                                        _ => None,
                                    })
                                {
                                    self.cumulative.push_str(text);
                                }
                                continue;
                            }
                            AggregateMode::Streaming => {
                                if self.cumulative.is_empty() {
                                    return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(
                                        item,
                                    ))));
                                } else {
                                    continue;
                                }
                            }
                        }
                    }

                    return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::RateLimits(snapshot)))) => {
                    return Poll::Ready(Some(Ok(ResponseEvent::RateLimits(snapshot))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                }))) => {
                    let mut emitted_any = false;

                    if !self.cumulative_reasoning.is_empty()
                        && matches!(self.mode, AggregateMode::AggregatedOnly)
                    {
                        let aggregated_reasoning = ResponseItem::Reasoning {
                            id: String::new(),
                            summary: Vec::new(),
                            content: Some(vec![ReasoningItemContent::ReasoningText {
                                text: std::mem::take(&mut self.cumulative_reasoning),
                            }]),
                            encrypted_content: None,
                        };
                        self.pending
                            .push_back(ResponseEvent::OutputItemDone(aggregated_reasoning));
                        emitted_any = true;
                    }

                    if !self.cumulative.is_empty() {
                        let aggregated_message = ResponseItem::Message {
                            id: None,
                            role: "assistant".to_string(),
                            content: vec![ContentItem::OutputText {
                                text: std::mem::take(&mut self.cumulative),
                            }],
                        };
                        self.pending
                            .push_back(ResponseEvent::OutputItemDone(aggregated_message));
                        emitted_any = true;
                    }

                    if emitted_any {
                        self.pending.push_back(ResponseEvent::Completed {
                            response_id: response_id.clone(),
                            token_usage: token_usage.clone(),
                        });
                        if let Some(ev) = self.pending.pop_front() {
                            return Poll::Ready(Some(Ok(ev)));
                        }
                    }

                    return Poll::Ready(Some(Ok(ResponseEvent::Completed {
                        response_id,
                        token_usage,
                    })));
                }
                Poll::Ready(Some(Ok(ResponseEvent::Created))) => continue,
                Poll::Ready(Some(Ok(ResponseEvent::OutputTextDelta(delta)))) => {
                    self.cumulative.push_str(&delta);
                    if matches!(self.mode, AggregateMode::Streaming) {
                        return Poll::Ready(Some(Ok(ResponseEvent::OutputTextDelta(delta))));
                    }
                }
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningContentDelta(delta)))) => {
                    self.cumulative_reasoning.push_str(&delta);
                    if matches!(self.mode, AggregateMode::Streaming) {
                        return Poll::Ready(Some(Ok(ResponseEvent::ReasoningContentDelta(delta))));
                    }
                }
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningSummaryDelta(_)))) => continue,
                Poll::Ready(Some(Ok(ResponseEvent::ReasoningSummaryPartAdded))) => continue,
                Poll::Ready(Some(Ok(ResponseEvent::OutputItemAdded(item)))) => {
                    return Poll::Ready(Some(Ok(ResponseEvent::OutputItemAdded(item))));
                }
            }
        }
    }
}

fn create_tools_json_for_chat_completions_api(
    tools: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>> {
    let tools_json = tools
        .iter()
        .filter_map(|tool| {
            if tool.get("type") != Some(&serde_json::Value::String("function".to_string())) {
                return None;
            }

            let function_value = if let Some(function) = tool.get("function") {
                function.clone()
            } else if let Some(map) = tool.as_object() {
                let mut function = map.clone();
                function.remove("type");
                Value::Object(function)
            } else {
                return None;
            };

            Some(json!({
                "type": "function",
                "function": function_value,
            }))
        })
        .collect::<Vec<serde_json::Value>>();
    Ok(tools_json)
}

fn backoff(attempt: u64) -> Duration {
    let capped = attempt.min(6);
    Duration::from_millis(100 * 2u64.pow(capped as u32))
}
