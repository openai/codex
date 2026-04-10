//! Browser-facing prototype facade for a future Codex WASM harness.
//!
//! This crate intentionally starts outside `codex-core`: the first milestone is
//! a working browser boundary that streams Codex-shaped turn events. Later
//! iterations can replace the sampler callback with the real Codex
//! `RegularTask` / `run_turn` path as host services become injectable.

use js_sys::Function;
use js_sys::Promise;
use js_sys::Reflect;
use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

const DEFAULT_INSTRUCTIONS: &str = "You are Codex running in a browser WASM prototype.";

/// Browser entrypoint for the prototype harness.
#[wasm_bindgen]
pub struct BrowserCodex {
    sampler: Option<Function>,
    next_turn_id: u32,
}

#[wasm_bindgen]
impl BrowserCodex {
    /// Creates a new browser harness.
    ///
    /// `sampler` may be a JavaScript function that accepts a request object and
    /// returns either a string, `{ message: string }`, or a Promise for either.
    /// When omitted, the harness uses a deterministic local demo response.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(sampler: JsValue) -> Self {
        Self {
            sampler: sampler.dyn_into::<Function>().ok(),
            next_turn_id: 0,
        }
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

        let request = SamplingRequest::new(turn_id.clone(), prompt.clone());
        let agent_message = self.sample(request).await?;

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
    async fn sample(&self, request: SamplingRequest) -> Result<String, JsValue> {
        let Some(sampler) = &self.sampler else {
            return Ok(default_demo_response(&request.prompt));
        };

        let request_json = serde_json::to_string(&request).map_err(js_error)?;
        let request_value = js_sys::JSON::parse(&request_json)?;
        let sampled = sampler.call1(&JsValue::NULL, &request_value)?;
        let resolved = JsFuture::from(Promise::resolve(&sampled)).await?;
        extract_message(resolved)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SamplingRequest {
    turn_id: String,
    prompt: String,
    instructions: &'static str,
    tools: Vec<String>,
}

impl SamplingRequest {
    fn new(turn_id: String, prompt: String) -> Self {
        Self {
            turn_id,
            prompt,
            instructions: DEFAULT_INSTRUCTIONS,
            tools: Vec::new(),
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
    TurnComplete {
        turn_id: String,
        last_agent_message: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SamplerResponse {
    message: Option<String>,
}

fn emit_event(on_event: &Function, event: &HarnessEvent<'_>) -> Result<(), JsValue> {
    let json = serde_json::to_string(event).map_err(js_error)?;
    let value = js_sys::JSON::parse(&json)?;
    on_event.call1(&JsValue::NULL, &value)?;
    Ok(())
}

fn extract_message(value: JsValue) -> Result<String, JsValue> {
    if let Some(message) = value.as_string() {
        return Ok(message);
    }

    if value.is_object() {
        if let Some(message) = Reflect::get(&value, &JsValue::from_str("message"))?.as_string() {
            return Ok(message);
        }

        let json = js_sys::JSON::stringify(&value)?;
        if let Some(json) = json.as_string()
            && let Ok(response) = serde_json::from_str::<SamplerResponse>(&json)
            && let Some(message) = response.message
        {
            return Ok(message);
        }
    }

    Err(js_error(
        "sampler must return a string or an object with a string message field",
    ))
}

fn default_demo_response(prompt: &str) -> String {
    if prompt.to_ascii_lowercase().contains("hello world") {
        "Here is a minimal hello world example:\n\n```js\nconsole.log(\"hello world\");\n```"
            .to_string()
    } else {
        format!(
            "Browser Codex prototype received the prompt, but no model sampler was configured: {prompt}"
        )
    }
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
