use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;
use std::pin::Pin;
use std::sync::LazyLock;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use bytes::Bytes;
use eventsource_stream::Eventsource;
use futures::prelude::*;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::io::ReaderStream;
use tracing::debug;
use tracing::trace;
use tracing::warn;

use crate::error::CodexErr;
use crate::error::Result;
use crate::flags::CODEX_RS_SSE_FIXTURE;
use crate::flags::OPENAI_API_BASE;
use crate::flags::OPENAI_REQUEST_MAX_RETRIES;
use crate::flags::OPENAI_STREAM_IDLE_TIMEOUT_MS;
use crate::flags::get_api_key;
use crate::models::ResponseItem;
use crate::util::backoff;

/// API request payload for a single model turn.
#[derive(Default, Debug, Clone)]
pub struct Prompt {
    /// Conversation context input items.
    pub input: Vec<ResponseItem>,
    /// Optional previous response ID (when storage is enabled).
    pub prev_id: Option<String>,
    /// Optional initial instructions (only sent on first turn).
    pub instructions: Option<String>,
    /// Whether to store response on server side (disable_response_storage = !store).
    pub store: bool,

    /// Additional tools sourced from external MCP servers. Note each key is
    /// the "fully qualified" tool name (i.e., prefixed with the server name),
    /// which should be reported to the model in place of Tool::name.
    pub extra_tools: HashMap<String, mcp_types::Tool>,
}

#[derive(Debug)]
pub enum ResponseEvent {
    OutputItemDone(ResponseItem),
    Completed { response_id: String },
}

#[derive(Debug, Serialize)]
struct Payload<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<&'a String>,
    // TODO(mbolin): ResponseItem::Other should not be serialized. Currently,
    // we code defensively to avoid this case, but perhaps we should use a
    // separate enum for serialization.
    input: &'a Vec<ResponseItem>,
    tools: &'a [serde_json::Value],
    tool_choice: &'static str,
    parallel_tool_calls: bool,
    reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<String>,
    /// true when using the Responses API.
    store: bool,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct Reasoning {
    effort: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    generate_summary: Option<bool>,
}

/// When serialized as JSON, this produces a valid "Tool" in the OpenAI
/// Responses API.
#[derive(Debug, Serialize)]
struct ResponsesApiTool {
    name: &'static str,
    r#type: &'static str, // "function"
    description: &'static str,
    strict: bool,
    parameters: JsonSchema,
}

/// Generic JSON‑Schema subset needed for our tool definitions
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum JsonSchema {
    String,
    Number,
    Array {
        items: Box<JsonSchema>,
    },
    Object {
        properties: BTreeMap<String, JsonSchema>,
        required: &'static [&'static str],
        #[serde(rename = "additionalProperties")]
        additional_properties: bool,
    },
}

/// Tool usage specification
static DEFAULT_TOOLS: LazyLock<Vec<ResponsesApiTool>> = LazyLock::new(|| {
    let mut properties = BTreeMap::new();
    properties.insert(
        "command".to_string(),
        JsonSchema::Array {
            items: Box::new(JsonSchema::String),
        },
    );
    properties.insert("workdir".to_string(), JsonSchema::String);
    properties.insert("timeout".to_string(), JsonSchema::Number);

    vec![ResponsesApiTool {
        name: "shell",
        r#type: "function",
        description: "Runs a shell command, and returns its output.",
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: &["command"],
            additional_properties: false,
        },
    }]
});

#[derive(Clone)]
pub struct ModelClient {
    model: String,
    client: reqwest::Client,
}

impl ModelClient {
    pub fn new(model: impl ToString) -> Self {
        let model = model.to_string();
        let client = reqwest::Client::new();
        Self { model, client }
    }

    pub async fn stream(&mut self, prompt: &Prompt) -> Result<ResponseStream> {
        if let Some(path) = &*CODEX_RS_SSE_FIXTURE {
            // short circuit for tests
            warn!(path, "Streaming from fixture");
            return stream_from_fixture(path).await;
        }

        // Assemble tool list: built-in tools + any extra tools from the prompt.
        let mut tools_json: Vec<serde_json::Value> = DEFAULT_TOOLS
            .iter()
            .map(|t| serde_json::to_value(t).expect("serialize builtin tool"))
            .collect();
        tools_json.extend(
            prompt
                .extra_tools
                .clone()
                .into_iter()
                .map(|(name, tool)| mcp_tool_to_openai_tool(name, tool)),
        );

        debug!("tools_json: {}", serde_json::to_string_pretty(&tools_json)?);

        let payload = Payload {
            model: &self.model,
            instructions: prompt.instructions.as_ref(),
            input: &prompt.input,
            tools: &tools_json,
            tool_choice: "auto",
            parallel_tool_calls: false,
            reasoning: Some(Reasoning {
                effort: "high",
                generate_summary: None,
            }),
            previous_response_id: prompt.prev_id.clone(),
            store: prompt.store,
            stream: true,
        };

        let url = format!("{}/v1/responses", *OPENAI_API_BASE);
        debug!(url, "POST");
        trace!("request payload: {}", serde_json::to_string(&payload)?);

        let mut attempt = 0;
        loop {
            attempt += 1;

            let res = self
                .client
                .post(&url)
                .bearer_auth(get_api_key()?)
                .header("OpenAI-Beta", "responses=experimental")
                .header(reqwest::header::ACCEPT, "text/event-stream")
                .json(&payload)
                .send()
                .await;
            match res {
                Ok(resp) if resp.status().is_success() => {
                    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(16);

                    // spawn task to process SSE
                    let stream = resp.bytes_stream().map_err(CodexErr::Reqwest);
                    tokio::spawn(process_sse(stream, tx_event));

                    return Ok(ResponseStream { rx_event });
                }
                Ok(res) => {
                    let status = res.status();
                    // The OpenAI Responses endpoint returns structured JSON bodies even for 4xx/5xx
                    // errors. When we bubble early with only the HTTP status the caller sees an opaque
                    // "unexpected status 400 Bad Request" which makes debugging nearly impossible.
                    // Instead, read (and include) the response text so higher layers and users see the
                    // exact error message (e.g. "Unknown parameter: 'input[0].metadata'"). The body is
                    // small and this branch only runs on error paths so the extra allocation is
                    // negligible.
                    if !(status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()) {
                        // Surface the error body to callers. Use `unwrap_or_default` per Clippy.
                        let body = (res.text().await).unwrap_or_default();
                        return Err(CodexErr::UnexpectedStatus(status, body));
                    }

                    if attempt > *OPENAI_REQUEST_MAX_RETRIES {
                        return Err(CodexErr::RetryLimit(status));
                    }

                    // Pull out Retry‑After header if present.
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
}

fn mcp_tool_to_openai_tool(
    fully_qualified_name: String,
    tool: mcp_types::Tool,
) -> serde_json::Value {
    // TODO(mbolin): Change the contract of this function to return
    // ResponsesApiTool.
    json!({
        "name": fully_qualified_name,
        "description": tool.description,
        "parameters": tool.input_schema,
        "type": "function",
    })
}

#[derive(Debug, Deserialize, Serialize)]
struct SseEvent {
    #[serde(rename = "type")]
    kind: String,
    response: Option<Value>,
    item: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ResponseCompleted {
    id: String,
}

async fn process_sse<S>(stream: S, tx_event: mpsc::Sender<Result<ResponseEvent>>)
where
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    let mut stream = stream.eventsource();

    // If the stream stays completely silent for an extended period treat it as disconnected.
    let idle_timeout = *OPENAI_STREAM_IDLE_TIMEOUT_MS;
    // The response id returned from the "complete" message.
    let mut response_id = None;

    loop {
        let sse = match timeout(idle_timeout, stream.next()).await {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("SSE Error: {e:#}");
                let event = CodexErr::Stream(e.to_string());
                let _ = tx_event.send(Err(event)).await;
                return;
            }
            Ok(None) => {
                match response_id {
                    Some(response_id) => {
                        let event = ResponseEvent::Completed { response_id };
                        let _ = tx_event.send(Ok(event)).await;
                    }
                    None => {
                        let _ = tx_event
                            .send(Err(CodexErr::Stream(
                                "stream closed before response.completed".into(),
                            )))
                            .await;
                    }
                }
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(CodexErr::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        let event: SseEvent = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(e) => {
                debug!("Failed to parse SSE event: {e}, data: {}", &sse.data);
                continue;
            }
        };

        trace!(?event, "SSE event");
        match event.kind.as_str() {
            // Individual output item finalised. Forward immediately so the
            // rest of the agent can stream assistant text/functions *live*
            // instead of waiting for the final `response.completed` envelope.
            //
            // IMPORTANT: We used to ignore these events and forward the
            // duplicated `output` array embedded in the `response.completed`
            // payload.  That produced two concrete issues:
            //   1. No real‑time streaming – the user only saw output after the
            //      entire turn had finished, which broke the “typing” UX and
            //      made long‑running turns look stalled.
            //   2. Duplicate `function_call_output` items – both the
            //      individual *and* the completed array were forwarded, which
            //      confused the backend and triggered 400
            //      "previous_response_not_found" errors because the duplicated
            //      IDs did not match the incremental turn chain.
            //
            // The fix is to forward the incremental events *as they come* and
            // drop the duplicated list inside `response.completed`.
            "response.output_item.done" => {
                let Some(item_val) = event.item else { continue };
                let Ok(item) = serde_json::from_value::<ResponseItem>(item_val) else {
                    debug!("failed to parse ResponseItem from output_item.done");
                    continue;
                };

                let event = ResponseEvent::OutputItemDone(item);
                if tx_event.send(Ok(event)).await.is_err() {
                    return;
                }
            }
            // Final response completed – includes array of output items & id
            "response.completed" => {
                if let Some(resp_val) = event.response {
                    match serde_json::from_value::<ResponseCompleted>(resp_val) {
                        Ok(r) => {
                            response_id = Some(r.id);
                        }
                        Err(e) => {
                            debug!("failed to parse ResponseCompleted: {e}");
                            continue;
                        }
                    };
                };
            }
            other => debug!(other, "sse event"),
        }
    }
}

pub struct ResponseStream {
    rx_event: mpsc::Receiver<Result<ResponseEvent>>,
}

impl Stream for ResponseStream {
    type Item = Result<ResponseEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx_event.poll_recv(cx)
    }
}

/// used in tests to stream from a text SSE file
async fn stream_from_fixture(path: impl AsRef<Path>) -> Result<ResponseStream> {
    let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent>>(16);
    let f = std::fs::File::open(path.as_ref())?;
    let lines = std::io::BufReader::new(f).lines();

    // insert \n\n after each line for proper SSE parsing
    let mut content = String::new();
    for line in lines {
        content.push_str(&line?);
        content.push_str("\n\n");
    }

    let rdr = std::io::Cursor::new(content);
    let stream = ReaderStream::new(rdr).map_err(CodexErr::Io);
    tokio::spawn(process_sse(stream, tx_event));
    Ok(ResponseStream { rx_event })
}
