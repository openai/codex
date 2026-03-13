use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseInputItem;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::OnceCell;
use tokio::sync::RwLock;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::trace;
use tracing::warn;
use uuid::Uuid;

use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecExpiration;
use crate::exec::ExecToolCallOutput;
use crate::exec::MAX_EXEC_OUTPUT_DELTAS_PER_CALL;
use crate::exec::StreamOutput;
use crate::exec_env::create_env;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecCommandSource;
use crate::protocol::ExecOutputStream;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxManager;
use crate::sandboxing::SandboxPermissions;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventFailure;
use crate::tools::events::ToolEventStage;
use crate::tools::sandboxing::SandboxAttempt;
use crate::tools::sandboxing::SandboxOverride;
use crate::tools::sandboxing::SandboxablePreference;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;
use crate::unified_exec::ManagedSplitProcess;
use crate::unified_exec::UnifiedExecProcess;

mod node;
mod output;
mod protocol;

pub(crate) use node::resolve_compatible_node;
use output::*;
use protocol::*;

pub(crate) const JS_REPL_PRAGMA_PREFIX: &str = "// codex-js-repl:";
const KERNEL_SOURCE: &str = include_str!("kernel.js");
const MERIYAH_UMD: &str = include_str!("meriyah.umd.min.js");
const JS_REPL_MIN_NODE_VERSION: &str = include_str!("../../../../node-version.txt");
const JS_REPL_STDERR_TAIL_LINE_LIMIT: usize = 20;
const JS_REPL_STDERR_TAIL_LINE_MAX_BYTES: usize = 512;
const JS_REPL_STDERR_TAIL_MAX_BYTES: usize = 4_096;
const JS_REPL_STDERR_TAIL_SEPARATOR: &str = " | ";
const JS_REPL_EXEC_ID_LOG_LIMIT: usize = 8;
const JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES: usize = 1_024;
const JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES: usize = 256;
const JS_REPL_TOOL_RESPONSE_TEXT_PREVIEW_MAX_BYTES: usize = 512;
const JS_REPL_POLL_MIN_MS: u64 = 50;
const JS_REPL_POLL_MAX_MS: u64 = crate::unified_exec::DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS;
const JS_REPL_POLL_DEFAULT_MS: u64 = crate::unified_exec::MIN_EMPTY_YIELD_TIME_MS;
const JS_REPL_POLL_MAX_SESSIONS: usize = 16;
const JS_REPL_POLL_MAX_COMPLETED_EXECS: usize = 64;
const JS_REPL_POLL_ALL_LOGS_MAX_BYTES: usize = crate::unified_exec::UNIFIED_EXEC_OUTPUT_MAX_BYTES;
const JS_REPL_POLL_LOG_QUEUE_MAX_BYTES: usize = 64 * 1024;
const JS_REPL_OUTPUT_DELTA_MAX_BYTES: usize = 8192;
const JS_REPL_POLL_COMPLETED_EXEC_RETENTION: Duration = Duration::from_secs(300);
const JS_REPL_KILL_WAIT_TIMEOUT: Duration = Duration::from_millis(250);
const JS_REPL_POLL_LOGS_TRUNCATED_MARKER: &str =
    "[js_repl logs truncated; poll more frequently for complete streaming logs]";
const JS_REPL_POLL_ALL_LOGS_TRUNCATED_MARKER: &str =
    "[js_repl logs truncated; output exceeds byte limit]";
pub(crate) const JS_REPL_TIMEOUT_ERROR_MESSAGE: &str =
    "js_repl execution timed out; kernel reset, rerun your request";
const JS_REPL_CANCEL_ERROR_MESSAGE: &str = "js_repl execution canceled";
pub(crate) const JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE: &str =
    "js_repl timeout_ms is not supported when poll=true; use js_repl_poll yield_time_ms";
static NEXT_COMPLETED_EXEC_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Per-task js_repl handle stored on the turn context.
pub(crate) struct JsReplHandle {
    node_path: Option<PathBuf>,
    node_module_dirs: Vec<PathBuf>,
    cell: OnceCell<Arc<JsReplManager>>,
}

impl fmt::Debug for JsReplHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsReplHandle").finish_non_exhaustive()
    }
}

impl JsReplHandle {
    pub(crate) fn with_node_path(
        node_path: Option<PathBuf>,
        node_module_dirs: Vec<PathBuf>,
    ) -> Self {
        Self {
            node_path,
            node_module_dirs,
            cell: OnceCell::new(),
        }
    }

    pub(crate) async fn manager(&self) -> Result<Arc<JsReplManager>, FunctionCallError> {
        self.cell
            .get_or_try_init(|| async {
                JsReplManager::new(self.node_path.clone(), self.node_module_dirs.clone()).await
            })
            .await
            .cloned()
    }

    pub(crate) fn manager_if_initialized(&self) -> Option<Arc<JsReplManager>> {
        self.cell.get().cloned()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// Arguments accepted by the `js_repl` tool.
pub struct JsReplArgs {
    pub code: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub poll: bool,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Clone, Debug)]
/// Final output from a non-polling `js_repl` execution.
pub struct JsExecResult {
    pub output: String,
    pub content_items: Vec<FunctionCallOutputContentItem>,
}

#[derive(Debug, Error, PartialEq)]
/// Errors returned by non-polling `js_repl` execution.
pub enum JsReplExecuteError {
    #[error("{0}")]
    RespondToModel(String),
    #[error("{JS_REPL_TIMEOUT_ERROR_MESSAGE}")]
    TimedOut,
}

impl From<JsReplExecuteError> for FunctionCallError {
    fn from(value: JsReplExecuteError) -> Self {
        match value {
            JsReplExecuteError::RespondToModel(message) => Self::RespondToModel(message),
            JsReplExecuteError::TimedOut => {
                Self::RespondToModel(JS_REPL_TIMEOUT_ERROR_MESSAGE.to_string())
            }
        }
    }
}

#[derive(Clone, Debug)]
/// Submission identifiers for a polling `js_repl` execution.
pub struct JsExecSubmission {
    pub exec_id: String,
    pub session_id: String,
}

#[derive(Clone, Debug)]
/// Current state returned by `js_repl_poll`.
pub struct JsExecPollResult {
    pub exec_id: String,
    pub session_id: String,
    pub logs: Vec<String>,
    pub final_output: Option<String>,
    pub content_items: Vec<FunctionCallOutputContentItem>,
    pub error: Option<String>,
    pub done: bool,
}

#[derive(Clone)]
struct KernelState {
    process: Arc<UnifiedExecProcess>,
    recent_stderr: Arc<Mutex<VecDeque<String>>>,
    stdin: tokio::sync::mpsc::Sender<Vec<u8>>,
    pending_execs: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<ExecResultMessage>>>>,
    exec_contexts: Arc<Mutex<HashMap<String, ExecContext>>>,
    top_level_exec_state: TopLevelExecState,
    protocol_reader_drained: CancellationToken,
    shutdown: CancellationToken,
}

struct PollSessionState {
    kernel: KernelState,
    active_exec: Option<String>,
    last_used: Instant,
}

#[derive(Clone)]
struct ExecContext {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tracker: SharedTurnDiffTracker,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum TopLevelExecState {
    #[default]
    Idle,
    FreshKernel {
        turn_id: String,
        exec_id: Option<String>,
    },
    ReusedKernelPending {
        turn_id: String,
        exec_id: String,
    },
    Submitted {
        turn_id: String,
        exec_id: String,
    },
}

impl TopLevelExecState {
    fn registered_exec_id(&self) -> Option<&str> {
        match self {
            Self::Idle => None,
            Self::FreshKernel {
                exec_id: Some(exec_id),
                ..
            }
            | Self::ReusedKernelPending { exec_id, .. }
            | Self::Submitted { exec_id, .. } => Some(exec_id.as_str()),
            Self::FreshKernel { exec_id: None, .. } => None,
        }
    }

    fn should_reset_for_interrupt(&self, turn_id: &str) -> bool {
        match self {
            Self::Idle => false,
            Self::FreshKernel {
                turn_id: active_turn_id,
                ..
            }
            | Self::Submitted {
                turn_id: active_turn_id,
                ..
            } => active_turn_id == turn_id,
            Self::ReusedKernelPending { .. } => false,
        }
    }
}

#[derive(Default)]
struct ExecToolCalls {
    in_flight: usize,
    content_items: Vec<FunctionCallOutputContentItem>,
    notify: Arc<Notify>,
    cancel: CancellationToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
enum JsReplToolCallPayloadKind {
    MessageContent,
    FunctionText,
    FunctionContentItems,
    CustomText,
    CustomContentItems,
    McpResult,
    McpErrorResult,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct JsReplToolCallResponseSummary {
    response_type: Option<String>,
    payload_kind: Option<JsReplToolCallPayloadKind>,
    payload_text_preview: Option<String>,
    payload_text_length: Option<usize>,
    payload_item_count: Option<usize>,
    text_item_count: Option<usize>,
    image_item_count: Option<usize>,
    structured_content_present: Option<bool>,
    result_is_error: Option<bool>,
}

pub(crate) async fn emit_js_repl_exec_end(
    session: &crate::codex::Session,
    turn: &crate::codex::TurnContext,
    call_id: &str,
    output: &str,
    error: Option<&str>,
    duration: Duration,
    timed_out: bool,
) {
    let exec_output = build_js_repl_exec_output(output, error, duration, timed_out);
    let emitter = ToolEmitter::shell(
        vec!["js_repl".to_string()],
        turn.cwd.clone(),
        ExecCommandSource::Agent,
        false,
    );
    let ctx = ToolEventCtx::new(session, turn, call_id, None);
    let stage = if error.is_some() {
        ToolEventStage::Failure(ToolEventFailure::Output(exec_output))
    } else {
        ToolEventStage::Success(exec_output)
    };
    emitter.emit(ctx, stage).await;
}

/// Manages the per-turn `js_repl` kernel and polling sessions.
pub struct JsReplManager {
    node_path: Option<PathBuf>,
    node_module_dirs: Vec<PathBuf>,
    tmp_dir: tempfile::TempDir,
    kernel: Arc<Mutex<Option<KernelState>>>,
    kernel_script_path: PathBuf,
    exec_lock: Arc<Semaphore>,
    exec_tool_calls: Arc<Mutex<HashMap<String, ExecToolCalls>>>,
    exec_store: Arc<Mutex<HashMap<String, ExecBuffer>>>,
    poll_sessions: Arc<Mutex<HashMap<String, PollSessionState>>>,
    exec_to_session: Arc<Mutex<HashMap<String, String>>>,
    poll_lifecycle: Arc<RwLock<()>>,
}

impl JsReplManager {
    async fn new(
        node_path: Option<PathBuf>,
        node_module_dirs: Vec<PathBuf>,
    ) -> Result<Arc<Self>, FunctionCallError> {
        let tmp_dir = tempfile::tempdir().map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to create js_repl temp dir: {err}"))
        })?;
        let kernel_script_path =
            Self::write_kernel_script(tmp_dir.path())
                .await
                .map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to stage js_repl kernel script: {err}"
                    ))
                })?;

        let manager = Arc::new(Self {
            node_path,
            node_module_dirs,
            tmp_dir,
            kernel: Arc::new(Mutex::new(None)),
            kernel_script_path,
            exec_lock: Arc::new(Semaphore::new(1)),
            exec_tool_calls: Arc::new(Mutex::new(HashMap::new())),
            exec_store: Arc::new(Mutex::new(HashMap::new())),
            poll_sessions: Arc::new(Mutex::new(HashMap::new())),
            exec_to_session: Arc::new(Mutex::new(HashMap::new())),
            poll_lifecycle: Arc::new(RwLock::new(())),
        });

        Ok(manager)
    }

    async fn register_exec_tool_calls(&self, exec_id: &str) {
        self.exec_tool_calls
            .lock()
            .await
            .insert(exec_id.to_string(), ExecToolCalls::default());
    }

    async fn begin_exec_tool_call(
        exec_tool_calls: &Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_id: &str,
    ) -> Option<CancellationToken> {
        let mut calls = exec_tool_calls.lock().await;
        let state = calls.get_mut(exec_id)?;
        state.in_flight += 1;
        Some(state.cancel.clone())
    }

    async fn record_exec_content_item(
        exec_tool_calls: &Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_id: &str,
        content_item: FunctionCallOutputContentItem,
    ) {
        let mut calls = exec_tool_calls.lock().await;
        if let Some(state) = calls.get_mut(exec_id) {
            state.content_items.push(content_item);
        }
    }

    async fn finish_exec_tool_call(
        exec_tool_calls: &Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_id: &str,
    ) {
        let notify = {
            let mut calls = exec_tool_calls.lock().await;
            let Some(state) = calls.get_mut(exec_id) else {
                return;
            };
            if state.in_flight == 0 {
                return;
            }
            state.in_flight -= 1;
            if state.in_flight == 0 {
                Some(Arc::clone(&state.notify))
            } else {
                None
            }
        };
        if let Some(notify) = notify {
            notify.notify_waiters();
        }
    }

    async fn wait_for_exec_tool_calls_map(
        exec_tool_calls: &Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_id: &str,
    ) {
        loop {
            let notified = {
                let calls = exec_tool_calls.lock().await;
                calls
                    .get(exec_id)
                    .filter(|state| state.in_flight > 0)
                    .map(|state| Arc::clone(&state.notify).notified_owned())
            };
            match notified {
                Some(notified) => notified.await,
                None => return,
            }
        }
    }

    async fn clear_exec_tool_calls_map(
        exec_tool_calls: &Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_id: &str,
    ) {
        if let Some(state) = exec_tool_calls.lock().await.remove(exec_id) {
            state.cancel.cancel();
            state.notify.notify_waiters();
        }
    }

    async fn cancel_exec_tool_calls_map(
        exec_tool_calls: &Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_id: &str,
    ) {
        let notify = {
            let calls = exec_tool_calls.lock().await;
            calls.get(exec_id).map(|state| {
                state.cancel.cancel();
                Arc::clone(&state.notify)
            })
        };
        if let Some(notify) = notify {
            notify.notify_waiters();
        }
    }

    async fn clear_all_exec_tool_calls_map(
        exec_tool_calls: &Arc<Mutex<HashMap<String, ExecToolCalls>>>,
    ) {
        let states = {
            let mut calls = exec_tool_calls.lock().await;
            calls.drain().map(|(_, state)| state).collect::<Vec<_>>()
        };
        for state in states {
            state.cancel.cancel();
            state.notify.notify_waiters();
        }
    }

    async fn register_top_level_exec(&self, exec_id: String, turn_id: String) {
        let mut kernel = self.kernel.lock().await;
        let Some(state) = kernel.as_mut() else {
            return;
        };
        state.top_level_exec_state = match &state.top_level_exec_state {
            TopLevelExecState::FreshKernel {
                turn_id: active_turn_id,
                ..
            } if active_turn_id == &turn_id => TopLevelExecState::FreshKernel {
                turn_id,
                exec_id: Some(exec_id),
            },
            TopLevelExecState::Idle
            | TopLevelExecState::ReusedKernelPending { .. }
            | TopLevelExecState::Submitted { .. }
            | TopLevelExecState::FreshKernel { .. } => {
                TopLevelExecState::ReusedKernelPending { turn_id, exec_id }
            }
        };
    }

    async fn mark_top_level_exec_submitted(&self, exec_id: &str) {
        let mut kernel = self.kernel.lock().await;
        let Some(state) = kernel.as_mut() else {
            return;
        };
        let next_state = match &state.top_level_exec_state {
            TopLevelExecState::FreshKernel {
                turn_id,
                exec_id: Some(active_exec_id),
            }
            | TopLevelExecState::ReusedKernelPending {
                turn_id,
                exec_id: active_exec_id,
            } if active_exec_id == exec_id => Some(TopLevelExecState::Submitted {
                turn_id: turn_id.clone(),
                exec_id: active_exec_id.clone(),
            }),
            TopLevelExecState::Idle
            | TopLevelExecState::FreshKernel { .. }
            | TopLevelExecState::ReusedKernelPending { .. }
            | TopLevelExecState::Submitted { .. } => None,
        };
        if let Some(next_state) = next_state {
            state.top_level_exec_state = next_state;
        }
    }

    async fn clear_top_level_exec_if_matches(&self, exec_id: &str) {
        Self::clear_top_level_exec_if_matches_map(&self.kernel, exec_id).await;
    }

    async fn clear_top_level_exec_if_matches_map(
        kernel: &Arc<Mutex<Option<KernelState>>>,
        exec_id: &str,
    ) {
        let mut kernel = kernel.lock().await;
        if let Some(state) = kernel.as_mut()
            && state.top_level_exec_state.registered_exec_id() == Some(exec_id)
        {
            state.top_level_exec_state = TopLevelExecState::Idle;
        }
    }

    async fn clear_top_level_exec_if_matches_any_map(
        kernel: &Arc<Mutex<Option<KernelState>>>,
        exec_ids: &[String],
    ) {
        let mut kernel = kernel.lock().await;
        if let Some(state) = kernel.as_mut()
            && state
                .top_level_exec_state
                .registered_exec_id()
                .is_some_and(|exec_id| exec_ids.iter().any(|pending_id| pending_id == exec_id))
        {
            state.top_level_exec_state = TopLevelExecState::Idle;
        }
    }

    async fn turn_interrupt_requires_reset(&self, turn_id: &str) -> bool {
        self.kernel.lock().await.as_ref().is_some_and(|state| {
            state
                .top_level_exec_state
                .should_reset_for_interrupt(turn_id)
        })
    }
    async fn clear_poll_exec_state_for_session(
        &self,
        session_id: &str,
        preserved_exec_id: Option<&str>,
    ) {
        self.exec_to_session
            .lock()
            .await
            .retain(|_, mapped_session_id| mapped_session_id != session_id);
        self.exec_store.lock().await.retain(|exec_id, entry| {
            entry.session_id.as_deref() != Some(session_id)
                || preserved_exec_id.is_some_and(|preserved_exec_id| exec_id == preserved_exec_id)
        });
    }

    async fn clear_all_poll_exec_state(&self, preserved_exec_ids: &HashSet<String>) {
        self.exec_to_session.lock().await.clear();
        self.exec_store
            .lock()
            .await
            .retain(|exec_id, _| preserved_exec_ids.contains(exec_id));
    }

    async fn wait_for_exec_terminal_or_protocol_reader_drained(
        exec_store: &Arc<Mutex<HashMap<String, ExecBuffer>>>,
        exec_id: &str,
        protocol_reader_drained: &CancellationToken,
    ) {
        loop {
            let protocol_reader_drained_wait = protocol_reader_drained.cancelled();
            tokio::pin!(protocol_reader_drained_wait);
            let notified = {
                let store = exec_store.lock().await;
                match store.get(exec_id) {
                    Some(entry) if entry.done => return,
                    Some(entry) => Arc::clone(&entry.notify).notified_owned(),
                    None => return,
                }
            };
            tokio::pin!(notified);
            tokio::select! {
                _ = &mut notified => {}
                _ = &mut protocol_reader_drained_wait => return,
            }
        }
    }
    fn log_tool_call_response(
        req: &RunToolRequest,
        ok: bool,
        summary: &JsReplToolCallResponseSummary,
        response: Option<&JsonValue>,
        error: Option<&str>,
    ) {
        info!(
            exec_id = %req.exec_id,
            tool_call_id = %req.id,
            tool_name = %req.tool_name,
            ok,
            summary = ?summary,
            "js_repl nested tool call completed"
        );
        if let Some(response) = response {
            trace!(
                exec_id = %req.exec_id,
                tool_call_id = %req.id,
                tool_name = %req.tool_name,
                response_json = %response,
                "js_repl nested tool call raw response"
            );
        }
        if let Some(error) = error {
            trace!(
                exec_id = %req.exec_id,
                tool_call_id = %req.id,
                tool_name = %req.tool_name,
                error = %error,
                "js_repl nested tool call raw error"
            );
        }
    }

    fn summarize_text_payload(
        response_type: Option<&str>,
        payload_kind: JsReplToolCallPayloadKind,
        text: &str,
    ) -> JsReplToolCallResponseSummary {
        JsReplToolCallResponseSummary {
            response_type: response_type.map(str::to_owned),
            payload_kind: Some(payload_kind),
            payload_text_preview: (!text.is_empty()).then(|| {
                truncate_text(
                    text,
                    TruncationPolicy::Bytes(JS_REPL_TOOL_RESPONSE_TEXT_PREVIEW_MAX_BYTES),
                )
            }),
            payload_text_length: Some(text.len()),
            ..Default::default()
        }
    }

    fn summarize_function_output_payload(
        response_type: &str,
        payload_kind: JsReplToolCallPayloadKind,
        output: &FunctionCallOutputPayload,
    ) -> JsReplToolCallResponseSummary {
        let (payload_item_count, text_item_count, image_item_count) =
            if let Some(items) = output.content_items() {
                let text_item_count = items
                    .iter()
                    .filter(|item| matches!(item, FunctionCallOutputContentItem::InputText { .. }))
                    .count();
                let image_item_count = items.len().saturating_sub(text_item_count);
                (
                    Some(items.len()),
                    Some(text_item_count),
                    Some(image_item_count),
                )
            } else {
                (None, None, None)
            };
        let payload_text = output.body.to_text();
        JsReplToolCallResponseSummary {
            response_type: Some(response_type.to_string()),
            payload_kind: Some(payload_kind),
            payload_text_preview: payload_text.as_deref().and_then(|text| {
                (!text.is_empty()).then(|| {
                    truncate_text(
                        text,
                        TruncationPolicy::Bytes(JS_REPL_TOOL_RESPONSE_TEXT_PREVIEW_MAX_BYTES),
                    )
                })
            }),
            payload_text_length: payload_text.as_ref().map(String::len),
            payload_item_count,
            text_item_count,
            image_item_count,
            ..Default::default()
        }
    }

    fn summarize_message_payload(content: &[ContentItem]) -> JsReplToolCallResponseSummary {
        let text_item_count = content
            .iter()
            .filter(|item| {
                matches!(
                    item,
                    ContentItem::InputText { .. } | ContentItem::OutputText { .. }
                )
            })
            .count();
        let image_item_count = content.len().saturating_sub(text_item_count);
        let payload_text = content
            .iter()
            .filter_map(|item| match item {
                ContentItem::InputText { text } | ContentItem::OutputText { text }
                    if !text.trim().is_empty() =>
                {
                    Some(text.as_str())
                }
                ContentItem::InputText { .. }
                | ContentItem::InputImage { .. }
                | ContentItem::OutputText { .. } => None,
            })
            .collect::<Vec<_>>();
        let payload_text = if payload_text.is_empty() {
            None
        } else {
            Some(payload_text.join("\n"))
        };
        JsReplToolCallResponseSummary {
            response_type: Some("message".to_string()),
            payload_kind: Some(JsReplToolCallPayloadKind::MessageContent),
            payload_text_preview: payload_text.as_deref().and_then(|text| {
                (!text.is_empty()).then(|| {
                    truncate_text(
                        text,
                        TruncationPolicy::Bytes(JS_REPL_TOOL_RESPONSE_TEXT_PREVIEW_MAX_BYTES),
                    )
                })
            }),
            payload_text_length: payload_text.as_ref().map(String::len),
            payload_item_count: Some(content.len()),
            text_item_count: Some(text_item_count),
            image_item_count: Some(image_item_count),
            ..Default::default()
        }
    }

    fn summarize_tool_call_response(response: &ResponseInputItem) -> JsReplToolCallResponseSummary {
        match response {
            ResponseInputItem::Message { content, .. } => Self::summarize_message_payload(content),
            ResponseInputItem::FunctionCallOutput { output, .. } => {
                let payload_kind = if output.content_items().is_some() {
                    JsReplToolCallPayloadKind::FunctionContentItems
                } else {
                    JsReplToolCallPayloadKind::FunctionText
                };
                Self::summarize_function_output_payload(
                    "function_call_output",
                    payload_kind,
                    output,
                )
            }
            ResponseInputItem::CustomToolCallOutput { output, .. } => {
                let payload_kind = if output.content_items().is_some() {
                    JsReplToolCallPayloadKind::CustomContentItems
                } else {
                    JsReplToolCallPayloadKind::CustomText
                };
                Self::summarize_function_output_payload(
                    "custom_tool_call_output",
                    payload_kind,
                    output,
                )
            }
            ResponseInputItem::McpToolCallOutput { output, .. } => {
                let function_output = output.as_function_call_output_payload();
                let payload_kind = if output.success() {
                    JsReplToolCallPayloadKind::McpResult
                } else {
                    JsReplToolCallPayloadKind::McpErrorResult
                };
                let mut summary = Self::summarize_function_output_payload(
                    "mcp_tool_call_output",
                    payload_kind,
                    &function_output,
                );
                summary.payload_item_count = Some(output.content.len());
                summary.structured_content_present = Some(output.structured_content.is_some());
                summary.result_is_error = Some(!output.success());
                summary
            }
            ResponseInputItem::ToolSearchOutput { tools, .. } => JsReplToolCallResponseSummary {
                response_type: Some("tool_search_output".to_string()),
                payload_kind: Some(JsReplToolCallPayloadKind::FunctionText),
                payload_text_preview: Some(serde_json::Value::Array(tools.clone()).to_string()),
                payload_text_length: Some(
                    serde_json::Value::Array(tools.clone()).to_string().len(),
                ),
                payload_item_count: Some(tools.len()),
                ..Default::default()
            },
        }
    }

    fn summarize_tool_call_error(error: &str) -> JsReplToolCallResponseSummary {
        Self::summarize_text_payload(None, JsReplToolCallPayloadKind::Error, error)
    }

    fn schedule_completed_exec_eviction(
        exec_store: Arc<Mutex<HashMap<String, ExecBuffer>>>,
        exec_id: String,
    ) {
        tokio::spawn(async move {
            tokio::time::sleep(JS_REPL_POLL_COMPLETED_EXEC_RETENTION).await;
            let mut store = exec_store.lock().await;
            if store.get(&exec_id).is_some_and(|entry| entry.done) {
                store.remove(&exec_id);
            }
        });
    }

    async fn emit_completion_event(event: ExecCompletionEvent) {
        emit_js_repl_exec_end(
            event.session.as_ref(),
            event.turn.as_ref(),
            &event.event_call_id,
            &event.output,
            event.error.as_deref(),
            event.duration,
            event.timed_out,
        )
        .await;
    }

    async fn complete_exec_in_store(
        exec_store: &Arc<Mutex<HashMap<String, ExecBuffer>>>,
        exec_id: &str,
        terminal_kind: ExecTerminalKind,
        final_output: Option<String>,
        content_items: Option<Vec<FunctionCallOutputContentItem>>,
        error: Option<String>,
    ) -> bool {
        let event = {
            let mut store = exec_store.lock().await;
            let Some(entry) = store.get_mut(exec_id) else {
                return false;
            };
            if terminal_kind == ExecTerminalKind::KernelExit && entry.host_terminating {
                return false;
            }
            if entry.done {
                return false;
            }

            entry.done = true;
            entry.host_terminating = false;
            if let Some(final_output) = final_output {
                entry.final_output = Some(final_output);
            }
            if let Some(content_items) = content_items {
                entry.content_items = content_items;
            }
            if error.is_some() || terminal_kind != ExecTerminalKind::Success {
                entry.error = error;
            } else {
                entry.error = None;
            }
            entry.terminal_kind = Some(terminal_kind);
            entry.completed_sequence =
                Some(NEXT_COMPLETED_EXEC_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed));
            entry.notify.notify_waiters();
            let event = ExecCompletionEvent {
                session: Arc::clone(&entry.session),
                turn: Arc::clone(&entry.turn),
                event_call_id: entry.event_call_id.clone(),
                output: entry.display_output(),
                error: entry.error.clone(),
                duration: entry.started_at.elapsed(),
                timed_out: false,
            };
            let completed_exec_count = store.values().filter(|entry| entry.done).count();
            let excess_completed_execs =
                completed_exec_count.saturating_sub(JS_REPL_POLL_MAX_COMPLETED_EXECS);
            if excess_completed_execs > 0 {
                let mut completed_execs = store
                    .iter()
                    .filter_map(|(exec_id, entry)| {
                        entry
                            .done
                            .then_some((exec_id.clone(), entry.completed_sequence.unwrap_or(0)))
                    })
                    .collect::<Vec<_>>();
                completed_execs.sort_by_key(|(_, completed_sequence)| *completed_sequence);
                for exec_id in completed_execs
                    .into_iter()
                    .take(excess_completed_execs)
                    .map(|(exec_id, _)| exec_id)
                {
                    store.remove(&exec_id);
                }
            }

            Some(event)
        };

        if let Some(event) = event {
            Self::schedule_completed_exec_eviction(Arc::clone(exec_store), exec_id.to_string());
            Self::emit_completion_event(event).await;
        }
        true
    }

    fn poll_result_from_entry(
        exec_id: &str,
        entry: &mut ExecBuffer,
    ) -> Result<JsExecPollResult, FunctionCallError> {
        let Some(session_id) = entry.session_id.clone() else {
            return Err(FunctionCallError::RespondToModel(
                "js_repl exec id is not pollable".to_string(),
            ));
        };
        let error = entry.error.clone();
        let done = entry.done;
        Ok(JsExecPollResult {
            exec_id: exec_id.to_string(),
            session_id,
            logs: entry.poll_logs(),
            final_output: entry.poll_final_output(),
            content_items: entry.poll_content_items(),
            error,
            done,
        })
    }

    fn poll_result_from_store(
        exec_id: &str,
        store: &mut HashMap<String, ExecBuffer>,
    ) -> Result<JsExecPollResult, FunctionCallError> {
        let Some(entry) = store.get_mut(exec_id) else {
            return Err(FunctionCallError::RespondToModel(
                "js_repl exec id not found".to_string(),
            ));
        };
        Self::poll_result_from_entry(exec_id, entry)
    }

    /// Resets the singleton kernel and all polling sessions for this turn.
    pub async fn reset(&self) -> Result<(), FunctionCallError> {
        let _permit = self.exec_lock.clone().acquire_owned().await.map_err(|_| {
            FunctionCallError::RespondToModel("js_repl execution unavailable".to_string())
        })?;
        let _poll_lifecycle = self.poll_lifecycle.write().await;
        self.reset_kernel().await;
        self.reset_all_poll_sessions().await;
        Self::clear_all_exec_tool_calls_map(&self.exec_tool_calls).await;
        Ok(())
    }

    pub async fn interrupt_turn_exec(&self, turn_id: &str) -> Result<bool, FunctionCallError> {
        let _permit = self.exec_lock.clone().acquire_owned().await.map_err(|_| {
            FunctionCallError::RespondToModel("js_repl execution unavailable".to_string())
        })?;
        if !self.turn_interrupt_requires_reset(turn_id).await {
            return Ok(false);
        }
        self.reset_kernel().await;
        Self::clear_all_exec_tool_calls_map(&self.exec_tool_calls).await;
        Ok(true)
    }

    /// Resets a specific polling session by session id.
    pub async fn reset_session(&self, session_id: &str) -> Result<(), FunctionCallError> {
        let _poll_lifecycle = self.poll_lifecycle.write().await;
        if self.reset_poll_session(session_id, "poll_reset").await {
            return Ok(());
        }
        Err(FunctionCallError::RespondToModel(
            "js_repl session id not found".to_string(),
        ))
    }

    async fn reset_kernel(&self) {
        let state = {
            let mut guard = self.kernel.lock().await;
            guard.take()
        };
        if let Some(state) = state {
            Self::shutdown_kernel_state(state, "reset").await;
        }
    }

    async fn shutdown_kernel_state(state: KernelState, kill_reason: &'static str) {
        state.shutdown.cancel();
        Self::kill_kernel_child(&state.process, kill_reason).await;
        state.protocol_reader_drained.cancelled().await;
    }

    async fn mark_exec_host_terminating(&self, exec_id: &str) {
        let mut store = self.exec_store.lock().await;
        if let Some(entry) = store.get_mut(exec_id)
            && !entry.done
        {
            entry.host_terminating = true;
        }
    }

    async fn teardown_poll_session_state(
        &self,
        mut state: PollSessionState,
        kill_reason: &'static str,
    ) {
        let active_exec = state.active_exec.take();
        if let Some(exec_id) = active_exec.as_deref() {
            self.mark_exec_host_terminating(exec_id).await;
        }
        Self::kill_kernel_child(&state.kernel.process, kill_reason).await;
        if let Some(exec_id) = active_exec {
            self.exec_to_session.lock().await.remove(&exec_id);
            Self::cancel_exec_tool_calls_map(&self.exec_tool_calls, &exec_id).await;
            Self::wait_for_exec_tool_calls_map(&self.exec_tool_calls, &exec_id).await;
            Self::wait_for_exec_terminal_or_protocol_reader_drained(
                &self.exec_store,
                &exec_id,
                &state.kernel.protocol_reader_drained,
            )
            .await;
            Self::complete_exec_in_store(
                &self.exec_store,
                &exec_id,
                ExecTerminalKind::Cancelled,
                None,
                None,
                Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
            )
            .await;
            Self::clear_exec_tool_calls_map(&self.exec_tool_calls, &exec_id).await;
        }
        state.kernel.protocol_reader_drained.cancelled().await;
    }

    async fn reset_poll_session(&self, session_id: &str, kill_reason: &'static str) -> bool {
        let state = {
            let mut sessions = self.poll_sessions.lock().await;
            sessions.remove(session_id)
        };
        let Some(state) = state else {
            return false;
        };
        let preserved_exec_id = state.active_exec.clone();
        self.teardown_poll_session_state(state, kill_reason).await;
        self.clear_poll_exec_state_for_session(session_id, preserved_exec_id.as_deref())
            .await;
        true
    }

    async fn reset_all_poll_sessions(&self) {
        let states = {
            let mut sessions = self.poll_sessions.lock().await;
            sessions.drain().map(|(_, state)| state).collect::<Vec<_>>()
        };
        let preserved_exec_ids = states
            .iter()
            .filter_map(|state| state.active_exec.clone())
            .collect::<HashSet<_>>();
        for state in states {
            self.teardown_poll_session_state(state, "poll_reset_all")
                .await;
        }
        self.clear_all_poll_exec_state(&preserved_exec_ids).await;
    }

    /// Executes one non-polling `js_repl` request on the singleton kernel.
    pub async fn execute(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        args: JsReplArgs,
    ) -> Result<JsExecResult, JsReplExecuteError> {
        if args.session_id.is_some() {
            return Err(JsReplExecuteError::RespondToModel(
                "js_repl session_id is only supported when poll=true".to_string(),
            ));
        }
        let _permit = self.exec_lock.clone().acquire_owned().await.map_err(|_| {
            JsReplExecuteError::RespondToModel("js_repl execution unavailable".to_string())
        })?;

        let (stdin, pending_execs, exec_contexts, child, recent_stderr) = {
            let mut kernel = self.kernel.lock().await;
            if kernel.is_none() {
                let mut state = self
                    .start_kernel(Arc::clone(&session), Arc::clone(&turn), None)
                    .await
                    .map_err(JsReplExecuteError::RespondToModel)?;
                state.top_level_exec_state = TopLevelExecState::FreshKernel {
                    turn_id: turn.sub_id.clone(),
                    exec_id: None,
                };
                *kernel = Some(state);
            }

            let state = match kernel.as_ref() {
                Some(state) => state,
                None => {
                    return Err(JsReplExecuteError::RespondToModel(
                        "js_repl kernel unavailable".to_string(),
                    ));
                }
            };
            (
                state.stdin.clone(),
                Arc::clone(&state.pending_execs),
                Arc::clone(&state.exec_contexts),
                Arc::clone(&state.process),
                Arc::clone(&state.recent_stderr),
            )
        };

        let (req_id, rx) = {
            let req_id = Uuid::new_v4().to_string();
            let mut pending = pending_execs.lock().await;
            let (tx, rx) = tokio::sync::oneshot::channel();
            pending.insert(req_id.clone(), tx);
            exec_contexts.lock().await.insert(
                req_id.clone(),
                ExecContext {
                    session: Arc::clone(&session),
                    turn: Arc::clone(&turn),
                    tracker,
                },
            );
            (req_id, rx)
        };
        self.register_top_level_exec(req_id.clone(), turn.sub_id.clone())
            .await;
        self.register_exec_tool_calls(&req_id).await;

        let payload = HostToKernel::Exec {
            id: req_id.clone(),
            code: args.code,
            timeout_ms: args.timeout_ms,
            stream_logs: false,
        };

        let write_result = {
            // Treat the exec as submitted before the async pipe writes begin: once we start
            // awaiting `write_all`, the kernel may already observe runnable JS even if the turn is
            // aborted before control returns here.
            self.mark_top_level_exec_submitted(&req_id).await;
            let write_result = Self::write_message(&stdin, &payload).await;
            match write_result {
                Ok(()) => Ok(()),
                Err(err) => {
                    self.clear_top_level_exec_if_matches(&req_id).await;
                    Err(err)
                }
            }
        };

        if let Err(err) = write_result {
            if pending_execs.lock().await.remove(&req_id).is_some() {
                self.clear_top_level_exec_if_matches(&req_id).await;
            }
            exec_contexts.lock().await.remove(&req_id);
            Self::clear_exec_tool_calls_map(&self.exec_tool_calls, &req_id).await;
            let snapshot = Self::kernel_debug_snapshot(&child, &recent_stderr).await;
            let err_message = err.to_string();
            warn!(
                exec_id = %req_id,
                error = %err_message,
                kernel_pid = ?snapshot.pid,
                kernel_status = %snapshot.status,
                kernel_stderr_tail = %snapshot.stderr_tail,
                "failed to submit js_repl exec request to kernel"
            );
            let message =
                if should_include_model_diagnostics_for_write_error(&err_message, &snapshot) {
                    with_model_kernel_failure_message(
                        &err_message,
                        "write_failed",
                        Some(&err_message),
                        &snapshot,
                    )
                } else {
                    err_message
                };
            return Err(JsReplExecuteError::RespondToModel(message));
        }

        let timeout_ms = args.timeout_ms.unwrap_or(30_000);
        let response = match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(_)) => {
                let mut pending = pending_execs.lock().await;
                let removed = pending.remove(&req_id).is_some();
                drop(pending);
                if removed {
                    self.clear_top_level_exec_if_matches(&req_id).await;
                }
                exec_contexts.lock().await.remove(&req_id);
                Self::cancel_exec_tool_calls_map(&self.exec_tool_calls, &req_id).await;
                Self::wait_for_exec_tool_calls_map(&self.exec_tool_calls, &req_id).await;
                Self::clear_exec_tool_calls_map(&self.exec_tool_calls, &req_id).await;
                let snapshot = Self::kernel_debug_snapshot(&child, &recent_stderr).await;
                let message = if is_kernel_status_exited(&snapshot.status) {
                    with_model_kernel_failure_message(
                        "js_repl kernel closed unexpectedly",
                        "response_channel_closed",
                        None,
                        &snapshot,
                    )
                } else {
                    "js_repl kernel closed unexpectedly".to_string()
                };
                return Err(JsReplExecuteError::RespondToModel(message));
            }
            Err(_) => {
                pending_execs.lock().await.remove(&req_id);
                exec_contexts.lock().await.remove(&req_id);
                self.reset_kernel().await;
                self.clear_top_level_exec_if_matches(&req_id).await;
                Self::cancel_exec_tool_calls_map(&self.exec_tool_calls, &req_id).await;
                Self::wait_for_exec_tool_calls_map(&self.exec_tool_calls, &req_id).await;
                Self::clear_exec_tool_calls_map(&self.exec_tool_calls, &req_id).await;
                return Err(JsReplExecuteError::TimedOut);
            }
        };

        match response {
            ExecResultMessage::Ok { content_items } => {
                let (output, content_items) = split_exec_result_content_items(content_items);
                Ok(JsExecResult {
                    output,
                    content_items,
                })
            }
            ExecResultMessage::Err { message } => Err(JsReplExecuteError::RespondToModel(message)),
        }
    }

    /// Submits a polling `js_repl` execution and returns its exec/session ids.
    pub async fn submit(
        self: Arc<Self>,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        event_call_id: String,
        args: JsReplArgs,
    ) -> Result<JsExecSubmission, FunctionCallError> {
        if args.timeout_ms.is_some() {
            return Err(FunctionCallError::RespondToModel(
                JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE.to_string(),
            ));
        }
        let user_provided_session_id = args.session_id.is_some();
        let session_id = args
            .session_id
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        if session_id.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "js_repl session_id must not be empty".to_string(),
            ));
        }
        let max_sessions_error = || {
            FunctionCallError::RespondToModel(format!(
                "js_repl polling has reached the maximum of {JS_REPL_POLL_MAX_SESSIONS} active sessions; reset a session before creating another"
            ))
        };
        let session_busy_error = |active_exec: &str| {
            FunctionCallError::RespondToModel(format!(
                "js_repl session `{session_id}` already has a running exec: `{active_exec}`"
            ))
        };
        let req_id = Uuid::new_v4().to_string();
        let _poll_lifecycle = self.poll_lifecycle.read().await;

        enum PollSessionPlan {
            Reuse,
            Create,
        }

        let session_plan = loop {
            let maybe_active_exec = {
                let mut sessions = self.poll_sessions.lock().await;
                match sessions.get_mut(&session_id) {
                    Some(state) => {
                        if let Some(active_exec) = state.active_exec.clone() {
                            Some(active_exec)
                        } else {
                            state.last_used = Instant::now();
                            break PollSessionPlan::Reuse;
                        }
                    }
                    None if user_provided_session_id => {
                        return Err(FunctionCallError::RespondToModel(
                            "js_repl session id not found".to_string(),
                        ));
                    }
                    None => break PollSessionPlan::Create,
                }
            };
            let Some(active_exec) = maybe_active_exec else {
                continue;
            };

            let done = {
                let store = self.exec_store.lock().await;
                store.get(&active_exec).is_some_and(|entry| entry.done)
            };
            let mut sessions = self.poll_sessions.lock().await;
            let Some(state) = sessions.get_mut(&session_id) else {
                continue;
            };
            if state.active_exec.as_deref() != Some(active_exec.as_str()) {
                continue;
            }
            if done {
                state.active_exec = None;
                state.last_used = Instant::now();
                break PollSessionPlan::Reuse;
            }
            return Err(session_busy_error(&active_exec));
        };
        if let PollSessionPlan::Create = session_plan {
            let mut new_kernel = Some(
                self.start_kernel(
                    Arc::clone(&session),
                    Arc::clone(&turn),
                    Some(session_id.clone()),
                )
                .await
                .map_err(FunctionCallError::RespondToModel)?,
            );
            let mut pruned_idle_session = None;
            let mut stale_kernel = None;
            let mut capacity_kernel = None;
            {
                let mut sessions = self.poll_sessions.lock().await;
                if sessions.contains_key(&session_id) {
                    stale_kernel = new_kernel.take();
                } else {
                    if sessions.len() >= JS_REPL_POLL_MAX_SESSIONS {
                        let lru_idle_session = sessions
                            .iter()
                            .filter(|(_, state)| state.active_exec.is_none())
                            .min_by_key(|(_, state)| state.last_used)
                            .map(|(id, _)| id.clone());
                        if let Some(lru_idle_session) = lru_idle_session {
                            pruned_idle_session = sessions
                                .remove(&lru_idle_session)
                                .map(|state| (lru_idle_session, state));
                        }
                    }
                    if sessions.len() >= JS_REPL_POLL_MAX_SESSIONS {
                        capacity_kernel = new_kernel.take();
                    } else if let Some(kernel) = new_kernel.take() {
                        sessions.insert(
                            session_id.clone(),
                            PollSessionState {
                                kernel,
                                // Reserve the first exec before releasing the
                                // session map so an in-flight create cannot be
                                // pruned as an idle session by a concurrent
                                // submit.
                                active_exec: Some(req_id.clone()),
                                last_used: Instant::now(),
                            },
                        );
                    }
                }
            }
            if let Some((pruned_session_id, state)) = pruned_idle_session {
                self.clear_poll_exec_state_for_session(&pruned_session_id, None)
                    .await;
                Self::shutdown_kernel_state(state.kernel, "poll_prune_idle_session").await;
            }
            if let Some(kernel) = stale_kernel {
                Self::shutdown_kernel_state(kernel, "poll_submit_session_race").await;
            }
            if let Some(kernel) = capacity_kernel {
                Self::shutdown_kernel_state(kernel, "poll_submit_capacity_race").await;
                return Err(max_sessions_error());
            }
        }

        let (stdin, exec_contexts, child, recent_stderr) = loop {
            let maybe_active_exec = {
                let mut sessions = self.poll_sessions.lock().await;
                let Some(state) = sessions.get_mut(&session_id) else {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "js_repl session `{session_id}` is unavailable"
                    )));
                };
                match state.active_exec.clone() {
                    Some(active_exec) if active_exec != req_id => Some(active_exec),
                    _ => {
                        state.active_exec = Some(req_id.clone());
                        state.last_used = Instant::now();
                        break (
                            state.kernel.stdin.clone(),
                            Arc::clone(&state.kernel.exec_contexts),
                            Arc::clone(&state.kernel.process),
                            Arc::clone(&state.kernel.recent_stderr),
                        );
                    }
                }
            };
            let Some(active_exec) = maybe_active_exec else {
                continue;
            };

            let done = {
                let store = self.exec_store.lock().await;
                store.get(&active_exec).is_some_and(|entry| entry.done)
            };
            let mut sessions = self.poll_sessions.lock().await;
            let Some(state) = sessions.get_mut(&session_id) else {
                continue;
            };
            if state.active_exec.as_deref() != Some(active_exec.as_str()) {
                continue;
            }
            if done {
                state.active_exec = None;
                continue;
            }
            return Err(session_busy_error(&active_exec));
        };

        exec_contexts.lock().await.insert(
            req_id.clone(),
            ExecContext {
                session: Arc::clone(&session),
                turn: Arc::clone(&turn),
                tracker,
            },
        );
        self.exec_store.lock().await.insert(
            req_id.clone(),
            ExecBuffer::new(
                event_call_id,
                Some(session_id.clone()),
                Arc::clone(&session),
                Arc::clone(&turn),
            ),
        );
        self.exec_to_session
            .lock()
            .await
            .insert(req_id.clone(), session_id.clone());
        self.register_exec_tool_calls(&req_id).await;

        let payload = HostToKernel::Exec {
            id: req_id.clone(),
            code: args.code,
            timeout_ms: args.timeout_ms,
            stream_logs: true,
        };
        if let Err(err) = Self::write_message(&stdin, &payload).await {
            self.exec_store.lock().await.remove(&req_id);
            exec_contexts.lock().await.remove(&req_id);
            self.exec_to_session.lock().await.remove(&req_id);
            Self::clear_exec_tool_calls_map(&self.exec_tool_calls, &req_id).await;
            let removed_state = {
                let mut sessions = self.poll_sessions.lock().await;
                let should_remove = sessions
                    .get(&session_id)
                    .is_some_and(|state| state.active_exec.as_deref() == Some(req_id.as_str()));
                if should_remove {
                    sessions.remove(&session_id)
                } else {
                    None
                }
            };
            if let Some(state) = removed_state {
                state.kernel.shutdown.cancel();
                Self::kill_kernel_child(&state.kernel.process, "poll_submit_write_failed").await;
            }
            let snapshot = Self::kernel_debug_snapshot(&child, &recent_stderr).await;
            let err_message = err.to_string();
            warn!(
                exec_id = %req_id,
                session_id = %session_id,
                error = %err_message,
                kernel_pid = ?snapshot.pid,
                kernel_status = %snapshot.status,
                kernel_stderr_tail = %snapshot.stderr_tail,
                "failed to submit polled js_repl exec request to kernel"
            );
            let message =
                if should_include_model_diagnostics_for_write_error(&err_message, &snapshot) {
                    with_model_kernel_failure_message(
                        &err_message,
                        "write_failed",
                        Some(&err_message),
                        &snapshot,
                    )
                } else {
                    err_message
                };
            return Err(FunctionCallError::RespondToModel(message));
        }

        Ok(JsExecSubmission {
            exec_id: req_id,
            session_id,
        })
    }

    /// Polls a submitted execution for new logs or terminal output.
    pub async fn poll(
        &self,
        exec_id: &str,
        yield_time_ms: Option<u64>,
    ) -> Result<JsExecPollResult, FunctionCallError> {
        let deadline = Instant::now() + Duration::from_millis(clamp_poll_ms(yield_time_ms));

        loop {
            let (wait_for_update, session_id) = {
                let mut store = self.exec_store.lock().await;
                let Some(entry) = store.get_mut(exec_id) else {
                    return Err(FunctionCallError::RespondToModel(
                        "js_repl exec id not found".to_string(),
                    ));
                };
                if !entry.logs.is_empty() || entry.done {
                    return Self::poll_result_from_entry(exec_id, entry);
                }
                let Some(session_id) = entry.session_id.clone() else {
                    return Err(FunctionCallError::RespondToModel(
                        "js_repl exec id is not pollable".to_string(),
                    ));
                };
                // Capture the wait future while holding the store lock so the
                // next notify lines up with the state snapshot, mirroring the
                // unified_exec background poll path.
                (Arc::clone(&entry.notify).notified_owned(), session_id)
            };
            if let Some(state) = self.poll_sessions.lock().await.get_mut(&session_id) {
                state.last_used = Instant::now();
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                let mut store = self.exec_store.lock().await;
                return Self::poll_result_from_store(exec_id, &mut store);
            }

            if tokio::time::timeout(remaining, wait_for_update)
                .await
                .is_err()
            {
                // Re-snapshot after timeout so a missed notify cannot return stale data.
                let mut store = self.exec_store.lock().await;
                return Self::poll_result_from_store(exec_id, &mut store);
            }
        }
    }
    async fn start_kernel(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        poll_session_id: Option<String>,
    ) -> Result<KernelState, String> {
        let node_path = resolve_compatible_node(self.node_path.as_deref()).await?;

        let kernel_path = self.kernel_script_path.clone();

        let mut env = create_env(
            &turn.shell_environment_policy,
            Some(session.conversation_id),
        );
        env.insert(
            "CODEX_JS_TMP_DIR".to_string(),
            self.tmp_dir.path().to_string_lossy().to_string(),
        );
        let node_module_dirs_key = "CODEX_JS_REPL_NODE_MODULE_DIRS";
        if !self.node_module_dirs.is_empty() && !env.contains_key(node_module_dirs_key) {
            let joined = std::env::join_paths(&self.node_module_dirs)
                .map_err(|err| format!("failed to join js_repl_node_module_dirs: {err}"))?;
            env.insert(
                node_module_dirs_key.to_string(),
                joined.to_string_lossy().to_string(),
            );
        }

        let spec = CommandSpec {
            program: node_path.to_string_lossy().to_string(),
            args: vec![
                "--experimental-vm-modules".to_string(),
                kernel_path.to_string_lossy().to_string(),
            ],
            cwd: turn.cwd.clone(),
            env,
            expiration: ExecExpiration::DefaultTimeout,
            sandbox_permissions: SandboxPermissions::UseDefault,
            additional_permissions: None,
            justification: None,
        };

        let sandbox = SandboxManager::new();
        let attempt = SandboxAttempt::initial_for_turn(
            &sandbox,
            turn.as_ref(),
            SandboxablePreference::Auto,
            SandboxOverride::NoOverride,
        );
        let exec_env = attempt
            .env_for(spec, None)
            .map_err(|err| format!("failed to configure sandbox for js_repl: {err}"))?;
        let ManagedSplitProcess {
            process,
            stdin,
            stdout_rx,
            stderr_rx,
        } = session
            .services
            .unified_exec_manager
            .open_split_pipe_session_with_exec_env(&exec_env)
            .await
            .map_err(|err| format!("failed to start Node runtime: {err}"))?;
        let process = Arc::new(process);

        let shutdown = CancellationToken::new();
        let pending_execs: Arc<
            Mutex<HashMap<String, tokio::sync::oneshot::Sender<ExecResultMessage>>>,
        > = Arc::new(Mutex::new(HashMap::new()));
        let exec_contexts: Arc<Mutex<HashMap<String, ExecContext>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let recent_stderr = Arc::new(Mutex::new(VecDeque::with_capacity(
            JS_REPL_STDERR_TAIL_LINE_LIMIT,
        )));
        let protocol_reader_drained = CancellationToken::new();

        tokio::spawn(Self::read_stdout(
            stdout_rx,
            Arc::clone(&process),
            Arc::clone(&self.kernel),
            Arc::clone(&recent_stderr),
            Arc::clone(&pending_execs),
            Arc::clone(&exec_contexts),
            Arc::clone(&self.exec_tool_calls),
            Arc::clone(&self.exec_store),
            Arc::clone(&self.poll_sessions),
            Arc::clone(&self.exec_to_session),
            stdin.clone(),
            poll_session_id,
            protocol_reader_drained.clone(),
            shutdown.clone(),
        ));
        tokio::spawn(Self::read_stderr(
            stderr_rx,
            Arc::clone(&recent_stderr),
            shutdown.clone(),
        ));

        Ok(KernelState {
            process,
            recent_stderr,
            stdin,
            pending_execs,
            exec_contexts,
            top_level_exec_state: TopLevelExecState::Idle,
            protocol_reader_drained,
            shutdown,
        })
    }

    async fn write_kernel_script(dir: &Path) -> Result<PathBuf, std::io::Error> {
        let kernel_path = dir.join("js_repl_kernel.js");
        let meriyah_path = dir.join("meriyah.umd.min.js");
        tokio::fs::write(&kernel_path, KERNEL_SOURCE).await?;
        tokio::fs::write(&meriyah_path, MERIYAH_UMD).await?;
        Ok(kernel_path)
    }

    async fn write_message(
        stdin: &tokio::sync::mpsc::Sender<Vec<u8>>,
        msg: &HostToKernel,
    ) -> Result<(), FunctionCallError> {
        let encoded = serde_json::to_string(msg).map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to serialize kernel message: {err}"))
        })?;
        let mut bytes = encoded.into_bytes();
        bytes.push(b'\n');
        stdin.send(bytes).await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to write to kernel: {err}"))
        })?;
        Ok(())
    }

    async fn kernel_stderr_tail_snapshot(recent_stderr: &Arc<Mutex<VecDeque<String>>>) -> String {
        let tail = recent_stderr.lock().await;
        format_stderr_tail(&tail)
    }

    async fn kernel_debug_snapshot(
        process: &Arc<UnifiedExecProcess>,
        recent_stderr: &Arc<Mutex<VecDeque<String>>>,
    ) -> KernelDebugSnapshot {
        let pid = process.pid();
        let status = if process.has_exited() {
            match process.exit_code() {
                Some(code) => format!("exited({code})"),
                None => "exited(unknown)".to_string(),
            }
        } else {
            "running".to_string()
        };
        let stderr_tail = {
            let tail = recent_stderr.lock().await;
            format_stderr_tail(&tail)
        };
        KernelDebugSnapshot {
            pid,
            status,
            stderr_tail,
        }
    }

    async fn kill_kernel_child(process: &Arc<UnifiedExecProcess>, reason: &'static str) {
        if process.has_exited() {
            return;
        }

        let pid = process.pid();
        process.request_terminate();
        let exited = tokio::time::timeout(JS_REPL_KILL_WAIT_TIMEOUT, async {
            while !process.has_exited() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .is_ok();
        if !exited {
            warn!(
                kernel_pid = ?pid,
                kill_reason = reason,
                "js_repl kernel process did not report exit before timeout"
            );
            process.terminate();
        }
        warn!(
            kernel_pid = ?pid,
            kill_reason = reason,
            "terminated js_repl kernel process"
        );
    }

    fn truncate_id_list(ids: &[String]) -> Vec<String> {
        if ids.len() <= JS_REPL_EXEC_ID_LOG_LIMIT {
            return ids.to_vec();
        }
        let mut output = ids[..JS_REPL_EXEC_ID_LOG_LIMIT].to_vec();
        output.push(format!("...+{}", ids.len() - JS_REPL_EXEC_ID_LOG_LIMIT));
        output
    }

    #[allow(clippy::too_many_arguments)]
    async fn read_stdout(
        mut stdout: mpsc::Receiver<Vec<u8>>,
        process: Arc<UnifiedExecProcess>,
        manager_kernel: Arc<Mutex<Option<KernelState>>>,
        recent_stderr: Arc<Mutex<VecDeque<String>>>,
        pending_execs: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<ExecResultMessage>>>>,
        exec_contexts: Arc<Mutex<HashMap<String, ExecContext>>>,
        exec_tool_calls: Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_store: Arc<Mutex<HashMap<String, ExecBuffer>>>,
        poll_sessions: Arc<Mutex<HashMap<String, PollSessionState>>>,
        exec_to_session: Arc<Mutex<HashMap<String, String>>>,
        stdin: tokio::sync::mpsc::Sender<Vec<u8>>,
        poll_session_id: Option<String>,
        protocol_reader_drained: CancellationToken,
        shutdown: CancellationToken,
    ) {
        let mut pending_line = Vec::new();
        let mut ready_lines = VecDeque::new();
        let end_reason = 'outer: loop {
            let line = if let Some(line) = ready_lines.pop_front() {
                line
            } else {
                loop {
                    let chunk = tokio::select! {
                        _ = shutdown.cancelled() => break 'outer KernelStreamEnd::Shutdown,
                        res = stdout.recv() => match res {
                            Some(chunk) => chunk,
                            None => {
                                if let Some(line) = finish_broadcast_line(&mut pending_line) {
                                    break line;
                                }
                                break 'outer KernelStreamEnd::StdoutEof;
                            }
                        },
                    };
                    pending_line.extend_from_slice(&chunk);
                    let lines = drain_broadcast_lines(&mut pending_line);
                    if lines.is_empty() {
                        continue;
                    }
                    ready_lines.extend(lines);
                    let Some(line) = ready_lines.pop_front() else {
                        continue;
                    };
                    break line;
                }
            };

            let parsed: Result<KernelToHost, _> = serde_json::from_str(&line);
            let msg = match parsed {
                Ok(m) => m,
                Err(err) => {
                    warn!("js_repl kernel sent invalid json: {err} (line: {line})");
                    continue;
                }
            };

            match msg {
                KernelToHost::ExecLog { id, text } => {
                    let (session, turn, event_call_id, delta_chunks) = {
                        let mut store = exec_store.lock().await;
                        let Some(entry) = store.get_mut(&id) else {
                            continue;
                        };
                        entry.push_log(text.clone());
                        let delta_chunks = entry.output_delta_chunks_for_log_line(&text);
                        entry.notify.notify_waiters();
                        (
                            Arc::clone(&entry.session),
                            Arc::clone(&entry.turn),
                            entry.event_call_id.clone(),
                            delta_chunks,
                        )
                    };

                    for chunk in delta_chunks {
                        let event = ExecCommandOutputDeltaEvent {
                            call_id: event_call_id.clone(),
                            stream: ExecOutputStream::Stdout,
                            chunk,
                        };
                        session
                            .send_event(turn.as_ref(), EventMsg::ExecCommandOutputDelta(event))
                            .await;
                    }
                }
                KernelToHost::ExecResult {
                    id,
                    ok,
                    output,
                    error,
                } => {
                    let session_id = exec_to_session.lock().await.remove(&id);
                    JsReplManager::wait_for_exec_tool_calls_map(&exec_tool_calls, &id).await;
                    let content_items = {
                        let calls = exec_tool_calls.lock().await;
                        calls
                            .get(&id)
                            .map(|state| state.content_items.clone())
                            .unwrap_or_default()
                    };
                    let tx = {
                        let mut pending = pending_execs.lock().await;
                        pending.remove(&id)
                    };
                    if let Some(tx) = tx {
                        Self::clear_top_level_exec_if_matches_map(&manager_kernel, &id).await;
                        let payload = if ok {
                            ExecResultMessage::Ok {
                                content_items: build_exec_result_content_items(
                                    output.clone(),
                                    content_items.clone(),
                                ),
                            }
                        } else {
                            ExecResultMessage::Err {
                                message: error
                                    .clone()
                                    .unwrap_or_else(|| "js_repl execution failed".to_string()),
                            }
                        };
                        let _ = tx.send(payload);
                    }
                    let terminal_kind = if ok {
                        ExecTerminalKind::Success
                    } else {
                        ExecTerminalKind::Error
                    };
                    let completion_error = if ok {
                        None
                    } else {
                        Some(error.unwrap_or_else(|| "js_repl execution failed".to_string()))
                    };
                    Self::complete_exec_in_store(
                        &exec_store,
                        &id,
                        terminal_kind,
                        Some(output),
                        ok.then_some(content_items),
                        completion_error,
                    )
                    .await;
                    exec_contexts.lock().await.remove(&id);
                    JsReplManager::clear_exec_tool_calls_map(&exec_tool_calls, &id).await;
                    if let Some(session_id) = session_id.as_ref() {
                        let mut sessions = poll_sessions.lock().await;
                        if let Some(state) = sessions.get_mut(session_id)
                            && state.active_exec.as_deref() == Some(id.as_str())
                        {
                            // Make the session reusable only after nested tool
                            // results have been written back to the kernel and
                            // terminal state is committed.
                            state.active_exec = None;
                            state.last_used = Instant::now();
                        }
                    }
                }
                KernelToHost::EmitImage(req) => {
                    let exec_id = req.exec_id.clone();
                    let emit_id = req.id.clone();
                    let response =
                        if let Some(ctx) = exec_contexts.lock().await.get(&exec_id).cloned() {
                            match validate_emitted_image_url(&req.image_url) {
                                Ok(()) => {
                                    let content_item = emitted_image_content_item(
                                        ctx.turn.as_ref(),
                                        req.image_url,
                                        req.detail,
                                    );
                                    JsReplManager::record_exec_content_item(
                                        &exec_tool_calls,
                                        &exec_id,
                                        content_item,
                                    )
                                    .await;
                                    HostToKernel::EmitImageResult(EmitImageResult {
                                        id: emit_id,
                                        ok: true,
                                        error: None,
                                    })
                                }
                                Err(error) => HostToKernel::EmitImageResult(EmitImageResult {
                                    id: emit_id,
                                    ok: false,
                                    error: Some(error),
                                }),
                            }
                        } else {
                            HostToKernel::EmitImageResult(EmitImageResult {
                                id: emit_id,
                                ok: false,
                                error: Some("js_repl exec context not found".to_string()),
                            })
                        };

                    if let Err(err) = JsReplManager::write_message(&stdin, &response).await {
                        let snapshot =
                            JsReplManager::kernel_debug_snapshot(&process, &recent_stderr).await;
                        warn!(
                            exec_id = %exec_id,
                            emit_id = %req.id,
                            error = %err,
                            kernel_pid = ?snapshot.pid,
                            kernel_status = %snapshot.status,
                            kernel_stderr_tail = %snapshot.stderr_tail,
                            "failed to reply to kernel emit_image request"
                        );
                    }
                }
                KernelToHost::RunTool(req) => {
                    let Some(reset_cancel) =
                        JsReplManager::begin_exec_tool_call(&exec_tool_calls, &req.exec_id).await
                    else {
                        let exec_id = req.exec_id.clone();
                        let tool_call_id = req.id.clone();
                        let payload = HostToKernel::RunToolResult(RunToolResult {
                            id: req.id,
                            ok: false,
                            response: None,
                            error: Some("js_repl exec context not found".to_string()),
                        });
                        if let Err(err) = JsReplManager::write_message(&stdin, &payload).await {
                            let snapshot =
                                JsReplManager::kernel_debug_snapshot(&process, &recent_stderr)
                                    .await;
                            warn!(
                                exec_id = %exec_id,
                                tool_call_id = %tool_call_id,
                                error = %err,
                                kernel_pid = ?snapshot.pid,
                                kernel_status = %snapshot.status,
                                kernel_stderr_tail = %snapshot.stderr_tail,
                                "failed to reply to kernel run_tool request"
                            );
                        }
                        continue;
                    };
                    let stdin_clone = stdin.clone();
                    let exec_contexts = Arc::clone(&exec_contexts);
                    let exec_tool_calls_for_task = Arc::clone(&exec_tool_calls);
                    let recent_stderr = Arc::clone(&recent_stderr);
                    tokio::spawn(async move {
                        let exec_id = req.exec_id.clone();
                        let tool_call_id = req.id.clone();
                        let tool_name = req.tool_name.clone();
                        let context = { exec_contexts.lock().await.get(&exec_id).cloned() };
                        let result = match context {
                            Some(ctx) => {
                                tokio::select! {
                                    _ = reset_cancel.cancelled() => RunToolResult {
                                        id: tool_call_id.clone(),
                                        ok: false,
                                        response: None,
                                        error: Some("js_repl execution reset".to_string()),
                                    },
                                    result = JsReplManager::run_tool_request(ctx, req) => result,
                                }
                            }
                            None => RunToolResult {
                                id: tool_call_id.clone(),
                                ok: false,
                                response: None,
                                error: Some("js_repl exec context not found".to_string()),
                            },
                        };
                        let payload = HostToKernel::RunToolResult(result);
                        let write_result =
                            JsReplManager::write_message(&stdin_clone, &payload).await;
                        JsReplManager::finish_exec_tool_call(&exec_tool_calls_for_task, &exec_id)
                            .await;
                        if let Err(err) = write_result {
                            let stderr_tail =
                                JsReplManager::kernel_stderr_tail_snapshot(&recent_stderr).await;
                            warn!(
                                exec_id = %exec_id,
                                tool_call_id = %tool_call_id,
                                tool_name = %tool_name,
                                error = %err,
                                kernel_stderr_tail = %stderr_tail,
                                "failed to reply to kernel run_tool request"
                            );
                        }
                    });
                }
            }
        };

        let mut exec_ids_from_contexts = {
            let mut contexts = exec_contexts.lock().await;
            let ids = contexts.keys().cloned().collect::<Vec<_>>();
            contexts.clear();
            ids
        };
        for exec_id in &exec_ids_from_contexts {
            JsReplManager::cancel_exec_tool_calls_map(&exec_tool_calls, exec_id).await;
            JsReplManager::wait_for_exec_tool_calls_map(&exec_tool_calls, exec_id).await;
            JsReplManager::clear_exec_tool_calls_map(&exec_tool_calls, exec_id).await;
        }
        let unexpected_snapshot = if matches!(end_reason, KernelStreamEnd::Shutdown) {
            None
        } else {
            Some(Self::kernel_debug_snapshot(&process, &recent_stderr).await)
        };
        let kernel_failure_message = unexpected_snapshot.as_ref().map(|snapshot| {
            with_model_kernel_failure_message(
                "js_repl kernel exited unexpectedly",
                end_reason.reason(),
                end_reason.error(),
                snapshot,
            )
        });
        let kernel_exit_message = kernel_failure_message
            .clone()
            .unwrap_or_else(|| "js_repl kernel exited unexpectedly".to_string());

        {
            let mut kernel = manager_kernel.lock().await;
            let should_clear = kernel
                .as_ref()
                .is_some_and(|state| Arc::ptr_eq(&state.process, &process));
            if should_clear {
                kernel.take();
            }
        }

        let mut pending = pending_execs.lock().await;
        let pending_exec_ids = pending.keys().cloned().collect::<Vec<_>>();
        for (_id, tx) in pending.drain() {
            let _ = tx.send(ExecResultMessage::Err {
                message: kernel_exit_message.clone(),
            });
        }
        drop(pending);
        if !pending_exec_ids.is_empty() {
            Self::clear_top_level_exec_if_matches_any_map(&manager_kernel, &pending_exec_ids).await;
        }
        let mut affected_exec_ids: HashSet<String> = exec_ids_from_contexts.drain(..).collect();
        affected_exec_ids.extend(pending_exec_ids.iter().cloned());
        if let Some(poll_session_id) = poll_session_id.as_ref() {
            let removed_session = {
                let mut sessions = poll_sessions.lock().await;
                let should_remove = sessions
                    .get(poll_session_id)
                    .is_some_and(|state| Arc::ptr_eq(&state.kernel.process, &process));
                if should_remove {
                    sessions.remove(poll_session_id)
                } else {
                    None
                }
            };
            if let Some(state) = removed_session
                && let Some(active_exec) = state.active_exec
            {
                affected_exec_ids.insert(active_exec);
            }
        }
        for exec_id in &affected_exec_ids {
            exec_to_session.lock().await.remove(exec_id);
        }
        for exec_id in &affected_exec_ids {
            Self::complete_exec_in_store(
                &exec_store,
                exec_id,
                ExecTerminalKind::KernelExit,
                None,
                None,
                Some(kernel_exit_message.clone()),
            )
            .await;
        }
        let mut affected_exec_ids = affected_exec_ids.into_iter().collect::<Vec<_>>();
        affected_exec_ids.sort_unstable();

        if let Some(snapshot) = unexpected_snapshot {
            let mut pending_exec_ids = pending_exec_ids;
            pending_exec_ids.sort_unstable();
            warn!(
                reason = %end_reason.reason(),
                stream_error = %end_reason.error().unwrap_or(""),
                kernel_pid = ?snapshot.pid,
                kernel_status = %snapshot.status,
                pending_exec_count = pending_exec_ids.len(),
                pending_exec_ids = ?Self::truncate_id_list(&pending_exec_ids),
                affected_exec_count = affected_exec_ids.len(),
                affected_exec_ids = ?Self::truncate_id_list(&affected_exec_ids),
                kernel_stderr_tail = %snapshot.stderr_tail,
                "js_repl kernel terminated unexpectedly"
            );
        }
        protocol_reader_drained.cancel();
    }

    async fn run_tool_request(exec: ExecContext, req: RunToolRequest) -> RunToolResult {
        if is_js_repl_internal_tool(&req.tool_name) {
            let error = "js_repl cannot invoke itself".to_string();
            let summary = Self::summarize_tool_call_error(&error);
            Self::log_tool_call_response(&req, false, &summary, None, Some(&error));
            return RunToolResult {
                id: req.id,
                ok: false,
                response: None,
                error: Some(error),
            };
        }

        let mcp_tools = exec
            .session
            .services
            .mcp_connection_manager
            .read()
            .await
            .list_all_tools()
            .await;

        let router = ToolRouter::from_config(
            &exec.turn.tools_config,
            crate::tools::router::ToolRouterParams {
                mcp_tools: Some(
                    mcp_tools
                        .into_iter()
                        .map(|(name, tool)| (name, tool.tool))
                        .collect(),
                ),
                app_tools: None,
                discoverable_tools: None,
                dynamic_tools: exec.turn.dynamic_tools.as_slice(),
            },
        );

        let payload = if let Some((server, tool)) = exec
            .session
            .parse_mcp_tool_name(&req.tool_name, &None)
            .await
        {
            crate::tools::context::ToolPayload::Mcp {
                server,
                tool,
                raw_arguments: req.arguments.clone(),
            }
        } else if is_freeform_tool(&router.specs(), &req.tool_name) {
            crate::tools::context::ToolPayload::Custom {
                input: req.arguments.clone(),
            }
        } else {
            crate::tools::context::ToolPayload::Function {
                arguments: req.arguments.clone(),
            }
        };

        let tool_name = req.tool_name.clone();
        let call = crate::tools::router::ToolCall {
            tool_name: tool_name.clone(),
            tool_namespace: None,
            call_id: req.id.clone(),
            payload,
        };

        let session = Arc::clone(&exec.session);
        let turn = Arc::clone(&exec.turn);
        let tracker = Arc::clone(&exec.tracker);

        match router
            .dispatch_tool_call(
                session.clone(),
                turn,
                tracker,
                call,
                crate::tools::router::ToolCallSource::JsRepl,
            )
            .await
        {
            Ok(response) => {
                let summary = Self::summarize_tool_call_response(&response);
                match serde_json::to_value(response) {
                    Ok(value) => {
                        Self::log_tool_call_response(&req, true, &summary, Some(&value), None);
                        RunToolResult {
                            id: req.id,
                            ok: true,
                            response: Some(value),
                            error: None,
                        }
                    }
                    Err(err) => {
                        let error = format!("failed to serialize tool output: {err}");
                        let summary = Self::summarize_tool_call_error(&error);
                        Self::log_tool_call_response(&req, false, &summary, None, Some(&error));
                        RunToolResult {
                            id: req.id,
                            ok: false,
                            response: None,
                            error: Some(error),
                        }
                    }
                }
            }
            Err(err) => {
                let error = err.to_string();
                let summary = Self::summarize_tool_call_error(&error);
                Self::log_tool_call_response(&req, false, &summary, None, Some(&error));
                RunToolResult {
                    id: req.id,
                    ok: false,
                    response: None,
                    error: Some(error),
                }
            }
        }
    }

    async fn read_stderr(
        mut stderr: mpsc::Receiver<Vec<u8>>,
        recent_stderr: Arc<Mutex<VecDeque<String>>>,
        shutdown: CancellationToken,
    ) {
        let mut pending_line = Vec::new();
        let mut ready_lines = VecDeque::new();

        loop {
            let line = if let Some(line) = ready_lines.pop_front() {
                line
            } else {
                loop {
                    let chunk = tokio::select! {
                        _ = shutdown.cancelled() => return,
                        res = stderr.recv() => match res {
                            Some(chunk) => chunk,
                            None => {
                                if let Some(line) = finish_broadcast_line(&mut pending_line) {
                                    break line;
                                }
                                return;
                            }
                        },
                    };
                    pending_line.extend_from_slice(&chunk);
                    let lines = drain_broadcast_lines(&mut pending_line);
                    if lines.is_empty() {
                        continue;
                    }
                    ready_lines.extend(lines);
                    let Some(line) = ready_lines.pop_front() else {
                        continue;
                    };
                    break line;
                }
            };
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let bounded_line = {
                    let mut tail = recent_stderr.lock().await;
                    push_stderr_tail_line(&mut tail, trimmed)
                };
                if bounded_line.is_empty() {
                    continue;
                }
                warn!("js_repl stderr: {bounded_line}");
            }
        }
    }
}

fn is_freeform_tool(specs: &[ToolSpec], name: &str) -> bool {
    specs
        .iter()
        .any(|spec| spec.name() == name && matches!(spec, ToolSpec::Freeform(_)))
}

fn is_js_repl_internal_tool(name: &str) -> bool {
    matches!(name, "js_repl" | "js_repl_poll" | "js_repl_reset")
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
