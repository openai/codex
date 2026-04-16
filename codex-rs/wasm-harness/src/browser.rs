use crate::HarnessError;
use async_trait::async_trait;
use codex_core::AuthManager;
use codex_core::CodeModeRuntime;
use codex_core::CodexAuth;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_exec_server::EnvironmentManager;
use codex_features::Feature;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionConfiguredEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput;
use js_sys::Function;
use js_sys::Promise;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::task::LocalSet;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

const BROWSER_CODEX_HOME: &str = "/codex-wasm";
const BROWSER_CWD: &str = "/workspace";
const DEFAULT_MODEL: &str = "gpt-5-codex";

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrowserInstructionOverrides {
    base: Option<String>,
    developer: Option<String>,
    user: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BrowserSessionOptions {
    cwd: Option<String>,
    #[serde(default)]
    instructions: BrowserInstructionOverrides,
}

#[derive(Default)]
struct BrowserRuntimeState {
    executor: Mutex<Option<JsFunctionHandle>>,
    stored_values: Mutex<HashMap<String, JsonValue>>,
}

#[derive(Clone)]
struct JsFunctionHandle(Function);

unsafe impl Send for JsFunctionHandle {}
unsafe impl Sync for JsFunctionHandle {}

#[derive(Clone)]
struct BrowserCodeModeRuntime {
    state: Arc<BrowserRuntimeState>,
}

struct BrowserCodeModeTurnWorker;

impl codex_code_mode::CodeModeTurnWorkerHandle for BrowserCodeModeTurnWorker {}

#[derive(Serialize)]
struct JsExecutorToolDefinition {
    name: String,
    description: String,
}

#[derive(Serialize)]
struct JsExecutorRequest {
    source: String,
    stored_values: HashMap<String, JsonValue>,
    enabled_tools: Vec<JsExecutorToolDefinition>,
}

#[derive(Deserialize)]
struct JsExecutorResponse {
    output: Option<String>,
    #[serde(default)]
    stored_values: HashMap<String, JsonValue>,
    error_text: Option<String>,
}

impl BrowserCodeModeRuntime {
    fn new(state: Arc<BrowserRuntimeState>) -> Self {
        Self { state }
    }
}

#[async_trait(?Send)]
impl CodeModeRuntime for BrowserCodeModeRuntime {
    async fn stored_values(&self) -> HashMap<String, JsonValue> {
        self.state
            .stored_values
            .lock()
            .expect("browser stored values mutex poisoned")
            .clone()
    }

    async fn replace_stored_values(&self, values: HashMap<String, JsonValue>) {
        *self
            .state
            .stored_values
            .lock()
            .expect("browser stored values mutex poisoned") = values;
    }

    async fn execute(
        &self,
        request: codex_code_mode::ExecuteRequest,
    ) -> Result<codex_code_mode::RuntimeResponse, String> {
        let executor_request = JsExecutorRequest {
            source: request.source,
            stored_values: request.stored_values,
            enabled_tools: request
                .enabled_tools
                .into_iter()
                .map(|tool| JsExecutorToolDefinition {
                    name: tool.name,
                    description: tool.description,
                })
                .collect(),
        };
        let executor_input = serde_json::to_string(&executor_request)
            .map_err(|error| format!("failed to serialize browser exec request: {error}"))?;
        let executor = self
            .state
            .executor
            .lock()
            .expect("browser code executor mutex poisoned")
            .clone()
            .ok_or_else(|| "no browser code executor is registered".to_string())?;
        let value = executor
            .0
            .call1(&JsValue::NULL, &JsValue::from_str(&executor_input))
            .map_err(|error| js_value_to_string_lossy(&error))?;
        let value = await_possible_promise(value)
            .await
            .map_err(|err| err.to_string())?;
        let response_json = js_value_to_string(value).map_err(|err| err.to_string())?;
        let response: JsExecutorResponse = serde_json::from_str(&response_json)
            .map_err(|error| format!("failed to parse browser exec response: {error}"))?;
        let text = response.output.unwrap_or_default();
        let content_items = if text.is_empty() {
            Vec::new()
        } else {
            vec![codex_code_mode::FunctionCallOutputContentItem::InputText { text }]
        };
        Ok(codex_code_mode::RuntimeResponse::Result {
            cell_id: request.tool_call_id,
            content_items,
            stored_values: response.stored_values,
            error_text: response.error_text,
        })
    }

    async fn wait(
        &self,
        request: codex_code_mode::WaitRequest,
    ) -> Result<codex_code_mode::RuntimeResponse, String> {
        Ok(codex_code_mode::RuntimeResponse::Result {
            cell_id: request.cell_id,
            content_items: Vec::new(),
            stored_values: self.stored_values().await,
            error_text: Some("browser code mode wait is not implemented".to_string()),
        })
    }

    fn start_turn_worker(
        &self,
        _host: Arc<dyn codex_code_mode::CodeModeTurnHost>,
    ) -> Box<dyn codex_code_mode::CodeModeTurnWorkerHandle> {
        Box::new(BrowserCodeModeTurnWorker)
    }
}

struct BrowserSession {
    config: Config,
    thread: Arc<codex_core::CodexThread>,
    session_configured: SessionConfiguredEvent,
}

struct JsEventSink<'a> {
    on_event: &'a Function,
}

impl JsEventSink<'_> {
    fn emit_debug(&self, stage: &str) -> Result<(), HarnessError> {
        let payload = serde_json::json!({
            "type": "debug",
            "stage": stage,
        });
        let value = js_sys::JSON::parse(&payload.to_string()).map_err(js_exception)?;
        self.on_event
            .call1(&JsValue::NULL, &value)
            .map_err(js_exception)?;
        Ok(())
    }

    fn emit_event(&self, event: &Event) -> Result<(), HarnessError> {
        let json = serde_json::to_string(event)?;
        let value = js_sys::JSON::parse(&json).map_err(js_exception)?;
        self.on_event
            .call1(&JsValue::NULL, &value)
            .map_err(js_exception)?;
        Ok(())
    }

    fn emit_msg(&self, msg: EventMsg) -> Result<(), HarnessError> {
        self.emit_event(&Event {
            id: String::new(),
            msg,
        })
    }
}

/// Browser entrypoint for the wasm Codex harness.
#[wasm_bindgen]
pub struct BrowserCodex {
    api_key: String,
    runtime_state: Arc<BrowserRuntimeState>,
    session: Option<BrowserSession>,
    session_options: BrowserSessionOptions,
}

#[wasm_bindgen]
impl BrowserCodex {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            runtime_state: Arc::new(BrowserRuntimeState::default()),
            session: None,
            session_options: BrowserSessionOptions::default(),
        }
    }

    pub fn set_api_key(&mut self, api_key: String) {
        self.api_key = api_key;
        self.session = None;
    }

    #[wasm_bindgen(js_name = setSessionOptions)]
    pub fn set_session_options(&mut self, options: JsValue) -> Result<(), JsValue> {
        let parsed = parse_browser_session_options(options).map_err(harness_error_to_js)?;
        if self.session_options != parsed {
            self.session = None;
        }
        self.session_options = parsed;
        Ok(())
    }

    #[wasm_bindgen(js_name = clearSessionOptions)]
    pub fn clear_session_options(&mut self) {
        self.session_options = BrowserSessionOptions::default();
        self.session = None;
    }

    pub fn set_code_executor(&mut self, executor: Function) {
        *self
            .runtime_state
            .executor
            .lock()
            .expect("browser code executor mutex poisoned") = Some(JsFunctionHandle(executor));
    }

    pub fn clear_code_executor(&mut self) {
        *self
            .runtime_state
            .executor
            .lock()
            .expect("browser code executor mutex poisoned") = None;
    }

    pub async fn submit_turn(
        &mut self,
        prompt: String,
        on_event: Function,
    ) -> Result<JsValue, JsValue> {
        let local = LocalSet::new();
        let result = local
            .run_until(self.submit_turn_inner(prompt, on_event))
            .await;
        // Local tasks spawned for a turn are scoped to this LocalSet, so the
        // browser prototype starts a fresh harness session for each submission.
        self.session = None;
        result
    }
}

impl BrowserCodex {
    async fn submit_turn_inner(
        &mut self,
        prompt: String,
        on_event: Function,
    ) -> Result<JsValue, JsValue> {
        if self.api_key.trim().is_empty() {
            return Err(harness_error_to_js(HarnessError::new(
                "an OpenAI API key is required to run the real Codex harness",
            )));
        }

        let sink = JsEventSink {
            on_event: &on_event,
        };
        sink.emit_debug("submit_turn:start")
            .map_err(harness_error_to_js)?;
        let session = if let Some(session) = self.session.as_ref() {
            sink.emit_debug("submit_turn:reuse_session")
                .map_err(harness_error_to_js)?;
            session
        } else {
            sink.emit_debug("submit_turn:create_session")
                .map_err(harness_error_to_js)?;
            let session = self.create_session().await.map_err(|error| {
                harness_error_to_js(HarnessError::new(format!("create_session failed: {error}")))
            })?;
            sink.emit_msg(EventMsg::SessionConfigured(
                session.session_configured.clone(),
            ))
            .map_err(harness_error_to_js)?;
            self.session = Some(session);
            sink.emit_debug("submit_turn:session_created")
                .map_err(harness_error_to_js)?;
            self.session
                .as_ref()
                .expect("browser session just initialized")
        };

        sink.emit_debug("submit_turn:calling_submit")
            .map_err(harness_error_to_js)?;
        let submission_id = session
            .thread
            .submit(Op::UserTurn {
                items: vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }],
                cwd: session.config.cwd.as_path().to_path_buf(),
                approval_policy: session.config.permissions.approval_policy.get().clone(),
                approvals_reviewer: Some(session.config.approvals_reviewer),
                sandbox_policy: session.config.permissions.sandbox_policy.get().clone(),
                model: session
                    .config
                    .model
                    .clone()
                    .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
                effort: session.config.model_reasoning_effort,
                summary: session.config.model_reasoning_summary,
                service_tier: None,
                final_output_json_schema: None,
                collaboration_mode: None,
                personality: None,
            })
            .await
            .map_err(|error| {
                harness_error_to_js(HarnessError::new(format!("submit failed: {error}")))
            })?;
        sink.emit_debug("submit_turn:submit_returned")
            .map_err(harness_error_to_js)?;

        loop {
            let event = session.thread.next_event().await.map_err(|error| {
                harness_error_to_js(HarnessError::new(format!("next_event failed: {error}")))
            })?;
            sink.emit_event(&event).map_err(harness_error_to_js)?;
            if event.id == submission_id
                && matches!(
                    event.msg,
                    EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) | EventMsg::Error(_)
                )
            {
                break;
            }
        }

        Ok(JsValue::from_str(&submission_id))
    }

    async fn create_session(&self) -> Result<BrowserSession, HarnessError> {
        let config = build_browser_config(&self.session_options)?;
        let auth_manager =
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key(self.api_key.trim()));
        let environment_manager = Arc::new(EnvironmentManager::new(None));
        let manager = ThreadManager::new(
            &config,
            auth_manager,
            SessionSource::Custom("codex_wasm".to_string()),
            CollaborationModesConfig::default(),
            environment_manager,
        );
        let thread = manager
            .start_thread_with_code_mode_runtime(
                config.clone(),
                Arc::new(BrowserCodeModeRuntime::new(Arc::clone(&self.runtime_state))),
            )
            .await
            .map_err(|error| {
                HarnessError::new(format!("start_thread_with_code_mode_runtime: {error}"))
            })?;
        Ok(BrowserSession {
            config,
            thread: thread.thread,
            session_configured: thread.session_configured,
        })
    }
}

fn build_browser_config(session_options: &BrowserSessionOptions) -> Result<Config, HarnessError> {
    let cwd = session_options.cwd.as_deref().unwrap_or(BROWSER_CWD);
    let mut config =
        Config::load_embedded_defaults(PathBuf::from(BROWSER_CODEX_HOME), PathBuf::from(cwd))
            .map_err(|error| HarnessError::new(error.to_string()))?;
    let _ = config.features.enable(Feature::CodeMode);
    let _ = config.features.enable(Feature::CodeModeOnly);
    config.model = Some(
        config
            .model
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL.to_string()),
    );
    config.base_instructions = session_options.instructions.base.clone();
    config.developer_instructions = session_options.instructions.developer.clone();
    config.user_instructions = session_options.instructions.user.clone();
    Ok(config)
}

fn parse_browser_session_options(options: JsValue) -> Result<BrowserSessionOptions, HarnessError> {
    if options.is_null() || options.is_undefined() {
        return Ok(BrowserSessionOptions::default());
    }

    let json = js_sys::JSON::stringify(&options).map_err(js_exception)?;
    let text = json
        .as_string()
        .ok_or_else(|| HarnessError::new("browser session options must be JSON-serializable"))?;
    serde_json::from_str(&text)
        .map_err(|error| HarnessError::new(format!("invalid browser session options: {error}")))
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

#[cfg(test)]
mod tests {
    use super::BrowserInstructionOverrides;
    use super::BrowserSessionOptions;
    use super::build_browser_config;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_browser_config_applies_session_options() {
        let config = build_browser_config(&BrowserSessionOptions {
            cwd: Some("/workspace/repo".to_string()),
            instructions: BrowserInstructionOverrides {
                base: Some("base".to_string()),
                developer: Some("developer".to_string()),
                user: Some("user".to_string()),
            },
        })
        .expect("browser config");

        assert_eq!(config.cwd.as_path().to_string_lossy(), "/workspace/repo");
        assert_eq!(config.base_instructions.as_deref(), Some("base"));
        assert_eq!(config.developer_instructions.as_deref(), Some("developer"));
        assert_eq!(config.user_instructions.as_deref(), Some("user"));
    }

    #[test]
    fn build_browser_config_defaults_to_workspace_cwd() {
        let config =
            build_browser_config(&BrowserSessionOptions::default()).expect("browser config");

        assert_eq!(config.cwd.as_path().to_string_lossy(), "/workspace");
    }
}
