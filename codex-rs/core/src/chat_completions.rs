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
use crate::openai_tools::create_tools_json_for_chat_completions_api;
use crate::util::backoff;

/// Implementation for the classic Chat Completions API.
pub(crate) async fn stream_chat_completions(
    prompt: &Prompt,
    model: &str,
    client: &reqwest::Client,
    provider: &ModelProviderInfo,
) -> Result<ResponseStream> {
    // Build messages array
    let mut messages = Vec::<serde_json::Value>::new();

    let full_instructions = prompt.get_full_instructions(model);
    messages.push(json!({"role": "system", "content": full_instructions}));

    for item in &prompt.input {
        match item {
            ResponseItem::Message { role, content } => {
                let mut text = String::new();
                for c in content {
                    match c {
                        ContentItem::InputText { text: t }
                        | ContentItem::OutputText { text: t } => {
                            text.push_str(t);
                        }
                        _ => {}
                    }
                }
                messages.push(json!({"role": role, "content": text}));
            }
            ResponseItem::FunctionCall {
                name,
                arguments,
                call_id,
            } => {
                messages.push(json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": call_id,
                        "type": "function",
                        "function": {
                            "name": name,
                            "arguments": arguments,
                        }
                    }]
                }));
            }
            ResponseItem::LocalShellCall {
                id,
                call_id: _,
                status,
                action,
            } => {
                // Confirm with API team.
                messages.push(json!({
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": id.clone().unwrap_or_else(|| "".to_string()),
                        "type": "local_shell_call",
                        "status": status,
                        "action": action,
                    }]
                }));
            }
            ResponseItem::FunctionCallOutput { call_id, output } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": output.content,
                }));
            }
            ResponseItem::Reasoning { .. } | ResponseItem::Other => {
                // Omit these items from the conversation history.
                continue;
            }
        }
    }

    let tools_json = create_tools_json_for_chat_completions_api(prompt, model)?;
    let payload = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "tools": tools_json,
    });

    debug!(
        "POST to {}: {}",
        provider.get_full_url(),
        serde_json::to_string_pretty(&payload).unwrap_or_default()
    );

    let mut attempt = 0;
    loop {
        attempt += 1;

        let req_builder = provider.create_request_builder(client)?;

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
                        token_usage: None,
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
            // Handle assistant content tokens.
            if let Some(content) = choice
                .get("delta")
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
            {
                let item = ResponseItem::Message {
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: content.to_string(),
                    }],
                };

                let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
            }

            // Handle streaming function / tool calls.
            if let Some(tool_calls) = choice
                .get("delta")
                .and_then(|d| d.get("tool_calls"))
                .and_then(|tc| tc.as_array())
            {
                if let Some(tool_call) = tool_calls.first() {
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

                        if let Some(args_fragment) =
                            function.get("arguments").and_then(|a| a.as_str())
                        {
                            fn_call_state.arguments.push_str(args_fragment);
                        }
                    }
                }
            }

            // Emit end-of-turn when finish_reason signals completion.
            if let Some(finish_reason) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                match finish_reason {
                    "tool_calls" if fn_call_state.active => {
                        // Build the FunctionCall response item.
                        let item = ResponseItem::FunctionCall {
                            name: fn_call_state.name.clone().unwrap_or_else(|| "".to_string()),
                            arguments: fn_call_state.arguments.clone(),
                            call_id: fn_call_state.call_id.clone().unwrap_or_else(String::new),
                        };

                        // Emit it downstream.
                        let _ = tx_event.send(Ok(ResponseEvent::OutputItemDone(item))).await;
                    }
                    "stop" => {
                        // Regular turn without tool-call.
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
                    // If this is an incremental assistant message chunk, accumulate but
                    // do NOT emit yet. Forward any other item (e.g. FunctionCall) right
                    // away so downstream consumers see it.

                    let is_assistant_delta = matches!(&item, crate::models::ResponseItem::Message { role, .. } if role == "assistant");

                    if is_assistant_delta {
                        if let crate::models::ResponseItem::Message { content, .. } = &item {
                            if let Some(text) = content.iter().find_map(|c| match c {
                                crate::models::ContentItem::OutputText { text } => Some(text),
                                _ => None,
                            }) {
                                this.cumulative.push_str(text);
                            }
                        }

                        // Swallow partial assistant chunk; keep polling.
                        continue;
                    }

                    // Not an assistant message – forward immediately.
                    return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(item))));
                }
                Poll::Ready(Some(Ok(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                }))) => {
                    if !this.cumulative.is_empty() {
                        let aggregated_item = crate::models::ResponseItem::Message {
                            role: "assistant".to_string(),
                            content: vec![crate::models::ContentItem::OutputText {
                                text: std::mem::take(&mut this.cumulative),
                            }],
                        };

                        // Buffer Completed so it is returned *after* the aggregated message.
                        this.pending_completed = Some(ResponseEvent::Completed {
                            response_id,
                            token_usage,
                        });

                        return Poll::Ready(Some(Ok(ResponseEvent::OutputItemDone(
                            aggregated_item,
                        ))));
                    }

                    // Nothing aggregated – forward Completed directly.
                    return Poll::Ready(Some(Ok(ResponseEvent::Completed {
                        response_id,
                        token_usage,
                    })));
                }
                Poll::Ready(Some(Ok(ResponseEvent::Created))) => {
                    // These events are exclusive to the Responses API and
                    // will never appear in a Chat Completions stream.
                    continue;
                }
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
    ///     Completed
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
    ///
    /// See [`tests::aggregates_consecutive_message_chunks`] for an example.
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::models::FunctionCallOutputPayload;
    use crate::models::LocalShellAction;
    use crate::models::LocalShellExecAction;
    use crate::models::LocalShellStatus;
    use futures::StreamExt;
    use futures::stream;

    /// Helper constructing a minimal assistant text chunk.
    fn text_chunk(txt: &str) -> ResponseEvent {
        ResponseEvent::OutputItemDone(ResponseItem::Message {
            role: "assistant".to_string(),
            content: vec![ContentItem::OutputText { text: txt.into() }],
        })
    }

    #[tokio::test]
    async fn aggregates_consecutive_message_chunks() {
        let events = vec![
            Ok(text_chunk("Hello")),
            Ok(text_chunk(", world")),
            Ok(ResponseEvent::Completed {
                response_id: "r1".to_string(),
                token_usage: None,
            }),
        ];

        let stream = stream::iter(events).aggregate();
        let collected: Vec<_> = stream.map(Result::unwrap).collect().await;

        assert_eq!(collected.len(), 2, "only final message and Completed");

        match &collected[0] {
            ResponseEvent::OutputItemDone(ResponseItem::Message { content, .. }) => {
                let text = match &content[0] {
                    ContentItem::OutputText { text } => text,
                    _ => panic!("unexpected content item"),
                };
                assert_eq!(text, "Hello, world");
            }
            other => panic!("unexpected first event: {other:?}"),
        }

        assert!(matches!(
            collected[1],
            ResponseEvent::Completed { response_id: ref id, token_usage: None } if id == "r1"
        ));
    }

    #[tokio::test]
    async fn forwards_non_text_items_without_merging() {
        let func_call = ResponseItem::FunctionCall {
            name: "shell".to_string(),
            arguments: "{}".to_string(),
            call_id: "call1".to_string(),
        };

        let events = vec![
            Ok(text_chunk("foo")),
            Ok(ResponseEvent::OutputItemDone(func_call.clone())),
            Ok(text_chunk("bar")),
            Ok(ResponseEvent::Completed {
                response_id: "r2".to_string(),
                token_usage: None,
            }),
        ];

        let stream = stream::iter(events).aggregate();
        let collected: Vec<_> = stream.map(Result::unwrap).collect().await;

        assert_eq!(collected.len(), 3);

        // First event should be the function call forwarded directly.
        assert!(matches!(
            collected[0],
            ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { .. })
        ));

        // Second is the combined assistant message.
        match &collected[1] {
            ResponseEvent::OutputItemDone(ResponseItem::Message { content, .. }) => {
                let text = match &content[0] {
                    ContentItem::OutputText { text } => text,
                    _ => panic!("unexpected content item"),
                };
                assert_eq!(text, "foobar");
            }
            other => panic!("unexpected second event: {other:?}"),
        }

        // Final Completed event.
        assert!(matches!(
            collected[2],
            ResponseEvent::Completed { response_id: ref id, token_usage: None } if id == "r2"
        ));
    }

    #[tokio::test]
    async fn formats_tool_calls_in_chat_payload() {
        use serde_json::Value;
        use std::sync::Arc;
        use std::sync::Mutex;
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::Request;
        use wiremock::Respond;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        struct CaptureResponder(Arc<Mutex<Option<Value>>>);
        impl Respond for CaptureResponder {
            fn respond(&self, req: &Request) -> ResponseTemplate {
                let v: Value = serde_json::from_slice(&req.body).unwrap();
                *self.0.lock().unwrap() = Some(v);
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_raw(
                        "event: response.completed\n\
data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp\",\"output\":[]}}\n\n",
                        "text/event-stream",
                    )
            }
        }

        let server = MockServer::start().await;
        let captured = Arc::new(Mutex::new(None));

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(CaptureResponder(captured.clone()))
            .expect(1)
            .mount(&server)
            .await;

        unsafe {
            std::env::set_var("OPENAI_REQUEST_MAX_RETRIES", "0");
        }

        let provider = ModelProviderInfo {
            name: "openai".into(),
            base_url: format!("{}/v1", server.uri()),
            env_key: Some("PATH".into()),
            env_key_instructions: None,
            wire_api: crate::WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
        };

        let mut prompt = Prompt::default();
        prompt.input.push(ResponseItem::Message {
            role: "user".into(),
            content: vec![ContentItem::InputText { text: "hi".into() }],
        });
        prompt.input.push(ResponseItem::FunctionCall {
            name: "shell".into(),
            arguments: "[]".into(),
            call_id: "call123".into(),
        });
        prompt.input.push(ResponseItem::FunctionCallOutput {
            call_id: "call123".into(),
            output: FunctionCallOutputPayload {
                content: "ok".into(),
                success: Some(true),
            },
        });
        prompt.input.push(ResponseItem::LocalShellCall {
            id: Some("ls1".into()),
            call_id: Some("call456".into()),
            status: LocalShellStatus::Completed,
            action: LocalShellAction::Exec(LocalShellExecAction {
                command: vec!["echo".into(), "hi".into()],
                timeout_ms: Some(1),
                working_directory: None,
                env: None,
                user: None,
            }),
        });

        let client = reqwest::Client::new();
        let _ = stream_chat_completions(&prompt, "model", &client, &provider)
            .await
            .unwrap();

        let body = captured.lock().unwrap().take().unwrap();
        let messages = body.get("messages").unwrap().as_array().unwrap();

        // function_call
        let call = messages[2].get("tool_calls").unwrap().as_array().unwrap()[0].clone();
        assert_eq!(call.get("id").unwrap().as_str().unwrap(), "call123");
        assert_eq!(call.get("type").unwrap().as_str().unwrap(), "function");
        let func = call.get("function").unwrap();
        assert_eq!(func.get("name").unwrap().as_str().unwrap(), "shell");
        assert_eq!(func.get("arguments").unwrap().as_str().unwrap(), "[]");

        // function_call_output
        assert_eq!(messages[3].get("role").unwrap().as_str().unwrap(), "tool");
        assert_eq!(
            messages[3].get("tool_call_id").unwrap().as_str().unwrap(),
            "call123"
        );
        assert_eq!(messages[3].get("content").unwrap().as_str().unwrap(), "ok");

        // local_shell_call
        let shell_call = messages[4].get("tool_calls").unwrap().as_array().unwrap()[0].clone();
        assert_eq!(
            shell_call.get("type").unwrap().as_str().unwrap(),
            "local_shell_call"
        );
        assert_eq!(shell_call.get("id").unwrap().as_str().unwrap(), "ls1");
        let action = shell_call.get("action").unwrap();
        assert_eq!(
            action.get("command").unwrap().as_array().unwrap()[0]
                .as_str()
                .unwrap(),
            "echo"
        );
    }
}
