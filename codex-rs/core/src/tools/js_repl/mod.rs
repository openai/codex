use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
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
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
pub struct JsExecResult {
    pub output: String,
    pub content_items: Vec<FunctionCallOutputContentItem>,
}

#[derive(Debug, Error, PartialEq)]
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
pub struct JsExecSubmission {
    pub exec_id: String,
    pub session_id: String,
}

#[derive(Clone, Debug)]
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

struct ExecBuffer {
    event_call_id: String,
    session_id: Option<String>,
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    logs: VecDeque<String>,
    logs_bytes: usize,
    logs_truncated: bool,
    all_logs: Vec<String>,
    all_logs_bytes: usize,
    all_logs_truncated: bool,
    final_output: Option<String>,
    content_items: Vec<FunctionCallOutputContentItem>,
    error: Option<String>,
    done: bool,
    host_terminating: bool,
    terminal_kind: Option<ExecTerminalKind>,
    started_at: Instant,
    notify: Arc<Notify>,
    emitted_deltas: usize,
}

impl ExecBuffer {
    fn new(
        event_call_id: String,
        session_id: Option<String>,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
    ) -> Self {
        Self {
            event_call_id,
            session_id,
            session,
            turn,
            logs: VecDeque::new(),
            logs_bytes: 0,
            logs_truncated: false,
            all_logs: Vec::new(),
            all_logs_bytes: 0,
            all_logs_truncated: false,
            final_output: None,
            content_items: Vec::new(),
            error: None,
            done: false,
            host_terminating: false,
            terminal_kind: None,
            started_at: Instant::now(),
            notify: Arc::new(Notify::new()),
            emitted_deltas: 0,
        }
    }

    fn push_log(&mut self, text: String) {
        self.logs.push_back(text.clone());
        self.logs_bytes = self.logs_bytes.saturating_add(text.len());
        while self.logs_bytes > JS_REPL_POLL_LOG_QUEUE_MAX_BYTES {
            let Some(removed) = self.logs.pop_front() else {
                break;
            };
            self.logs_bytes = self.logs_bytes.saturating_sub(removed.len());
            self.logs_truncated = true;
        }
        if self.logs_truncated
            && self
                .logs
                .front()
                .is_none_or(|line| line != JS_REPL_POLL_LOGS_TRUNCATED_MARKER)
        {
            let marker_len = JS_REPL_POLL_LOGS_TRUNCATED_MARKER.len();
            while self.logs_bytes.saturating_add(marker_len) > JS_REPL_POLL_LOG_QUEUE_MAX_BYTES {
                let Some(removed) = self.logs.pop_front() else {
                    break;
                };
                self.logs_bytes = self.logs_bytes.saturating_sub(removed.len());
            }
            self.logs
                .push_front(JS_REPL_POLL_LOGS_TRUNCATED_MARKER.to_string());
            self.logs_bytes = self.logs_bytes.saturating_add(marker_len);
        }

        if self.all_logs_truncated {
            return;
        }
        let separator_bytes = if self.all_logs.is_empty() { 0 } else { 1 };
        let next_bytes = text.len() + separator_bytes;
        if self.all_logs_bytes.saturating_add(next_bytes) > JS_REPL_POLL_ALL_LOGS_MAX_BYTES {
            self.all_logs
                .push(JS_REPL_POLL_ALL_LOGS_TRUNCATED_MARKER.to_string());
            self.all_logs_truncated = true;
            return;
        }

        self.all_logs.push(text);
        self.all_logs_bytes = self.all_logs_bytes.saturating_add(next_bytes);
    }

    fn poll_logs(&mut self) -> Vec<String> {
        let drained: Vec<String> = self.logs.drain(..).collect();
        self.logs_bytes = 0;
        self.logs_truncated = false;
        drained
    }

    fn display_output(&self) -> String {
        if let Some(final_output) = self.final_output.as_deref()
            && !final_output.is_empty()
        {
            return final_output.to_string();
        }
        self.all_logs.join("\n")
    }

    fn poll_final_output(&self) -> Option<String> {
        if self.done {
            self.final_output.clone()
        } else {
            None
        }
    }

    fn poll_content_items(&self) -> Vec<FunctionCallOutputContentItem> {
        if self.done && self.error.is_none() {
            self.content_items.clone()
        } else {
            Vec::new()
        }
    }

    fn output_delta_chunks_for_log_line(&mut self, line: &str) -> Vec<Vec<u8>> {
        if self.emitted_deltas >= MAX_EXEC_OUTPUT_DELTAS_PER_CALL {
            return Vec::new();
        }

        let mut text = String::with_capacity(line.len() + 1);
        text.push_str(line);
        text.push('\n');

        let remaining = MAX_EXEC_OUTPUT_DELTAS_PER_CALL - self.emitted_deltas;
        let chunks =
            split_utf8_chunks_with_limits(&text, JS_REPL_OUTPUT_DELTA_MAX_BYTES, remaining);
        self.emitted_deltas += chunks.len();
        chunks
    }
}

fn split_utf8_chunks_with_limits(input: &str, max_bytes: usize, max_chunks: usize) -> Vec<Vec<u8>> {
    if input.is_empty() || max_bytes == 0 || max_chunks == 0 {
        return Vec::new();
    }

    let bytes = input.as_bytes();
    let mut output = Vec::new();
    let mut start = 0usize;
    while start < input.len() && output.len() < max_chunks {
        let mut end = (start + max_bytes).min(input.len());
        while end > start && !input.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            if let Some(ch) = input[start..].chars().next() {
                end = (start + ch.len_utf8()).min(input.len());
            } else {
                break;
            }
        }

        output.push(bytes[start..end].to_vec());
        start = end;
    }
    output
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecTerminalKind {
    Success,
    Error,
    KernelExit,
    Cancelled,
}

struct ExecCompletionEvent {
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    event_call_id: String,
    output: String,
    error: Option<String>,
    duration: Duration,
    timed_out: bool,
}

enum KernelStreamEnd {
    Shutdown,
    StdoutEof,
}

impl KernelStreamEnd {
    fn reason(&self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown",
            Self::StdoutEof => "stdout_eof",
        }
    }

    fn error(&self) -> Option<&str> {
        None
    }
}

struct KernelDebugSnapshot {
    pid: Option<u32>,
    status: String,
    stderr_tail: String,
}

fn format_stderr_tail(lines: &VecDeque<String>) -> String {
    if lines.is_empty() {
        return "<empty>".to_string();
    }
    lines
        .iter()
        .cloned()
        .collect::<Vec<_>>()
        .join(JS_REPL_STDERR_TAIL_SEPARATOR)
}

fn truncate_utf8_prefix_by_bytes(input: &str, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input.to_string();
    }
    if max_bytes == 0 {
        return String::new();
    }
    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    input[..end].to_string()
}

fn stderr_tail_formatted_bytes(lines: &VecDeque<String>) -> usize {
    if lines.is_empty() {
        return 0;
    }
    let payload_bytes: usize = lines.iter().map(String::len).sum();
    let separator_bytes = JS_REPL_STDERR_TAIL_SEPARATOR.len() * (lines.len() - 1);
    payload_bytes + separator_bytes
}

fn stderr_tail_bytes_with_candidate(lines: &VecDeque<String>, line: &str) -> usize {
    if lines.is_empty() {
        return line.len();
    }
    stderr_tail_formatted_bytes(lines) + JS_REPL_STDERR_TAIL_SEPARATOR.len() + line.len()
}

fn push_stderr_tail_line(lines: &mut VecDeque<String>, line: &str) -> String {
    let max_line_bytes = JS_REPL_STDERR_TAIL_LINE_MAX_BYTES.min(JS_REPL_STDERR_TAIL_MAX_BYTES);
    let bounded_line = truncate_utf8_prefix_by_bytes(line, max_line_bytes);
    if bounded_line.is_empty() {
        return bounded_line;
    }

    while !lines.is_empty()
        && (lines.len() >= JS_REPL_STDERR_TAIL_LINE_LIMIT
            || stderr_tail_bytes_with_candidate(lines, &bounded_line)
                > JS_REPL_STDERR_TAIL_MAX_BYTES)
    {
        lines.pop_front();
    }

    lines.push_back(bounded_line.clone());
    bounded_line
}

fn is_kernel_status_exited(status: &str) -> bool {
    status.starts_with("exited(")
}

fn should_include_model_diagnostics_for_write_error(
    err_message: &str,
    snapshot: &KernelDebugSnapshot,
) -> bool {
    is_kernel_status_exited(&snapshot.status)
        || err_message.to_ascii_lowercase().contains("broken pipe")
}

fn format_model_kernel_failure_details(
    reason: &str,
    stream_error: Option<&str>,
    snapshot: &KernelDebugSnapshot,
) -> String {
    let payload = serde_json::json!({
        "reason": reason,
        "stream_error": stream_error
            .map(|err| truncate_utf8_prefix_by_bytes(err, JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES)),
        "kernel_pid": snapshot.pid,
        "kernel_status": snapshot.status,
        "kernel_stderr_tail": truncate_utf8_prefix_by_bytes(
            &snapshot.stderr_tail,
            JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES,
        ),
    });
    let encoded = serde_json::to_string(&payload)
        .unwrap_or_else(|err| format!(r#"{{"reason":"serialization_error","error":"{err}"}}"#));
    format!("js_repl diagnostics: {encoded}")
}

fn with_model_kernel_failure_message(
    base_message: &str,
    reason: &str,
    stream_error: Option<&str>,
    snapshot: &KernelDebugSnapshot,
) -> String {
    format!(
        "{base_message}\n\n{}",
        format_model_kernel_failure_details(reason, stream_error, snapshot)
    )
}

fn join_outputs(stdout: &str, stderr: &str) -> String {
    if stdout.is_empty() {
        stderr.to_string()
    } else if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    }
}

fn build_js_repl_exec_output(
    output: &str,
    error: Option<&str>,
    duration: Duration,
    timed_out: bool,
) -> ExecToolCallOutput {
    let stdout = output.to_string();
    let stderr = error.unwrap_or("").to_string();
    let aggregated_output = join_outputs(&stdout, &stderr);
    ExecToolCallOutput {
        exit_code: if error.is_some() { 1 } else { 0 },
        stdout: StreamOutput::new(stdout),
        stderr: StreamOutput::new(stderr),
        aggregated_output: StreamOutput::new(aggregated_output),
        duration,
        timed_out,
    }
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
            ResponseInputItem::McpToolCallOutput { result, .. } => match result {
                Ok(result) => {
                    let output = FunctionCallOutputPayload::from(result);
                    let mut summary = Self::summarize_function_output_payload(
                        "mcp_tool_call_output",
                        JsReplToolCallPayloadKind::McpResult,
                        &output,
                    );
                    summary.payload_item_count = Some(result.content.len());
                    summary.structured_content_present = Some(result.structured_content.is_some());
                    summary.result_is_error = Some(result.is_error.unwrap_or(false));
                    summary
                }
                Err(error) => {
                    let mut summary = Self::summarize_text_payload(
                        Some("mcp_tool_call_output"),
                        JsReplToolCallPayloadKind::McpErrorResult,
                        error,
                    );
                    summary.result_is_error = Some(true);
                    summary
                }
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
            entry.notify.notify_waiters();

            Some(ExecCompletionEvent {
                session: Arc::clone(&entry.session),
                turn: Arc::clone(&entry.turn),
                event_call_id: entry.event_call_id.clone(),
                output: entry.display_output(),
                error: entry.error.clone(),
                duration: entry.started_at.elapsed(),
                timed_out: false,
            })
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
                let state = self
                    .start_kernel(Arc::clone(&session), Arc::clone(&turn), None)
                    .await
                    .map_err(JsReplExecuteError::RespondToModel)?;
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
        self.register_exec_tool_calls(&req_id).await;

        let payload = HostToKernel::Exec {
            id: req_id.clone(),
            code: args.code,
            timeout_ms: args.timeout_ms,
            stream_logs: false,
        };

        if let Err(err) = Self::write_message(&stdin, &payload).await {
            pending_execs.lock().await.remove(&req_id);
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
                pending.remove(&req_id);
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
        let _poll_lifecycle = self.poll_lifecycle.read().await;

        enum PollSessionPlan {
            Reuse,
            Create,
        }

        let session_plan = {
            let mut sessions = self.poll_sessions.lock().await;
            match sessions.get_mut(&session_id) {
                Some(state) => {
                    if let Some(active_exec) = state.active_exec.as_deref() {
                        return Err(session_busy_error(active_exec));
                    }
                    state.last_used = Instant::now();
                    PollSessionPlan::Reuse
                }
                None if user_provided_session_id => {
                    return Err(FunctionCallError::RespondToModel(
                        "js_repl session id not found".to_string(),
                    ));
                }
                None => PollSessionPlan::Create,
            }
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
                                active_exec: None,
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

        let req_id = Uuid::new_v4().to_string();
        let (stdin, exec_contexts, child, recent_stderr) = {
            let mut sessions = self.poll_sessions.lock().await;
            let Some(state) = sessions.get_mut(&session_id) else {
                return Err(FunctionCallError::RespondToModel(format!(
                    "js_repl session `{session_id}` is unavailable"
                )));
            };
            if let Some(active_exec) = state.active_exec.as_deref() {
                return Err(session_busy_error(active_exec));
            }
            state.active_exec = Some(req_id.clone());
            state.last_used = Instant::now();
            (
                state.kernel.stdin.clone(),
                Arc::clone(&state.kernel.exec_contexts),
                Arc::clone(&state.kernel.process),
                Arc::clone(&state.kernel.recent_stderr),
            )
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
                    let mut pending = pending_execs.lock().await;
                    if let Some(tx) = pending.remove(&id) {
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
                    drop(pending);
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
            Some(
                mcp_tools
                    .into_iter()
                    .map(|(name, tool)| (name, tool.tool))
                    .collect(),
            ),
            None,
            exec.turn.dynamic_tools.as_slice(),
        );

        let payload =
            if let Some((server, tool)) = exec.session.parse_mcp_tool_name(&req.tool_name).await {
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

fn emitted_image_content_item(
    turn: &TurnContext,
    image_url: String,
    detail: Option<ImageDetail>,
) -> FunctionCallOutputContentItem {
    FunctionCallOutputContentItem::InputImage {
        image_url,
        detail: detail.or_else(|| default_output_image_detail_for_turn(turn)),
    }
}

fn drain_broadcast_lines(buffer: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    loop {
        let Some(pos) = buffer.iter().position(|byte| *byte == b'\n') else {
            break;
        };
        let line = buffer.drain(..=pos).collect::<Vec<_>>();
        lines.push(decode_broadcast_line(&line));
    }
    lines
}

fn finish_broadcast_line(buffer: &mut Vec<u8>) -> Option<String> {
    if buffer.is_empty() {
        None
    } else {
        Some(decode_broadcast_line(&std::mem::take(buffer)))
    }
}

fn decode_broadcast_line(line: &[u8]) -> String {
    let line = String::from_utf8_lossy(line);
    line.trim_end_matches(['\n', '\r']).to_string()
}

fn validate_emitted_image_url(image_url: &str) -> Result<(), String> {
    if image_url
        .get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:"))
    {
        Ok(())
    } else {
        Err("codex.emitImage only accepts data URLs".to_string())
    }
}

fn default_output_image_detail_for_turn(turn: &TurnContext) -> Option<ImageDetail> {
    (turn.config.features.enabled(Feature::ImageDetailOriginal)
        && turn.model_info.supports_image_detail_original)
        .then_some(ImageDetail::Original)
}

fn build_exec_result_content_items(
    output: String,
    content_items: Vec<FunctionCallOutputContentItem>,
) -> Vec<FunctionCallOutputContentItem> {
    let mut all_content_items = Vec::with_capacity(content_items.len() + 1);
    all_content_items.push(FunctionCallOutputContentItem::InputText { text: output });
    all_content_items.extend(content_items);
    all_content_items
}

fn split_exec_result_content_items(
    mut content_items: Vec<FunctionCallOutputContentItem>,
) -> (String, Vec<FunctionCallOutputContentItem>) {
    match content_items.first() {
        Some(FunctionCallOutputContentItem::InputText { .. }) => {
            let FunctionCallOutputContentItem::InputText { text } = content_items.remove(0) else {
                unreachable!("first content item should be input_text");
            };
            (text, content_items)
        }
        Some(FunctionCallOutputContentItem::InputImage { .. }) | None => {
            (String::new(), content_items)
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

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum KernelToHost {
    ExecLog {
        id: String,
        text: String,
    },
    ExecResult {
        id: String,
        ok: bool,
        output: String,
        #[serde(default)]
        error: Option<String>,
    },
    RunTool(RunToolRequest),
    EmitImage(EmitImageRequest),
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HostToKernel {
    Exec {
        id: String,
        code: String,
        #[serde(default)]
        timeout_ms: Option<u64>,
        #[serde(default)]
        stream_logs: bool,
    },
    RunToolResult(RunToolResult),
    EmitImageResult(EmitImageResult),
}

#[derive(Clone, Debug, Deserialize)]
struct RunToolRequest {
    id: String,
    exec_id: String,
    tool_name: String,
    arguments: String,
}

#[derive(Clone, Debug, Serialize)]
struct RunToolResult {
    id: String,
    ok: bool,
    #[serde(default)]
    response: Option<JsonValue>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct EmitImageRequest {
    id: String,
    exec_id: String,
    image_url: String,
    #[serde(default)]
    detail: Option<ImageDetail>,
}

#[derive(Clone, Debug, Serialize)]
struct EmitImageResult {
    id: String,
    ok: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug)]
enum ExecResultMessage {
    Ok {
        content_items: Vec<FunctionCallOutputContentItem>,
    },
    Err {
        message: String,
    },
}

fn clamp_poll_ms(value: Option<u64>) -> u64 {
    value
        .unwrap_or(JS_REPL_POLL_DEFAULT_MS)
        .clamp(JS_REPL_POLL_MIN_MS, JS_REPL_POLL_MAX_MS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NodeVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

impl fmt::Display for NodeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl NodeVersion {
    fn parse(input: &str) -> Result<Self, String> {
        let trimmed = input.trim().trim_start_matches('v');
        let mut parts = trimmed.split(['.', '-', '+']);
        let major = parts
            .next()
            .ok_or_else(|| "missing major version".to_string())?
            .parse::<u64>()
            .map_err(|err| format!("invalid major version: {err}"))?;
        let minor = parts
            .next()
            .ok_or_else(|| "missing minor version".to_string())?
            .parse::<u64>()
            .map_err(|err| format!("invalid minor version: {err}"))?;
        let patch = parts
            .next()
            .ok_or_else(|| "missing patch version".to_string())?
            .parse::<u64>()
            .map_err(|err| format!("invalid patch version: {err}"))?;
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

fn required_node_version() -> Result<NodeVersion, String> {
    NodeVersion::parse(JS_REPL_MIN_NODE_VERSION)
}

async fn read_node_version(node_path: &Path) -> Result<NodeVersion, String> {
    let output = tokio::process::Command::new(node_path)
        .arg("--version")
        .output()
        .await
        .map_err(|err| format!("failed to execute Node: {err}"))?;

    if !output.status.success() {
        let mut details = String::new();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = stdout.trim();
        let stderr = stderr.trim();
        if !stdout.is_empty() {
            details.push_str(" stdout: ");
            details.push_str(stdout);
        }
        if !stderr.is_empty() {
            details.push_str(" stderr: ");
            details.push_str(stderr);
        }
        let details = if details.is_empty() {
            String::new()
        } else {
            format!(" ({details})")
        };
        return Err(format!(
            "failed to read Node version (status {status}){details}",
            status = output.status
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();
    NodeVersion::parse(stdout)
        .map_err(|err| format!("failed to parse Node version output `{stdout}`: {err}"))
}

async fn ensure_node_version(node_path: &Path) -> Result<(), String> {
    let required = required_node_version()?;
    let found = read_node_version(node_path).await?;
    if found < required {
        return Err(format!(
            "Node runtime too old for js_repl (resolved {node_path}): found v{found}, requires >= v{required}. Install/update Node or set js_repl_node_path to a newer runtime.",
            node_path = node_path.display()
        ));
    }
    Ok(())
}

pub(crate) async fn resolve_compatible_node(config_path: Option<&Path>) -> Result<PathBuf, String> {
    let node_path = resolve_node(config_path).ok_or_else(|| {
        "Node runtime not found; install Node or set CODEX_JS_REPL_NODE_PATH".to_string()
    })?;
    ensure_node_version(&node_path).await?;
    Ok(node_path)
}

pub(crate) fn resolve_node(config_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CODEX_JS_REPL_NODE_PATH") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    if let Some(path) = config_path
        && path.exists()
    {
        return Some(path.to_path_buf());
    }

    if let Ok(path) = which::which("node") {
        return Some(path);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use crate::codex::make_session_and_context_with_dynamic_tools_and_rx;
    use crate::codex::make_session_and_context_with_rx;
    use crate::features::Feature;
    use crate::protocol::AskForApproval;
    use crate::protocol::EventMsg;
    use crate::protocol::SandboxPolicy;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::dynamic_tools::DynamicToolCallOutputContentItem;
    use codex_protocol::dynamic_tools::DynamicToolResponse;
    use codex_protocol::dynamic_tools::DynamicToolSpec;
    use codex_protocol::models::FunctionCallOutputContentItem;
    use codex_protocol::models::FunctionCallOutputPayload;
    use codex_protocol::models::ImageDetail;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::openai_models::InputModality;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn node_version_parses_v_prefix_and_suffix() {
        let version = NodeVersion::parse("v25.1.0-nightly.2024").unwrap();
        assert_eq!(
            version,
            NodeVersion {
                major: 25,
                minor: 1,
                patch: 0,
            }
        );
    }

    #[test]
    fn clamp_poll_ms_defaults_to_background_window() {
        assert_eq!(
            clamp_poll_ms(None),
            crate::unified_exec::MIN_EMPTY_YIELD_TIME_MS
        );
        assert_eq!(
            clamp_poll_ms(Some(JS_REPL_POLL_MIN_MS)),
            JS_REPL_POLL_MIN_MS
        );
        assert_eq!(
            clamp_poll_ms(Some(
                crate::unified_exec::DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS * 2
            )),
            crate::unified_exec::DEFAULT_MAX_BACKGROUND_TERMINAL_TIMEOUT_MS
        );
    }

    #[test]
    fn truncate_utf8_prefix_by_bytes_preserves_character_boundaries() {
        let input = "aé🙂z";
        assert_eq!(truncate_utf8_prefix_by_bytes(input, 0), "");
        assert_eq!(truncate_utf8_prefix_by_bytes(input, 1), "a");
        assert_eq!(truncate_utf8_prefix_by_bytes(input, 2), "a");
        assert_eq!(truncate_utf8_prefix_by_bytes(input, 3), "aé");
        assert_eq!(truncate_utf8_prefix_by_bytes(input, 6), "aé");
        assert_eq!(truncate_utf8_prefix_by_bytes(input, 7), "aé🙂");
        assert_eq!(truncate_utf8_prefix_by_bytes(input, 8), "aé🙂z");
    }

    #[test]
    fn split_utf8_chunks_with_limits_respects_boundaries_and_limits() {
        let chunks = split_utf8_chunks_with_limits("éé🙂z", 3, 2);
        assert_eq!(chunks.len(), 2);
        assert_eq!(std::str::from_utf8(&chunks[0]).unwrap(), "é");
        assert_eq!(std::str::from_utf8(&chunks[1]).unwrap(), "é");
    }

    #[tokio::test]
    async fn exec_buffer_output_deltas_honor_remaining_budget() {
        let (session, turn) = make_session_and_context().await;
        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            None,
            Arc::new(session),
            Arc::new(turn),
        );
        entry.emitted_deltas = MAX_EXEC_OUTPUT_DELTAS_PER_CALL - 1;

        let first = entry.output_delta_chunks_for_log_line("hello");
        assert_eq!(first.len(), 1);
        assert_eq!(String::from_utf8(first[0].clone()).unwrap(), "hello\n");

        let second = entry.output_delta_chunks_for_log_line("world");
        assert!(second.is_empty());
    }

    #[test]
    fn stderr_tail_applies_line_and_byte_limits() {
        let mut lines = VecDeque::new();
        let per_line_cap = JS_REPL_STDERR_TAIL_LINE_MAX_BYTES.min(JS_REPL_STDERR_TAIL_MAX_BYTES);
        let long = "x".repeat(per_line_cap + 128);
        let bounded = push_stderr_tail_line(&mut lines, &long);
        assert_eq!(bounded.len(), per_line_cap);

        for i in 0..50 {
            let line = format!("line-{i}-{}", "y".repeat(200));
            push_stderr_tail_line(&mut lines, &line);
        }

        assert!(lines.len() <= JS_REPL_STDERR_TAIL_LINE_LIMIT);
        assert!(lines.iter().all(|line| line.len() <= per_line_cap));
        assert!(stderr_tail_formatted_bytes(&lines) <= JS_REPL_STDERR_TAIL_MAX_BYTES);
        assert_eq!(
            format_stderr_tail(&lines).len(),
            stderr_tail_formatted_bytes(&lines)
        );
    }

    #[test]
    fn model_kernel_failure_details_are_structured_and_truncated() {
        let snapshot = KernelDebugSnapshot {
            pid: Some(42),
            status: "exited(code=1)".to_string(),
            stderr_tail: "s".repeat(JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES + 400),
        };
        let stream_error = "e".repeat(JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES + 200);
        let message = with_model_kernel_failure_message(
            "js_repl kernel exited unexpectedly",
            "stdout_eof",
            Some(&stream_error),
            &snapshot,
        );
        assert!(message.starts_with("js_repl kernel exited unexpectedly\n\njs_repl diagnostics: "));
        let (_prefix, encoded) = message
            .split_once("js_repl diagnostics: ")
            .expect("diagnostics suffix should be present");
        let parsed: serde_json::Value =
            serde_json::from_str(encoded).expect("diagnostics should be valid json");
        assert_eq!(
            parsed.get("reason").and_then(|v| v.as_str()),
            Some("stdout_eof")
        );
        assert_eq!(
            parsed.get("kernel_pid").and_then(serde_json::Value::as_u64),
            Some(42)
        );
        assert_eq!(
            parsed.get("kernel_status").and_then(|v| v.as_str()),
            Some("exited(code=1)")
        );
        assert!(
            parsed
                .get("kernel_stderr_tail")
                .and_then(|v| v.as_str())
                .expect("kernel_stderr_tail should be present")
                .len()
                <= JS_REPL_MODEL_DIAG_STDERR_MAX_BYTES
        );
        assert!(
            parsed
                .get("stream_error")
                .and_then(|v| v.as_str())
                .expect("stream_error should be present")
                .len()
                <= JS_REPL_MODEL_DIAG_ERROR_MAX_BYTES
        );
    }

    #[test]
    fn write_error_diagnostics_only_attach_for_likely_kernel_failures() {
        let running = KernelDebugSnapshot {
            pid: Some(7),
            status: "running".to_string(),
            stderr_tail: "<empty>".to_string(),
        };
        let exited = KernelDebugSnapshot {
            pid: Some(7),
            status: "exited(code=1)".to_string(),
            stderr_tail: "<empty>".to_string(),
        };
        assert!(!should_include_model_diagnostics_for_write_error(
            "failed to flush kernel message: other io error",
            &running
        ));
        assert!(should_include_model_diagnostics_for_write_error(
            "failed to write to kernel: Broken pipe (os error 32)",
            &running
        ));
        assert!(should_include_model_diagnostics_for_write_error(
            "failed to write to kernel: some other io error",
            &exited
        ));
    }

    #[test]
    fn js_repl_internal_tool_guard_matches_expected_names() {
        assert!(is_js_repl_internal_tool("js_repl"));
        assert!(is_js_repl_internal_tool("js_repl_poll"));
        assert!(is_js_repl_internal_tool("js_repl_reset"));
        assert!(!is_js_repl_internal_tool("shell_command"));
        assert!(!is_js_repl_internal_tool("list_mcp_resources"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_exec_tool_calls_map_drains_inflight_calls_without_hanging() {
        let exec_tool_calls = Arc::new(Mutex::new(HashMap::new()));

        for _ in 0..128 {
            let exec_id = Uuid::new_v4().to_string();
            exec_tool_calls
                .lock()
                .await
                .insert(exec_id.clone(), ExecToolCalls::default());
            assert!(
                JsReplManager::begin_exec_tool_call(&exec_tool_calls, &exec_id)
                    .await
                    .is_some()
            );

            let wait_map = Arc::clone(&exec_tool_calls);
            let wait_exec_id = exec_id.clone();
            let waiter = tokio::spawn(async move {
                JsReplManager::wait_for_exec_tool_calls_map(&wait_map, &wait_exec_id).await;
            });

            let finish_map = Arc::clone(&exec_tool_calls);
            let finish_exec_id = exec_id.clone();
            let finisher = tokio::spawn(async move {
                tokio::task::yield_now().await;
                JsReplManager::finish_exec_tool_call(&finish_map, &finish_exec_id).await;
            });

            tokio::time::timeout(Duration::from_secs(1), waiter)
                .await
                .expect("wait_for_exec_tool_calls_map should not hang")
                .expect("wait task should not panic");
            finisher.await.expect("finish task should not panic");

            JsReplManager::clear_exec_tool_calls_map(&exec_tool_calls, &exec_id).await;
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reset_waits_for_exec_lock_before_clearing_exec_tool_calls() {
        let manager = JsReplManager::new(None, Vec::new())
            .await
            .expect("manager should initialize");
        let permit = manager
            .exec_lock
            .clone()
            .acquire_owned()
            .await
            .expect("lock should be acquirable");
        let exec_id = Uuid::new_v4().to_string();
        manager.register_exec_tool_calls(&exec_id).await;

        let reset_manager = Arc::clone(&manager);
        let mut reset_task = tokio::spawn(async move { reset_manager.reset().await });
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(
            !reset_task.is_finished(),
            "reset should wait until execute lock is released"
        );
        assert!(
            manager.exec_tool_calls.lock().await.contains_key(&exec_id),
            "reset must not clear tool-call contexts while execute lock is held"
        );

        drop(permit);

        tokio::time::timeout(Duration::from_secs(1), &mut reset_task)
            .await
            .expect("reset should complete after execute lock release")
            .expect("reset task should not panic")
            .expect("reset should succeed");
        assert!(
            !manager.exec_tool_calls.lock().await.contains_key(&exec_id),
            "reset should clear tool-call contexts after lock acquisition"
        );
    }

    #[test]
    fn summarize_tool_call_response_for_multimodal_function_output() {
        let response = ResponseInputItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,abcd".to_string(),
                    detail: None,
                },
            ]),
        };

        let actual = JsReplManager::summarize_tool_call_response(&response);

        assert_eq!(
            actual,
            JsReplToolCallResponseSummary {
                response_type: Some("function_call_output".to_string()),
                payload_kind: Some(JsReplToolCallPayloadKind::FunctionContentItems),
                payload_text_preview: None,
                payload_text_length: None,
                payload_item_count: Some(1),
                text_item_count: Some(0),
                image_item_count: Some(1),
                structured_content_present: None,
                result_is_error: None,
            }
        );
    }

    #[tokio::test]
    async fn emitted_image_content_item_preserves_explicit_detail() {
        let (_session, turn) = make_session_and_context().await;
        let content_item = emitted_image_content_item(
            &turn,
            "data:image/png;base64,AAA".to_string(),
            Some(ImageDetail::Low),
        );
        assert_eq!(
            content_item,
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: Some(ImageDetail::Low),
            }
        );
    }

    #[tokio::test]
    async fn emitted_image_content_item_uses_turn_original_detail_when_enabled() {
        let (_session, mut turn) = make_session_and_context().await;
        Arc::make_mut(&mut turn.config)
            .features
            .enable(Feature::ImageDetailOriginal)
            .expect("test config should allow feature update");
        turn.model_info.supports_image_detail_original = true;

        let content_item =
            emitted_image_content_item(&turn, "data:image/png;base64,AAA".to_string(), None);

        assert_eq!(
            content_item,
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: Some(ImageDetail::Original),
            }
        );
    }

    #[test]
    fn validate_emitted_image_url_accepts_case_insensitive_data_scheme() {
        assert_eq!(
            validate_emitted_image_url("DATA:image/png;base64,AAA"),
            Ok(())
        );
    }

    #[test]
    fn validate_emitted_image_url_rejects_non_data_scheme() {
        assert_eq!(
            validate_emitted_image_url("https://example.com/image.png"),
            Err("codex.emitImage only accepts data URLs".to_string())
        );
    }

    #[test]
    fn summarize_tool_call_response_for_multimodal_custom_output() {
        let response = ResponseInputItem::CustomToolCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: "data:image/png;base64,abcd".to_string(),
                    detail: None,
                },
            ]),
        };

        let actual = JsReplManager::summarize_tool_call_response(&response);

        assert_eq!(
            actual,
            JsReplToolCallResponseSummary {
                response_type: Some("custom_tool_call_output".to_string()),
                payload_kind: Some(JsReplToolCallPayloadKind::CustomContentItems),
                payload_text_preview: None,
                payload_text_length: None,
                payload_item_count: Some(1),
                text_item_count: Some(0),
                image_item_count: Some(1),
                structured_content_present: None,
                result_is_error: None,
            }
        );
    }

    #[test]
    fn summarize_tool_call_error_marks_error_payload() {
        let actual = JsReplManager::summarize_tool_call_error("tool failed");

        assert_eq!(
            actual,
            JsReplToolCallResponseSummary {
                response_type: None,
                payload_kind: Some(JsReplToolCallPayloadKind::Error),
                payload_text_preview: Some("tool failed".to_string()),
                payload_text_length: Some("tool failed".len()),
                payload_item_count: None,
                text_item_count: None,
                image_item_count: None,
                structured_content_present: None,
                result_is_error: None,
            }
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reset_clears_inflight_exec_tool_calls_without_waiting() {
        let manager = JsReplManager::new(None, Vec::new())
            .await
            .expect("manager should initialize");
        let exec_id = Uuid::new_v4().to_string();
        manager.register_exec_tool_calls(&exec_id).await;
        assert!(
            JsReplManager::begin_exec_tool_call(&manager.exec_tool_calls, &exec_id)
                .await
                .is_some()
        );

        let wait_manager = Arc::clone(&manager);
        let wait_exec_id = exec_id.clone();
        let waiter = tokio::spawn(async move {
            JsReplManager::wait_for_exec_tool_calls_map(
                &wait_manager.exec_tool_calls,
                &wait_exec_id,
            )
            .await;
        });
        tokio::task::yield_now().await;

        tokio::time::timeout(Duration::from_secs(1), manager.reset())
            .await
            .expect("reset should not hang")
            .expect("reset should succeed");

        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("waiter should be released")
            .expect("wait task should not panic");

        assert!(manager.exec_tool_calls.lock().await.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reset_aborts_inflight_exec_tool_tasks() {
        let manager = JsReplManager::new(None, Vec::new())
            .await
            .expect("manager should initialize");
        let exec_id = Uuid::new_v4().to_string();
        manager.register_exec_tool_calls(&exec_id).await;
        let reset_cancel = JsReplManager::begin_exec_tool_call(&manager.exec_tool_calls, &exec_id)
            .await
            .expect("exec should be registered");

        let task = tokio::spawn(async move {
            tokio::select! {
                _ = reset_cancel.cancelled() => "cancelled",
                _ = tokio::time::sleep(Duration::from_secs(60)) => "timed_out",
            }
        });

        tokio::time::timeout(Duration::from_secs(1), manager.reset())
            .await
            .expect("reset should not hang")
            .expect("reset should succeed");

        let outcome = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("cancelled task should resolve promptly")
            .expect("task should not panic");
        assert_eq!(outcome, "cancelled");
    }
    #[tokio::test]
    async fn exec_buffer_caps_all_logs_by_bytes() {
        let (session, turn) = make_session_and_context().await;
        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            None,
            Arc::new(session),
            Arc::new(turn),
        );
        let chunk = "x".repeat(16 * 1024);
        for _ in 0..96 {
            entry.push_log(chunk.clone());
        }
        assert!(entry.all_logs_truncated);
        assert!(entry.all_logs_bytes <= JS_REPL_POLL_ALL_LOGS_MAX_BYTES);
        assert!(
            entry
                .all_logs
                .last()
                .is_some_and(|line| line.contains("logs truncated"))
        );
    }

    #[tokio::test]
    async fn exec_buffer_log_marker_keeps_newest_logs() {
        let (session, turn) = make_session_and_context().await;
        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            None,
            Arc::new(session),
            Arc::new(turn),
        );
        let filler = "x".repeat(8 * 1024);
        for i in 0..20 {
            entry.push_log(format!("id{i}:{filler}"));
        }

        let drained = entry.poll_logs();
        assert_eq!(
            drained.first().map(String::as_str),
            Some(JS_REPL_POLL_LOGS_TRUNCATED_MARKER)
        );
        assert!(drained.iter().any(|line| line.starts_with("id19:")));
        assert!(!drained.iter().any(|line| line.starts_with("id0:")));
    }

    #[tokio::test]
    async fn exec_buffer_poll_final_output_only_returns_terminal_output() {
        let (session, turn) = make_session_and_context().await;
        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            None,
            Arc::new(session),
            Arc::new(turn),
        );
        entry.push_log("line 1".to_string());
        entry.push_log("line 2".to_string());
        entry.done = true;

        assert_eq!(entry.poll_final_output(), None);
    }

    #[tokio::test]
    async fn complete_exec_in_store_suppresses_kernel_exit_when_host_terminating() {
        let (session, turn) = make_session_and_context().await;
        let exec_id = "exec-1";
        let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            None,
            Arc::new(session),
            Arc::new(turn),
        );
        entry.host_terminating = true;
        exec_store.lock().await.insert(exec_id.to_string(), entry);

        let kernel_exit_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::KernelExit,
            None,
            None,
            Some("js_repl kernel exited unexpectedly".to_string()),
        )
        .await;
        assert!(!kernel_exit_completed);

        {
            let store = exec_store.lock().await;
            let entry = store.get(exec_id).expect("exec entry should exist");
            assert!(!entry.done);
            assert!(entry.terminal_kind.is_none());
            assert!(entry.error.is_none());
            assert!(entry.host_terminating);
        }

        let cancelled_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::Cancelled,
            None,
            None,
            Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
        )
        .await;
        assert!(cancelled_completed);

        let store = exec_store.lock().await;
        let entry = store.get(exec_id).expect("exec entry should exist");
        assert!(entry.done);
        assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Cancelled));
        assert_eq!(entry.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
        assert!(!entry.host_terminating);
    }

    #[tokio::test]
    async fn wait_for_exec_terminal_or_protocol_reader_drained_allows_late_terminal_result_to_win()
    {
        let (session, turn) = make_session_and_context().await;
        let exec_id = "exec-1";
        let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let protocol_reader_drained = CancellationToken::new();

        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            Some("session-1".to_string()),
            Arc::new(session),
            Arc::new(turn),
        );
        entry.host_terminating = true;
        exec_store.lock().await.insert(exec_id.to_string(), entry);

        let exec_store_for_task = Arc::clone(&exec_store);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = JsReplManager::complete_exec_in_store(
                &exec_store_for_task,
                exec_id,
                ExecTerminalKind::Success,
                Some("done".to_string()),
                Some(Vec::new()),
                None,
            )
            .await;
        });

        JsReplManager::wait_for_exec_terminal_or_protocol_reader_drained(
            &exec_store,
            exec_id,
            &protocol_reader_drained,
        )
        .await;

        let cancelled_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::Cancelled,
            None,
            None,
            Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
        )
        .await;
        assert!(!cancelled_completed);

        let store = exec_store.lock().await;
        let entry = store.get(exec_id).expect("exec entry should exist");
        assert!(entry.done);
        assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Success));
        assert_eq!(entry.final_output.as_deref(), Some("done"));
        assert_eq!(entry.error, None);
    }

    #[tokio::test]
    async fn wait_for_exec_terminal_or_protocol_reader_drained_ignores_non_terminal_notifications()
    {
        let (session, turn) = make_session_and_context().await;
        let exec_id = "exec-1";
        let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let protocol_reader_drained = CancellationToken::new();

        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            Some("session-1".to_string()),
            Arc::new(session),
            Arc::new(turn),
        );
        entry.host_terminating = true;
        exec_store.lock().await.insert(exec_id.to_string(), entry);

        let exec_store_for_task = Arc::clone(&exec_store);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            {
                let mut store = exec_store_for_task.lock().await;
                let entry = store.get_mut(exec_id).expect("exec entry should exist");
                entry.push_log("still running".to_string());
                entry.notify.notify_waiters();
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
            let _ = JsReplManager::complete_exec_in_store(
                &exec_store_for_task,
                exec_id,
                ExecTerminalKind::Success,
                Some("done".to_string()),
                Some(Vec::new()),
                None,
            )
            .await;
        });

        JsReplManager::wait_for_exec_terminal_or_protocol_reader_drained(
            &exec_store,
            exec_id,
            &protocol_reader_drained,
        )
        .await;

        let cancelled_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::Cancelled,
            None,
            None,
            Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
        )
        .await;
        assert!(!cancelled_completed);

        let store = exec_store.lock().await;
        let entry = store.get(exec_id).expect("exec entry should exist");
        assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Success));
        assert_eq!(entry.final_output.as_deref(), Some("done"));
        assert_eq!(entry.error, None);
    }

    #[tokio::test]
    async fn wait_for_exec_terminal_or_protocol_reader_drained_returns_after_reader_drained() {
        let (session, turn) = make_session_and_context().await;
        let exec_id = "exec-1";
        let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));
        let protocol_reader_drained = CancellationToken::new();

        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            Some("session-1".to_string()),
            Arc::new(session),
            Arc::new(turn),
        );
        entry.host_terminating = true;
        exec_store.lock().await.insert(exec_id.to_string(), entry);

        let protocol_reader_drained_for_task = protocol_reader_drained.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            protocol_reader_drained_for_task.cancel();
        });

        JsReplManager::wait_for_exec_terminal_or_protocol_reader_drained(
            &exec_store,
            exec_id,
            &protocol_reader_drained,
        )
        .await;

        let cancelled_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::Cancelled,
            None,
            None,
            Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
        )
        .await;
        assert!(cancelled_completed);

        let store = exec_store.lock().await;
        let entry = store.get(exec_id).expect("exec entry should exist");
        assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Cancelled));
        assert_eq!(entry.final_output, None);
        assert_eq!(entry.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
    }

    #[tokio::test]
    async fn late_terminal_result_after_forced_cancel_is_ignored() {
        let (session, turn) = make_session_and_context().await;
        let exec_id = "exec-1";
        let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            Some("session-1".to_string()),
            Arc::new(session),
            Arc::new(turn),
        );
        entry.host_terminating = true;
        exec_store.lock().await.insert(exec_id.to_string(), entry);

        let cancelled_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::Cancelled,
            None,
            None,
            Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
        )
        .await;
        assert!(cancelled_completed);

        let success_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::Success,
            Some("done".to_string()),
            Some(Vec::new()),
            None,
        )
        .await;
        assert!(!success_completed);

        let store = exec_store.lock().await;
        let entry = store.get(exec_id).expect("exec entry should exist");
        assert!(entry.done);
        assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Cancelled));
        assert_eq!(entry.final_output, None);
        assert_eq!(entry.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
    }

    #[tokio::test]
    async fn late_terminal_result_after_forced_cancel_keeps_state_and_event_aligned() {
        let (session, turn, rx) = make_session_and_context_with_rx().await;
        let exec_id = "exec-1";
        let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let mut entry = ExecBuffer::new(
            "call-1".to_string(),
            Some("session-1".to_string()),
            Arc::clone(&session),
            Arc::clone(&turn),
        );
        entry.host_terminating = true;
        exec_store.lock().await.insert(exec_id.to_string(), entry);

        let cancelled_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::Cancelled,
            None,
            None,
            Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
        )
        .await;
        assert!(cancelled_completed);

        let first_end = tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let event = rx.recv().await.expect("event");
                if let EventMsg::ExecCommandEnd(end) = event.msg {
                    break end;
                }
            }
        })
        .await
        .expect("timed out waiting for first exec end");
        assert_eq!(first_end.call_id, "call-1");
        assert_eq!(first_end.stderr, JS_REPL_CANCEL_ERROR_MESSAGE);

        let success_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::Success,
            Some("done".to_string()),
            Some(Vec::new()),
            None,
        )
        .await;
        assert!(!success_completed);

        assert!(
            tokio::time::timeout(Duration::from_millis(100), rx.recv())
                .await
                .is_err(),
            "expected no second exec end event after ignored late terminal result"
        );

        let store = exec_store.lock().await;
        let entry = store.get(exec_id).expect("exec entry should exist");
        assert!(entry.done);
        assert_eq!(entry.terminal_kind, Some(ExecTerminalKind::Cancelled));
        assert_eq!(entry.final_output, None);
        assert_eq!(entry.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
    }

    #[test]
    fn build_js_repl_exec_output_sets_timed_out() {
        let out = build_js_repl_exec_output("", Some("timeout"), Duration::from_millis(50), true);
        assert!(out.timed_out);
    }

    async fn can_run_js_repl_runtime_tests() -> bool {
        // These white-box runtime tests are required on macOS. Linux relies on
        // the codex-linux-sandbox arg0 dispatch path, which is exercised in
        // integration tests instead.
        cfg!(target_os = "macos")
    }
    fn write_js_repl_test_package_source(
        base: &Path,
        name: &str,
        source: &str,
    ) -> anyhow::Result<()> {
        let pkg_dir = base.join("node_modules").join(name);
        fs::create_dir_all(&pkg_dir)?;
        fs::write(
            pkg_dir.join("package.json"),
            format!(
                "{{\n  \"name\": \"{name}\",\n  \"version\": \"1.0.0\",\n  \"type\": \"module\",\n  \"exports\": {{\n    \"import\": \"./index.js\"\n  }}\n}}\n"
            ),
        )?;
        fs::write(pkg_dir.join("index.js"), source)?;
        Ok(())
    }

    fn write_js_repl_test_package(base: &Path, name: &str, value: &str) -> anyhow::Result<()> {
        write_js_repl_test_package_source(
            base,
            name,
            &format!("export const value = \"{value}\";\n"),
        )?;
        Ok(())
    }

    fn write_js_repl_test_module(
        base: &Path,
        relative: &str,
        contents: &str,
    ) -> anyhow::Result<()> {
        let module_path = base.join(relative);
        if let Some(parent) = module_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(module_path, contents)?;
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_timeout_does_not_deadlock() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = tokio::time::timeout(
            Duration::from_secs(3),
            manager.execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "while (true) {}".to_string(),
                    timeout_ms: Some(50),
                    poll: false,
                    session_id: None,
                },
            ),
        )
        .await
        .expect("execute should return, not deadlock")
        .expect_err("expected timeout error");

        assert_eq!(result, JsReplExecuteError::TimedOut);
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_timeout_kills_kernel_process() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "console.log('warmup');".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;

        let process = {
            let guard = manager.kernel.lock().await;
            let state = guard.as_ref().expect("kernel should exist after warmup");
            Arc::clone(&state.process)
        };

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "while (true) {}".to_string(),
                    timeout_ms: Some(50),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected timeout error");

        assert_eq!(result, JsReplExecuteError::TimedOut);

        assert!(
            process.has_exited(),
            "timed out js_repl execution should kill previous kernel process"
        );
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_forced_kernel_exit_recovers_on_next_exec() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "console.log('warmup');".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;

        let process = {
            let guard = manager.kernel.lock().await;
            let state = guard.as_ref().expect("kernel should exist after warmup");
            Arc::clone(&state.process)
        };
        JsReplManager::kill_kernel_child(&process, "test_crash").await;
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let cleared = {
                    let guard = manager.kernel.lock().await;
                    guard
                        .as_ref()
                        .is_none_or(|state| !Arc::ptr_eq(&state.process, &process))
                };
                if cleared {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("host should clear dead kernel state promptly");

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "console.log('after-kill');".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("after-kill"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_uncaught_exception_returns_exec_error_and_recovers() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = crate::codex::make_session_and_context().await;
        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "console.log('warmup');".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;

        let process = {
            let guard = manager.kernel.lock().await;
            let state = guard.as_ref().expect("kernel should exist after warmup");
            Arc::clone(&state.process)
        };

        let err = tokio::time::timeout(
            Duration::from_secs(3),
            manager.execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "setTimeout(() => { throw new Error('boom'); }, 0);\nawait new Promise(() => {});".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            ),
        )
        .await
        .expect("uncaught exception should fail promptly")
        .expect_err("expected uncaught exception to fail the exec");

        let message = err.to_string();
        assert!(message.contains("js_repl kernel uncaught exception: boom"));
        assert!(message.contains("kernel reset."));
        assert!(message.contains("Catch or handle async errors"));
        assert!(!message.contains("js_repl kernel exited unexpectedly"));

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if process.has_exited() {
                    return Ok::<(), anyhow::Error>(());
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("uncaught exception should terminate the previous kernel process")?;

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let cleared = {
                    let guard = manager.kernel.lock().await;
                    guard
                        .as_ref()
                        .is_none_or(|state| !Arc::ptr_eq(&state.process, &process))
                };
                if cleared {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("host should clear dead kernel state promptly");

        let next = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "console.log('after reset');".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(next.output.contains("after reset"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_waits_for_unawaited_tool_calls_before_completion() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let marker = turn
            .cwd
            .join(format!("js-repl-unawaited-marker-{}.txt", Uuid::new_v4()));
        let marker_json = serde_json::to_string(&marker.to_string_lossy().to_string())?;
        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: format!(
                        r#"
const marker = {marker_json};
void codex.tool("shell_command", {{ command: `sleep 0.35; printf js_repl_unawaited_done > "${{marker}}"` }});
console.log("cell-complete");
"#
                    ),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("cell-complete"));
        let marker_contents = tokio::fs::read_to_string(&marker).await?;
        assert_eq!(marker_contents, "js_repl_unawaited_done");
        let _ = tokio::fs::remove_file(&marker).await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_does_not_auto_attach_image_via_view_image_tool() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-view-image.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
console.log(out.type);
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("function_call_output"));
        assert!(result.content_items.is_empty());
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_can_emit_image_via_view_image_tool() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-view-image-explicit.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
await codex.emitImage(out);
console.log(out.type);
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("function_call_output"));
        assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: None,
            }]
            .as_slice()
        );
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_multiple_view_image_calls_attach_multiple_images() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
const imagePathA = path.join(codex.tmpDir, "js-repl-view-image-a.png");
const imagePathB = path.join(codex.tmpDir, "js-repl-view-image-b.png");
await fs.writeFile(imagePathA, png);
await fs.writeFile(imagePathB, png);
const outA = await codex.tool("view_image", { path: imagePathA });
const outB = await codex.tool("view_image", { path: imagePathB });
await codex.emitImage(outA);
await codex.emitImage(outB);
console.log("attached-two-images");
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("attached-two-images"));
        assert_eq!(
            result.content_items.len(),
            2,
            "expected one input_image content item per nested view_image call"
        );
        for item in &result.content_items {
            let FunctionCallOutputContentItem::InputImage { image_url, .. } = item else {
                panic!("expected each content item to be an image");
            };
            assert!(image_url.starts_with("data:image/png;base64,"));
        }
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_poll_multiple_view_image_calls_attach_multiple_images() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
const imagePathA = path.join(codex.tmpDir, "js-repl-poll-view-image-a.png");
const imagePathB = path.join(codex.tmpDir, "js-repl-poll-view-image-b.png");
await fs.writeFile(imagePathA, png);
await fs.writeFile(imagePathB, png);
const outA = await codex.tool("view_image", { path: imagePathA });
const outB = await codex.tool("view_image", { path: imagePathB });
await codex.emitImage(outA);
await codex.emitImage(outB);
console.log("attached-two-images");
"#;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-poll-two-view-images".to_string(),
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed_logs = Vec::new();
        let result = loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            observed_logs.extend(result.logs.iter().cloned());
            if result.done {
                assert_eq!(result.session_id, submission.session_id);
                assert_eq!(result.error, None);
                let logs = observed_logs.join("\n");
                assert!(logs.contains("attached-two-images"));
                assert_eq!(result.final_output.as_deref(), Some(""));
                break result;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for polling multi-view_image exec completion");
            }
        };
        assert_eq!(
            result.content_items.len(),
            2,
            "expected one input_image content item per nested view_image call"
        );
        for item in &result.content_items {
            let FunctionCallOutputContentItem::InputImage { image_url, .. } = item else {
                panic!("expected each content item to be an image");
            };
            assert!(image_url.starts_with("data:image/png;base64,"));
        }
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_poll_completed_multimodal_exec_is_replayable() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const imagePath = path.join(codex.tmpDir, "js-repl-poll-replay-image.png");
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await fs.writeFile(imagePath, png);
const out = await codex.tool("view_image", { path: imagePath });
await codex.emitImage(out);
console.log("replay-image-ready");
"#;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-poll-replay-image".to_string(),
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed_logs = Vec::new();
        let first_result = loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            observed_logs.extend(result.logs.iter().cloned());
            if result.done {
                break result;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for polling replay-image exec completion");
            }
        };
        assert_eq!(first_result.session_id, submission.session_id);
        assert_eq!(first_result.error, None);
        assert!(
            observed_logs
                .iter()
                .any(|line| line.contains("replay-image-ready"))
        );
        assert_eq!(first_result.final_output.as_deref(), Some(""));
        assert_eq!(
            first_result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: None,
            }]
            .as_slice()
        );

        let second_result = manager.poll(&submission.exec_id, Some(50)).await?;
        assert!(second_result.done);
        assert_eq!(second_result.session_id, submission.session_id);
        assert_eq!(second_result.error, None);
        assert!(second_result.logs.is_empty());
        assert_eq!(second_result.final_output, first_result.final_output);
        assert_eq!(second_result.content_items, first_result.content_items);
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_can_emit_image_from_bytes_and_mime_type() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png, mimeType: "image/png" });
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: None,
            }]
            .as_slice()
        );
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_can_emit_multiple_images_in_one_cell() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
await codex.emitImage(
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
);
await codex.emitImage(
  "data:image/gif;base64,R0lGODdhAQABAIAAAP///////ywAAAAAAQABAAACAkQBADs="
);
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert_eq!(
            result.content_items.as_slice(),
            [
                FunctionCallOutputContentItem::InputImage {
                    image_url:
                        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                            .to_string(),
                    detail: None,
                },
                FunctionCallOutputContentItem::InputImage {
                    image_url:
                        "data:image/gif;base64,R0lGODdhAQABAIAAAP///////ywAAAAAAQABAAACAkQBADs="
                            .to_string(),
                    detail: None,
                },
            ]
            .as_slice()
        );
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_waits_for_unawaited_emit_image_before_completion() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
void codex.emitImage(
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
);
console.log("cell-complete");
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("cell-complete"));
        assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url:
                    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg=="
                        .to_string(),
                detail: None,
            }]
            .as_slice()
        );
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_unawaited_emit_image_errors_fail_cell() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
void codex.emitImage({ bytes: new Uint8Array(), mimeType: "image/png" });
console.log("cell-complete");
"#;

        let err = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("unawaited invalid emitImage should fail");
        assert!(err.to_string().contains("expected non-empty bytes"));
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_caught_emit_image_error_does_not_fail_cell() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
try {
  await codex.emitImage({ bytes: new Uint8Array(), mimeType: "image/png" });
} catch (error) {
  console.log(error.message);
}
console.log("cell-complete");
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("expected non-empty bytes"));
        assert!(result.output.contains("cell-complete"));
        assert!(result.content_items.is_empty());
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_emit_image_requires_explicit_mime_type_for_bytes() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png });
"#;

        let err = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("missing mimeType should fail");
        assert!(err.to_string().contains("expected a non-empty mimeType"));
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_emit_image_rejects_non_data_url() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
await codex.emitImage("https://example.com/image.png");
"#;

        let err = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("non-data URLs should fail");
        assert!(err.to_string().contains("only accepts data URLs"));
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_emit_image_accepts_case_insensitive_data_url() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
await codex.emitImage("DATA:image/png;base64,AAA");
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url: "DATA:image/png;base64,AAA".to_string(),
                detail: None,
            }]
            .as_slice()
        );
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_emit_image_rejects_invalid_detail() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==",
  "base64"
);
await codex.emitImage({ bytes: png, mimeType: "image/png", detail: "ultra" });
"#;

        let err = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("invalid detail should fail");
        assert!(err.to_string().contains("expected detail to be one of"));
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_emit_image_rejects_mixed_content() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn, rx_event) =
            make_session_and_context_with_dynamic_tools_and_rx(vec![DynamicToolSpec {
                name: "inline_image".to_string(),
                description: "Returns inline text and image content.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            }])
            .await;
        if !turn
            .model_info
            .input_modalities
            .contains(&InputModality::Image)
        {
            return Ok(());
        }

        *session.active_turn.lock().await = Some(crate::state::ActiveTurn::default());

        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let code = r#"
const out = await codex.tool("inline_image", {});
await codex.emitImage(out);
"#;
        let image_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

        let session_for_response = Arc::clone(&session);
        let response_watcher = async move {
            loop {
                let event = tokio::time::timeout(Duration::from_secs(2), rx_event.recv()).await??;
                if let EventMsg::DynamicToolCallRequest(request) = event.msg {
                    session_for_response
                        .notify_dynamic_tool_response(
                            &request.call_id,
                            DynamicToolResponse {
                                content_items: vec![
                                    DynamicToolCallOutputContentItem::InputText {
                                        text: "inline image note".to_string(),
                                    },
                                    DynamicToolCallOutputContentItem::InputImage {
                                        image_url: image_url.to_string(),
                                    },
                                ],
                                success: true,
                            },
                        )
                        .await;
                    return Ok::<(), anyhow::Error>(());
                }
            }
        };

        let (result, response_watcher_result) = tokio::join!(
            manager.execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                    session_id: None,
                },
            ),
            response_watcher,
        );
        response_watcher_result?;
        let err = result.expect_err("mixed content should fail");
        assert!(
            err.to_string()
                .contains("does not accept mixed text and image content")
        );
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }
    #[tokio::test]
    async fn js_repl_prefers_env_node_module_dirs_over_config() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let env_base = tempdir()?;
        write_js_repl_test_package(env_base.path(), "repl_probe", "env")?;

        let config_base = tempdir()?;
        let cwd_dir = tempdir()?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy.r#set.insert(
            "CODEX_JS_REPL_NODE_MODULE_DIRS".to_string(),
            env_base.path().to_string_lossy().to_string(),
        );
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            vec![config_base.path().to_path_buf()],
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                        .to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("env"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_submit_and_complete() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-1".to_string(),
                JsReplArgs {
                    code: "console.log('poll-ok');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;
        assert!(!submission.session_id.is_empty());

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed_logs = Vec::new();
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            assert_eq!(result.session_id, submission.session_id);
            observed_logs.extend(result.logs.iter().cloned());
            if result.done {
                let logs = observed_logs.join("\n");
                assert!(logs.contains("poll-ok"));
                assert_eq!(result.final_output.as_deref(), Some(""));
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for js_repl poll completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_session_reuse_preserves_state() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let first = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                "call-session-first".to_string(),
                JsReplArgs {
                    code: "let persisted = 41;".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;
        loop {
            let result = manager.poll(&first.exec_id, Some(200)).await?;
            if result.done {
                break;
            }
        }

        let second = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-session-second".to_string(),
                JsReplArgs {
                    code: "console.log(persisted + 1);".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: Some(first.session_id.clone()),
                },
            )
            .await?;
        assert_eq!(second.session_id, first.session_id);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed_logs = Vec::new();
        loop {
            let result = manager.poll(&second.exec_id, Some(200)).await?;
            observed_logs.extend(result.logs.iter().cloned());
            if result.done {
                let logs = observed_logs.join("\n");
                assert!(logs.contains("42"));
                assert_eq!(result.final_output.as_deref(), Some(""));
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for reused polling session completion");
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_rejects_submit_with_unknown_session_id() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;
        let err = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-session-missing".to_string(),
                JsReplArgs {
                    code: "console.log('should not run');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: Some("missing-session".to_string()),
                },
            )
            .await
            .expect_err("expected missing session submit rejection");
        assert_eq!(err.to_string(), "js_repl session id not found");

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_rejects_timeout_ms_on_submit() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;
        let err = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-session-timeout-unsupported".to_string(),
                JsReplArgs {
                    code: "console.log('should not run');".to_string(),
                    timeout_ms: Some(5_000),
                    poll: true,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected timeout_ms polling submit rejection");
        assert_eq!(err.to_string(), JS_REPL_POLL_TIMEOUT_ARG_ERROR_MESSAGE);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_poll_concurrent_submit_same_session_rejects_second_exec() -> anyhow::Result<()>
    {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;
        let seed_submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-concurrent-seed".to_string(),
                JsReplArgs {
                    code: "console.log('seed');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;
        loop {
            let result = manager.poll(&seed_submission.exec_id, Some(200)).await?;
            if result.done {
                break;
            }
        }
        let shared_session_id = seed_submission.session_id.clone();

        let manager_a = Arc::clone(&manager);
        let session_a = Arc::clone(&session);
        let turn_a = Arc::clone(&turn);
        let shared_session_id_a = shared_session_id.clone();
        let submit_a = tokio::spawn(async move {
            Arc::clone(&manager_a)
                .submit(
                    Arc::clone(&session_a),
                    Arc::clone(&turn_a),
                    Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                    "call-concurrent-a".to_string(),
                    JsReplArgs {
                        code: "await new Promise((resolve) => setTimeout(resolve, 500));"
                            .to_string(),
                        timeout_ms: None,
                        poll: true,
                        session_id: Some(shared_session_id_a),
                    },
                )
                .await
        });

        let manager_b = Arc::clone(&manager);
        let session_b = Arc::clone(&session);
        let turn_b = Arc::clone(&turn);
        let shared_session_id_b = shared_session_id.clone();
        let submit_b = tokio::spawn(async move {
            Arc::clone(&manager_b)
                .submit(
                    Arc::clone(&session_b),
                    Arc::clone(&turn_b),
                    Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                    "call-concurrent-b".to_string(),
                    JsReplArgs {
                        code: "console.log('blocked');".to_string(),
                        timeout_ms: None,
                        poll: true,
                        session_id: Some(shared_session_id_b),
                    },
                )
                .await
        });

        let (result_a, result_b) = tokio::join!(submit_a, submit_b);
        let result_a = result_a.expect("task A should not panic");
        let result_b = result_b.expect("task B should not panic");
        let mut outcomes = vec![result_a, result_b];

        let first_error_index = outcomes.iter().position(Result::is_err);
        let Some(error_index) = first_error_index else {
            panic!("expected one submit to fail due to active exec in shared session");
        };
        assert_eq!(
            outcomes.iter().filter(|result| result.is_ok()).count(),
            1,
            "exactly one submit should succeed for a shared session id",
        );
        let err = outcomes
            .swap_remove(error_index)
            .expect_err("expected submit failure");
        assert!(
            err.to_string().contains("already has a running exec"),
            "unexpected concurrent-submit error: {err}",
        );
        let submission = outcomes
            .pop()
            .expect("one submission should remain")
            .expect("remaining submission should succeed");
        assert_eq!(submission.session_id, shared_session_id);

        let deadline = Instant::now() + Duration::from_secs(6);
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for shared-session winner completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let _ = manager.reset_session(&shared_session_id).await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_poll_submit_enforces_capacity_during_concurrent_inserts() -> anyhow::Result<()>
    {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;
        let template_kernel = manager
            .start_kernel(Arc::clone(&session), Arc::clone(&turn), None)
            .await
            .map_err(anyhow::Error::msg)?;

        let submit_a;
        let submit_b;
        {
            let mut sessions = manager.poll_sessions.lock().await;
            for idx in 0..(JS_REPL_POLL_MAX_SESSIONS - 1) {
                sessions.insert(
                    format!("prefill-{idx}"),
                    PollSessionState {
                        kernel: template_kernel.clone(),
                        active_exec: Some(format!("busy-{idx}")),
                        last_used: Instant::now(),
                    },
                );
            }

            let manager_a = Arc::clone(&manager);
            let session_a = Arc::clone(&session);
            let turn_a = Arc::clone(&turn);
            submit_a = tokio::spawn(async move {
                Arc::clone(&manager_a)
                    .submit(
                        Arc::clone(&session_a),
                        Arc::clone(&turn_a),
                        Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                        "call-capacity-a".to_string(),
                        JsReplArgs {
                            code: "await new Promise((resolve) => setTimeout(resolve, 300));"
                                .to_string(),
                            timeout_ms: None,
                            poll: true,
                            session_id: None,
                        },
                    )
                    .await
            });

            let manager_b = Arc::clone(&manager);
            let session_b = Arc::clone(&session);
            let turn_b = Arc::clone(&turn);
            submit_b = tokio::spawn(async move {
                Arc::clone(&manager_b)
                    .submit(
                        Arc::clone(&session_b),
                        Arc::clone(&turn_b),
                        Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                        "call-capacity-b".to_string(),
                        JsReplArgs {
                            code: "await new Promise((resolve) => setTimeout(resolve, 300));"
                                .to_string(),
                            timeout_ms: None,
                            poll: true,
                            session_id: None,
                        },
                    )
                    .await
            });

            tokio::task::yield_now().await;
        }

        let (result_a, result_b) = tokio::join!(submit_a, submit_b);
        let result_a = result_a.expect("task A should not panic");
        let result_b = result_b.expect("task B should not panic");
        let outcomes = [result_a, result_b];
        assert_eq!(
            outcomes.iter().filter(|result| result.is_ok()).count(),
            1,
            "exactly one concurrent submit should succeed when one slot remains",
        );
        assert_eq!(
            outcomes.iter().filter(|result| result.is_err()).count(),
            1,
            "exactly one concurrent submit should fail when one slot remains",
        );
        let err = outcomes
            .iter()
            .find_map(|result| result.as_ref().err())
            .expect("one submission should fail");
        assert!(
            err.to_string()
                .contains("has reached the maximum of 16 active sessions"),
            "unexpected capacity error: {err}",
        );
        assert!(
            manager.poll_sessions.lock().await.len() <= JS_REPL_POLL_MAX_SESSIONS,
            "poll session map must never exceed configured capacity",
        );

        manager.reset().await?;
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_rejects_submit_when_session_has_active_exec() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-session-active".to_string(),
                JsReplArgs {
                    code: "await new Promise((resolve) => setTimeout(resolve, 10_000));"
                        .to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let err = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-session-active-conflict".to_string(),
                JsReplArgs {
                    code: "console.log('should not run');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: Some(submission.session_id.clone()),
                },
            )
            .await
            .expect_err("expected active session submit rejection");
        assert_eq!(
            err.to_string(),
            format!(
                "js_repl session `{}` already has a running exec: `{}`",
                submission.session_id, submission.exec_id
            )
        );

        manager.reset_session(&submission.session_id).await?;
        let done = manager.poll(&submission.exec_id, Some(200)).await?;
        assert!(done.done);

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_emits_exec_output_delta_events() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn, rx) = crate::codex::make_session_and_context_with_rx().await;
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-delta-stream".to_string(),
                JsReplArgs {
                    code: "console.log('delta-one'); console.log('delta-two');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_one = false;
        let mut saw_two = false;
        loop {
            if saw_one && saw_two {
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for js_repl output delta events");
            }
            if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await
                && let EventMsg::ExecCommandOutputDelta(delta) = event.msg
                && delta.call_id == "call-delta-stream"
            {
                let text = String::from_utf8_lossy(&delta.chunk);
                if text.contains("delta-one") {
                    saw_one = true;
                }
                if text.contains("delta-two") {
                    saw_two = true;
                }
            }
            let result = manager.poll(&submission.exec_id, Some(50)).await?;
            if result.done && saw_one && saw_two {
                break;
            }
        }

        let completion_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = manager.poll(&submission.exec_id, Some(100)).await?;
            if result.done {
                break;
            }
            if Instant::now() >= completion_deadline {
                panic!("timed out waiting for js_repl poll completion");
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_submit_supports_parallel_execs() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let slow_submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                "call-slow".to_string(),
                JsReplArgs {
                    code: "await new Promise((resolve) => setTimeout(resolve, 2000)); console.log('slow-done');".to_string(),
                    timeout_ms: None,
                    poll: true,
                session_id: None,
                },
            )
            .await?;

        let fast_submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-fast".to_string(),
                JsReplArgs {
                    code: "console.log('fast-done');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;
        assert_ne!(slow_submission.session_id, fast_submission.session_id);

        let fast_start = Instant::now();
        let mut fast_logs = Vec::new();
        let fast_output = loop {
            let result = manager.poll(&fast_submission.exec_id, Some(200)).await?;
            fast_logs.extend(result.logs.iter().cloned());
            if result.done {
                assert_eq!(result.final_output.as_deref(), Some(""));
                break fast_logs.join("\n");
            }
            if fast_start.elapsed() > Duration::from_millis(1_500) {
                panic!("fast polled exec did not complete quickly; submit appears serialized");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        assert!(fast_output.contains("fast-done"));

        let slow_deadline = Instant::now() + Duration::from_secs(8);
        let mut slow_logs = Vec::new();
        loop {
            let result = manager.poll(&slow_submission.exec_id, Some(200)).await?;
            slow_logs.extend(result.logs.iter().cloned());
            if result.done {
                let logs = slow_logs.join("\n");
                assert!(logs.contains("slow-done"));
                assert_eq!(result.final_output.as_deref(), Some(""));
                break;
            }
            if Instant::now() >= slow_deadline {
                panic!("timed out waiting for slow polled exec completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_completed_exec_is_replayable() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-replay".to_string(),
                JsReplArgs {
                    code: "console.log('replay-ok');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed_logs = Vec::new();
        let first_result = loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            observed_logs.extend(result.logs.iter().cloned());
            if result.done {
                break result;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for js_repl poll completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        assert!(observed_logs.iter().any(|line| line.contains("replay-ok")));
        assert_eq!(first_result.final_output.as_deref(), Some(""));
        assert_eq!(first_result.session_id, submission.session_id);

        let second_result = manager.poll(&submission.exec_id, Some(50)).await?;
        assert!(second_result.done);
        assert_eq!(second_result.session_id, submission.session_id);
        assert!(second_result.logs.is_empty());
        assert_eq!(second_result.final_output.as_deref(), Some(""));

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_timeout_resnapshots_state_before_returning() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;

        let exec_id = format!("exec-missed-notify-{}", Uuid::new_v4());
        let poll_session_id = format!("session-missed-notify-{}", Uuid::new_v4());
        manager.exec_store.lock().await.insert(
            exec_id.clone(),
            ExecBuffer::new(
                "call-missed-notify".to_string(),
                Some(poll_session_id.clone()),
                Arc::clone(&session),
                Arc::clone(&turn),
            ),
        );

        let manager_for_poll = Arc::clone(&manager);
        let exec_id_for_poll = exec_id.clone();
        let poll_task =
            tokio::spawn(async move { manager_for_poll.poll(&exec_id_for_poll, Some(80)).await });

        tokio::time::sleep(Duration::from_millis(20)).await;
        {
            let mut store = manager.exec_store.lock().await;
            let entry = store
                .get_mut(&exec_id)
                .expect("exec entry should exist while polling");
            entry.push_log("late log".to_string());
            entry.final_output = Some("late log".to_string());
            entry.done = true;
            // Intentionally skip notify_waiters to emulate a missed wake window.
        }

        let result = poll_task
            .await
            .expect("poll task should not panic")
            .expect("poll should succeed");
        assert!(result.done);
        assert_eq!(result.session_id, poll_session_id);
        assert_eq!(result.logs, vec!["late log".to_string()]);
        assert_eq!(result.final_output.as_deref(), Some("late log"));

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_reset_session_succeeds_for_idle_session() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-reset-idle".to_string(),
                JsReplArgs {
                    code: "console.log('idle');".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for js_repl poll completion");
            }
        }

        let replay_before_reset = manager.poll(&submission.exec_id, Some(50)).await?;
        assert!(replay_before_reset.done);

        manager.reset_session(&submission.session_id).await?;
        let poll_err = manager
            .poll(&submission.exec_id, Some(50))
            .await
            .expect_err("expected completed poll state to be cleared by reset");
        assert_eq!(poll_err.to_string(), "js_repl exec id not found");
        let err = manager
            .reset_session(&submission.session_id)
            .await
            .expect_err("expected missing session id after reset");
        assert_eq!(err.to_string(), "js_repl session id not found");

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_resolves_from_first_config_dir() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let first_base = tempdir()?;
        let second_base = tempdir()?;
        write_js_repl_test_package(first_base.path(), "repl_probe", "first")?;
        write_js_repl_test_package(second_base.path(), "repl_probe", "second")?;

        let cwd_dir = tempdir()?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            vec![
                first_base.path().to_path_buf(),
                second_base.path().to_path_buf(),
            ],
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                        .to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("first"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_falls_back_to_cwd_node_modules() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let config_base = tempdir()?;
        let cwd_dir = tempdir()?;
        write_js_repl_test_package(cwd_dir.path(), "repl_probe", "cwd")?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            vec![config_base.path().to_path_buf()],
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                        .to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("cwd"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_accepts_node_modules_dir_entries() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let base_dir = tempdir()?;
        let cwd_dir = tempdir()?;
        write_js_repl_test_package(base_dir.path(), "repl_probe", "normalized")?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            vec![base_dir.path().join("node_modules")],
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "const mod = await import(\"repl_probe\"); console.log(mod.value);"
                        .to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("normalized"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_supports_relative_file_imports() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let cwd_dir = tempdir()?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "child.js",
            "export const value = \"child\";\n",
        )?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "parent.js",
            "import { value as childValue } from \"./child.js\";\nexport const value = `${childValue}-parent`;\n",
        )?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "local.mjs",
            "export const value = \"mjs\";\n",
        )?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "const parent = await import(\"./parent.js\"); const other = await import(\"./local.mjs\"); console.log(parent.value); console.log(other.value);".to_string(),
                    timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("child-parent"));
        assert!(result.output.contains("mjs"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_supports_absolute_file_imports() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let module_dir = tempdir()?;
        let cwd_dir = tempdir()?;
        write_js_repl_test_module(
            module_dir.path(),
            "absolute.js",
            "export const value = \"absolute\";\n",
        )?;
        let absolute_path_json =
            serde_json::to_string(&module_dir.path().join("absolute.js").display().to_string())?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: format!(
                        "const mod = await import({absolute_path_json}); console.log(mod.value);"
                    ),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("absolute"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_imported_local_files_can_access_repl_globals() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let cwd_dir = tempdir()?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "globals.js",
            "console.log(codex.tmpDir === tmpDir);\nconsole.log(typeof codex.tool);\nconsole.log(\"local-file-console-ok\");\n",
        )?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "await import(\"./globals.js\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("true"));
        assert!(result.output.contains("function"));
        assert!(result.output.contains("local-file-console-ok"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_reimports_local_files_after_edit() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let cwd_dir = tempdir()?;
        let helper_path = cwd_dir.path().join("helper.js");
        fs::write(&helper_path, "export const value = \"v1\";\n")?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let first = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "const { value: firstValue } = await import(\"./helper.js\");\nconsole.log(firstValue);".to_string(),
                    timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
                },
            )
            .await?;
        assert!(first.output.contains("v1"));

        fs::write(&helper_path, "export const value = \"v2\";\n")?;

        let second = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "console.log(firstValue);\nconst { value: secondValue } = await import(\"./helper.js\");\nconsole.log(secondValue);".to_string(),
                    timeout_ms: Some(10_000),
                poll: false,
                session_id: None,
                },
            )
            .await?;
        assert!(second.output.contains("v1"));
        assert!(second.output.contains("v2"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_reimports_local_files_after_fixing_failure() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let cwd_dir = tempdir()?;
        let helper_path = cwd_dir.path().join("broken.js");
        fs::write(&helper_path, "throw new Error(\"boom\");\n")?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let err = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "await import(\"./broken.js\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected broken module import to fail");
        assert!(err.to_string().contains("boom"));

        fs::write(&helper_path, "export const value = \"fixed\";\n")?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "console.log((await import(\"./broken.js\")).value);".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        assert!(result.output.contains("fixed"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_local_files_expose_node_like_import_meta() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let cwd_dir = tempdir()?;
        let pkg_dir = cwd_dir.path().join("node_modules").join("repl_meta_pkg");
        fs::create_dir_all(&pkg_dir)?;
        fs::write(
            pkg_dir.join("package.json"),
            "{\n  \"name\": \"repl_meta_pkg\",\n  \"version\": \"1.0.0\",\n  \"type\": \"module\",\n  \"exports\": {\n    \"import\": \"./index.js\"\n  }\n}\n",
        )?;
        fs::write(
            pkg_dir.join("index.js"),
            "import { sep } from \"node:path\";\nexport const value = `pkg:${typeof sep}`;\n",
        )?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "child.js",
            "export const value = \"child-export\";\n",
        )?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "meta.js",
            "console.log(import.meta.url);\nconsole.log(import.meta.filename);\nconsole.log(import.meta.dirname);\nconsole.log(import.meta.main);\nconsole.log(import.meta.resolve(\"./child.js\"));\nconsole.log(import.meta.resolve(\"repl_meta_pkg\"));\nconsole.log(import.meta.resolve(\"node:fs\"));\nconsole.log((await import(import.meta.resolve(\"./child.js\"))).value);\nconsole.log((await import(import.meta.resolve(\"repl_meta_pkg\"))).value);\n",
        )?;
        let child_path = fs::canonicalize(cwd_dir.path().join("child.js"))?;
        let child_url = url::Url::from_file_path(&child_path)
            .expect("child path should convert to file URL")
            .to_string();

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let result = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "await import(\"./meta.js\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await?;
        let cwd_display = cwd_dir.path().display().to_string();
        let meta_path_display = cwd_dir.path().join("meta.js").display().to_string();
        assert!(result.output.contains("file://"));
        assert!(result.output.contains(&meta_path_display));
        assert!(result.output.contains(&cwd_display));
        assert!(result.output.contains("false"));
        assert!(result.output.contains(&child_url));
        assert!(result.output.contains("repl_meta_pkg"));
        assert!(result.output.contains("node:fs"));
        assert!(result.output.contains("child-export"));
        assert!(result.output.contains("pkg:string"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_rejects_top_level_static_imports_with_clear_error() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let err = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "import \"./local.js\";".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected top-level static import to be rejected");
        assert!(
            err.to_string()
                .contains("Top-level static import \"./local.js\" is not supported in js_repl")
        );
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_local_files_reject_static_bare_imports() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let cwd_dir = tempdir()?;
        write_js_repl_test_package(cwd_dir.path(), "repl_counter", "pkg")?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "entry.js",
            "import { value } from \"repl_counter\";\nconsole.log(value);\n",
        )?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let err = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "await import(\"./entry.js\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected static bare import to be rejected");
        assert!(
            err.to_string().contains(
                "Static import \"repl_counter\" is not supported from js_repl local files"
            )
        );
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_rejects_unsupported_file_specifiers() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let cwd_dir = tempdir()?;
        write_js_repl_test_module(cwd_dir.path(), "local.ts", "export const value = \"ts\";\n")?;
        write_js_repl_test_module(cwd_dir.path(), "local", "export const value = \"noext\";\n")?;
        fs::create_dir_all(cwd_dir.path().join("dir"))?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let unsupported_extension = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "await import(\"./local.ts\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected unsupported extension to be rejected");
        assert!(
            unsupported_extension
                .to_string()
                .contains("Only .js and .mjs files are supported")
        );

        let extensionless = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "await import(\"./local\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected extensionless import to be rejected");
        assert!(
            extensionless
                .to_string()
                .contains("Only .js and .mjs files are supported")
        );

        let directory = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "await import(\"./dir\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected directory import to be rejected");
        assert!(
            directory
                .to_string()
                .contains("Directory imports are not supported")
        );

        let unsupported_url = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "await import(\"https://example.com/test.js\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected unsupported url import to be rejected");
        assert!(
            unsupported_url
                .to_string()
                .contains("Unsupported import specifier")
        );
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_blocks_sensitive_builtin_imports_from_local_files() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let cwd_dir = tempdir()?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "blocked.js",
            "import process from \"node:process\";\nconsole.log(process.pid);\n",
        )?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.path().to_path_buf();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let err = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "await import(\"./blocked.js\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected blocked builtin import to be rejected");
        assert!(
            err.to_string()
                .contains("Importing module \"node:process\" is not allowed in js_repl")
        );
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_local_files_do_not_escape_node_module_search_roots() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let parent_dir = tempdir()?;
        write_js_repl_test_package(parent_dir.path(), "repl_probe", "parent")?;
        let cwd_dir = parent_dir.path().join("workspace");
        fs::create_dir_all(&cwd_dir)?;
        write_js_repl_test_module(
            &cwd_dir,
            "entry.js",
            "const { value } = await import(\"repl_probe\");\nconsole.log(value);\n",
        )?;

        let (session, mut turn) = make_session_and_context().await;
        turn.shell_environment_policy
            .r#set
            .remove("CODEX_JS_REPL_NODE_MODULE_DIRS");
        turn.cwd = cwd_dir.clone();
        turn.js_repl = Arc::new(JsReplHandle::with_node_path(
            turn.config.js_repl_node_path.clone(),
            Vec::new(),
        ));

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let err = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "await import(\"./entry.js\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                    session_id: None,
                },
            )
            .await
            .expect_err("expected parent node_modules lookup to be rejected");
        assert!(err.to_string().contains("repl_probe"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_does_not_auto_timeout_running_execs() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-timeout".to_string(),
                JsReplArgs {
                    code: "await new Promise((resolve) => setTimeout(resolve, 5_000));".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let no_timeout_deadline = Instant::now() + Duration::from_millis(800);
        while Instant::now() < no_timeout_deadline {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            assert!(
                !result.done,
                "polling exec should remain running without reset"
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        manager.reset_session(&submission.session_id).await?;

        let cancel_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
                break;
            }
            if Instant::now() >= cancel_deadline {
                panic!("timed out waiting for reset cancellation");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_reset_session_cancels_inflight_tool_call_promptly() -> anyhow::Result<()>
    {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;

        let started_marker = turn.cwd.join(format!(
            "js-repl-poll-reset-timeout-race-started-{}.txt",
            Uuid::new_v4()
        ));
        let done_marker = turn.cwd.join(format!(
            "js-repl-poll-reset-timeout-race-done-{}.txt",
            Uuid::new_v4()
        ));
        let started_json = serde_json::to_string(&started_marker.to_string_lossy().to_string())?;
        let done_json = serde_json::to_string(&done_marker.to_string_lossy().to_string())?;
        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-reset-timeout-race".to_string(),
                JsReplArgs {
                    code: format!(
                        r#"
const started = {started_json};
const done = {done_json};
await codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 8; printf done > "${{done}}"` }});
console.log("unexpected");
"#
                    ),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let started_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::fs::metadata(&started_marker).await.is_ok() {
                break;
            }
            if Instant::now() >= started_deadline {
                panic!("timed out waiting for in-flight tool call to start");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        tokio::time::timeout(
            Duration::from_secs(2),
            manager.reset_session(&submission.session_id),
        )
        .await
        .expect("reset_session should complete promptly")
        .expect("reset_session should succeed");

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for reset_session cancellation completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let _ = tokio::fs::remove_file(&started_marker).await;
        let _ = tokio::fs::remove_file(&done_marker).await;

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_reset_all_cancels_inflight_tool_call_promptly() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;

        let started_marker = turn.cwd.join(format!(
            "js-repl-poll-reset-all-timeout-race-started-{}.txt",
            Uuid::new_v4()
        ));
        let done_marker = turn.cwd.join(format!(
            "js-repl-poll-reset-all-timeout-race-done-{}.txt",
            Uuid::new_v4()
        ));
        let started_json = serde_json::to_string(&started_marker.to_string_lossy().to_string())?;
        let done_json = serde_json::to_string(&done_marker.to_string_lossy().to_string())?;
        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-reset-all-timeout-race".to_string(),
                JsReplArgs {
                    code: format!(
                        r#"
const started = {started_json};
const done = {done_json};
await codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 8; printf done > "${{done}}"` }});
console.log("unexpected");
"#
                    ),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let started_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if tokio::fs::metadata(&started_marker).await.is_ok() {
                break;
            }
            if Instant::now() >= started_deadline {
                panic!("timed out waiting for in-flight tool call to start");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        tokio::time::timeout(Duration::from_secs(2), manager.reset())
            .await
            .expect("reset should complete promptly")
            .expect("reset should succeed");

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for reset-all cancellation completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let _ = tokio::fs::remove_file(&started_marker).await;
        let _ = tokio::fs::remove_file(&done_marker).await;

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_reset_session_cancels_only_target_session_tool_calls()
    -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;

        let started_a = turn
            .cwd
            .join(format!("js-repl-poll-reset-scope-a-{}.txt", Uuid::new_v4()));
        let started_b = turn
            .cwd
            .join(format!("js-repl-poll-reset-scope-b-{}.txt", Uuid::new_v4()));
        let started_a_json = serde_json::to_string(&started_a.to_string_lossy().to_string())?;
        let started_b_json = serde_json::to_string(&started_b.to_string_lossy().to_string())?;

        let session_a = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-reset-scope-a".to_string(),
                JsReplArgs {
                    code: format!(
                        r#"
const started = {started_a_json};
await codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 8` }});
console.log("session-a-complete");
"#
                    ),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let session_b = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-reset-scope-b".to_string(),
                JsReplArgs {
                    code: format!(
                        r#"
const started = {started_b_json};
await codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 0.4` }});
console.log("session-b-complete");
"#
                    ),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let started_deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_started_a = false;
        let mut saw_started_b = false;
        while !(saw_started_a && saw_started_b) {
            if tokio::fs::metadata(&started_a).await.is_ok() {
                saw_started_a = true;
            }
            if tokio::fs::metadata(&started_b).await.is_ok() {
                saw_started_b = true;
            }
            if Instant::now() >= started_deadline {
                panic!("timed out waiting for both sessions to start tool calls");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        tokio::time::timeout(
            Duration::from_secs(2),
            manager.reset_session(&session_a.session_id),
        )
        .await
        .expect("session-scoped reset should complete promptly")
        .expect("session-scoped reset should succeed");

        let session_a_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = manager.poll(&session_a.exec_id, Some(200)).await?;
            if result.done {
                assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));
                break;
            }
            if Instant::now() >= session_a_deadline {
                panic!("timed out waiting for target session cancellation");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let session_b_deadline = Instant::now() + Duration::from_secs(8);
        let mut session_b_logs = Vec::new();
        loop {
            let result = manager.poll(&session_b.exec_id, Some(200)).await?;
            session_b_logs.extend(result.logs.iter().cloned());
            if result.done {
                assert_eq!(result.error, None);
                assert!(
                    session_b_logs
                        .iter()
                        .any(|line| line.contains("session-b-complete"))
                );
                assert_eq!(result.final_output.as_deref(), Some(""));
                break;
            }
            if Instant::now() >= session_b_deadline {
                panic!("timed out waiting for non-target session completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let _ = tokio::fs::remove_file(&started_a).await;
        let _ = tokio::fs::remove_file(&started_b).await;
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_unawaited_tool_result_preserves_session() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;

        let done_marker = turn.cwd.join(format!(
            "js-repl-poll-unawaited-done-{}.txt",
            Uuid::new_v4()
        ));
        let done_marker_json = serde_json::to_string(&done_marker.to_string_lossy().to_string())?;
        let first = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-unawaited-timeout-race".to_string(),
                JsReplArgs {
                    code: format!(
                        r#"
let persisted = 7;
const done = {done_marker_json};
void codex.tool("shell_command", {{ command: `sleep 0.35; printf done > "${{done}}"` }});
console.log("main-complete");
"#
                    ),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        let first_deadline = Instant::now() + Duration::from_secs(6);
        let mut first_logs = Vec::new();
        loop {
            let result = manager.poll(&first.exec_id, Some(200)).await?;
            first_logs.extend(result.logs.iter().cloned());
            if result.done {
                assert_eq!(result.error, None);
                assert!(
                    first_logs.iter().any(|line| line.contains("main-complete")),
                    "first exec should complete successfully before timeout teardown"
                );
                assert_eq!(result.final_output.as_deref(), Some(""));
                break;
            }
            if Instant::now() >= first_deadline {
                panic!("timed out waiting for first exec completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let marker_deadline = Instant::now() + Duration::from_secs(6);
        loop {
            if tokio::fs::metadata(&done_marker).await.is_ok() {
                break;
            }
            if Instant::now() >= marker_deadline {
                panic!("timed out waiting for unawaited tool call completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let second = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default())),
                "call-unawaited-timeout-race-reuse".to_string(),
                JsReplArgs {
                    code: "console.log(persisted);".to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: Some(first.session_id.clone()),
                },
            )
            .await?;
        assert_eq!(second.session_id, first.session_id);

        let second_deadline = Instant::now() + Duration::from_secs(6);
        let mut second_logs = Vec::new();
        loop {
            let result = manager.poll(&second.exec_id, Some(200)).await?;
            second_logs.extend(result.logs.iter().cloned());
            if result.done {
                assert_eq!(result.error, None);
                assert!(
                    second_logs.iter().any(|line| line.contains("7")),
                    "session should remain reusable after first exec completion"
                );
                assert_eq!(result.final_output.as_deref(), Some(""));
                break;
            }
            if Instant::now() >= second_deadline {
                panic!("timed out waiting for second exec completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let _ = tokio::fs::remove_file(&done_marker).await;
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_reset_session_marks_exec_canceled() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let manager = turn.js_repl.manager().await?;

        for attempt in 0..4 {
            let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
            let submission = Arc::clone(&manager)
                .submit(
                    Arc::clone(&session),
                    Arc::clone(&turn),
                    tracker,
                    format!("call-cancel-{attempt}"),
                    JsReplArgs {
                        code: "await new Promise((resolve) => setTimeout(resolve, 10_000));"
                            .to_string(),
                        timeout_ms: None,
                        poll: true,
                        session_id: None,
                    },
                )
                .await?;

            tokio::time::sleep(Duration::from_millis(100)).await;
            manager.reset_session(&submission.session_id).await?;

            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let result = manager.poll(&submission.exec_id, Some(200)).await?;
                if result.done {
                    let err = result.error.as_deref();
                    assert_eq!(err, Some(JS_REPL_CANCEL_ERROR_MESSAGE));
                    assert!(
                        !err.is_some_and(|message| message.contains("kernel exited unexpectedly"))
                    );
                    break;
                }
                if Instant::now() >= deadline {
                    panic!("timed out waiting for js_repl poll reset completion");
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_reset_session_rejects_unknown_session_id() -> anyhow::Result<()> {
        let (_session, turn) = make_session_and_context().await;
        let manager = turn.js_repl.manager().await?;
        let err = manager
            .reset_session("missing-session")
            .await
            .expect_err("expected missing session id error");
        assert_eq!(err.to_string(), "js_repl session id not found");
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_reset_marks_running_exec_canceled() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy
            .set(AskForApproval::Never)
            .expect("test setup should allow updating approval policy");
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-reset".to_string(),
                JsReplArgs {
                    code: "await new Promise((resolve) => setTimeout(resolve, 10_000));"
                        .to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        tokio::time::sleep(Duration::from_millis(100)).await;
        manager.reset().await?;

        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        assert!(result.done);
        assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_reset_emits_exec_end_for_running_exec() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn, rx) = crate::codex::make_session_and_context_with_rx().await;
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;
        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-reset-end".to_string(),
                JsReplArgs {
                    code: "await new Promise((resolve) => setTimeout(resolve, 10_000));"
                        .to_string(),
                    timeout_ms: None,
                    poll: true,
                    session_id: None,
                },
            )
            .await?;

        tokio::time::sleep(Duration::from_millis(100)).await;
        manager.reset().await?;

        let end = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let event = rx.recv().await.expect("event");
                if let EventMsg::ExecCommandEnd(end) = event.msg
                    && end.call_id == "call-reset-end"
                {
                    break end;
                }
            }
        })
        .await
        .expect("timed out waiting for js_repl reset exec end event");
        assert_eq!(end.stderr, JS_REPL_CANCEL_ERROR_MESSAGE);

        let result = manager.poll(&submission.exec_id, Some(200)).await?;
        assert!(result.done);
        assert_eq!(result.error.as_deref(), Some(JS_REPL_CANCEL_ERROR_MESSAGE));

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_rejects_unknown_exec_id() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (_session, turn) = make_session_and_context().await;
        let manager = turn.js_repl.manager().await?;
        let err = manager
            .poll("missing-exec-id", Some(50))
            .await
            .expect_err("expected missing exec id error");
        assert_eq!(err.to_string(), "js_repl exec id not found");
        Ok(())
    }
}
