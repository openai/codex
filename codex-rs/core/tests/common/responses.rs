use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use base64::Engine;
use codex_protocol::openai_models::ModelsResponse;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::oneshot;
use wiremock::BodyPrintLimit;
use wiremock::Match;
use wiremock::Mock;
use wiremock::MockBuilder;
use wiremock::MockServer;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path_regex;

use crate::test_codex::ApplyPatchModelOutput;

#[derive(Debug, Clone)]
pub struct ResponseMock {
    requests: Arc<Mutex<Vec<ResponsesRequest>>>,
}

impl ResponseMock {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn single_request(&self) -> ResponsesRequest {
        let requests = self.requests.lock().unwrap();
        if requests.len() != 1 {
            panic!("expected 1 request, got {}", requests.len());
        }
        requests.first().unwrap().clone()
    }

    pub fn requests(&self) -> Vec<ResponsesRequest> {
        self.requests.lock().unwrap().clone()
    }

    pub fn last_request(&self) -> Option<ResponsesRequest> {
        self.requests.lock().unwrap().last().cloned()
    }

    /// Returns true if any captured request contains a `function_call` with the
    /// provided `call_id`.
    pub fn saw_function_call(&self, call_id: &str) -> bool {
        self.requests()
            .iter()
            .any(|req| req.has_function_call(call_id))
    }

    /// Returns the `output` string for a matching `function_call_output` with
    /// the provided `call_id`, searching across all captured requests.
    pub fn function_call_output_text(&self, call_id: &str) -> Option<String> {
        self.requests()
            .iter()
            .find_map(|req| req.function_call_output_text(call_id))
    }
}

#[derive(Debug, Clone)]
pub struct ResponsesRequest(wiremock::Request);

impl ResponsesRequest {
    pub fn body_json(&self) -> Value {
        self.0.body_json().unwrap()
    }

    /// Returns all `input_text` spans from `message` inputs for the provided role.
    pub fn message_input_texts(&self, role: &str) -> Vec<String> {
        self.inputs_of_type("message")
            .into_iter()
            .filter(|item| item.get("role").and_then(Value::as_str) == Some(role))
            .filter_map(|item| item.get("content").and_then(Value::as_array).cloned())
            .flatten()
            .filter(|span| span.get("type").and_then(Value::as_str) == Some("input_text"))
            .filter_map(|span| span.get("text").and_then(Value::as_str).map(str::to_owned))
            .collect()
    }

    pub fn input(&self) -> Vec<Value> {
        self.0.body_json::<Value>().unwrap()["input"]
            .as_array()
            .expect("input array not found in request")
            .clone()
    }

    pub fn inputs_of_type(&self, ty: &str) -> Vec<Value> {
        self.input()
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some(ty))
            .cloned()
            .collect()
    }

    pub fn function_call_output(&self, call_id: &str) -> Value {
        self.call_output(call_id, "function_call_output")
    }

    pub fn custom_tool_call_output(&self, call_id: &str) -> Value {
        self.call_output(call_id, "custom_tool_call_output")
    }

    pub fn call_output(&self, call_id: &str, call_type: &str) -> Value {
        self.input()
            .iter()
            .find(|item| {
                item.get("type").unwrap() == call_type && item.get("call_id").unwrap() == call_id
            })
            .cloned()
            .unwrap_or_else(|| panic!("function call output {call_id} item not found in request"))
    }

    /// Returns true if this request's `input` contains a `function_call` with
    /// the specified `call_id`.
    pub fn has_function_call(&self, call_id: &str) -> bool {
        self.input().iter().any(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })
    }

    /// If present, returns the `output` string of the `function_call_output`
    /// entry matching `call_id` in this request's `input`.
    pub fn function_call_output_text(&self, call_id: &str) -> Option<String> {
        let binding = self.input();
        let item = binding.iter().find(|item| {
            item.get("type").and_then(Value::as_str) == Some("function_call_output")
                && item.get("call_id").and_then(Value::as_str) == Some(call_id)
        })?;
        item.get("output")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    pub fn function_call_output_content_and_success(
        &self,
        call_id: &str,
    ) -> Option<(Option<String>, Option<bool>)> {
        self.call_output_content_and_success(call_id, "function_call_output")
    }

    pub fn custom_tool_call_output_content_and_success(
        &self,
        call_id: &str,
    ) -> Option<(Option<String>, Option<bool>)> {
        self.call_output_content_and_success(call_id, "custom_tool_call_output")
    }

    fn call_output_content_and_success(
        &self,
        call_id: &str,
        call_type: &str,
    ) -> Option<(Option<String>, Option<bool>)> {
        let output = self
            .call_output(call_id, call_type)
            .get("output")
            .cloned()
            .unwrap_or(Value::Null);
        match output {
            Value::String(text) => Some((Some(text), None)),
            Value::Object(obj) => Some((
                obj.get("content")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                obj.get("success").and_then(Value::as_bool),
            )),
            _ => Some((None, None)),
        }
    }

    pub fn header(&self, name: &str) -> Option<String> {
        self.0
            .headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    }

    pub fn path(&self) -> String {
        self.0.url.path().to_string()
    }

    pub fn query_param(&self, name: &str) -> Option<String> {
        self.0
            .url
            .query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct ModelsMock {
    requests: Arc<Mutex<Vec<wiremock::Request>>>,
}

impl ModelsMock {
    fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn requests(&self) -> Vec<wiremock::Request> {
        self.requests.lock().unwrap().clone()
    }

    pub fn single_request_path(&self) -> String {
        let requests = self.requests.lock().unwrap();
        if requests.len() != 1 {
            panic!("expected 1 request, got {}", requests.len());
        }
        requests.first().unwrap().url.path().to_string()
    }
}

impl Match for ModelsMock {
    fn matches(&self, request: &wiremock::Request) -> bool {
        self.requests.lock().unwrap().push(request.clone());
        true
    }
}

impl Match for ResponseMock {
    fn matches(&self, request: &wiremock::Request) -> bool {
        self.requests
            .lock()
            .unwrap()
            .push(ResponsesRequest(request.clone()));

        // Enforce invariant checks on every request body captured by the mock.
        // Panic on orphan tool outputs or calls to catch regressions early.
        validate_request_body_invariants(request);
        true
    }
}

/// Build an SSE stream body from a list of JSON events.
pub fn sse(events: Vec<Value>) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for ev in events {
        let kind = ev.get("type").and_then(|v| v.as_str()).unwrap();
        writeln!(&mut out, "event: {kind}").unwrap();
        if !ev.as_object().map(|o| o.len() == 1).unwrap_or(false) {
            write!(&mut out, "data: {ev}\n\n").unwrap();
        } else {
            out.push('\n');
        }
    }
    out
}

/// Convenience: SSE event for a completed response with a specific id.
pub fn ev_completed(id: &str) -> Value {
    serde_json::json!({
        "type": "response.completed",
        "response": {
            "id": id,
            "usage": {"input_tokens":0,"input_tokens_details":null,"output_tokens":0,"output_tokens_details":null,"total_tokens":0}
        }
    })
}

/// Convenience: SSE event for a created response with a specific id.
pub fn ev_response_created(id: &str) -> Value {
    serde_json::json!({
        "type": "response.created",
        "response": {
            "id": id,
        }
    })
}

pub fn ev_completed_with_tokens(id: &str, total_tokens: i64) -> Value {
    serde_json::json!({
        "type": "response.completed",
        "response": {
            "id": id,
            "usage": {
                "input_tokens": total_tokens,
                "input_tokens_details": null,
                "output_tokens": 0,
                "output_tokens_details": null,
                "total_tokens": total_tokens
            }
        }
    })
}

/// Convenience: SSE event for a single assistant message output item.
pub fn ev_assistant_message(id: &str, text: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": id,
            "content": [{"type": "output_text", "text": text}]
        }
    })
}

pub fn ev_message_item_added(id: &str, text: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.added",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": id,
            "content": [{"type": "output_text", "text": text}]
        }
    })
}

pub fn ev_output_text_delta(delta: &str) -> Value {
    serde_json::json!({
        "type": "response.output_text.delta",
        "delta": delta,
    })
}

pub fn ev_reasoning_item(id: &str, summary: &[&str], raw_content: &[&str]) -> Value {
    let summary_entries: Vec<Value> = summary
        .iter()
        .map(|text| serde_json::json!({"type": "summary_text", "text": text}))
        .collect();

    let overhead = "b".repeat(550);
    let raw_content_joined = raw_content.join("");
    let encrypted_content =
        base64::engine::general_purpose::STANDARD.encode(overhead + raw_content_joined.as_str());

    let mut event = serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "reasoning",
            "id": id,
            "summary": summary_entries,
            "encrypted_content": encrypted_content,
        }
    });

    if !raw_content.is_empty() {
        let content_entries: Vec<Value> = raw_content
            .iter()
            .map(|text| serde_json::json!({"type": "reasoning_text", "text": text}))
            .collect();
        event["item"]["content"] = Value::Array(content_entries);
    }

    event
}

pub fn ev_reasoning_item_added(id: &str, summary: &[&str]) -> Value {
    let summary_entries: Vec<Value> = summary
        .iter()
        .map(|text| serde_json::json!({"type": "summary_text", "text": text}))
        .collect();

    serde_json::json!({
        "type": "response.output_item.added",
        "item": {
            "type": "reasoning",
            "id": id,
            "summary": summary_entries,
        }
    })
}

pub fn ev_reasoning_summary_text_delta(delta: &str) -> Value {
    serde_json::json!({
        "type": "response.reasoning_summary_text.delta",
        "delta": delta,
        "summary_index": 0,
    })
}

pub fn ev_reasoning_text_delta(delta: &str) -> Value {
    serde_json::json!({
        "type": "response.reasoning_text.delta",
        "delta": delta,
        "content_index": 0,
    })
}

pub fn ev_web_search_call_added(id: &str, status: &str, query: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.added",
        "item": {
            "type": "web_search_call",
            "id": id,
            "status": status,
            "action": {"type": "search", "query": query}
        }
    })
}

pub fn ev_web_search_call_done(id: &str, status: &str, query: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "web_search_call",
            "id": id,
            "status": status,
            "action": {"type": "search", "query": query}
        }
    })
}

pub fn ev_function_call(call_id: &str, name: &str, arguments: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": arguments
        }
    })
}

pub fn ev_custom_tool_call(call_id: &str, name: &str, input: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "custom_tool_call",
            "call_id": call_id,
            "name": name,
            "input": input
        }
    })
}

pub fn ev_local_shell_call(call_id: &str, status: &str, command: Vec<&str>) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "local_shell_call",
            "call_id": call_id,
            "status": status,
            "action": {
                "type": "exec",
                "command": command,
            }
        }
    })
}

pub fn ev_apply_patch_call(
    call_id: &str,
    patch: &str,
    output_type: ApplyPatchModelOutput,
) -> Value {
    match output_type {
        ApplyPatchModelOutput::Freeform => ev_apply_patch_custom_tool_call(call_id, patch),
        ApplyPatchModelOutput::Function => ev_apply_patch_function_call(call_id, patch),
        ApplyPatchModelOutput::Shell => ev_apply_patch_shell_call(call_id, patch),
        ApplyPatchModelOutput::ShellViaHeredoc => {
            ev_apply_patch_shell_call_via_heredoc(call_id, patch)
        }
        ApplyPatchModelOutput::ShellCommandViaHeredoc => {
            ev_apply_patch_shell_command_call_via_heredoc(call_id, patch)
        }
    }
}

/// Convenience: SSE event for an `apply_patch` custom tool call with raw patch
/// text. This mirrors the payload produced by the Responses API when the model
/// invokes `apply_patch` directly (before we convert it to a function call).
pub fn ev_apply_patch_custom_tool_call(call_id: &str, patch: &str) -> Value {
    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "custom_tool_call",
            "name": "apply_patch",
            "input": patch,
            "call_id": call_id
        }
    })
}

/// Convenience: SSE event for an `apply_patch` function call. The Responses API
/// wraps the patch content in a JSON string under the `input` key; we recreate
/// the same structure so downstream code exercises the full parsing path.
pub fn ev_apply_patch_function_call(call_id: &str, patch: &str) -> Value {
    let arguments = serde_json::json!({ "input": patch });
    let arguments = serde_json::to_string(&arguments).expect("serialize apply_patch arguments");

    serde_json::json!({
        "type": "response.output_item.done",
        "item": {
            "type": "function_call",
            "name": "apply_patch",
            "arguments": arguments,
            "call_id": call_id
        }
    })
}

pub fn ev_shell_command_call(call_id: &str, command: &str) -> Value {
    let args = serde_json::json!({ "command": command });
    ev_shell_command_call_with_args(call_id, &args)
}

pub fn ev_shell_command_call_with_args(call_id: &str, args: &serde_json::Value) -> Value {
    let arguments = serde_json::to_string(args).expect("serialize shell command arguments");
    ev_function_call(call_id, "shell_command", &arguments)
}

pub fn ev_apply_patch_shell_call(call_id: &str, patch: &str) -> Value {
    let args = serde_json::json!({ "command": ["apply_patch", patch] });
    let arguments = serde_json::to_string(&args).expect("serialize apply_patch arguments");

    ev_function_call(call_id, "shell", &arguments)
}

pub fn ev_apply_patch_shell_call_via_heredoc(call_id: &str, patch: &str) -> Value {
    let script = format!("apply_patch <<'EOF'\n{patch}\nEOF\n");
    let args = serde_json::json!({ "command": ["bash", "-lc", script] });
    let arguments = serde_json::to_string(&args).expect("serialize apply_patch arguments");

    ev_function_call(call_id, "shell", &arguments)
}

pub fn ev_apply_patch_shell_command_call_via_heredoc(call_id: &str, patch: &str) -> Value {
    let args = serde_json::json!({ "command": format!("apply_patch <<'EOF'\n{patch}\nEOF\n") });
    let arguments = serde_json::to_string(&args).expect("serialize apply_patch arguments");

    ev_function_call(call_id, "shell_command", &arguments)
}

pub fn sse_failed(id: &str, code: &str, message: &str) -> String {
    sse(vec![serde_json::json!({
        "type": "response.failed",
        "response": {
            "id": id,
            "error": {"code": code, "message": message}
        }
    })])
}

pub fn sse_response(body: String) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .insert_header("content-type", "text/event-stream")
        .set_body_raw(body, "text/event-stream")
}

pub async fn mount_response_once(server: &MockServer, response: ResponseTemplate) -> ResponseMock {
    let (mock, response_mock) = base_mock();
    mock.respond_with(response)
        .up_to_n_times(1)
        .mount(server)
        .await;
    response_mock
}

pub async fn mount_response_once_match<M>(
    server: &MockServer,
    matcher: M,
    response: ResponseTemplate,
) -> ResponseMock
where
    M: wiremock::Match + Send + Sync + 'static,
{
    let (mock, response_mock) = base_mock();
    mock.and(matcher)
        .respond_with(response)
        .up_to_n_times(1)
        .mount(server)
        .await;
    response_mock
}

fn base_mock() -> (MockBuilder, ResponseMock) {
    let response_mock = ResponseMock::new();
    let mock = Mock::given(method("POST"))
        .and(path_regex(".*/responses$"))
        .and(response_mock.clone());
    (mock, response_mock)
}

fn compact_mock() -> (MockBuilder, ResponseMock) {
    let response_mock = ResponseMock::new();
    let mock = Mock::given(method("POST"))
        .and(path_regex(".*/responses/compact$"))
        .and(response_mock.clone());
    (mock, response_mock)
}

fn models_mock() -> (MockBuilder, ModelsMock) {
    let models_mock = ModelsMock::new();
    let mock = Mock::given(method("GET"))
        .and(path_regex(".*/models$"))
        .and(models_mock.clone());
    (mock, models_mock)
}

pub async fn mount_sse_once_match<M>(server: &MockServer, matcher: M, body: String) -> ResponseMock
where
    M: wiremock::Match + Send + Sync + 'static,
{
    let (mock, response_mock) = base_mock();
    mock.and(matcher)
        .respond_with(sse_response(body))
        .up_to_n_times(1)
        .mount(server)
        .await;
    response_mock
}

pub async fn mount_sse_once(server: &MockServer, body: String) -> ResponseMock {
    let (mock, response_mock) = base_mock();
    mock.respond_with(sse_response(body))
        .up_to_n_times(1)
        .mount(server)
        .await;
    response_mock
}

pub async fn mount_compact_json_once_match<M>(
    server: &MockServer,
    matcher: M,
    body: serde_json::Value,
) -> ResponseMock
where
    M: wiremock::Match + Send + Sync + 'static,
{
    let (mock, response_mock) = compact_mock();
    mock.and(matcher)
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_json(body.clone()),
        )
        .up_to_n_times(1)
        .mount(server)
        .await;
    response_mock
}

pub async fn mount_compact_json_once(server: &MockServer, body: serde_json::Value) -> ResponseMock {
    let (mock, response_mock) = compact_mock();
    mock.respond_with(
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .set_body_json(body.clone()),
    )
    .up_to_n_times(1)
    .mount(server)
    .await;
    response_mock
}

/// Streaming SSE chunk payload with a per-chunk delay.
#[derive(Clone, Debug)]
pub struct StreamingSseChunk {
    pub delay: Duration,
    pub body: String,
}

/// Minimal streaming SSE server for tests that need per-chunk delays.
pub struct StreamingSseServer {
    uri: String,
    shutdown: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

impl StreamingSseServer {
    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
        let _ = self.task.await;
    }
}

/// Starts a lightweight HTTP server that supports:
/// - GET /v1/models -> empty models response
/// - POST /v1/responses -> SSE stream with per-chunk delays, served in order
///
/// Returns the server handle and a list of receivers that fire when each
/// response stream finishes sending its final chunk.
pub async fn start_streaming_sse_server(
    responses: Vec<Vec<StreamingSseChunk>>,
) -> (StreamingSseServer, Vec<oneshot::Receiver<i64>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind streaming SSE server");
    let addr = listener.local_addr().expect("streaming SSE server address");
    let uri = format!("http://{addr}");

    let mut completion_senders = Vec::with_capacity(responses.len());
    let mut completion_receivers = Vec::with_capacity(responses.len());
    for _ in 0..responses.len() {
        let (tx, rx) = oneshot::channel();
        completion_senders.push(tx);
        completion_receivers.push(rx);
    }

    let state = Arc::new(TokioMutex::new(StreamingSseState {
        responses: VecDeque::from(responses),
        completions: VecDeque::from(completion_senders),
    }));
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accept_res = listener.accept() => {
                    let (mut stream, _) = accept_res.expect("accept streaming SSE connection");
                    let state = Arc::clone(&state);
                    tokio::spawn(async move {
                        let request = read_http_request(&mut stream).await;
                        let Some((method, path)) = parse_request_line(&request) else {
                            let _ = write_http_response(&mut stream, 400, "bad request", "text/plain").await;
                            return;
                        };

                        if method == "GET" && path == "/v1/models" {
                            let body = serde_json::json!({
                                "data": [],
                                "object": "list"
                            })
                            .to_string();
                            let _ = write_http_response(&mut stream, 200, &body, "application/json").await;
                            return;
                        }

                        if method == "POST" && path == "/v1/responses" {
                            let Some((chunks, completion)) = take_next_stream(&state).await else {
                                let _ = write_http_response(&mut stream, 500, "no responses queued", "text/plain").await;
                                return;
                            };

                            if write_sse_headers(&mut stream).await.is_err() {
                                return;
                            }

                            for chunk in chunks {
                                if !chunk.delay.is_zero() {
                                    tokio::time::sleep(chunk.delay).await;
                                }
                                if stream.write_all(chunk.body.as_bytes()).await.is_err() {
                                    return;
                                }
                                let _ = stream.flush().await;
                            }

                            let _ = completion.send(unix_ms_now());
                            let _ = stream.shutdown().await;
                            return;
                        }

                        let _ = write_http_response(&mut stream, 404, "not found", "text/plain").await;
                    });
                }
            }
        }
    });

    (
        StreamingSseServer {
            uri,
            shutdown: shutdown_tx,
            task,
        },
        completion_receivers,
    )
}

struct StreamingSseState {
    responses: VecDeque<Vec<StreamingSseChunk>>,
    completions: VecDeque<oneshot::Sender<i64>>,
}

async fn take_next_stream(
    state: &TokioMutex<StreamingSseState>,
) -> Option<(Vec<StreamingSseChunk>, oneshot::Sender<i64>)> {
    let mut guard = state.lock().await;
    let chunks = guard.responses.pop_front()?;
    let completion = guard.completions.pop_front()?;
    Some((chunks, completion))
}

async fn read_http_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut buf = Vec::new();
    let mut scratch = [0u8; 1024];
    loop {
        let read = stream.read(&mut scratch).await.unwrap_or(0);
        if read == 0 {
            break;
        }
        buf.extend_from_slice(&scratch[..read]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

fn parse_request_line(request: &str) -> Option<(&str, &str)> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    Some((method, path))
}

async fn write_sse_headers(stream: &mut tokio::net::TcpStream) -> std::io::Result<()> {
    let headers = "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-cache\r\nconnection: close\r\n\r\n";
    stream.write_all(headers.as_bytes()).await
}

async fn write_http_response(
    stream: &mut tokio::net::TcpStream,
    status: i64,
    body: &str,
    content_type: &str,
) -> std::io::Result<()> {
    let body_len = body.len();
    let headers = format!(
        "HTTP/1.1 {status} OK\r\ncontent-type: {content_type}\r\ncontent-length: {body_len}\r\nconnection: close\r\n\r\n"
    );
    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.shutdown().await
}

fn unix_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub async fn mount_models_once(server: &MockServer, body: ModelsResponse) -> ModelsMock {
    let (mock, models_mock) = models_mock();
    mock.respond_with(
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/json")
            .set_body_json(body.clone()),
    )
    .up_to_n_times(1)
    .mount(server)
    .await;
    models_mock
}

pub async fn start_mock_server() -> MockServer {
    let server = MockServer::builder()
        .body_print_limit(BodyPrintLimit::Limited(80_000))
        .start()
        .await;

    // Provide a default `/models` response so tests remain hermetic when the client queries it.
    let _ = mount_models_once(
        &server,
        ModelsResponse {
            models: Vec::new(),
            etag: String::new(),
        },
    )
    .await;

    server
}

// todo(aibrahim): remove this and use our search matching patterns directly
/// Get all POST requests to `/responses` endpoints from the mock server.
/// Filters out GET requests (e.g., `/models`) .
pub async fn get_responses_requests(server: &MockServer) -> Vec<wiremock::Request> {
    server
        .received_requests()
        .await
        .expect("mock server should not fail")
        .into_iter()
        .filter(|req| req.method == "POST" && req.url.path().ends_with("/responses"))
        .collect()
}

// todo(aibrahim): remove this and use our search matching patterns directly
/// Get request bodies as JSON values from POST requests to `/responses` endpoints.
/// Filters out GET requests (e.g., `/models`) .
pub async fn get_responses_request_bodies(server: &MockServer) -> Vec<Value> {
    get_responses_requests(server)
        .await
        .into_iter()
        .map(|req| {
            req.body_json::<Value>()
                .expect("request body to be valid JSON")
        })
        .collect()
}

#[derive(Clone)]
pub struct FunctionCallResponseMocks {
    pub function_call: ResponseMock,
    pub completion: ResponseMock,
}

pub async fn mount_function_call_agent_response(
    server: &MockServer,
    call_id: &str,
    arguments: &str,
    tool_name: &str,
) -> FunctionCallResponseMocks {
    let first_response = sse(vec![
        ev_response_created("resp-1"),
        ev_function_call(call_id, tool_name, arguments),
        ev_completed("resp-1"),
    ]);
    let function_call = mount_sse_once(server, first_response).await;

    let second_response = sse(vec![
        ev_assistant_message("msg-1", "done"),
        ev_completed("resp-2"),
    ]);
    let completion = mount_sse_once(server, second_response).await;

    FunctionCallResponseMocks {
        function_call,
        completion,
    }
}

/// Mounts a sequence of SSE response bodies and serves them in order for each
/// POST to `/v1/responses`. Panics if more requests are received than bodies
/// provided. Also asserts the exact number of expected calls.
pub async fn mount_sse_sequence(server: &MockServer, bodies: Vec<String>) -> ResponseMock {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    struct SeqResponder {
        num_calls: AtomicUsize,
        responses: Vec<String>,
    }

    impl Respond for SeqResponder {
        fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
            let call_num = self.num_calls.fetch_add(1, Ordering::SeqCst);
            match self.responses.get(call_num) {
                Some(body) => ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream")
                    .set_body_string(body.clone()),
                None => panic!("no response for {call_num}"),
            }
        }
    }

    let num_calls = bodies.len();
    let responder = SeqResponder {
        num_calls: AtomicUsize::new(0),
        responses: bodies,
    };

    let (mock, response_mock) = base_mock();
    mock.respond_with(responder)
        .up_to_n_times(num_calls as u64)
        .expect(num_calls as u64)
        .mount(server)
        .await;

    response_mock
}

/// Validate invariants on the request body sent to `/v1/responses`.
///
/// - No `function_call_output`/`custom_tool_call_output` with missing/empty `call_id`.
/// - Every `function_call_output` must match a prior `function_call` or
///   `local_shell_call` with the same `call_id` in the same `input`.
/// - Every `custom_tool_call_output` must match a prior `custom_tool_call`.
/// - Additionally, enforce symmetry: every `function_call`/`custom_tool_call`
///   in the `input` must have a matching output entry.
fn validate_request_body_invariants(request: &wiremock::Request) {
    // Skip GET requests (e.g., /models)
    if request.method != "POST" || !request.url.path().ends_with("/responses") {
        return;
    }
    let Ok(body): Result<Value, _> = request.body_json() else {
        return;
    };
    let Some(items) = body.get("input").and_then(Value::as_array) else {
        panic!("input array not found in request");
    };

    use std::collections::HashSet;

    fn get_call_id(item: &Value) -> Option<&str> {
        item.get("call_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
    }

    fn gather_ids(items: &[Value], kind: &str) -> HashSet<String> {
        items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some(kind))
            .filter_map(get_call_id)
            .map(str::to_string)
            .collect()
    }

    fn gather_output_ids(items: &[Value], kind: &str, missing_msg: &str) -> HashSet<String> {
        items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some(kind))
            .map(|item| {
                let Some(id) = get_call_id(item) else {
                    panic!("{missing_msg}");
                };
                id.to_string()
            })
            .collect()
    }

    let function_calls = gather_ids(items, "function_call");
    let custom_tool_calls = gather_ids(items, "custom_tool_call");
    let local_shell_calls = gather_ids(items, "local_shell_call");
    let function_call_outputs = gather_output_ids(
        items,
        "function_call_output",
        "orphan function_call_output with empty call_id should be dropped",
    );
    let custom_tool_call_outputs = gather_output_ids(
        items,
        "custom_tool_call_output",
        "orphan custom_tool_call_output with empty call_id should be dropped",
    );

    for cid in &function_call_outputs {
        assert!(
            function_calls.contains(cid) || local_shell_calls.contains(cid),
            "function_call_output without matching call in input: {cid}",
        );
    }
    for cid in &custom_tool_call_outputs {
        assert!(
            custom_tool_calls.contains(cid),
            "custom_tool_call_output without matching call in input: {cid}",
        );
    }

    for cid in &function_calls {
        assert!(
            function_call_outputs.contains(cid),
            "Function call output is missing for call id: {cid}",
        );
    }
    for cid in &custom_tool_calls {
        assert!(
            custom_tool_call_outputs.contains(cid),
            "Custom tool call output is missing for call id: {cid}",
        );
    }
}
