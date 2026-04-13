//! Browser-facing prototype facade for a future Codex WASM harness.
//!
//! This crate intentionally starts outside `codex-core`: the first milestone is
//! a working browser boundary that can run a minimal Codex-shaped turn loop in
//! the browser. The loop in this crate now uses the real Responses API and can
//! execute a browser-provided code tool callback, but it still does not call
//! `codex-core::run_turn` or `RegularTask::run`.

use js_sys::Function;
use js_sys::Promise;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Headers;
use web_sys::RequestInit;
use web_sys::RequestMode;
use web_sys::Response;

const BASE_INSTRUCTIONS: &str = include_str!("../../core/prompt.md");
const BROWSER_TOOLING_NOTE: &str = concat!(
    "Browser prototype note:\n",
    "- Only one host tool is available: `exec_js`.\n",
    "- `exec_js` runs JavaScript inside a browser-managed sandbox and returns stdout-like text.\n",
    "- Native shell, filesystem, MCP, and plugin tools are unavailable in this prototype."
);
const RESPONSES_API_URL: &str = "https://api.openai.com/v1/responses";
const RESPONSES_MODEL: &str = "gpt-5.1";
const MAX_TOOL_ROUNDS: usize = 8;
const EXEC_JS_TOOL_NAME: &str = "exec_js";

/// Browser entrypoint for the prototype harness.
#[wasm_bindgen]
pub struct BrowserCodex {
    api_key: String,
    next_turn_id: u32,
    code_executor: Option<Function>,
}

#[wasm_bindgen]
impl BrowserCodex {
    /// Creates a new browser harness.
    ///
    /// `api_key` may be empty. When it is empty, the harness uses a
    /// deterministic local demo response. When it is present, the harness makes
    /// browser `fetch` calls to the Responses API from Rust/WASM.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            next_turn_id: 0,
            code_executor: None,
        }
    }

    /// Updates the API key used by future turns.
    pub fn set_api_key(&mut self, api_key: String) {
        self.api_key = api_key;
    }

    /// Registers the JavaScript callback used for the `exec_js` tool.
    pub fn set_code_executor(&mut self, executor: Function) {
        self.code_executor = Some(executor);
    }

    /// Clears the browser-side `exec_js` tool callback.
    pub fn clear_code_executor(&mut self) {
        self.code_executor = None;
    }

    /// Submits one browser turn and calls `on_event` for every emitted event.
    ///
    /// The method resolves after `turn_complete` has been emitted. This mirrors
    /// Codex's event-driven shape rather than exposing a `prompt -> string`
    /// shortcut.
    pub async fn submit_turn(
        &mut self,
        prompt: String,
        on_event: Function,
    ) -> Result<JsValue, JsValue> {
        self.next_turn_id += 1;
        let turn_id = format!("browser-turn-{}", self.next_turn_id);

        emit_event(
            &on_event,
            &HarnessEvent::TurnStarted {
                turn_id: turn_id.clone(),
                model_context_window: None,
                collaboration_mode_kind: "default",
            },
        )?;
        emit_event(
            &on_event,
            &HarnessEvent::UserMessage {
                turn_id: turn_id.clone(),
                message: prompt.clone(),
            },
        )?;

        let agent_message = if self.api_key.trim().is_empty() {
            default_demo_response(&prompt)
        } else {
            self.run_turn_with_responses_api(&turn_id, &prompt, &on_event)
                .await?
        };

        emit_event(
            &on_event,
            &HarnessEvent::TurnComplete {
                turn_id,
                last_agent_message: Some(agent_message.clone()),
            },
        )?;

        Ok(JsValue::from_str(&agent_message))
    }
}

impl BrowserCodex {
    async fn run_turn_with_responses_api(
        &self,
        turn_id: &str,
        prompt: &str,
        on_event: &Function,
    ) -> Result<String, JsValue> {
        let mut previous_response_id: Option<String> = None;
        let mut input = Value::String(prompt.to_string());
        let instructions = build_browser_instructions();
        let mut last_agent_message: Option<String> = None;

        for round in 0..MAX_TOOL_ROUNDS {
            let response = fetch_responses_api(
                self.api_key.trim(),
                ResponsesRequestBody {
                    model: RESPONSES_MODEL,
                    instructions: instructions.clone(),
                    input,
                    previous_response_id: previous_response_id.clone(),
                    tools: self.responses_tools(),
                    parallel_tool_calls: false,
                },
            )
            .await?;

            previous_response_id = response.id.clone();

            let agent_message = extract_response_text(&response);
            if !agent_message.is_empty() {
                emit_event(
                    on_event,
                    &HarnessEvent::AgentMessageDelta {
                        turn_id: turn_id.to_string(),
                        delta: agent_message.clone(),
                    },
                )?;
                emit_event(
                    on_event,
                    &HarnessEvent::AgentMessage {
                        turn_id: turn_id.to_string(),
                        message: agent_message.clone(),
                    },
                )?;
                last_agent_message = Some(agent_message);
            }

            let function_calls = extract_function_calls(&response)?;
            if function_calls.is_empty() {
                return Ok(last_agent_message.unwrap_or_else(|| {
                    "Responses API returned no assistant message.".to_string()
                }));
            }

            let response_id = previous_response_id.clone().ok_or_else(|| {
                js_error("Responses API omitted response.id for a tool-calling turn")
            })?;
            let mut tool_outputs = Vec::with_capacity(function_calls.len());
            for function_call in function_calls {
                emit_event(
                    on_event,
                    &HarnessEvent::ToolCallStarted {
                        turn_id: turn_id.to_string(),
                        response_id: response_id.clone(),
                        call_id: function_call.call_id.clone(),
                        name: function_call.name.clone(),
                        arguments: function_call.arguments.clone(),
                    },
                )?;

                let output = self.execute_function_call(&function_call).await;
                match output {
                    Ok(output) => {
                        emit_event(
                            on_event,
                            &HarnessEvent::ToolCallCompleted {
                                turn_id: turn_id.to_string(),
                                response_id: response_id.clone(),
                                call_id: function_call.call_id.clone(),
                                name: function_call.name.clone(),
                                output: output.clone(),
                            },
                        )?;
                        tool_outputs.push(tool_output_item(&function_call.call_id, output));
                    }
                    Err(err) => {
                        emit_event(
                            on_event,
                            &HarnessEvent::TurnError {
                                turn_id: turn_id.to_string(),
                                message: err.as_string().unwrap_or_else(|| {
                                    "tool execution failed with a non-string JavaScript error"
                                        .to_string()
                                }),
                            },
                        )?;
                        return Err(err);
                    }
                }
            }

            input = Value::Array(tool_outputs);

            if round + 1 == MAX_TOOL_ROUNDS {
                return Err(js_error("turn exceeded the browser tool-round limit"));
            }
        }

        Err(js_error("browser turn loop exited unexpectedly"))
    }

    fn responses_tools(&self) -> Option<Vec<ResponsesTool>> {
        self.code_executor
            .as_ref()
            .map(|_| vec![ResponsesTool::exec_js()])
    }

    async fn execute_function_call(
        &self,
        function_call: &ResponsesFunctionCall,
    ) -> Result<String, JsValue> {
        if function_call.name != EXEC_JS_TOOL_NAME {
            return Err(js_error(format!(
                "browser prototype does not implement tool `{}`",
                function_call.name
            )));
        }

        let executor = self.code_executor.as_ref().ok_or_else(|| {
            js_error("`exec_js` was requested but no browser executor is registered")
        })?;
        let args: ExecJsArguments =
            serde_json::from_str(&function_call.arguments).map_err(js_error)?;
        let value = executor.call1(&JsValue::NULL, &JsValue::from_str(&args.code))?;
        let value = await_possible_promise(value).await?;
        js_value_to_string(value)
    }
}

#[derive(Debug, Serialize)]
struct ResponsesRequestBody {
    model: &'static str,
    instructions: String,
    input: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    previous_response_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ResponsesTool>>,
    parallel_tool_calls: bool,
}

#[derive(Debug, Serialize)]
struct ResponsesTool {
    #[serde(rename = "type")]
    kind: &'static str,
    name: &'static str,
    description: &'static str,
    parameters: Value,
}

impl ResponsesTool {
    fn exec_js() -> Self {
        Self {
            kind: "function",
            name: EXEC_JS_TOOL_NAME,
            description: "Execute JavaScript inside the browser sandbox and return the textual result.",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "code": {
                        "type": "string",
                        "description": "JavaScript source code to execute in the browser sandbox."
                    }
                },
                "required": ["code"],
                "additionalProperties": false
            }),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HarnessEvent<'a> {
    TurnStarted {
        turn_id: String,
        model_context_window: Option<i64>,
        collaboration_mode_kind: &'a str,
    },
    UserMessage {
        turn_id: String,
        message: String,
    },
    AgentMessageDelta {
        turn_id: String,
        delta: String,
    },
    AgentMessage {
        turn_id: String,
        message: String,
    },
    ToolCallStarted {
        turn_id: String,
        response_id: String,
        call_id: String,
        name: String,
        arguments: String,
    },
    ToolCallCompleted {
        turn_id: String,
        response_id: String,
        call_id: String,
        name: String,
        output: String,
    },
    TurnError {
        turn_id: String,
        message: String,
    },
    TurnComplete {
        turn_id: String,
        last_agent_message: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct ResponsesBody {
    id: Option<String>,
    output_text: Option<String>,
    output: Option<Vec<ResponsesOutputItem>>,
    error: Option<ResponsesError>,
}

#[derive(Debug, Deserialize)]
struct ResponsesError {
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponsesOutputItem {
    #[serde(rename = "type")]
    kind: String,
    call_id: Option<String>,
    name: Option<String>,
    arguments: Option<String>,
    content: Option<Vec<ResponsesContentItem>>,
}

#[derive(Debug, Deserialize)]
struct ResponsesContentItem {
    text: Option<String>,
    output_text: Option<String>,
}

#[derive(Debug)]
struct ResponsesFunctionCall {
    call_id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ExecJsArguments {
    code: String,
}

async fn fetch_responses_api(
    api_key: &str,
    body: ResponsesRequestBody,
) -> Result<ResponsesBody, JsValue> {
    let body = serde_json::to_string(&body).map_err(js_error)?;

    let headers = Headers::new()?;
    headers.append("Authorization", &format!("Bearer {api_key}"))?;
    headers.append("Content-Type", "application/json")?;

    let request = RequestInit::new();
    request.set_method("POST");
    request.set_mode(RequestMode::Cors);
    request.set_headers(&headers);
    request.set_body(&JsValue::from_str(&body));

    let window = web_sys::window().ok_or_else(|| js_error("window is unavailable"))?;
    let response_value =
        JsFuture::from(window.fetch_with_str_and_init(RESPONSES_API_URL, &request))
            .await
            .map_err(js_fetch_error)?;
    let response: Response = response_value.dyn_into()?;
    let status = response.status();
    let ok = response.ok();
    let json = JsFuture::from(response.json()?)
        .await
        .map_err(js_fetch_error)?;
    let response_body = parse_response_body(json)?;

    if !ok {
        let message = response_body
            .error
            .and_then(|err| err.message)
            .unwrap_or_else(|| format!("Responses API returned {status}"));
        return Err(js_error(message));
    }

    Ok(response_body)
}

fn parse_response_body(value: JsValue) -> Result<ResponsesBody, JsValue> {
    let json = js_sys::JSON::stringify(&value)?
        .as_string()
        .ok_or_else(|| js_error("Responses API returned non-JSON output"))?;
    serde_json::from_str(&json).map_err(js_error)
}

fn extract_response_text(response: &ResponsesBody) -> String {
    if let Some(output_text) = &response.output_text
        && !output_text.is_empty()
    {
        return output_text.clone();
    }

    let mut chunks = Vec::new();
    for item in response.output.as_ref().into_iter().flatten() {
        for content in item.content.as_ref().into_iter().flatten() {
            if let Some(text) = &content.text {
                chunks.push(text.clone());
            } else if let Some(output_text) = &content.output_text {
                chunks.push(output_text.clone());
            }
        }
    }

    chunks.join("\n")
}

fn extract_function_calls(response: &ResponsesBody) -> Result<Vec<ResponsesFunctionCall>, JsValue> {
    let mut function_calls = Vec::new();
    for item in response.output.as_ref().into_iter().flatten() {
        if item.kind != "function_call" {
            continue;
        }
        let call_id = item
            .call_id
            .clone()
            .ok_or_else(|| js_error("Responses API function_call item omitted call_id"))?;
        let name = item
            .name
            .clone()
            .ok_or_else(|| js_error("Responses API function_call item omitted name"))?;
        let arguments = item
            .arguments
            .clone()
            .ok_or_else(|| js_error("Responses API function_call item omitted arguments"))?;
        function_calls.push(ResponsesFunctionCall {
            call_id,
            name,
            arguments,
        });
    }
    Ok(function_calls)
}

fn tool_output_item(call_id: &str, output: String) -> Value {
    serde_json::json!({
        "type": "function_call_output",
        "call_id": call_id,
        "output": output,
    })
}

fn build_browser_instructions() -> String {
    format!("{BASE_INSTRUCTIONS}\n\n{BROWSER_TOOLING_NOTE}")
}

async fn await_possible_promise(value: JsValue) -> Result<JsValue, JsValue> {
    if let Ok(promise) = value.clone().dyn_into::<Promise>() {
        JsFuture::from(promise).await
    } else {
        Ok(value)
    }
}

fn js_value_to_string(value: JsValue) -> Result<String, JsValue> {
    if let Some(text) = value.as_string() {
        return Ok(text);
    }

    if value.is_undefined() || value.is_null() {
        return Ok(String::new());
    }

    let json = js_sys::JSON::stringify(&value)?;
    Ok(json
        .as_string()
        .unwrap_or_else(|| "[non-string value]".to_string()))
}

fn emit_event(on_event: &Function, event: &HarnessEvent<'_>) -> Result<(), JsValue> {
    let json = serde_json::to_string(event).map_err(js_error)?;
    let value = js_sys::JSON::parse(&json)?;
    on_event.call1(&JsValue::NULL, &value)?;
    Ok(())
}

fn default_demo_response(prompt: &str) -> String {
    if prompt.to_ascii_lowercase().contains("hello world") {
        "Here is a minimal hello world example:\n\n```js\nconsole.log(\"hello world\");\n```"
            .to_string()
    } else {
        format!("Demo mode is active because no API key was provided. Prompt received:\n\n{prompt}")
    }
}

fn js_error(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn js_fetch_error(error: JsValue) -> JsValue {
    JsValue::from_str(&format!(
        "browser fetch failed: {}",
        js_value_to_string_lossy(&error)
    ))
}

fn js_value_to_string_lossy(value: &JsValue) -> String {
    if let Some(text) = value.as_string() {
        return text;
    }

    js_sys::JSON::stringify(value)
        .ok()
        .and_then(|text| text.as_string())
        .unwrap_or_else(|| "[non-string javascript error]".to_string())
}

#[cfg(test)]
mod tests {
    use super::ResponsesBody;
    use super::build_browser_instructions;
    use super::extract_function_calls;
    use super::extract_response_text;
    use pretty_assertions::assert_eq;

    #[test]
    fn browser_instructions_append_browser_note() {
        let instructions = build_browser_instructions();
        assert!(instructions.contains("Only one host tool is available: `exec_js`"));
        assert!(instructions.contains("You are a coding agent running in the Codex CLI"));
    }

    #[test]
    fn extracts_output_text_when_present() {
        let response: ResponsesBody = serde_json::from_str(
            r#"{
                "id": "resp_123",
                "output_text": "final answer",
                "output": []
            }"#,
        )
        .expect("response should deserialize");
        assert_eq!(extract_response_text(&response), "final answer");
    }

    #[test]
    fn extracts_function_calls_from_output_items() {
        let response: ResponsesBody = serde_json::from_str(
            r#"{
                "id": "resp_123",
                "output": [
                    {
                        "type": "function_call",
                        "call_id": "call_123",
                        "name": "exec_js",
                        "arguments": "{\"code\":\"console.log('hi')\"}"
                    }
                ]
            }"#,
        )
        .expect("response should deserialize");

        let function_calls =
            extract_function_calls(&response).expect("function calls should parse");
        assert_eq!(function_calls.len(), 1);
        assert_eq!(function_calls[0].call_id, "call_123");
        assert_eq!(function_calls[0].name, "exec_js");
        assert_eq!(
            function_calls[0].arguments,
            r#"{"code":"console.log('hi')"}"#
        );
    }
}
