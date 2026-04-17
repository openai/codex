use crate::HarnessError;
use codex_app_server_protocol::ApprovalsReviewer;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ClientNotification;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::SandboxPolicy as AppSandboxPolicy;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::SessionSource as AppSessionSource;
use codex_app_server_protocol::Thread as AppThread;
use codex_app_server_protocol::ThreadReadResponse;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::ThreadStartedNotification;
use codex_app_server_protocol::ThreadStatus;
use codex_app_server_protocol::Turn as AppTurn;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnInterruptResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStartedNotification;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::TurnSteerResponse;
use codex_core::AuthManager;
use codex_core::CodeModeRuntime;
use codex_core::CodexAuth;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_exec_server::EnvironmentManager;
use codex_features::Feature;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SessionSource;
use js_sys::Function;
use js_sys::Promise;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
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

#[derive(Serialize)]
struct DebugEvent<'a> {
    r#type: &'a str,
    stage: &'a str,
}

#[derive(Serialize)]
struct RawCoreEventEnvelope {
    r#type: &'static str,
    event: Event,
}

enum BrowserCommand {
    SetApiKey(String),
    SetSessionOptions(BrowserSessionOptions),
    SetEventHandler(Option<JsFunctionHandle>),
    SetCodeExecutor(Option<JsFunctionHandle>),
    Request {
        request: ClientRequest,
        response_tx: oneshot::Sender<Result<JsonValue, HarnessError>>,
    },
    Notify {
        notification: ClientNotification,
        response_tx: oneshot::Sender<Result<(), HarnessError>>,
    },
    Shutdown {
        response_tx: oneshot::Sender<Result<(), HarnessError>>,
    },
}

struct BrowserWorkerState {
    api_key: String,
    session_options: BrowserSessionOptions,
    runtime_state: Arc<BrowserRuntimeState>,
    event_handler: Rc<RefCell<Option<JsFunctionHandle>>>,
    threads: HashMap<String, BrowserThreadHandle>,
}

struct BrowserThreadHandle {
    config: Config,
    thread: Arc<CodexThread>,
    state: Rc<RefCell<BrowserThreadState>>,
}

struct BrowserThreadState {
    thread: AppThread,
}

impl BrowserCodeModeRuntime {
    fn new(state: Arc<BrowserRuntimeState>) -> Self {
        Self { state }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
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
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = request;
            return Err("browser code mode execution is only supported on wasm32".to_string());
        }

        #[cfg(target_arch = "wasm32")]
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
        #[cfg(target_arch = "wasm32")]
        let executor_input = serde_json::to_string(&executor_request)
            .map_err(|error| format!("failed to serialize browser exec request: {error}"))?;
        #[cfg(target_arch = "wasm32")]
        let executor = self
            .state
            .executor
            .lock()
            .expect("browser code executor mutex poisoned")
            .clone()
            .ok_or_else(|| "no browser code executor is registered".to_string())?;
        #[cfg(target_arch = "wasm32")]
        let value = executor
            .0
            .call1(&JsValue::NULL, &JsValue::from_str(&executor_input))
            .map_err(|error| js_value_to_string_lossy(&error))?;
        #[cfg(target_arch = "wasm32")]
        let value = await_possible_promise(value)
            .await
            .map_err(|err| err.to_string())?;
        #[cfg(target_arch = "wasm32")]
        let response_json = js_value_to_string(value).map_err(|err| err.to_string())?;
        #[cfg(target_arch = "wasm32")]
        let response: JsExecutorResponse = serde_json::from_str(&response_json)
            .map_err(|error| format!("failed to parse browser exec response: {error}"))?;
        #[cfg(target_arch = "wasm32")]
        let text = response.output.unwrap_or_default();
        #[cfg(target_arch = "wasm32")]
        let content_items = if text.is_empty() {
            Vec::new()
        } else {
            vec![codex_code_mode::FunctionCallOutputContentItem::InputText { text }]
        };
        #[cfg(target_arch = "wasm32")]
        return Ok(codex_code_mode::RuntimeResponse::Result {
            cell_id: request.tool_call_id,
            content_items,
            stored_values: response.stored_values,
            error_text: response.error_text,
        });

        #[allow(unreachable_code)]
        Err("browser code mode execution is unavailable".to_string())
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

/// Browser entrypoint for the wasm Codex app-server-shaped prototype.
#[wasm_bindgen]
pub struct BrowserAppServer {
    command_tx: mpsc::UnboundedSender<BrowserCommand>,
}

#[wasm_bindgen]
impl BrowserAppServer {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(api_key: String) -> Self {
        let runtime_state = Arc::new(BrowserRuntimeState::default());
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        wasm_bindgen_futures::spawn_local(async move {
            let local = LocalSet::new();
            local
                .run_until(browser_worker_loop(command_rx, api_key, runtime_state))
                .await;
        });
        Self { command_tx }
    }

    pub fn set_api_key(&self, api_key: String) -> Result<(), JsValue> {
        self.command_tx
            .send(BrowserCommand::SetApiKey(api_key))
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)
    }

    #[wasm_bindgen(js_name = setSessionOptions)]
    pub fn set_session_options(&self, options: JsValue) -> Result<(), JsValue> {
        let parsed = parse_browser_session_options(options).map_err(harness_error_to_js)?;
        self.command_tx
            .send(BrowserCommand::SetSessionOptions(parsed))
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)
    }

    #[wasm_bindgen(js_name = clearSessionOptions)]
    pub fn clear_session_options(&self) -> Result<(), JsValue> {
        self.command_tx
            .send(BrowserCommand::SetSessionOptions(
                BrowserSessionOptions::default(),
            ))
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)
    }

    #[wasm_bindgen(js_name = setEventHandler)]
    pub fn set_event_handler(&self, handler: Function) -> Result<(), JsValue> {
        self.command_tx
            .send(BrowserCommand::SetEventHandler(Some(JsFunctionHandle(
                handler,
            ))))
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)
    }

    #[wasm_bindgen(js_name = clearEventHandler)]
    pub fn clear_event_handler(&self) -> Result<(), JsValue> {
        self.command_tx
            .send(BrowserCommand::SetEventHandler(None))
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)
    }

    pub fn set_code_executor(&self, executor: Function) -> Result<(), JsValue> {
        self.command_tx
            .send(BrowserCommand::SetCodeExecutor(Some(JsFunctionHandle(
                executor,
            ))))
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)
    }

    pub fn clear_code_executor(&self) -> Result<(), JsValue> {
        self.command_tx
            .send(BrowserCommand::SetCodeExecutor(None))
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)
    }

    pub async fn request(&self, request: JsValue) -> Result<JsValue, JsValue> {
        let request: ClientRequest = parse_json_value(request).map_err(harness_error_to_js)?;
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(BrowserCommand::Request {
                request,
                response_tx,
            })
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)?;
        let response = response_rx
            .await
            .map_err(|_| HarnessError::new("browser worker response channel closed"))
            .map_err(harness_error_to_js)?;
        let json = response.map_err(harness_error_to_js)?;
        json_value_to_js(json).map_err(harness_error_to_js)
    }

    pub async fn notify(&self, notification: JsValue) -> Result<(), JsValue> {
        let notification: ClientNotification =
            parse_json_value(notification).map_err(harness_error_to_js)?;
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(BrowserCommand::Notify {
                notification,
                response_tx,
            })
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)?;
        response_rx
            .await
            .map_err(|_| HarnessError::new("browser worker response channel closed"))
            .map_err(harness_error_to_js)?
            .map_err(harness_error_to_js)
    }

    pub async fn shutdown(&self) -> Result<(), JsValue> {
        let (response_tx, response_rx) = oneshot::channel();
        self.command_tx
            .send(BrowserCommand::Shutdown { response_tx })
            .map_err(|_| HarnessError::new("browser worker is unavailable"))
            .map_err(harness_error_to_js)?;
        response_rx
            .await
            .map_err(|_| HarnessError::new("browser worker response channel closed"))
            .map_err(harness_error_to_js)?
            .map_err(harness_error_to_js)
    }
}

async fn browser_worker_loop(
    mut command_rx: mpsc::UnboundedReceiver<BrowserCommand>,
    api_key: String,
    runtime_state: Arc<BrowserRuntimeState>,
) {
    let event_handler = Rc::new(RefCell::new(None::<JsFunctionHandle>));
    let mut state = BrowserWorkerState {
        api_key,
        session_options: BrowserSessionOptions::default(),
        runtime_state,
        event_handler,
        threads: HashMap::new(),
    };
    emit_debug(&state.event_handler, "worker:ready");

    while let Some(command) = command_rx.recv().await {
        match command {
            BrowserCommand::SetApiKey(api_key) => {
                if state.api_key != api_key {
                    if let Err(error) = reset_threads(&mut state).await {
                        emit_debug(
                            &state.event_handler,
                            &format!("reset_threads:error:{error}"),
                        );
                    }
                    state.api_key = api_key;
                }
            }
            BrowserCommand::SetSessionOptions(options) => {
                if state.session_options != options {
                    if let Err(error) = reset_threads(&mut state).await {
                        emit_debug(
                            &state.event_handler,
                            &format!("reset_threads:error:{error}"),
                        );
                    }
                    state.session_options = options;
                }
            }
            BrowserCommand::SetEventHandler(handler) => {
                *state.event_handler.borrow_mut() = handler;
            }
            BrowserCommand::SetCodeExecutor(executor) => {
                *state
                    .runtime_state
                    .executor
                    .lock()
                    .expect("browser code executor mutex poisoned") = executor;
            }
            BrowserCommand::Request {
                request,
                response_tx,
            } => {
                let response = handle_request(&mut state, request).await;
                let _ = response_tx.send(response);
            }
            BrowserCommand::Notify {
                notification,
                response_tx,
            } => {
                let response = match notification {
                    ClientNotification::Initialized => Ok(()),
                };
                let _ = response_tx.send(response);
            }
            BrowserCommand::Shutdown { response_tx } => {
                let response = reset_threads(&mut state).await;
                let _ = response_tx.send(response);
                break;
            }
        }
    }
}

async fn handle_request(
    state: &mut BrowserWorkerState,
    request: ClientRequest,
) -> Result<JsonValue, HarnessError> {
    emit_debug(
        &state.event_handler,
        &format!("request:{}", request.method()),
    );
    match request {
        ClientRequest::ThreadStart { params, .. } => {
            let response = handle_thread_start(state, params).await?;
            serde_json::to_value(response).map_err(Into::into)
        }
        ClientRequest::ThreadRead { params, .. } => {
            let response = handle_thread_read(state, params.thread_id, params.include_turns)?;
            serde_json::to_value(response).map_err(Into::into)
        }
        ClientRequest::TurnStart { params, .. } => {
            let response = handle_turn_start(state, params).await?;
            serde_json::to_value(response).map_err(Into::into)
        }
        ClientRequest::TurnInterrupt { params, .. } => {
            let response = handle_turn_interrupt(state, params.thread_id).await?;
            serde_json::to_value(response).map_err(Into::into)
        }
        ClientRequest::TurnSteer { params, .. } => {
            let response = handle_turn_steer(
                state,
                params.thread_id,
                params.input,
                params.expected_turn_id,
            )
            .await?;
            serde_json::to_value(response).map_err(Into::into)
        }
        unsupported => Err(HarnessError::new(format!(
            "unsupported browser request method: {}",
            unsupported.method()
        ))),
    }
}

async fn handle_thread_start(
    state: &mut BrowserWorkerState,
    params: ThreadStartParams,
) -> Result<ThreadStartResponse, HarnessError> {
    if state.api_key.trim().is_empty() {
        return Err(HarnessError::new(
            "an OpenAI API key is required to run the browser app-server prototype",
        ));
    }

    let config = build_browser_config(&state.session_options, Some(&params))?;
    let auth_manager =
        AuthManager::from_auth_for_testing(CodexAuth::from_api_key(state.api_key.trim()));
    let environment_manager = Arc::new(EnvironmentManager::new(None));
    let manager = ThreadManager::new(
        &config,
        auth_manager,
        SessionSource::Custom("codex_wasm".to_string()),
        CollaborationModesConfig::default(),
        environment_manager,
    );
    let new_thread = manager
        .start_thread_with_code_mode_runtime(
            config.clone(),
            Arc::new(BrowserCodeModeRuntime::new(Arc::clone(
                &state.runtime_state,
            ))),
        )
        .await
        .map_err(|error| {
            HarnessError::new(format!("start_thread_with_code_mode_runtime: {error}"))
        })?;

    let config_snapshot = new_thread.thread.config_snapshot().await;
    let created_at = now_seconds();
    let session_source = config_snapshot.session_source.clone();
    let app_thread = AppThread {
        id: new_thread.thread_id.to_string(),
        preview: String::new(),
        ephemeral: config.ephemeral,
        model_provider: config_snapshot.model_provider_id.clone(),
        created_at,
        updated_at: created_at,
        status: ThreadStatus::Idle,
        path: None,
        cwd: config.cwd.to_path_buf(),
        cli_version: env!("CARGO_PKG_VERSION").to_string(),
        source: AppSessionSource::from(session_source.clone()),
        agent_nickname: session_source.get_nickname(),
        agent_role: session_source.get_agent_role(),
        git_info: None,
        name: None,
        turns: Vec::new(),
    };
    let thread_state = Rc::new(RefCell::new(BrowserThreadState {
        thread: app_thread.clone(),
    }));
    let thread_id = app_thread.id.clone();
    let thread = Arc::clone(&new_thread.thread);
    let event_handler = Rc::clone(&state.event_handler);
    let listener_state = Rc::clone(&thread_state);
    tokio::task::spawn_local(async move {
        run_thread_listener(thread_id, thread, listener_state, event_handler).await;
    });

    state.threads.insert(
        app_thread.id.clone(),
        BrowserThreadHandle {
            config,
            thread: new_thread.thread,
            state: Rc::clone(&thread_state),
        },
    );

    emit_notification(
        &state.event_handler,
        ServerNotification::ThreadStarted(ThreadStartedNotification {
            thread: app_thread.clone(),
        }),
    )?;

    Ok(ThreadStartResponse {
        thread: app_thread,
        model: config_snapshot.model,
        model_provider: config_snapshot.model_provider_id,
        service_tier: config_snapshot.service_tier,
        approval_policy: AskForApproval::from(config_snapshot.approval_policy),
        approvals_reviewer: ApprovalsReviewer::from(config_snapshot.approvals_reviewer),
        sandbox: AppSandboxPolicy::from(config_snapshot.sandbox_policy),
        cwd: config_snapshot.cwd,
        reasoning_effort: config_snapshot.reasoning_effort,
    })
}

fn handle_thread_read(
    state: &BrowserWorkerState,
    thread_id: String,
    include_turns: bool,
) -> Result<ThreadReadResponse, HarnessError> {
    let handle = state
        .threads
        .get(&thread_id)
        .ok_or_else(|| HarnessError::new(format!("unknown thread id: {thread_id}")))?;
    let mut thread = handle.state.borrow().thread.clone();
    if !include_turns {
        thread.turns.clear();
    }
    Ok(ThreadReadResponse { thread })
}

async fn handle_turn_start(
    state: &mut BrowserWorkerState,
    params: TurnStartParams,
) -> Result<TurnStartResponse, HarnessError> {
    let handle = state
        .threads
        .get(&params.thread_id)
        .ok_or_else(|| HarnessError::new(format!("unknown thread id: {}", params.thread_id)))?;

    if let Some(preview) = first_text_preview(&params.input) {
        let mut thread_state = handle.state.borrow_mut();
        if thread_state.thread.preview.is_empty() {
            thread_state.thread.preview = preview;
        }
    }

    let items = params
        .input
        .clone()
        .into_iter()
        .map(codex_app_server_protocol::UserInput::into_core)
        .collect();

    let submission_id = handle
        .thread
        .submit(Op::UserTurn {
            items,
            cwd: params
                .cwd
                .unwrap_or_else(|| handle.config.cwd.as_path().to_path_buf()),
            approval_policy: params
                .approval_policy
                .map(AskForApproval::to_core)
                .unwrap_or_else(|| handle.config.permissions.approval_policy.get().clone()),
            approvals_reviewer: params
                .approvals_reviewer
                .map(ApprovalsReviewer::to_core)
                .or(Some(handle.config.approvals_reviewer)),
            sandbox_policy: params
                .sandbox_policy
                .as_ref()
                .map(AppSandboxPolicy::to_core)
                .unwrap_or_else(|| handle.config.permissions.sandbox_policy.get().clone()),
            model: params.model.unwrap_or_else(|| {
                handle
                    .config
                    .model
                    .clone()
                    .unwrap_or_else(|| DEFAULT_MODEL.to_string())
            }),
            effort: params.effort.or(handle.config.model_reasoning_effort),
            summary: params.summary.or(handle.config.model_reasoning_summary),
            service_tier: params.service_tier,
            final_output_json_schema: params.output_schema,
            collaboration_mode: None,
            personality: params.personality,
        })
        .await
        .map_err(|error| HarnessError::new(format!("turn/start failed: {error}")))?;

    {
        let mut thread_state = handle.state.borrow_mut();
        thread_state.thread.status = ThreadStatus::Active {
            active_flags: Vec::new(),
        };
        thread_state.thread.updated_at = now_seconds();
        upsert_turn(
            &mut thread_state.thread.turns,
            AppTurn {
                id: submission_id.clone(),
                items: Vec::new(),
                status: TurnStatus::InProgress,
                error: None,
            },
        );
    }

    Ok(TurnStartResponse {
        turn: AppTurn {
            id: submission_id,
            items: Vec::new(),
            status: TurnStatus::InProgress,
            error: None,
        },
    })
}

async fn handle_turn_interrupt(
    state: &BrowserWorkerState,
    thread_id: String,
) -> Result<TurnInterruptResponse, HarnessError> {
    let handle = state
        .threads
        .get(&thread_id)
        .ok_or_else(|| HarnessError::new(format!("unknown thread id: {thread_id}")))?;
    handle
        .thread
        .submit(Op::Interrupt)
        .await
        .map_err(|error| HarnessError::new(format!("turn/interrupt failed: {error}")))?;
    Ok(TurnInterruptResponse {})
}

async fn handle_turn_steer(
    state: &BrowserWorkerState,
    thread_id: String,
    input: Vec<codex_app_server_protocol::UserInput>,
    expected_turn_id: String,
) -> Result<TurnSteerResponse, HarnessError> {
    let handle = state
        .threads
        .get(&thread_id)
        .ok_or_else(|| HarnessError::new(format!("unknown thread id: {thread_id}")))?;
    let turn_id = handle
        .thread
        .steer_input(
            input
                .into_iter()
                .map(codex_app_server_protocol::UserInput::into_core)
                .collect(),
            Some(expected_turn_id.as_str()),
        )
        .await
        .map_err(|error| HarnessError::new(format!("turn/steer failed: {error:?}")))?;
    Ok(TurnSteerResponse { turn_id })
}

async fn reset_threads(state: &mut BrowserWorkerState) -> Result<(), HarnessError> {
    let threads = state
        .threads
        .drain()
        .map(|(_, handle)| handle.thread)
        .collect::<Vec<_>>();
    for thread in threads {
        thread
            .shutdown_and_wait()
            .await
            .map_err(|error| HarnessError::new(format!("thread shutdown failed: {error}")))?;
    }
    Ok(())
}

async fn run_thread_listener(
    thread_id: String,
    thread: Arc<CodexThread>,
    thread_state: Rc<RefCell<BrowserThreadState>>,
    event_handler: Rc<RefCell<Option<JsFunctionHandle>>>,
) {
    loop {
        let event = match thread.next_event().await {
            Ok(event) => event,
            Err(error) => {
                emit_debug(&event_handler, &format!("thread_listener:error:{error}"));
                break;
            }
        };
        emit_raw_core_event(&event_handler, event.clone());
        match event.msg {
            EventMsg::TurnStarted(turn) => {
                let turn_snapshot = {
                    let mut state = thread_state.borrow_mut();
                    state.thread.status = ThreadStatus::Active {
                        active_flags: Vec::new(),
                    };
                    state.thread.updated_at = now_seconds();
                    let turn_snapshot = AppTurn {
                        id: turn.turn_id.clone(),
                        items: Vec::new(),
                        status: TurnStatus::InProgress,
                        error: None,
                    };
                    upsert_turn(&mut state.thread.turns, turn_snapshot.clone());
                    turn_snapshot
                };
                let _ = emit_notification(
                    &event_handler,
                    ServerNotification::TurnStarted(TurnStartedNotification {
                        thread_id: thread_id.clone(),
                        turn: turn_snapshot,
                    }),
                );
            }
            EventMsg::TurnComplete(turn) => {
                let turn_snapshot = {
                    let mut state = thread_state.borrow_mut();
                    state.thread.status = ThreadStatus::Idle;
                    state.thread.updated_at = now_seconds();
                    let turn_snapshot = AppTurn {
                        id: turn.turn_id.clone(),
                        items: Vec::new(),
                        status: TurnStatus::Completed,
                        error: None,
                    };
                    upsert_turn(&mut state.thread.turns, turn_snapshot.clone());
                    turn_snapshot
                };
                let _ = emit_notification(
                    &event_handler,
                    ServerNotification::TurnCompleted(TurnCompletedNotification {
                        thread_id: thread_id.clone(),
                        turn: turn_snapshot,
                    }),
                );
            }
            EventMsg::TurnAborted(turn) => {
                if let Some(turn_id) = turn.turn_id {
                    let turn_snapshot = {
                        let mut state = thread_state.borrow_mut();
                        state.thread.status = ThreadStatus::Idle;
                        state.thread.updated_at = now_seconds();
                        let turn_snapshot = AppTurn {
                            id: turn_id.clone(),
                            items: Vec::new(),
                            status: TurnStatus::Interrupted,
                            error: None,
                        };
                        upsert_turn(&mut state.thread.turns, turn_snapshot.clone());
                        turn_snapshot
                    };
                    let _ = emit_notification(
                        &event_handler,
                        ServerNotification::TurnCompleted(TurnCompletedNotification {
                            thread_id: thread_id.clone(),
                            turn: turn_snapshot,
                        }),
                    );
                }
            }
            _ => {}
        }
    }
}

fn upsert_turn(turns: &mut Vec<AppTurn>, turn: AppTurn) {
    if let Some(existing) = turns.iter_mut().find(|existing| existing.id == turn.id) {
        *existing = turn;
    } else {
        turns.push(turn);
    }
}

fn first_text_preview(input: &[codex_app_server_protocol::UserInput]) -> Option<String> {
    input.iter().find_map(|item| match item {
        codex_app_server_protocol::UserInput::Text { text, .. } => Some(text.clone()),
        _ => None,
    })
}

fn build_browser_config(
    session_options: &BrowserSessionOptions,
    thread_start: Option<&ThreadStartParams>,
) -> Result<Config, HarnessError> {
    build_browser_config_at(
        session_options,
        thread_start,
        PathBuf::from(BROWSER_CODEX_HOME),
    )
}

fn build_browser_config_at(
    session_options: &BrowserSessionOptions,
    thread_start: Option<&ThreadStartParams>,
    codex_home: PathBuf,
) -> Result<Config, HarnessError> {
    let cwd = thread_start
        .and_then(|params| params.cwd.as_deref())
        .or(session_options.cwd.as_deref())
        .unwrap_or(BROWSER_CWD);
    let mut config = Config::load_embedded_defaults(codex_home, PathBuf::from(cwd))
        .map_err(|error| HarnessError::new(error.to_string()))?;
    let _ = config.features.enable(Feature::CodeMode);
    let _ = config.features.enable(Feature::CodeModeOnly);
    config.model = thread_start
        .and_then(|params| params.model.clone())
        .or_else(|| config.model.clone())
        .or_else(|| Some(DEFAULT_MODEL.to_string()));
    config.base_instructions = thread_start
        .and_then(|params| params.base_instructions.clone())
        .or_else(|| session_options.instructions.base.clone());
    config.developer_instructions = thread_start
        .and_then(|params| params.developer_instructions.clone())
        .or_else(|| session_options.instructions.developer.clone());
    config.user_instructions = session_options.instructions.user.clone();
    if let Some(params) = thread_start {
        if let Some(model_provider) = params.model_provider.clone() {
            config.model_provider_id = model_provider;
        }
        if let Some(ephemeral) = params.ephemeral {
            config.ephemeral = ephemeral;
        }
    }
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

fn parse_json_value<T>(value: JsValue) -> Result<T, HarnessError>
where
    T: for<'de> Deserialize<'de>,
{
    let text = js_sys::JSON::stringify(&value)
        .map_err(js_exception)?
        .as_string()
        .ok_or_else(|| HarnessError::new("javascript value must be JSON-serializable"))?;
    serde_json::from_str(&text).map_err(|error| HarnessError::new(error.to_string()))
}

fn emit_debug(event_handler: &Rc<RefCell<Option<JsFunctionHandle>>>, stage: &str) {
    let payload = DebugEvent {
        r#type: "debug",
        stage,
    };
    let _ = emit_payload(event_handler, &payload);
}

fn emit_raw_core_event(event_handler: &Rc<RefCell<Option<JsFunctionHandle>>>, event: Event) {
    let payload = RawCoreEventEnvelope {
        r#type: "coreEvent",
        event,
    };
    let _ = emit_payload(event_handler, &payload);
}

fn emit_notification(
    event_handler: &Rc<RefCell<Option<JsFunctionHandle>>>,
    notification: ServerNotification,
) -> Result<(), HarnessError> {
    emit_payload(event_handler, &notification)
}

fn emit_payload<T: Serialize>(
    event_handler: &Rc<RefCell<Option<JsFunctionHandle>>>,
    payload: &T,
) -> Result<(), HarnessError> {
    let Some(handler) = event_handler.borrow().clone() else {
        return Ok(());
    };
    let value = serde_json::to_value(payload)?;
    let js_value = json_value_to_js(value)?;
    handler
        .0
        .call1(&JsValue::NULL, &js_value)
        .map_err(js_exception)?;
    Ok(())
}

fn json_value_to_js(value: JsonValue) -> Result<JsValue, HarnessError> {
    let text = serde_json::to_string(&value)?;
    js_sys::JSON::parse(&text).map_err(js_exception)
}

fn now_seconds() -> i64 {
    (js_sys::Date::now() / 1000.0).floor() as i64
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
    use super::build_browser_config_at;
    use codex_app_server_protocol::ThreadStartParams;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn test_codex_home(test_name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "codex-wasm-harness-{test_name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp codex home");
        path
    }

    #[test]
    fn build_browser_config_applies_session_options() {
        let config = build_browser_config_at(
            &BrowserSessionOptions {
                cwd: Some("/workspace/repo".to_string()),
                instructions: BrowserInstructionOverrides {
                    base: Some("base".to_string()),
                    developer: Some("developer".to_string()),
                    user: Some("user".to_string()),
                },
            },
            None,
            test_codex_home("session-options"),
        )
        .expect("browser config");

        assert_eq!(config.cwd.as_path().to_string_lossy(), "/workspace/repo");
        assert_eq!(config.base_instructions.as_deref(), Some("base"));
        assert_eq!(config.developer_instructions.as_deref(), Some("developer"));
        assert_eq!(config.user_instructions.as_deref(), Some("user"));
    }

    #[test]
    fn build_browser_config_applies_thread_start_overrides() {
        let config = build_browser_config_at(
            &BrowserSessionOptions::default(),
            Some(&ThreadStartParams {
                model: Some("gpt-test".to_string()),
                cwd: Some("/workspace/override".to_string()),
                base_instructions: Some("base".to_string()),
                developer_instructions: Some("developer".to_string()),
                ..ThreadStartParams::default()
            }),
            test_codex_home("thread-start"),
        )
        .expect("browser config");

        assert_eq!(
            config.cwd.as_path().to_string_lossy(),
            "/workspace/override"
        );
        assert_eq!(config.model.as_deref(), Some("gpt-test"));
        assert_eq!(config.base_instructions.as_deref(), Some("base"));
        assert_eq!(config.developer_instructions.as_deref(), Some("developer"));
    }
}
