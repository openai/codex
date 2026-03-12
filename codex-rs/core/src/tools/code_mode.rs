use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::ToolRouter;
use crate::tools::code_mode_description::augment_tool_spec_for_code_mode;
use crate::tools::code_mode_description::code_mode_tool_reference;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolPayload;
use crate::tools::js_repl::resolve_compatible_node;
use crate::tools::router::ToolCall;
use crate::tools::router::ToolCallSource;
use crate::tools::router::ToolRouterParams;
use crate::truncate::TruncationPolicy;
use crate::truncate::formatted_truncate_text_content_items_with_policy;
use crate::truncate::truncate_function_output_items_with_policy;
use crate::unified_exec::resolve_max_tokens;
use codex_protocol::models::FunctionCallOutputContentItem;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::warn;

const CODE_MODE_RUNNER_SOURCE: &str = include_str!("code_mode_runner.cjs");
const CODE_MODE_BRIDGE_SOURCE: &str = include_str!("code_mode_bridge.js");
pub(crate) const PUBLIC_TOOL_NAME: &str = "exec";
pub(crate) const WAIT_TOOL_NAME: &str = "exec_wait";
pub(crate) const DEFAULT_WAIT_YIELD_TIME_MS: u64 = 10_000;

#[derive(Clone)]
struct ExecContext {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
}

pub(crate) struct CodeModeProcess {
    child: tokio::process::Child,
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    stdout_task: JoinHandle<()>,
    // A set of current requests waiting for a response from code mode host
    response_waiters: Arc<Mutex<HashMap<String, oneshot::Sender<NodeToHostMessage>>>>,
    // When there is an active worker it listens for tool calls from code mode and processes them
    tool_call_rx: Arc<Mutex<mpsc::UnboundedReceiver<CodeModeToolCall>>>,
}

pub(crate) struct CodeModeWorker {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct CodeModeToolCall {
    request_id: String,
    id: String,
    name: String,
    #[serde(default)]
    input: Option<JsonValue>,
}

impl Drop for CodeModeWorker {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

impl CodeModeProcess {
    fn worker(&self, exec: ExecContext) -> CodeModeWorker {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let stdin = Arc::clone(&self.stdin);
        let tool_call_rx = Arc::clone(&self.tool_call_rx);
        tokio::spawn(async move {
            loop {
                let tool_call = tokio::select! {
                    _ = &mut shutdown_rx => break,
                    tool_call = async {
                        let mut tool_call_rx = tool_call_rx.lock().await;
                        tool_call_rx.recv().await
                    } => tool_call,
                };
                let Some(tool_call) = tool_call else {
                    break;
                };
                let exec = exec.clone();
                let stdin = Arc::clone(&stdin);
                tokio::spawn(async move {
                    let response = HostToNodeMessage::Response {
                        request_id: tool_call.request_id,
                        id: tool_call.id,
                        code_mode_result: call_nested_tool(exec, tool_call.name, tool_call.input)
                            .await,
                    };
                    if let Err(err) = write_message(&stdin, &response).await {
                        warn!("failed to write {PUBLIC_TOOL_NAME} tool response: {err}");
                    }
                });
            }
        });

        CodeModeWorker {
            shutdown_tx: Some(shutdown_tx),
        }
    }

    async fn send(
        &mut self,
        request_id: &str,
        message: &HostToNodeMessage,
    ) -> Result<NodeToHostMessage, std::io::Error> {
        if self.stdout_task.is_finished() {
            return Err(std::io::Error::other(format!(
                "{PUBLIC_TOOL_NAME} runner is not available"
            )));
        }

        let (tx, rx) = oneshot::channel();
        self.response_waiters
            .lock()
            .await
            .insert(request_id.to_string(), tx);
        if let Err(err) = write_message(&self.stdin, message).await {
            self.response_waiters.lock().await.remove(request_id);
            return Err(err);
        }

        match rx.await {
            Ok(message) => Ok(message),
            Err(_) => Err(std::io::Error::other(format!(
                "{PUBLIC_TOOL_NAME} runner is not available"
            ))),
        }
    }

    fn has_exited(&mut self) -> Result<bool, std::io::Error> {
        self.child
            .try_wait()
            .map(|status| status.is_some())
            .map_err(std::io::Error::other)
    }
}

pub(crate) struct CodeModeService {
    js_repl_node_path: Option<PathBuf>,
    stored_values: Mutex<HashMap<String, JsonValue>>,
    process: Arc<Mutex<Option<CodeModeProcess>>>,
    next_session_id: Mutex<i32>,
}

impl CodeModeService {
    pub(crate) fn new(js_repl_node_path: Option<PathBuf>) -> Self {
        Self {
            js_repl_node_path,
            stored_values: Mutex::new(HashMap::new()),
            process: Arc::new(Mutex::new(None)),
            next_session_id: Mutex::new(1),
        }
    }

    pub(crate) async fn stored_values(&self) -> HashMap<String, JsonValue> {
        self.stored_values.lock().await.clone()
    }

    pub(crate) async fn replace_stored_values(&self, values: HashMap<String, JsonValue>) {
        *self.stored_values.lock().await = values;
    }

    async fn ensure_started(
        &self,
    ) -> Result<tokio::sync::OwnedMutexGuard<Option<CodeModeProcess>>, std::io::Error> {
        let mut process_slot = self.process.lock().await;
        let needs_spawn = match process_slot.as_mut() {
            Some(process) => !matches!(process.has_exited(), Ok(false)),
            None => true,
        };
        if needs_spawn {
            let node_path = resolve_compatible_node(self.js_repl_node_path.as_deref())
                .await
                .map_err(std::io::Error::other)?;
            *process_slot = Some(spawn_code_mode_process(&node_path).await?);
        }
        drop(process_slot);
        Ok(self.process.clone().lock_owned().await)
    }

    pub(crate) async fn start_turn_worker(
        &self,
        session: &Arc<Session>,
        turn: &Arc<TurnContext>,
        tracker: &SharedTurnDiffTracker,
    ) -> Option<CodeModeWorker> {
        if !turn.features.enabled(Feature::CodeMode) {
            return None;
        }
        let exec = ExecContext {
            session: Arc::clone(session),
            turn: Arc::clone(turn),
            tracker: Arc::clone(tracker),
        };
        let mut process_slot = match self.ensure_started().await {
            Ok(process_slot) => process_slot,
            Err(err) => {
                warn!("failed to start {PUBLIC_TOOL_NAME} worker for turn: {err}");
                return None;
            }
        };
        let Some(process) = process_slot.as_mut() else {
            warn!(
                "failed to start {PUBLIC_TOOL_NAME} worker for turn: {PUBLIC_TOOL_NAME} runner failed to start"
            );
            return None;
        };
        Some(process.worker(exec))
    }

    pub(crate) async fn allocate_session_id(&self) -> i32 {
        let mut next_session_id = self.next_session_id.lock().await;
        let session_id = *next_session_id;
        *next_session_id = next_session_id.saturating_add(1);
        session_id
    }

    pub(crate) async fn allocate_request_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CodeModeToolKind {
    Function,
    Freeform,
}

#[derive(Clone, Debug, Serialize)]
struct EnabledTool {
    tool_name: String,
    #[serde(rename = "module")]
    module_path: String,
    namespace: Vec<String>,
    name: String,
    description: String,
    kind: CodeModeToolKind,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HostToNodeMessage {
    Start {
        request_id: String,
        session_id: i32,
        enabled_tools: Vec<EnabledTool>,
        stored_values: HashMap<String, JsonValue>,
        source: String,
    },
    Poll {
        request_id: String,
        session_id: i32,
        yield_time_ms: u64,
    },
    Terminate {
        request_id: String,
        session_id: i32,
    },
    Response {
        request_id: String,
        id: String,
        code_mode_result: JsonValue,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum NodeToHostMessage {
    ToolCall {
        #[serde(flatten)]
        tool_call: CodeModeToolCall,
    },
    Yielded {
        request_id: String,
        content_items: Vec<JsonValue>,
    },
    Terminated {
        request_id: String,
        content_items: Vec<JsonValue>,
    },
    Result {
        request_id: String,
        content_items: Vec<JsonValue>,
        stored_values: HashMap<String, JsonValue>,
        #[serde(default)]
        error_text: Option<String>,
        #[serde(default)]
        max_output_tokens_per_exec_call: Option<usize>,
    },
}

enum CodeModeSessionProgress {
    Finished(FunctionToolOutput),
    Yielded { output: FunctionToolOutput },
}

enum CodeModeExecutionStatus {
    Completed,
    Failed,
    Running(i32),
    Terminated,
}

pub(crate) fn instructions(config: &Config) -> Option<String> {
    if !config.features.enabled(Feature::CodeMode) {
        return None;
    }

    let mut section = String::from("## Exec\n");
    section.push_str(&format!(
        "- Use `{PUBLIC_TOOL_NAME}` for JavaScript execution in a Node-backed `node:vm` context.\n",
    ));
    section.push_str(&format!(
        "- `{PUBLIC_TOOL_NAME}` is a freeform/custom tool. Direct `{PUBLIC_TOOL_NAME}` calls must send raw JavaScript tool input. Do not wrap code in JSON, quotes, or markdown code fences.\n",
    ));
    section.push_str(&format!(
        "- Direct tool calls remain available while `{PUBLIC_TOOL_NAME}` is enabled.\n",
    ));
    section.push_str(&format!(
        "- `{PUBLIC_TOOL_NAME}` uses the same Node runtime resolution as `js_repl`. If needed, point `js_repl_node_path` at the Node binary you want Codex to use.\n",
    ));
    section.push_str("- Import nested tools from `tools.js`, for example `import { exec_command } from \"tools.js\"` or `import { ALL_TOOLS } from \"tools.js\"` to inspect the available `{ module, name, description }` entries. Namespaced tools are also available from `tools/<namespace...>.js`; MCP tools use `tools/mcp/<server>.js`, for example `import { append_notebook_logs_chart } from \"tools/mcp/ologs.js\"`. Nested tool calls resolve to their code-mode result values.\n");
    section.push_str(&format!(
        "- Import `{{ background, output_text, output_image, set_max_output_tokens_per_exec_call, set_yield_time, store, load }}` from `@openai/code_mode` (or `\"openai/code_mode\"`). `output_text(value)` surfaces text back to the model and stringifies non-string objects with `JSON.stringify(...)` when possible. `output_image(imageUrl)` appends an `input_image` content item for `http(s)` or `data:` URLs. `store(key, value)` persists JSON-serializable values across `{PUBLIC_TOOL_NAME}` calls in the current session, and `load(key)` returns a cloned stored value or `undefined`. `set_max_output_tokens_per_exec_call(value)` sets the token budget used to truncate direct `{PUBLIC_TOOL_NAME}` returns; `{WAIT_TOOL_NAME}` uses its own `max_tokens` argument instead and defaults to `10000`. `set_yield_time(value)` asks `{PUBLIC_TOOL_NAME}` to return early if the script is still running after that many milliseconds so `{WAIT_TOOL_NAME}` can resume it later. `background()` returns a yielded `{PUBLIC_TOOL_NAME}` response immediately while the script keeps running in the background. The returned content starts with a separate `Script completed`, `Script failed`, or `Script running with session ID …` text item that includes wall time. When truncation happens, the final text may include `Total output lines:` and the usual `…N tokens truncated…` marker.\n",
    ));
    section.push_str(&format!(
        "- If `{PUBLIC_TOOL_NAME}` returns `Script running with session ID …`, call `{WAIT_TOOL_NAME}` with that `session_id` to keep waiting for more output, completion, or termination.\n",
    ));
    section.push_str(
        "- Function tools require JSON object arguments. Freeform tools require raw strings.\n",
    );
    section.push_str("- `add_content(value)` remains available for compatibility. It is synchronous and accepts a content item, an array of content items, or a string. Structured nested-tool results should be converted to text first, for example with `JSON.stringify(...)`.\n");
    section
        .push_str("- Only content passed to `output_text(...)`, `output_image(...)`, or `add_content(value)` is surfaced back to the model.");
    Some(section)
}

pub(crate) async fn execute(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
    code: String,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let exec = ExecContext {
        session,
        turn,
        tracker,
    };
    let enabled_tools = build_enabled_tools(&exec).await;
    let service = &exec.session.services.code_mode_service;
    let stored_values = service.stored_values().await;
    let source = build_source(&code, &enabled_tools).map_err(FunctionCallError::RespondToModel)?;
    let session_id = service.allocate_session_id().await;
    let request_id = service.allocate_request_id().await;
    let process_slot = service
        .ensure_started()
        .await
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
    let started_at = std::time::Instant::now();
    let message = HostToNodeMessage::Start {
        request_id: request_id.clone(),
        session_id,
        enabled_tools,
        stored_values,
        source,
    };
    let result = {
        let mut process_slot = process_slot;
        let Some(process) = process_slot.as_mut() else {
            return Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} runner failed to start"
            )));
        };
        let message = process
            .send(&request_id, &message)
            .await
            .map_err(|err| err.to_string());
        let message = match message {
            Ok(message) => message,
            Err(error) => return Err(FunctionCallError::RespondToModel(error)),
        };
        handle_node_message(&exec, session_id, message, None, started_at).await
    };
    match result {
        Ok(CodeModeSessionProgress::Finished(output))
        | Ok(CodeModeSessionProgress::Yielded { output }) => Ok(output),
        Err(error) => Err(FunctionCallError::RespondToModel(error)),
    }
}

pub(crate) async fn wait(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
    session_id: i32,
    yield_time_ms: u64,
    max_output_tokens: Option<usize>,
    terminate: bool,
) -> Result<FunctionToolOutput, FunctionCallError> {
    let exec = ExecContext {
        session,
        turn,
        tracker,
    };
    let request_id = exec
        .session
        .services
        .code_mode_service
        .allocate_request_id()
        .await;
    let started_at = std::time::Instant::now();
    let message = if terminate {
        HostToNodeMessage::Terminate {
            request_id: request_id.clone(),
            session_id,
        }
    } else {
        HostToNodeMessage::Poll {
            request_id: request_id.clone(),
            session_id,
            yield_time_ms,
        }
    };
    let process_slot = exec
        .session
        .services
        .code_mode_service
        .ensure_started()
        .await
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
    let result = {
        let mut process_slot = process_slot;
        let Some(process) = process_slot.as_mut() else {
            return Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} runner failed to start"
            )));
        };
        if !matches!(process.has_exited(), Ok(false)) {
            return Err(FunctionCallError::RespondToModel(format!(
                "{PUBLIC_TOOL_NAME} runner failed to start"
            )));
        }
        let message = process
            .send(&request_id, &message)
            .await
            .map_err(|err| err.to_string());
        let message = match message {
            Ok(message) => message,
            Err(error) => return Err(FunctionCallError::RespondToModel(error)),
        };
        handle_node_message(
            &exec,
            session_id,
            message,
            Some(max_output_tokens),
            started_at,
        )
        .await
    };
    match result {
        Ok(CodeModeSessionProgress::Finished(output))
        | Ok(CodeModeSessionProgress::Yielded { output }) => Ok(output),
        Err(error) => Err(FunctionCallError::RespondToModel(error)),
    }
}

async fn handle_node_message(
    exec: &ExecContext,
    session_id: i32,
    message: NodeToHostMessage,
    poll_max_output_tokens: Option<Option<usize>>,
    started_at: std::time::Instant,
) -> Result<CodeModeSessionProgress, String> {
    match message {
        NodeToHostMessage::ToolCall { .. } => Err(format!(
            "{PUBLIC_TOOL_NAME} received an unexpected tool call response"
        )),
        NodeToHostMessage::Yielded { content_items, .. } => {
            let mut delta_items = output_content_items_from_json_values(content_items)?;
            delta_items = truncate_code_mode_result(delta_items, poll_max_output_tokens.flatten());
            prepend_script_status(
                &mut delta_items,
                CodeModeExecutionStatus::Running(session_id),
                started_at.elapsed(),
            );
            Ok(CodeModeSessionProgress::Yielded {
                output: FunctionToolOutput::from_content(delta_items, Some(true)),
            })
        }
        NodeToHostMessage::Terminated { content_items, .. } => {
            let mut delta_items = output_content_items_from_json_values(content_items)?;
            delta_items = truncate_code_mode_result(delta_items, poll_max_output_tokens.flatten());
            prepend_script_status(
                &mut delta_items,
                CodeModeExecutionStatus::Terminated,
                started_at.elapsed(),
            );
            Ok(CodeModeSessionProgress::Finished(
                FunctionToolOutput::from_content(delta_items, Some(true)),
            ))
        }
        NodeToHostMessage::Result {
            content_items,
            stored_values,
            error_text,
            max_output_tokens_per_exec_call,
            ..
        } => {
            exec.session
                .services
                .code_mode_service
                .replace_stored_values(stored_values)
                .await;
            let mut delta_items = output_content_items_from_json_values(content_items)?;
            let success = error_text.is_none();
            if let Some(error_text) = error_text {
                delta_items.push(FunctionCallOutputContentItem::InputText {
                    text: format!("Script error:\n{error_text}"),
                });
            }

            let mut delta_items = truncate_code_mode_result(
                delta_items,
                poll_max_output_tokens.unwrap_or(max_output_tokens_per_exec_call),
            );
            prepend_script_status(
                &mut delta_items,
                if success {
                    CodeModeExecutionStatus::Completed
                } else {
                    CodeModeExecutionStatus::Failed
                },
                started_at.elapsed(),
            );
            Ok(CodeModeSessionProgress::Finished(
                FunctionToolOutput::from_content(delta_items, Some(success)),
            ))
        }
    }
}

async fn spawn_code_mode_process(
    node_path: &std::path::Path,
) -> Result<CodeModeProcess, std::io::Error> {
    let mut cmd = tokio::process::Command::new(node_path);
    cmd.arg("--experimental-vm-modules");
    cmd.arg("--eval");
    cmd.arg(CODE_MODE_RUNNER_SOURCE);
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(std::io::Error::other)?;
    let stdout = child.stdout.take().ok_or_else(|| {
        std::io::Error::other(format!("{PUBLIC_TOOL_NAME} runner missing stdout"))
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        std::io::Error::other(format!("{PUBLIC_TOOL_NAME} runner missing stderr"))
    })?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other(format!("{PUBLIC_TOOL_NAME} runner missing stdin")))?;
    let stdin = Arc::new(Mutex::new(stdin));
    let response_waiters = Arc::new(Mutex::new(HashMap::<
        String,
        oneshot::Sender<NodeToHostMessage>,
    >::new()));
    let (tool_call_tx, tool_call_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        match reader.read_to_end(&mut buf).await {
            Ok(_) => {
                let stderr = String::from_utf8_lossy(&buf).trim().to_string();
                if !stderr.is_empty() {
                    warn!("{PUBLIC_TOOL_NAME} runner stderr: {stderr}");
                }
            }
            Err(err) => {
                warn!("failed to read {PUBLIC_TOOL_NAME} stderr: {err}");
            }
        }
    });
    let stdout_task = tokio::spawn({
        let response_waiters = Arc::clone(&response_waiters);
        async move {
            let mut stdout_lines = BufReader::new(stdout).lines();
            loop {
                let line = match stdout_lines.next_line().await {
                    Ok(line) => line,
                    Err(err) => {
                        warn!("failed to read {PUBLIC_TOOL_NAME} stdout: {err}");
                        break;
                    }
                };
                let Some(line) = line else {
                    break;
                };
                if line.trim().is_empty() {
                    continue;
                }
                let message: NodeToHostMessage = match serde_json::from_str(&line) {
                    Ok(message) => message,
                    Err(err) => {
                        warn!("failed to parse {PUBLIC_TOOL_NAME} stdout message: {err}");
                        break;
                    }
                };
                match message {
                    NodeToHostMessage::ToolCall { tool_call } => {
                        let _ = tool_call_tx.send(tool_call);
                    }
                    message => {
                        let request_id = message_request_id(&message).to_string();
                        if let Some(waiter) = response_waiters.lock().await.remove(&request_id) {
                            let _ = waiter.send(message);
                        }
                    }
                }
            }
            response_waiters.lock().await.clear();
        }
    });

    Ok(CodeModeProcess {
        child,
        stdin,
        stdout_task,
        response_waiters,
        tool_call_rx: Arc::new(Mutex::new(tool_call_rx)),
    })
}

async fn write_message(
    stdin: &Arc<Mutex<tokio::process::ChildStdin>>,
    message: &HostToNodeMessage,
) -> Result<(), std::io::Error> {
    let line = serde_json::to_string(message).map_err(std::io::Error::other)?;
    let mut stdin = stdin.lock().await;
    stdin.write_all(line.as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    Ok(())
}

fn message_request_id(message: &NodeToHostMessage) -> &str {
    match message {
        NodeToHostMessage::ToolCall { tool_call } => &tool_call.request_id,
        NodeToHostMessage::Yielded { request_id, .. }
        | NodeToHostMessage::Terminated { request_id, .. }
        | NodeToHostMessage::Result { request_id, .. } => request_id,
    }
}

fn prepend_script_status(
    content_items: &mut Vec<FunctionCallOutputContentItem>,
    status: CodeModeExecutionStatus,
    wall_time: Duration,
) {
    let wall_time_seconds = ((wall_time.as_secs_f32()) * 10.0).round() / 10.0;
    let header = format!(
        "{}\nWall time {wall_time_seconds:.1} seconds\nOutput:\n",
        match status {
            CodeModeExecutionStatus::Completed => "Script completed".to_string(),
            CodeModeExecutionStatus::Failed => "Script failed".to_string(),
            CodeModeExecutionStatus::Running(session_id) => {
                format!("Script running with session ID {session_id}")
            }
            CodeModeExecutionStatus::Terminated => "Script terminated".to_string(),
        }
    );
    content_items.insert(0, FunctionCallOutputContentItem::InputText { text: header });
}

fn build_source(user_code: &str, enabled_tools: &[EnabledTool]) -> Result<String, String> {
    let enabled_tools_json = serde_json::to_string(enabled_tools)
        .map_err(|err| format!("failed to serialize enabled tools: {err}"))?;
    Ok(CODE_MODE_BRIDGE_SOURCE
        .replace(
            "__CODE_MODE_ENABLED_TOOLS_PLACEHOLDER__",
            &enabled_tools_json,
        )
        .replace("__CODE_MODE_USER_CODE_PLACEHOLDER__", user_code))
}

fn truncate_code_mode_result(
    items: Vec<FunctionCallOutputContentItem>,
    max_output_tokens_per_exec_call: Option<usize>,
) -> Vec<FunctionCallOutputContentItem> {
    let max_output_tokens = resolve_max_tokens(max_output_tokens_per_exec_call);
    let policy = TruncationPolicy::Tokens(max_output_tokens);
    if items
        .iter()
        .all(|item| matches!(item, FunctionCallOutputContentItem::InputText { .. }))
    {
        let (truncated_items, _) =
            formatted_truncate_text_content_items_with_policy(&items, policy);
        return truncated_items;
    }

    truncate_function_output_items_with_policy(&items, policy)
}

async fn build_enabled_tools(exec: &ExecContext) -> Vec<EnabledTool> {
    let router = build_nested_router(exec).await;
    let mut out = router
        .specs()
        .into_iter()
        .map(|spec| augment_tool_spec_for_code_mode(spec, true))
        .filter_map(enabled_tool_from_spec)
        .collect::<Vec<_>>();
    out.sort_by(|left, right| left.tool_name.cmp(&right.tool_name));
    out.dedup_by(|left, right| left.tool_name == right.tool_name);
    out
}

fn enabled_tool_from_spec(spec: ToolSpec) -> Option<EnabledTool> {
    let tool_name = spec.name().to_string();
    if tool_name == PUBLIC_TOOL_NAME || tool_name == WAIT_TOOL_NAME {
        return None;
    }

    let reference = code_mode_tool_reference(&tool_name);

    let (description, kind) = match spec {
        ToolSpec::Function(tool) => (tool.description, CodeModeToolKind::Function),
        ToolSpec::Freeform(tool) => (tool.description, CodeModeToolKind::Freeform),
        ToolSpec::LocalShell {}
        | ToolSpec::ImageGeneration { .. }
        | ToolSpec::ToolSearch { .. }
        | ToolSpec::WebSearch { .. } => {
            return None;
        }
    };

    Some(EnabledTool {
        tool_name,
        module_path: reference.module_path,
        namespace: reference.namespace,
        name: reference.tool_key,
        description,
        kind,
    })
}

async fn build_nested_router(exec: &ExecContext) -> ToolRouter {
    let nested_tools_config = exec.turn.tools_config.for_code_mode_nested_tools();
    let mcp_tools = exec
        .session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await
        .into_iter()
        .map(|(name, tool_info)| (name, tool_info.tool))
        .collect();

    ToolRouter::from_config(
        &nested_tools_config,
        ToolRouterParams {
            mcp_tools: Some(mcp_tools),
            app_tools: None,
            discoverable_tools: None,
            dynamic_tools: exec.turn.dynamic_tools.as_slice(),
        },
    )
}

async fn call_nested_tool(
    exec: ExecContext,
    tool_name: String,
    input: Option<JsonValue>,
) -> JsonValue {
    if tool_name == PUBLIC_TOOL_NAME {
        return JsonValue::String(format!("{PUBLIC_TOOL_NAME} cannot invoke itself"));
    }

    let router = build_nested_router(&exec).await;

    let specs = router.specs();
    let payload =
        if let Some((server, tool)) = exec.session.parse_mcp_tool_name(&tool_name, &None).await {
            match serialize_function_tool_arguments(&tool_name, input) {
                Ok(raw_arguments) => ToolPayload::Mcp {
                    server,
                    tool,
                    raw_arguments,
                },
                Err(error) => return JsonValue::String(error),
            }
        } else {
            match build_nested_tool_payload(&specs, &tool_name, input) {
                Ok(payload) => payload,
                Err(error) => return JsonValue::String(error),
            }
        };

    let call = ToolCall {
        tool_name: tool_name.clone(),
        call_id: format!("{PUBLIC_TOOL_NAME}-{}", uuid::Uuid::new_v4()),
        tool_namespace: None,
        payload,
    };
    let result = router
        .dispatch_tool_call_with_code_mode_result(
            Arc::clone(&exec.session),
            Arc::clone(&exec.turn),
            Arc::clone(&exec.tracker),
            call,
            ToolCallSource::CodeMode,
        )
        .await;

    match result {
        Ok(result) => result.code_mode_result(),
        Err(error) => JsonValue::String(error.to_string()),
    }
}

fn tool_kind_for_spec(spec: &ToolSpec) -> CodeModeToolKind {
    if matches!(spec, ToolSpec::Freeform(_)) {
        CodeModeToolKind::Freeform
    } else {
        CodeModeToolKind::Function
    }
}

fn tool_kind_for_name(specs: &[ToolSpec], tool_name: &str) -> Result<CodeModeToolKind, String> {
    specs
        .iter()
        .find(|spec| spec.name() == tool_name)
        .map(tool_kind_for_spec)
        .ok_or_else(|| format!("tool `{tool_name}` is not enabled in {PUBLIC_TOOL_NAME}"))
}

fn build_nested_tool_payload(
    specs: &[ToolSpec],
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let actual_kind = tool_kind_for_name(specs, tool_name)?;
    match actual_kind {
        CodeModeToolKind::Function => build_function_tool_payload(tool_name, input),
        CodeModeToolKind::Freeform => build_freeform_tool_payload(tool_name, input),
    }
}

fn build_function_tool_payload(
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    let arguments = serialize_function_tool_arguments(tool_name, input)?;
    Ok(ToolPayload::Function { arguments })
}

fn serialize_function_tool_arguments(
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<String, String> {
    match input {
        None => Ok("{}".to_string()),
        Some(JsonValue::Object(map)) => serde_json::to_string(&JsonValue::Object(map))
            .map_err(|err| format!("failed to serialize tool `{tool_name}` arguments: {err}")),
        Some(_) => Err(format!(
            "tool `{tool_name}` expects a JSON object for arguments"
        )),
    }
}

fn build_freeform_tool_payload(
    tool_name: &str,
    input: Option<JsonValue>,
) -> Result<ToolPayload, String> {
    match input {
        Some(JsonValue::String(input)) => Ok(ToolPayload::Custom { input }),
        _ => Err(format!("tool `{tool_name}` expects a string input")),
    }
}

fn output_content_items_from_json_values(
    content_items: Vec<JsonValue>,
) -> Result<Vec<FunctionCallOutputContentItem>, String> {
    content_items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            serde_json::from_value(item).map_err(|err| {
                format!("invalid {PUBLIC_TOOL_NAME} content item at index {index}: {err}")
            })
        })
        .collect()
}
