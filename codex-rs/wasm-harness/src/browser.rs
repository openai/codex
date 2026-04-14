use crate::EXEC_JS_TOOL_NAME;
use crate::EmbeddedHarness;
use crate::EventSink;
use crate::HarnessConfig;
use crate::HarnessError;
use crate::ResponsesClient;
use crate::ResponsesFunctionCall;
use crate::ResponsesRequest;
use crate::ResponsesResponse;
use crate::ResponsesTool;
use crate::ToolExecutor;
use async_trait::async_trait;
use js_sys::Function;
use js_sys::Promise;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::Headers;
use web_sys::RequestInit;
use web_sys::RequestMode;
use web_sys::Response;

const RESPONSES_API_URL: &str = "https://api.openai.com/v1/responses";

#[derive(Default)]
struct BrowserResponsesClient {
    api_key: String,
}

impl BrowserResponsesClient {
    fn set_api_key(&mut self, api_key: String) {
        self.api_key = api_key;
    }
}

#[async_trait(?Send)]
impl ResponsesClient for BrowserResponsesClient {
    async fn create_response(
        &self,
        request: ResponsesRequest,
    ) -> Result<ResponsesResponse, HarnessError> {
        if self.api_key.trim().is_empty() {
            return Ok(default_demo_response(&request.input.to_string()));
        }

        let body = serde_json::to_string(&request)?;

        let headers = Headers::new().map_err(js_exception)?;
        headers
            .append("Authorization", &format!("Bearer {}", self.api_key.trim()))
            .map_err(js_exception)?;
        headers
            .append("Content-Type", "application/json")
            .map_err(js_exception)?;

        let request_init = RequestInit::new();
        request_init.set_method("POST");
        request_init.set_mode(RequestMode::Cors);
        request_init.set_headers(&headers);
        request_init.set_body(&JsValue::from_str(&body));

        let window = web_sys::window().ok_or_else(|| HarnessError::new("window is unavailable"))?;
        let response_value =
            JsFuture::from(window.fetch_with_str_and_init(RESPONSES_API_URL, &request_init))
                .await
                .map_err(js_fetch_error)?;
        let response: Response = response_value.dyn_into().map_err(js_exception)?;
        let status = response.status();
        let ok = response.ok();
        let json = JsFuture::from(response.json().map_err(js_exception)?)
            .await
            .map_err(js_fetch_error)?;
        let response_body = parse_response_body(json)?;

        if !ok {
            let message = response_body
                .error
                .as_ref()
                .and_then(|err| err.message.clone())
                .unwrap_or_else(|| format!("Responses API returned {status}"));
            return Err(HarnessError::new(message));
        }

        Ok(response_body)
    }
}

#[derive(Default)]
struct BrowserToolExecutor {
    code_executor: Option<Function>,
}

impl BrowserToolExecutor {
    fn set_code_executor(&mut self, executor: Function) {
        self.code_executor = Some(executor);
    }

    fn clear_code_executor(&mut self) {
        self.code_executor = None;
    }
}

#[async_trait(?Send)]
impl ToolExecutor for BrowserToolExecutor {
    fn tools(&self) -> Vec<ResponsesTool> {
        self.code_executor
            .as_ref()
            .map(|_| vec![ResponsesTool::exec_js()])
            .unwrap_or_default()
    }

    async fn execute(&self, function_call: &ResponsesFunctionCall) -> Result<String, HarnessError> {
        if function_call.name != EXEC_JS_TOOL_NAME {
            return Err(HarnessError::new(format!(
                "browser prototype does not implement tool `{}`",
                function_call.name
            )));
        }

        let executor = self.code_executor.as_ref().ok_or_else(|| {
            HarnessError::new("`exec_js` was requested but no browser executor is registered")
        })?;
        let args: ExecJsArguments = serde_json::from_str(&function_call.arguments)?;
        let value = executor
            .call1(&JsValue::NULL, &JsValue::from_str(&args.code))
            .map_err(js_exception)?;
        let value = await_possible_promise(value).await?;
        js_value_to_string(value)
    }
}

struct JsEventSink<'a> {
    on_event: &'a Function,
}

impl EventSink for JsEventSink<'_> {
    fn emit(&self, event: &crate::HarnessEvent) -> Result<(), HarnessError> {
        let json = serde_json::to_string(event)?;
        let value = js_sys::JSON::parse(&json).map_err(js_exception)?;
        self.on_event
            .call1(&JsValue::NULL, &value)
            .map_err(js_exception)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct ExecJsArguments {
    code: String,
}

/// Browser entrypoint for the prototype harness.
#[wasm_bindgen]
pub struct BrowserCodex {
    harness: EmbeddedHarness<BrowserResponsesClient, BrowserToolExecutor>,
}

#[wasm_bindgen]
impl BrowserCodex {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(api_key: String) -> Self {
        let mut client = BrowserResponsesClient::default();
        client.set_api_key(api_key);
        let tool_executor = BrowserToolExecutor::default();
        let harness = EmbeddedHarness::new(HarnessConfig::default(), client, tool_executor);
        Self { harness }
    }

    pub fn set_api_key(&mut self, api_key: String) {
        self.harness.responses_client_mut().set_api_key(api_key);
    }

    pub fn set_code_executor(&mut self, executor: Function) {
        self.harness.tool_executor_mut().set_code_executor(executor);
    }

    pub fn clear_code_executor(&mut self) {
        self.harness.tool_executor_mut().clear_code_executor();
    }

    pub async fn submit_turn(
        &mut self,
        prompt: String,
        on_event: Function,
    ) -> Result<JsValue, JsValue> {
        let sink = JsEventSink {
            on_event: &on_event,
        };
        let agent_message = self
            .harness
            .submit_turn(prompt, &sink)
            .await
            .map_err(harness_error_to_js)?;
        Ok(JsValue::from_str(&agent_message))
    }
}

fn parse_response_body(value: JsValue) -> Result<ResponsesResponse, HarnessError> {
    let json = js_sys::JSON::stringify(&value)
        .map_err(js_exception)?
        .as_string()
        .ok_or_else(|| HarnessError::new("Responses API returned non-JSON output"))?;
    serde_json::from_str(&json).map_err(HarnessError::from)
}

async fn await_possible_promise(value: JsValue) -> Result<JsValue, HarnessError> {
    if let Ok(promise) = value.clone().dyn_into::<Promise>() {
        JsFuture::from(promise).await.map_err(js_exception)
    } else {
        Ok(value)
    }
}

fn js_value_to_string(value: JsValue) -> Result<String, HarnessError> {
    if let Some(text) = value.as_string() {
        return Ok(text);
    }

    if value.is_undefined() || value.is_null() {
        return Ok(String::new());
    }

    let json = js_sys::JSON::stringify(&value).map_err(js_exception)?;
    Ok(json
        .as_string()
        .unwrap_or_else(|| "[non-string value]".to_string()))
}

fn js_exception(error: JsValue) -> HarnessError {
    HarnessError::new(js_value_to_string_lossy(&error))
}

fn js_fetch_error(error: JsValue) -> HarnessError {
    HarnessError::new(format!(
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

fn harness_error_to_js(error: HarnessError) -> JsValue {
    JsValue::from_str(error.message())
}

fn default_demo_response(input: &str) -> ResponsesResponse {
    let prompt = serde_json::from_str::<String>(input).unwrap_or_else(|_| input.to_string());
    let output_text = if prompt.to_ascii_lowercase().contains("hello world") {
        "Here is a minimal hello world example:\n\n```js\nconsole.log(\"hello world\");\n```"
            .to_string()
    } else {
        format!("Demo mode is active because no API key was provided. Prompt received:\n\n{prompt}")
    };
    ResponsesResponse {
        id: Some("demo-response".to_string()),
        output_text: Some(output_text),
        output: Some(Vec::new()),
        error: None,
    }
}
