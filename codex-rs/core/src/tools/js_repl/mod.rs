use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_protocol::ThreadId;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::OnceCell;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
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
use crate::tools::sandboxing::SandboxablePreference;

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
const JS_REPL_POLL_MIN_MS: u64 = 50;
const JS_REPL_POLL_MAX_MS: u64 = 5_000;
const JS_REPL_POLL_DEFAULT_MS: u64 = 1_000;
const JS_REPL_POLL_ALL_LOGS_MAX_BYTES: usize = crate::unified_exec::UNIFIED_EXEC_OUTPUT_MAX_BYTES;
const JS_REPL_POLL_LOG_QUEUE_MAX_BYTES: usize = 64 * 1024;
const JS_REPL_OUTPUT_DELTA_MAX_BYTES: usize = 8192;
const JS_REPL_POLL_COMPLETED_EXEC_RETENTION: Duration = Duration::from_secs(300);
const JS_REPL_POLL_LOGS_TRUNCATED_MARKER: &str =
    "[js_repl logs truncated; poll more frequently for complete streaming logs]";
const JS_REPL_POLL_ALL_LOGS_TRUNCATED_MARKER: &str =
    "[js_repl logs truncated; output exceeds byte limit]";
pub(crate) const JS_REPL_TIMEOUT_ERROR_MESSAGE: &str =
    "js_repl execution timed out; kernel reset, rerun your request";
const JS_REPL_CANCEL_ERROR_MESSAGE: &str = "js_repl execution canceled";

/// Per-task js_repl handle stored on the turn context.
pub(crate) struct JsReplHandle {
    node_path: Option<PathBuf>,
    codex_home: PathBuf,
    cell: OnceCell<Arc<JsReplManager>>,
}

impl fmt::Debug for JsReplHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JsReplHandle").finish_non_exhaustive()
    }
}

impl JsReplHandle {
    pub(crate) fn with_node_path(node_path: Option<PathBuf>, codex_home: PathBuf) -> Self {
        Self {
            node_path,
            codex_home,
            cell: OnceCell::new(),
        }
    }

    pub(crate) async fn manager(&self) -> Result<Arc<JsReplManager>, FunctionCallError> {
        self.cell
            .get_or_try_init(|| async {
                JsReplManager::new(self.node_path.clone(), self.codex_home.clone()).await
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
}

#[derive(Clone, Debug)]
pub struct JsExecResult {
    pub output: String,
}

#[derive(Clone, Debug)]
pub struct JsExecSubmission {
    pub exec_id: String,
}

#[derive(Clone, Debug)]
pub struct JsExecPollResult {
    pub exec_id: String,
    pub logs: Vec<String>,
    pub output: Option<String>,
    pub error: Option<String>,
    pub done: bool,
}

struct KernelState {
    child: Arc<Mutex<Child>>,
    recent_stderr: Arc<Mutex<VecDeque<String>>>,
    stdin: Arc<Mutex<ChildStdin>>,
    pending_execs: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<ExecResultMessage>>>>,
    exec_contexts: Arc<Mutex<HashMap<String, ExecContext>>>,
    shutdown: CancellationToken,
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
    notify: Arc<Notify>,
    cancel: CancellationToken,
}

struct ExecBuffer {
    event_call_id: String,
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    logs: VecDeque<String>,
    logs_bytes: usize,
    logs_truncated: bool,
    all_logs: Vec<String>,
    all_logs_bytes: usize,
    all_logs_truncated: bool,
    output: Option<String>,
    error: Option<String>,
    done: bool,
    host_terminating: bool,
    terminal_kind: Option<ExecTerminalKind>,
    started_at: Instant,
    notify: Arc<Notify>,
    emitted_deltas: usize,
}

impl ExecBuffer {
    fn new(event_call_id: String, session: Arc<Session>, turn: Arc<TurnContext>) -> Self {
        Self {
            event_call_id,
            session,
            turn,
            logs: VecDeque::new(),
            logs_bytes: 0,
            logs_truncated: false,
            all_logs: Vec::new(),
            all_logs_bytes: 0,
            all_logs_truncated: false,
            output: None,
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
        if let Some(output) = self.output.as_deref()
            && !output.is_empty()
        {
            return output.to_string();
        }
        self.all_logs.join("\n")
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
    Timeout,
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
    StdoutReadError(String),
}

impl KernelStreamEnd {
    fn reason(&self) -> &'static str {
        match self {
            Self::Shutdown => "shutdown",
            Self::StdoutEof => "stdout_eof",
            Self::StdoutReadError(_) => "stdout_read_error",
        }
    }

    fn error(&self) -> Option<&str> {
        match self {
            Self::StdoutReadError(err) => Some(err),
            _ => None,
        }
    }
}

struct KernelDebugSnapshot {
    pid: Option<u32>,
    status: String,
    stderr_tail: String,
}

fn format_exit_status(status: std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("code={code}");
    }
    #[cfg(unix)]
    if let Some(signal) = status.signal() {
        return format!("signal={signal}");
    }
    "unknown".to_string()
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
    js_repl_home: PathBuf,
    vendor_node_modules: PathBuf,
    user_node_modules: PathBuf,
    npm_config_path: PathBuf,
    npm_cache_dir: PathBuf,
    npm_tmp_dir: PathBuf,
    npm_prefix_dir: PathBuf,
    xdg_config_dir: PathBuf,
    xdg_cache_dir: PathBuf,
    xdg_data_dir: PathBuf,
    yarn_cache_dir: PathBuf,
    pnpm_store_dir: PathBuf,
    corepack_home: PathBuf,
    tmp_dir: tempfile::TempDir,
    kernel_script_path: PathBuf,
    kernel: Mutex<Option<KernelState>>,
    exec_lock: Arc<Semaphore>,
    exec_tool_calls: Arc<Mutex<HashMap<String, ExecToolCalls>>>,
    exec_store: Arc<Mutex<HashMap<String, ExecBuffer>>>,
    poll_kernels: Arc<Mutex<HashMap<String, KernelState>>>,
}

impl JsReplManager {
    async fn new(
        node_path: Option<PathBuf>,
        codex_home: PathBuf,
    ) -> Result<Arc<Self>, FunctionCallError> {
        let js_repl_home = codex_home.join("js_repl");
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
        let (
            vendor_node_modules,
            user_node_modules,
            npm_config_path,
            npm_cache_dir,
            npm_tmp_dir,
            npm_prefix_dir,
            xdg_config_dir,
            xdg_cache_dir,
            xdg_data_dir,
            yarn_cache_dir,
            pnpm_store_dir,
            corepack_home,
        ) = prepare_js_repl_home(&js_repl_home).await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to prepare js_repl home: {err}"))
        })?;

        let manager = Arc::new(Self {
            node_path,
            js_repl_home,
            vendor_node_modules,
            user_node_modules,
            npm_config_path,
            npm_cache_dir,
            npm_tmp_dir,
            npm_prefix_dir,
            xdg_config_dir,
            xdg_cache_dir,
            xdg_data_dir,
            yarn_cache_dir,
            pnpm_store_dir,
            corepack_home,
            tmp_dir,
            kernel_script_path,
            kernel: Mutex::new(None),
            exec_lock: Arc::new(Semaphore::new(1)),
            exec_tool_calls: Arc::new(Mutex::new(HashMap::new())),
            exec_store: Arc::new(Mutex::new(HashMap::new())),
            poll_kernels: Arc::new(Mutex::new(HashMap::new())),
        });

        Ok(manager)
    }

    async fn register_exec_tool_calls(&self, exec_id: &str) {
        self.exec_tool_calls
            .lock()
            .await
            .insert(exec_id.to_string(), ExecToolCalls::default());
    }

    async fn clear_exec_tool_calls(&self, exec_id: &str) {
        if let Some(state) = self.exec_tool_calls.lock().await.remove(exec_id) {
            state.cancel.cancel();
            state.notify.notify_waiters();
        }
    }

    async fn wait_for_exec_tool_calls(&self, exec_id: &str) {
        loop {
            let notified = {
                let calls = self.exec_tool_calls.lock().await;
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

    async fn begin_exec_tool_call(
        exec_tool_calls: &Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_id: &str,
    ) -> Option<CancellationToken> {
        let mut calls = exec_tool_calls.lock().await;
        let state = calls.get_mut(exec_id)?;
        state.in_flight += 1;
        Some(state.cancel.clone())
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
        output: Option<String>,
        error: Option<String>,
        override_kernel_exit: bool,
    ) -> bool {
        let event = {
            let mut store = exec_store.lock().await;
            let Some(entry) = store.get_mut(exec_id) else {
                return false;
            };
            if terminal_kind == ExecTerminalKind::KernelExit && entry.host_terminating {
                return false;
            }
            let should_override = override_kernel_exit
                && entry.done
                && matches!(entry.terminal_kind, Some(ExecTerminalKind::KernelExit));
            if entry.done && !should_override {
                return false;
            }

            if !entry.done {
                entry.done = true;
            }
            entry.host_terminating = false;
            if let Some(output) = output {
                entry.output = Some(output);
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
                timed_out: terminal_kind == ExecTerminalKind::Timeout,
            })
        };

        if let Some(event) = event {
            Self::schedule_completed_exec_eviction(Arc::clone(exec_store), exec_id.to_string());
            Self::emit_completion_event(event).await;
            return true;
        }
        false
    }

    async fn complete_exec(
        &self,
        exec_id: &str,
        terminal_kind: ExecTerminalKind,
        output: Option<String>,
        error: Option<String>,
        override_kernel_exit: bool,
    ) -> bool {
        Self::complete_exec_in_store(
            &self.exec_store,
            exec_id,
            terminal_kind,
            output,
            error,
            override_kernel_exit,
        )
        .await
    }

    pub async fn reset(&self) -> Result<(), FunctionCallError> {
        let _permit = self.exec_lock.clone().acquire_owned().await.map_err(|_| {
            FunctionCallError::RespondToModel("js_repl execution unavailable".to_string())
        })?;
        self.reset_kernel().await;
        self.reset_poll_kernels().await;
        Self::clear_all_exec_tool_calls_map(&self.exec_tool_calls).await;
        Ok(())
    }

    async fn reset_kernel(&self) {
        let state = {
            let mut guard = self.kernel.lock().await;
            guard.take()
        };
        if let Some(state) = state {
            state.shutdown.cancel();
            Self::kill_kernel_child(&state.child, "reset").await;
        }
    }

    async fn reset_poll_kernel(&self, exec_id: &str) -> bool {
        let state = self.poll_kernels.lock().await.remove(exec_id);
        if let Some(state) = state {
            state.shutdown.cancel();
            Self::kill_kernel_child(&state.child, "poll_reset").await;
            return true;
        }
        false
    }

    async fn mark_exec_host_terminating(&self, exec_id: &str) {
        let mut store = self.exec_store.lock().await;
        if let Some(entry) = store.get_mut(exec_id)
            && !entry.done
        {
            entry.host_terminating = true;
        }
    }

    async fn reset_poll_kernels(&self) {
        let states = {
            let mut guard = self.poll_kernels.lock().await;
            guard.drain().collect::<Vec<_>>()
        };
        for (exec_id, state) in states {
            self.mark_exec_host_terminating(&exec_id).await;
            state.shutdown.cancel();
            Self::kill_kernel_child(&state.child, "poll_reset_all").await;
            self.wait_for_exec_tool_calls(&exec_id).await;
            self.complete_exec(
                &exec_id,
                ExecTerminalKind::Cancelled,
                None,
                Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
                true,
            )
            .await;
            self.clear_exec_tool_calls(&exec_id).await;
        }
    }

    pub async fn execute(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        tracker: SharedTurnDiffTracker,
        args: JsReplArgs,
    ) -> Result<JsExecResult, FunctionCallError> {
        let _permit = self.exec_lock.clone().acquire_owned().await.map_err(|_| {
            FunctionCallError::RespondToModel("js_repl execution unavailable".to_string())
        })?;

        let (stdin, pending_execs, exec_contexts, child, recent_stderr) = {
            let mut kernel = self.kernel.lock().await;
            if kernel.is_none() {
                let state = self
                    .start_kernel(Arc::clone(&turn), Some(session.conversation_id))
                    .await
                    .map_err(FunctionCallError::RespondToModel)?;
                *kernel = Some(state);
            }

            let state = match kernel.as_ref() {
                Some(state) => state,
                None => {
                    return Err(FunctionCallError::RespondToModel(
                        "js_repl kernel unavailable".to_string(),
                    ));
                }
            };
            (
                Arc::clone(&state.stdin),
                Arc::clone(&state.pending_execs),
                Arc::clone(&state.exec_contexts),
                Arc::clone(&state.child),
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
            self.clear_exec_tool_calls(&req_id).await;
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
            return Err(FunctionCallError::RespondToModel(message));
        }

        let timeout_ms = args.timeout_ms.unwrap_or(30_000);
        let response = match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(_)) => {
                let mut pending = pending_execs.lock().await;
                pending.remove(&req_id);
                exec_contexts.lock().await.remove(&req_id);
                self.wait_for_exec_tool_calls(&req_id).await;
                self.clear_exec_tool_calls(&req_id).await;
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
                return Err(FunctionCallError::RespondToModel(message));
            }
            Err(_) => {
                pending_execs.lock().await.remove(&req_id);
                exec_contexts.lock().await.remove(&req_id);
                self.reset_kernel().await;
                self.wait_for_exec_tool_calls(&req_id).await;
                self.clear_exec_tool_calls(&req_id).await;
                return Err(FunctionCallError::RespondToModel(
                    JS_REPL_TIMEOUT_ERROR_MESSAGE.to_string(),
                ));
            }
        };

        match response {
            ExecResultMessage::Ok { output } => Ok(JsExecResult { output }),
            ExecResultMessage::Err { message } => Err(FunctionCallError::RespondToModel(message)),
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
        let state = self
            .start_kernel(Arc::clone(&turn), Some(session.conversation_id))
            .await
            .map_err(FunctionCallError::RespondToModel)?;
        let exec_contexts = Arc::clone(&state.exec_contexts);
        let stdin = Arc::clone(&state.stdin);
        let child = Arc::clone(&state.child);
        let recent_stderr = Arc::clone(&state.recent_stderr);
        let shutdown = state.shutdown.clone();

        let req_id = Uuid::new_v4().to_string();
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
            ExecBuffer::new(event_call_id, Arc::clone(&session), Arc::clone(&turn)),
        );
        self.register_exec_tool_calls(&req_id).await;

        self.poll_kernels.lock().await.insert(req_id.clone(), state);

        let payload = HostToKernel::Exec {
            id: req_id.clone(),
            code: args.code,
            timeout_ms: args.timeout_ms,
            stream_logs: true,
        };
        if let Err(err) = Self::write_message(&stdin, &payload).await {
            self.exec_store.lock().await.remove(&req_id);
            exec_contexts.lock().await.remove(&req_id);
            self.poll_kernels.lock().await.remove(&req_id);
            self.clear_exec_tool_calls(&req_id).await;
            shutdown.cancel();
            Self::kill_kernel_child(&child, "poll_submit_write_failed").await;
            let snapshot = Self::kernel_debug_snapshot(&child, &recent_stderr).await;
            let err_message = err.to_string();
            warn!(
                exec_id = %req_id,
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

        let timeout_ms = args.timeout_ms.unwrap_or(30_000);
        let manager = Arc::clone(&self);
        let timeout_exec_id = req_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
            manager.mark_exec_host_terminating(&timeout_exec_id).await;
            let had_kernel = manager.reset_poll_kernel(&timeout_exec_id).await;
            manager.wait_for_exec_tool_calls(&timeout_exec_id).await;
            manager
                .complete_exec(
                    &timeout_exec_id,
                    ExecTerminalKind::Timeout,
                    None,
                    Some(JS_REPL_TIMEOUT_ERROR_MESSAGE.to_string()),
                    had_kernel,
                )
                .await;
            manager.clear_exec_tool_calls(&timeout_exec_id).await;
        });

        Ok(JsExecSubmission { exec_id: req_id })
    }

    pub async fn poll(
        &self,
        exec_id: &str,
        yield_time_ms: Option<u64>,
    ) -> Result<JsExecPollResult, FunctionCallError> {
        let deadline = Instant::now() + Duration::from_millis(clamp_poll_ms(yield_time_ms));

        loop {
            let (notify, done, logs, output, error) = {
                let mut store = self.exec_store.lock().await;
                let Some(entry) = store.get_mut(exec_id) else {
                    return Err(FunctionCallError::RespondToModel(
                        "js_repl exec id not found".to_string(),
                    ));
                };
                if !entry.logs.is_empty() || entry.done {
                    let drained_logs = entry.poll_logs();
                    let output = entry.output.clone();
                    let error = entry.error.clone();
                    let done = entry.done;
                    return Ok(JsExecPollResult {
                        exec_id: exec_id.to_string(),
                        logs: drained_logs,
                        output,
                        error,
                        done,
                    });
                }
                (
                    Arc::clone(&entry.notify),
                    entry.done,
                    Vec::new(),
                    None,
                    None,
                )
            };

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(JsExecPollResult {
                    exec_id: exec_id.to_string(),
                    logs,
                    output,
                    error,
                    done,
                });
            }

            if tokio::time::timeout(remaining, notify.notified())
                .await
                .is_err()
            {
                return Ok(JsExecPollResult {
                    exec_id: exec_id.to_string(),
                    logs,
                    output,
                    error,
                    done,
                });
            }
        }
    }

    pub async fn cancel(&self, exec_id: &str) -> Result<(), FunctionCallError> {
        {
            let store = self.exec_store.lock().await;
            let Some(entry) = store.get(exec_id) else {
                return Err(FunctionCallError::RespondToModel(
                    "js_repl exec id not found".to_string(),
                ));
            };
            if entry.done {
                return Err(FunctionCallError::RespondToModel(
                    "js_repl exec already completed".to_string(),
                ));
            }
        }

        self.mark_exec_host_terminating(exec_id).await;
        let had_kernel = self.reset_poll_kernel(exec_id).await;
        self.wait_for_exec_tool_calls(exec_id).await;
        let completed = self
            .complete_exec(
                exec_id,
                ExecTerminalKind::Cancelled,
                None,
                Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
                had_kernel,
            )
            .await;
        self.clear_exec_tool_calls(exec_id).await;
        if completed {
            Ok(())
        } else {
            Err(FunctionCallError::RespondToModel(
                "js_repl exec already completed".to_string(),
            ))
        }
    }
    async fn start_kernel(
        &self,
        turn: Arc<TurnContext>,
        thread_id: Option<ThreadId>,
    ) -> Result<KernelState, String> {
        let node_path = resolve_node(self.node_path.as_deref()).ok_or_else(|| {
            "Node runtime not found; install Node or set CODEX_JS_REPL_NODE_PATH".to_string()
        })?;
        ensure_node_version(&node_path).await?;

        let kernel_path = self.kernel_script_path.clone();

        let mut env = create_env(&turn.shell_environment_policy, thread_id);
        self.configure_js_repl_env(&mut env);

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
            justification: None,
        };

        let sandbox = SandboxManager::new();
        let has_managed_network_requirements = turn
            .config
            .config_layer_stack
            .requirements_toml()
            .network
            .is_some();
        let sandbox_type = sandbox.select_initial(
            &turn.sandbox_policy,
            SandboxablePreference::Auto,
            turn.windows_sandbox_level,
            has_managed_network_requirements,
        );
        let exec_env = sandbox
            .transform(crate::sandboxing::SandboxTransformRequest {
                spec,
                policy: &turn.sandbox_policy,
                sandbox: sandbox_type,
                enforce_managed_network: has_managed_network_requirements,
                network: None,
                sandbox_policy_cwd: &turn.cwd,
                codex_linux_sandbox_exe: turn.codex_linux_sandbox_exe.as_ref(),
                use_linux_sandbox_bwrap: turn
                    .features
                    .enabled(crate::features::Feature::UseLinuxSandboxBwrap),
                windows_sandbox_level: turn.windows_sandbox_level,
            })
            .map_err(|err| format!("failed to configure sandbox for js_repl: {err}"))?;

        let mut cmd =
            tokio::process::Command::new(exec_env.command.first().cloned().unwrap_or_default());
        if exec_env.command.len() > 1 {
            cmd.args(&exec_env.command[1..]);
        }
        #[cfg(unix)]
        cmd.arg0(
            exec_env
                .arg0
                .clone()
                .unwrap_or_else(|| exec_env.command.first().cloned().unwrap_or_default()),
        );
        cmd.current_dir(&exec_env.cwd);
        cmd.env_clear();
        cmd.envs(exec_env.env);
        cmd.stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|err| format!("failed to start Node runtime: {err}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "js_repl kernel missing stdout".to_string())?;
        let stderr = child.stderr.take();
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "js_repl kernel missing stdin".to_string())?;

        let shutdown = CancellationToken::new();
        let pending_execs: Arc<
            Mutex<HashMap<String, tokio::sync::oneshot::Sender<ExecResultMessage>>>,
        > = Arc::new(Mutex::new(HashMap::new()));
        let exec_contexts: Arc<Mutex<HashMap<String, ExecContext>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let stdin_arc = Arc::new(Mutex::new(stdin));
        let child = Arc::new(Mutex::new(child));
        let recent_stderr = Arc::new(Mutex::new(VecDeque::with_capacity(
            JS_REPL_STDERR_TAIL_LINE_LIMIT,
        )));

        tokio::spawn(Self::read_stdout(
            stdout,
            Arc::clone(&child),
            Arc::clone(&recent_stderr),
            Arc::clone(&pending_execs),
            Arc::clone(&exec_contexts),
            Arc::clone(&self.exec_tool_calls),
            Arc::clone(&self.exec_store),
            Arc::clone(&self.poll_kernels),
            Arc::clone(&stdin_arc),
            shutdown.clone(),
        ));
        if let Some(stderr) = stderr {
            tokio::spawn(Self::read_stderr(
                stderr,
                Arc::clone(&recent_stderr),
                shutdown.clone(),
            ));
        } else {
            warn!("js_repl kernel missing stderr");
        }

        Ok(KernelState {
            child,
            recent_stderr,
            stdin: stdin_arc,
            pending_execs,
            exec_contexts,
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
        stdin: &Arc<Mutex<ChildStdin>>,
        msg: &HostToKernel,
    ) -> Result<(), FunctionCallError> {
        let encoded = serde_json::to_string(msg).map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to serialize kernel message: {err}"))
        })?;
        let mut guard = stdin.lock().await;
        guard.write_all(encoded.as_bytes()).await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to write to kernel: {err}"))
        })?;
        guard.write_all(b"\n").await.map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to flush kernel message: {err}"))
        })?;
        Ok(())
    }

    async fn kernel_stderr_tail_snapshot(recent_stderr: &Arc<Mutex<VecDeque<String>>>) -> String {
        let tail = recent_stderr.lock().await;
        format_stderr_tail(&tail)
    }

    async fn kernel_debug_snapshot(
        child: &Arc<Mutex<Child>>,
        recent_stderr: &Arc<Mutex<VecDeque<String>>>,
    ) -> KernelDebugSnapshot {
        let (pid, status) = {
            let mut guard = child.lock().await;
            let pid = guard.id();
            let status = match guard.try_wait() {
                Ok(Some(status)) => format!("exited({})", format_exit_status(status)),
                Ok(None) => "running".to_string(),
                Err(err) => format!("unknown ({err})"),
            };
            (pid, status)
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

    async fn kill_kernel_child(child: &Arc<Mutex<Child>>, reason: &'static str) {
        let mut guard = child.lock().await;
        let pid = guard.id();
        match guard.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(err) => {
                warn!(
                    kernel_pid = ?pid,
                    kill_reason = reason,
                    error = %err,
                    "failed to inspect js_repl kernel before kill"
                );
            }
        }

        if let Err(err) = guard.start_kill() {
            warn!(
                kernel_pid = ?pid,
                kill_reason = reason,
                error = %err,
                "failed to send kill signal to js_repl kernel"
            );
            return;
        }

        match tokio::time::timeout(Duration::from_secs(2), guard.wait()).await {
            Ok(Ok(_status)) => {}
            Ok(Err(err)) => {
                warn!(
                    kernel_pid = ?pid,
                    kill_reason = reason,
                    error = %err,
                    "failed while waiting for js_repl kernel exit"
                );
            }
            Err(_) => {
                warn!(
                    kernel_pid = ?pid,
                    kill_reason = reason,
                    "timed out waiting for js_repl kernel to exit after kill"
                );
            }
        }
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
        stdout: tokio::process::ChildStdout,
        child: Arc<Mutex<Child>>,
        recent_stderr: Arc<Mutex<VecDeque<String>>>,
        pending_execs: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<ExecResultMessage>>>>,
        exec_contexts: Arc<Mutex<HashMap<String, ExecContext>>>,
        exec_tool_calls: Arc<Mutex<HashMap<String, ExecToolCalls>>>,
        exec_store: Arc<Mutex<HashMap<String, ExecBuffer>>>,
        poll_kernels: Arc<Mutex<HashMap<String, KernelState>>>,
        stdin: Arc<Mutex<ChildStdin>>,
        shutdown: CancellationToken,
    ) {
        let mut reader = BufReader::new(stdout).lines();
        let end_reason = loop {
            let line = tokio::select! {
                _ = shutdown.cancelled() => break KernelStreamEnd::Shutdown,
                res = reader.next_line() => match res {
                    Ok(Some(line)) => line,
                    Ok(None) => break KernelStreamEnd::StdoutEof,
                    Err(err) => break KernelStreamEnd::StdoutReadError(err.to_string()),
                },
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
                    JsReplManager::wait_for_exec_tool_calls_map(&exec_tool_calls, &id).await;
                    let mut pending = pending_execs.lock().await;
                    if let Some(tx) = pending.remove(&id) {
                        let payload = if ok {
                            ExecResultMessage::Ok {
                                output: output.clone(),
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
                        completion_error,
                        false,
                    )
                    .await;
                    exec_contexts.lock().await.remove(&id);
                    JsReplManager::clear_exec_tool_calls_map(&exec_tool_calls, &id).await;
                    let state = poll_kernels.lock().await.remove(&id);
                    if let Some(state) = state {
                        state.shutdown.cancel();
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
                                JsReplManager::kernel_debug_snapshot(&child, &recent_stderr).await;
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
                    let stdin_clone = Arc::clone(&stdin);
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
                        JsReplManager::finish_exec_tool_call(&exec_tool_calls_for_task, &exec_id)
                            .await;
                        let payload = HostToKernel::RunToolResult(result);
                        if let Err(err) = JsReplManager::write_message(&stdin_clone, &payload).await
                        {
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

        let exec_ids = {
            let mut contexts = exec_contexts.lock().await;
            let ids = contexts.keys().cloned().collect::<Vec<_>>();
            contexts.clear();
            ids
        };
        for exec_id in exec_ids {
            JsReplManager::wait_for_exec_tool_calls_map(&exec_tool_calls, &exec_id).await;
            JsReplManager::clear_exec_tool_calls_map(&exec_tool_calls, &exec_id).await;
        }
        let unexpected_snapshot = if matches!(end_reason, KernelStreamEnd::Shutdown) {
            None
        } else {
            Some(Self::kernel_debug_snapshot(&child, &recent_stderr).await)
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

        let mut pending = pending_execs.lock().await;
        let pending_exec_ids = pending.keys().cloned().collect::<Vec<_>>();
        for (_id, tx) in pending.drain() {
            let _ = tx.send(ExecResultMessage::Err {
                message: kernel_exit_message.clone(),
            });
        }
        drop(pending);
        let exec_ids_from_contexts = {
            let mut contexts = exec_contexts.lock().await;
            let ids: Vec<String> = contexts.keys().cloned().collect();
            contexts.clear();
            ids
        };
        let mut affected_exec_ids: HashSet<String> = exec_ids_from_contexts.into_iter().collect();
        {
            let kernels = poll_kernels.lock().await;
            affected_exec_ids.extend(
                kernels
                    .iter()
                    .filter(|(_, state)| Arc::ptr_eq(&state.stdin, &stdin))
                    .map(|(exec_id, _)| exec_id.clone()),
            );
        }
        for exec_id in &affected_exec_ids {
            Self::complete_exec_in_store(
                &exec_store,
                exec_id,
                ExecTerminalKind::KernelExit,
                None,
                Some(kernel_exit_message.clone()),
                false,
            )
            .await;
        }
        let mut kernels = poll_kernels.lock().await;
        let mut affected_exec_ids = affected_exec_ids.into_iter().collect::<Vec<_>>();
        affected_exec_ids.sort_unstable();
        for exec_id in &affected_exec_ids {
            kernels.remove(exec_id);
        }
        drop(kernels);

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
    }

    async fn run_tool_request(exec: ExecContext, req: RunToolRequest) -> RunToolResult {
        if is_js_repl_internal_tool(&req.tool_name) {
            return RunToolResult {
                id: req.id,
                ok: false,
                response: None,
                error: Some("js_repl cannot invoke itself".to_string()),
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

        let call = crate::tools::router::ToolCall {
            tool_name: req.tool_name,
            call_id: req.id.clone(),
            payload,
        };

        match router
            .dispatch_tool_call(
                exec.session,
                exec.turn,
                exec.tracker,
                call,
                crate::tools::router::ToolCallSource::JsRepl,
            )
            .await
        {
            Ok(response) => match serde_json::to_value(response) {
                Ok(value) => RunToolResult {
                    id: req.id,
                    ok: true,
                    response: Some(value),
                    error: None,
                },
                Err(err) => RunToolResult {
                    id: req.id,
                    ok: false,
                    response: None,
                    error: Some(format!("failed to serialize tool output: {err}")),
                },
            },
            Err(err) => RunToolResult {
                id: req.id,
                ok: false,
                response: None,
                error: Some(err.to_string()),
            },
        }
    }

    fn configure_js_repl_env(&self, env: &mut HashMap<String, String>) {
        scrub_js_repl_env(env);

        env.insert(
            "CODEX_JS_TMP_DIR".to_string(),
            self.tmp_dir.path().to_string_lossy().to_string(),
        );
        env.insert(
            "CODEX_JS_REPL_HOME".to_string(),
            self.js_repl_home.to_string_lossy().to_string(),
        );
        env.insert(
            "CODEX_JS_REPL_VENDOR_NODE_MODULES".to_string(),
            self.vendor_node_modules.to_string_lossy().to_string(),
        );
        env.insert(
            "CODEX_JS_REPL_USER_NODE_MODULES".to_string(),
            self.user_node_modules.to_string_lossy().to_string(),
        );

        if let Ok(node_path) = std::env::join_paths([
            self.vendor_node_modules.as_path(),
            self.user_node_modules.as_path(),
        ]) {
            env.insert(
                "NODE_PATH".to_string(),
                node_path.to_string_lossy().to_string(),
            );
        }
        env.insert(
            "NODE_REPL_HISTORY".to_string(),
            self.js_repl_home
                .join("node_repl_history")
                .to_string_lossy()
                .to_string(),
        );

        env.insert(
            "HOME".to_string(),
            self.js_repl_home.to_string_lossy().to_string(),
        );
        if cfg!(windows) {
            env.insert(
                "USERPROFILE".to_string(),
                self.js_repl_home.to_string_lossy().to_string(),
            );
            env.insert(
                "APPDATA".to_string(),
                self.js_repl_home
                    .join("appdata")
                    .to_string_lossy()
                    .to_string(),
            );
            env.insert(
                "LOCALAPPDATA".to_string(),
                self.js_repl_home
                    .join("localappdata")
                    .to_string_lossy()
                    .to_string(),
            );
        }

        env.insert(
            "XDG_CONFIG_HOME".to_string(),
            self.xdg_config_dir.to_string_lossy().to_string(),
        );
        env.insert(
            "XDG_CACHE_HOME".to_string(),
            self.xdg_cache_dir.to_string_lossy().to_string(),
        );
        env.insert(
            "XDG_DATA_HOME".to_string(),
            self.xdg_data_dir.to_string_lossy().to_string(),
        );

        let npm_config_path = self.npm_config_path.to_string_lossy().to_string();
        set_env_with_upper(env, "npm_config_userconfig", &npm_config_path);
        set_env_with_upper(env, "npm_config_globalconfig", &npm_config_path);
        set_env_with_upper(
            env,
            "npm_config_cache",
            self.npm_cache_dir.to_string_lossy().as_ref(),
        );
        set_env_with_upper(
            env,
            "npm_config_tmp",
            self.npm_tmp_dir.to_string_lossy().as_ref(),
        );
        set_env_with_upper(
            env,
            "npm_config_prefix",
            self.npm_prefix_dir.to_string_lossy().as_ref(),
        );
        set_env_with_upper(env, "npm_config_update_notifier", "false");
        set_env_with_upper(env, "npm_config_fund", "false");
        set_env_with_upper(env, "npm_config_audit", "false");

        env.insert(
            "YARN_CACHE_FOLDER".to_string(),
            self.yarn_cache_dir.to_string_lossy().to_string(),
        );
        env.insert(
            "YARN_RC_FILENAME".to_string(),
            self.js_repl_home
                .join(".codex-yarnrc")
                .to_string_lossy()
                .to_string(),
        );

        env.insert(
            "PNPM_STORE_PATH".to_string(),
            self.pnpm_store_dir.to_string_lossy().to_string(),
        );
        env.insert(
            "COREPACK_HOME".to_string(),
            self.corepack_home.to_string_lossy().to_string(),
        );
    }

    async fn read_stderr(
        stderr: tokio::process::ChildStderr,
        recent_stderr: Arc<Mutex<VecDeque<String>>>,
        shutdown: CancellationToken,
    ) {
        let mut reader = BufReader::new(stderr).lines();

        loop {
            let line = tokio::select! {
                _ = shutdown.cancelled() => break,
                res = reader.next_line() => match res {
                    Ok(Some(line)) => line,
                    Ok(None) => break,
                    Err(err) => {
                        warn!("js_repl kernel stderr ended: {err}");
                        break;
                    }
                },
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
    matches!(
        name,
        "js_repl" | "js_repl_poll" | "js_repl_cancel" | "js_repl_reset"
    )
}

fn scrub_js_repl_env(env: &mut HashMap<String, String>) {
    let prefixes = ["NODE_", "NPM_CONFIG_", "YARN_", "PNPM_", "COREPACK_"];
    let keys: Vec<String> = env.keys().cloned().collect();
    for key in keys {
        let upper = key.to_ascii_uppercase();
        if prefixes.iter().any(|prefix| upper.starts_with(prefix)) {
            env.remove(&key);
        }
    }
}

fn set_env_with_upper(env: &mut HashMap<String, String>, key: &str, value: &str) {
    env.insert(key.to_string(), value.to_string());
    let upper = key.to_ascii_uppercase();
    if upper != key {
        env.insert(upper, value.to_string());
    }
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

#[derive(Debug)]
enum ExecResultMessage {
    Ok { output: String },
    Err { message: String },
}

fn clamp_poll_ms(value: Option<u64>) -> u64 {
    value
        .unwrap_or(JS_REPL_POLL_DEFAULT_MS)
        .clamp(JS_REPL_POLL_MIN_MS, JS_REPL_POLL_MAX_MS)
}

async fn prepare_js_repl_home(
    js_repl_home: &Path,
) -> Result<
    (
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
    ),
    std::io::Error,
> {
    let vendor_root = js_repl_home.join("codex_node_modules");
    let vendor_node_modules = vendor_root.join("node_modules");
    let user_node_modules = js_repl_home.join("node_modules");
    let npm_config_path = js_repl_home.join("npmrc");
    let npm_cache_dir = js_repl_home.join("npm-cache");
    let npm_tmp_dir = js_repl_home.join("npm-tmp");
    let npm_prefix_dir = js_repl_home.join("npm-prefix");
    let xdg_config_dir = js_repl_home.join("xdg-config");
    let xdg_cache_dir = js_repl_home.join("xdg-cache");
    let xdg_data_dir = js_repl_home.join("xdg-data");
    let yarn_cache_dir = js_repl_home.join("yarn-cache");
    let pnpm_store_dir = js_repl_home.join("pnpm-store");
    let corepack_home = js_repl_home.join("corepack");

    for dir in [
        js_repl_home,
        &vendor_root,
        &vendor_node_modules,
        &user_node_modules,
        &npm_cache_dir,
        &npm_tmp_dir,
        &npm_prefix_dir,
        &xdg_config_dir,
        &xdg_cache_dir,
        &xdg_data_dir,
        &yarn_cache_dir,
        &pnpm_store_dir,
        &corepack_home,
    ] {
        tokio::fs::create_dir_all(dir).await?;
    }

    if tokio::fs::metadata(&npm_config_path).await.is_err() {
        tokio::fs::write(&npm_config_path, b"").await?;
    }

    Ok((
        vendor_node_modules,
        user_node_modules,
        npm_config_path,
        npm_cache_dir,
        npm_tmp_dir,
        npm_prefix_dir,
        xdg_config_dir,
        xdg_cache_dir,
        xdg_data_dir,
        yarn_cache_dir,
        pnpm_store_dir,
        corepack_home,
    ))
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

    if let Ok(exec_path) = std::env::current_exe()
        && let Some(candidate) = resolve_bundled_node(&exec_path)
    {
        return Some(candidate);
    }

    if let Ok(path) = which::which("node") {
        return Some(path);
    }

    None
}

fn resolve_bundled_node(exec_path: &Path) -> Option<PathBuf> {
    let target = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        _ => return None,
    };

    let mut path = exec_path.to_path_buf();
    if let Some(parent) = path.parent() {
        path = parent.to_path_buf();
    }
    let mut dir = path;
    for _ in 0..4 {
        if dir.join("vendor").exists() {
            break;
        }
        dir = match dir.parent() {
            Some(parent) => parent.to_path_buf(),
            None => break,
        };
    }
    let candidate = dir
        .join("vendor")
        .join(target)
        .join("node")
        .join(if cfg!(windows) { "node.exe" } else { "node" });
    if candidate.exists() {
        return Some(candidate);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use crate::codex::make_session_and_context_with_rx;
    use crate::protocol::AskForApproval;
    use crate::protocol::EventMsg;
    use crate::protocol::SandboxPolicy;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::ResponseInputItem;
    use codex_protocol::openai_models::InputModality;
    use pretty_assertions::assert_eq;

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
        let mut entry = ExecBuffer::new("call-1".to_string(), Arc::new(session), Arc::new(turn));
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
        assert!(is_js_repl_internal_tool("js_repl_cancel"));
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
        let manager = JsReplManager::new(None, PathBuf::from("."))
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reset_clears_inflight_exec_tool_calls_without_waiting() {
        let manager = JsReplManager::new(None, std::env::temp_dir())
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
            wait_manager.wait_for_exec_tool_calls(&wait_exec_id).await;
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
        let manager = JsReplManager::new(None, std::env::temp_dir())
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
        let mut entry = ExecBuffer::new("call-1".to_string(), Arc::new(session), Arc::new(turn));
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
        let mut entry = ExecBuffer::new("call-1".to_string(), Arc::new(session), Arc::new(turn));
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
    async fn complete_exec_in_store_suppresses_kernel_exit_when_host_terminating() {
        let (session, turn) = make_session_and_context().await;
        let exec_id = "exec-1";
        let exec_store = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

        let mut entry = ExecBuffer::new("call-1".to_string(), Arc::new(session), Arc::new(turn));
        entry.host_terminating = true;
        exec_store.lock().await.insert(exec_id.to_string(), entry);

        let kernel_exit_completed = JsReplManager::complete_exec_in_store(
            &exec_store,
            exec_id,
            ExecTerminalKind::KernelExit,
            None,
            Some("js_repl kernel exited unexpectedly".to_string()),
            false,
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
            Some(JS_REPL_CANCEL_ERROR_MESSAGE.to_string()),
            false,
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

    #[test]
    fn build_js_repl_exec_output_sets_timed_out() {
        let out = build_js_repl_exec_output("", Some("timeout"), Duration::from_millis(50), true);
        assert!(out.timed_out);
    }
    async fn can_run_js_repl_runtime_tests() -> bool {
        if std::env::var_os("CODEX_SANDBOX").is_some() {
            return false;
        }
        let Some(node_path) = resolve_node(None) else {
            return false;
        };
        let required = match required_node_version() {
            Ok(v) => v,
            Err(_) => return false,
        };
        let found = match read_node_version(&node_path).await {
            Ok(v) => v,
            Err(_) => return false,
        };
        found >= required
    }

    #[tokio::test]
    async fn js_repl_persists_top_level_bindings_and_supports_tla() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
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
                    code: "let x = await Promise.resolve(41); console.log(x);".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                },
            )
            .await?;
        assert!(first.output.contains("41"));

        let second = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "console.log(x + 1);".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                },
            )
            .await?;

        assert!(second.output.contains("42"));
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
                },
            ),
        )
        .await
        .expect("execute should return, not deadlock")
        .expect_err("expected timeout error");

        assert_eq!(
            result.to_string(),
            "js_repl execution timed out; kernel reset, rerun your request"
        );
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
                },
            )
            .await?;

        let child = {
            let guard = manager.kernel.lock().await;
            let state = guard.as_ref().expect("kernel should exist after warmup");
            Arc::clone(&state.child)
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
                },
            )
            .await
            .expect_err("expected timeout error");

        assert_eq!(
            result.to_string(),
            "js_repl execution timed out; kernel reset, rerun your request"
        );

        let exit_state = {
            let mut child = child.lock().await;
            child.try_wait()?
        };
        assert!(
            exit_state.is_some(),
            "timed out js_repl execution should kill previous kernel process"
        );
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_kernel_failure_includes_model_diagnostics() -> anyhow::Result<()> {
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
                },
            )
            .await?;

        let child = {
            let guard = manager.kernel.lock().await;
            let state = guard.as_ref().expect("kernel should exist after warmup");
            Arc::clone(&state.child)
        };
        JsReplManager::kill_kernel_child(&child, "test_crash").await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        let err = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: "console.log('after-kill');".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                },
            )
            .await
            .expect_err("expected kernel failure after forced kill");

        let message = err.to_string();
        assert!(message.contains("js_repl diagnostics:"));
        assert!(message.contains("\"reason\":\"write_failed\""));
        assert!(message.contains("\"kernel_status\":\"exited("));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_can_call_tools() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let shell = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "const shellOut = await codex.tool(\"shell_command\", { command: \"printf js_repl_shell_ok\" }); console.log(JSON.stringify(shellOut));".to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                },
            )
            .await?;
        assert!(shell.output.contains("js_repl_shell_ok"));

        let tool = manager
            .execute(
                Arc::clone(&session),
                Arc::clone(&turn),
                Arc::clone(&tracker),
                JsReplArgs {
                    code: "const toolOut = await codex.tool(\"list_mcp_resources\", {}); console.log(toolOut.type);".to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                },
            )
            .await?;
        assert!(tool.output.contains("function_call_output"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_tool_call_rejects_recursive_js_repl_invocation() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

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
                    code: r#"
try {
  await codex.tool("js_repl", "console.log('recursive')");
  console.log("unexpected-success");
} catch (err) {
  console.log(String(err));
}
"#
                    .to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                },
            )
            .await?;

        assert!(
            result.output.contains("js_repl cannot invoke itself"),
            "expected recursion guard message, got output: {}",
            result.output
        );
        assert!(
            !result.output.contains("unexpected-success"),
            "recursive js_repl tool call unexpectedly succeeded: {}",
            result.output
        );
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_waits_for_unawaited_tool_calls_before_completion() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await || cfg!(windows) {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

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
    async fn js_repl_can_attach_image_via_view_image_tool() -> anyhow::Result<()> {
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
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

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
console.log(out.output?.body?.text ?? "");
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
                },
            )
            .await?;
        assert!(result.output.contains("function_call_output"));

        let pending_input = session.get_pending_input().await;
        let image_url = pending_input
            .iter()
            .find_map(|item| match item {
                ResponseInputItem::Message { content, .. } => {
                    content.iter().find_map(|content_item| match content_item {
                        ContentItem::InputImage { image_url } => Some(image_url.as_str()),
                        _ => None,
                    })
                }
                _ => None,
            })
            .expect("view_image should inject an input_image message for the active turn");
        assert!(image_url.starts_with("data:image/png;base64,"));

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_does_not_expose_process_global() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn) = make_session_and_context().await;
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
                    code: "console.log(typeof process);".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                },
            )
            .await?;
        assert!(result.output.contains("undefined"));
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_blocks_sensitive_builtin_imports() -> anyhow::Result<()> {
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
                    code: "await import(\"node:process\");".to_string(),
                    timeout_ms: Some(10_000),
                    poll: false,
                },
            )
            .await
            .expect_err("node:process import should be blocked");
        assert!(
            err.to_string()
                .contains("Importing module \"node:process\" is not allowed in js_repl")
        );
        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_submit_and_complete() -> anyhow::Result<()> {
        if resolve_node(None).is_none() || std::env::var_os("CODEX_SANDBOX").is_some() {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

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
                    timeout_ms: Some(5_000),
                    poll: true,
                },
            )
            .await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                let output = result.output.unwrap_or_default();
                assert!(output.contains("poll-ok"));
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
    async fn js_repl_poll_emits_exec_output_delta_events() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, turn, rx) = make_session_and_context_with_rx().await;
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
                    timeout_ms: Some(5_000),
                    poll: true,
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
        if resolve_node(None).is_none() || std::env::var_os("CODEX_SANDBOX").is_some() {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

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
                    timeout_ms: Some(10_000),
                    poll: true,
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
                    timeout_ms: Some(10_000),
                    poll: true,
                },
            )
            .await?;

        let fast_start = Instant::now();
        let fast_output = loop {
            let result = manager.poll(&fast_submission.exec_id, Some(200)).await?;
            if result.done {
                break result.output.unwrap_or_default();
            }
            if fast_start.elapsed() > Duration::from_millis(1_500) {
                panic!("fast polled exec did not complete quickly; submit appears serialized");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        assert!(fast_output.contains("fast-done"));

        let slow_deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let result = manager.poll(&slow_submission.exec_id, Some(200)).await?;
            if result.done {
                let output = result.output.unwrap_or_default();
                assert!(output.contains("slow-done"));
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
        if resolve_node(None).is_none() || std::env::var_os("CODEX_SANDBOX").is_some() {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

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
                    timeout_ms: Some(5_000),
                    poll: true,
                },
            )
            .await?;

        let deadline = Instant::now() + Duration::from_secs(5);
        let first_result = loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                break result;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for js_repl poll completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        assert!(
            first_result
                .output
                .as_deref()
                .is_some_and(|output| output.contains("replay-ok"))
        );

        let second_result = manager.poll(&submission.exec_id, Some(50)).await?;
        assert!(second_result.done);
        assert!(
            second_result
                .output
                .as_deref()
                .is_some_and(|output| output.contains("replay-ok"))
        );

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_timeout_waits_for_inflight_tool_calls() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await || cfg!(windows) {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let started_marker = turn.cwd.join(format!(
            "js-repl-poll-timeout-started-{}.txt",
            Uuid::new_v4()
        ));
        let done_marker = turn
            .cwd
            .join(format!("js-repl-poll-timeout-done-{}.txt", Uuid::new_v4()));
        let started_json = serde_json::to_string(&started_marker.to_string_lossy().to_string())?;
        let done_json = serde_json::to_string(&done_marker.to_string_lossy().to_string())?;
        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-timeout".to_string(),
                JsReplArgs {
                    code: format!(
                        r#"
const started = {started_json};
const done = {done_json};
void codex.tool("shell_command", {{ command: `printf started > "${{started}}"; sleep 0.6; printf done > "${{done}}"` }});
await new Promise((resolve) => setTimeout(resolve, 150));
await new Promise(() => {{}});
"#
                    ),
                    timeout_ms: Some(350),
                    poll: true,
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

        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let result = manager.poll(&submission.exec_id, Some(200)).await?;
            if result.done {
                assert_eq!(
                    result.error.as_deref(),
                    Some("js_repl execution timed out; kernel reset, rerun your request")
                );
                break;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for js_repl poll timeout completion");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let done_contents = tokio::fs::read_to_string(&done_marker).await?;
        assert_eq!(done_contents, "done");
        let _ = tokio::fs::remove_file(&started_marker).await;
        let _ = tokio::fs::remove_file(&done_marker).await;

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_cancel_marks_exec_canceled() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

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
                        timeout_ms: Some(30_000),
                        poll: true,
                    },
                )
                .await?;

            tokio::time::sleep(Duration::from_millis(100)).await;
            manager.cancel(&submission.exec_id).await?;

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
                    panic!("timed out waiting for js_repl poll cancellation completion");
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_cancel_rejects_completed_exec() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager = turn.js_repl.manager().await?;

        let submission = Arc::clone(&manager)
            .submit(
                Arc::clone(&session),
                Arc::clone(&turn),
                tracker,
                "call-cancel-completed".to_string(),
                JsReplArgs {
                    code: "console.log('done');".to_string(),
                    timeout_ms: Some(5_000),
                    poll: true,
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
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        let err = manager
            .cancel(&submission.exec_id)
            .await
            .expect_err("expected completed exec to reject cancel");
        assert_eq!(err.to_string(), "js_repl exec already completed");

        Ok(())
    }

    #[tokio::test]
    async fn js_repl_poll_reset_marks_running_exec_canceled() -> anyhow::Result<()> {
        if !can_run_js_repl_runtime_tests().await {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;

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
                    timeout_ms: Some(30_000),
                    poll: true,
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

        let (session, turn, rx) = make_session_and_context_with_rx().await;

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
                    timeout_ms: Some(30_000),
                    poll: true,
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
        let (_session, turn) = make_session_and_context().await;
        let manager = turn.js_repl.manager().await?;
        let err = manager
            .poll("missing-exec-id", Some(50))
            .await
            .expect_err("expected missing exec id error");
        assert_eq!(err.to_string(), "js_repl exec id not found");
        Ok(())
    }
    #[tokio::test]
    async fn js_repl_cancel_rejects_unknown_exec_id() -> anyhow::Result<()> {
        let (_session, turn) = make_session_and_context().await;
        let manager = turn.js_repl.manager().await?;
        let err = manager
            .cancel("missing-exec-id")
            .await
            .expect_err("expected missing exec id error");
        assert_eq!(err.to_string(), "js_repl exec id not found");
        Ok(())
    }
    #[tokio::test]
    async fn js_repl_isolated_module_resolution() -> anyhow::Result<()> {
        if resolve_node(None).is_none() || std::env::var_os("CODEX_SANDBOX").is_some() {
            return Ok(());
        }

        let (session, mut turn) = make_session_and_context().await;
        turn.approval_policy = AskForApproval::Never;
        turn.sandbox_policy = SandboxPolicy::DangerFullAccess;
        turn.shell_environment_policy
            .r#set
            .insert("NODE_OPTIONS".to_string(), "--trace-warnings".to_string());
        turn.shell_environment_policy.r#set.insert(
            "npm_config_userconfig".to_string(),
            "/tmp/should-not-see".to_string(),
        );

        let session = Arc::new(session);
        let turn = Arc::new(turn);
        let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::default()));
        let manager: Arc<JsReplManager> = turn.js_repl.manager().await?;

        let code = r#"
const fs = await import("node:fs/promises");
const path = await import("node:path");
const os = await import("node:os");
const replHome = os.homedir();
const vendorRoot = path.join(replHome, "codex_node_modules", "node_modules");
const userRoot = path.join(replHome, "node_modules");

const dupeVendorDir = path.join(vendorRoot, "dupe");
await fs.mkdir(dupeVendorDir, { recursive: true });
await fs.writeFile(
  path.join(dupeVendorDir, "package.json"),
  JSON.stringify({ name: "dupe", type: "module", main: "index.js" })
);
await fs.writeFile(path.join(dupeVendorDir, "index.js"), 'export const source = "vendor";');

const dupeUserDir = path.join(userRoot, "dupe");
await fs.mkdir(dupeUserDir, { recursive: true });
await fs.writeFile(
  path.join(dupeUserDir, "package.json"),
  JSON.stringify({ name: "dupe", type: "module", main: "index.js" })
);
await fs.writeFile(path.join(dupeUserDir, "index.js"), 'export const source = "user";');

const userOnlyDir = path.join(userRoot, "user_only");
await fs.mkdir(userOnlyDir, { recursive: true });
await fs.writeFile(
  path.join(userOnlyDir, "package.json"),
  JSON.stringify({ name: "user_only", type: "module", main: "index.js" })
);
await fs.writeFile(path.join(userOnlyDir, "index.js"), 'export const source = "user_only";');

const dupe = await import("dupe");
const userOnly = await import("user_only");

console.log(
  JSON.stringify({
    env: {
      replHome,
      vendorRoot,
      userRoot,
    },
    dupe: dupe.source,
    userOnly: userOnly.source,
  })
);
"#;

        let output = manager
            .execute(
                session,
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                    poll: false,
                },
            )
            .await?
            .output;
        let parsed: serde_json::Value =
            serde_json::from_str(output.trim()).unwrap_or_else(|_| serde_json::json!({}));
        let env = parsed
            .get("env")
            .and_then(serde_json::Value::as_object)
            .cloned()
            .unwrap_or_default();
        let repl_home = env
            .get("replHome")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let vendor_root = env
            .get("vendorRoot")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let user_root = env
            .get("userRoot")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();

        assert_eq!(
            parsed.get("dupe").and_then(serde_json::Value::as_str),
            Some("vendor")
        );
        assert_eq!(
            parsed.get("userOnly").and_then(serde_json::Value::as_str),
            Some("user_only")
        );
        assert!(vendor_root.contains(repl_home));
        assert!(
            Path::new(vendor_root).ends_with(Path::new("codex_node_modules").join("node_modules"))
        );
        assert!(user_root.contains(repl_home));

        Ok(())
    }
}
