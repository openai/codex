//! Browser-facing prototype facade for a future Codex WASM harness.
//!
//! This crate intentionally starts outside `codex-core`: the first milestone is
//! a working browser boundary that streams Codex-shaped turn events. Later
//! iterations can replace the direct model request with the real Codex
//! `RegularTask` / `run_turn` path as host services become injectable.

use js_sys::Function;
use js_sys::Reflect;
use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Headers;
use web_sys::RequestInit;
use web_sys::RequestMode;
use web_sys::Response;

const DEFAULT_INSTRUCTIONS: &str = "You are Codex running in a browser WASM prototype.";
const RESPONSES_API_URL: &str = "https://api.openai.com/v1/responses";
const RESPONSES_MODEL: &str = "gpt-5.1";

/// Browser entrypoint for the prototype harness.
#[wasm_bindgen]
pub struct BrowserCodex {
    api_key: String,
    next_turn_id: u32,
}

#[wasm_bindgen]
impl BrowserCodex {
    /// Creates a new browser harness.
    ///
    /// `api_key` may be empty. When it is empty, the harness uses a
    /// deterministic local demo response. When it is present, the harness makes
    /// a browser `fetch` call to the Responses API from Rust/WASM.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            next_turn_id: 0,
        }
    }

    /// Updates the API key used by future turns.
    pub fn set_api_key(&mut self, api_key: String) {
        self.api_key = api_key;
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

        let agent_message = self.sample(&prompt).await?;

        emit_event(
            &on_event,
            &HarnessEvent::AgentMessageDelta {
                turn_id: turn_id.clone(),
                delta: agent_message.clone(),
            },
        )?;
        emit_event(
            &on_event,
            &HarnessEvent::AgentMessage {
                turn_id: turn_id.clone(),
                message: agent_message.clone(),
            },
        )?;
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
    async fn sample(&self, prompt: &str) -> Result<String, JsValue> {
        if self.api_key.trim().is_empty() {
            return Ok(default_demo_response(prompt));
        }

        sample_with_responses_api(self.api_key.trim(), prompt).await
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponsesRequest<'a> {
    model: &'static str,
    instructions: &'static str,
    input: &'a str,
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
    TurnComplete {
        turn_id: String,
        last_agent_message: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
struct ResponsesBody {
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
    content: Option<Vec<ResponsesContentItem>>,
}

#[derive(Debug, Deserialize)]
struct ResponsesContentItem {
    text: Option<String>,
    output_text: Option<String>,
}

async fn sample_with_responses_api(api_key: &str, prompt: &str) -> Result<String, JsValue> {
    let body = ResponsesRequest {
        model: RESPONSES_MODEL,
        instructions: DEFAULT_INSTRUCTIONS,
        input: prompt,
    };
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
            .map_err(response_error)?;
    let response: Response = response_value.dyn_into()?;
    let status = response.status();
    let ok = response.ok();
    let json = JsFuture::from(response.json()?)
        .await
        .map_err(response_error)?;
    let response_body = parse_response_body(json)?;

    if !ok {
        let message = response_body
            .error
            .and_then(|err| err.message)
            .unwrap_or_else(|| format!("Responses API returned {status}"));
        return Err(js_error(message));
    }

    Ok(extract_response_text(response_body))
}

fn parse_response_body(value: JsValue) -> Result<ResponsesBody, JsValue> {
    let json = js_sys::JSON::stringify(&value)?
        .as_string()
        .ok_or_else(|| js_error("Responses API returned non-JSON output"))?;
    serde_json::from_str(&json).map_err(js_error)
}

fn extract_response_text(response: ResponsesBody) -> String {
    if let Some(output_text) = response.output_text
        && !output_text.is_empty()
    {
        return output_text;
    }

    let mut chunks = Vec::new();
    for item in response.output.unwrap_or_default() {
        for content in item.content.unwrap_or_default() {
            if let Some(text) = content.text {
                chunks.push(text);
            } else if let Some(output_text) = content.output_text {
                chunks.push(output_text);
            }
        }
    }

    if chunks.is_empty() {
        "Responses API returned no output text.".to_string()
    } else {
        chunks.join("\n")
    }
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
        format!(
            "Browser Codex prototype received the prompt, but no API key was configured: {prompt}"
        )
    }
}

fn response_error(error: JsValue) -> JsValue {
    if let Some(message) = error.as_string() {
        return js_error(message);
    }

    if error.is_object()
        && let Ok(value) = Reflect::get(&error, &JsValue::from_str("message"))
        && let Some(message) = value.as_string()
    {
        return js_error(message);
    }

    js_error("Responses API request failed")
}

fn js_error(message: impl ToString) -> JsValue {
    JsValue::from_str(&message.to_string())
}

#[cfg(test)]
mod tests {
    use super::default_demo_response;
    use pretty_assertions::assert_eq;

    #[test]
    fn default_demo_response_handles_hello_world() {
        assert_eq!(
            default_demo_response("write hello world"),
            "Here is a minimal hello world example:\n\n```js\nconsole.log(\"hello world\");\n```"
        );
    }
}
