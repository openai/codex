use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseInputItem;
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
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::trace;
use tracing::warn;
use uuid::Uuid;

use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecExpiration;
use crate::exec_env::create_env;
use crate::function_tool::FunctionCallError;
use crate::original_image_detail::normalize_output_image_detail;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxManager;
use crate::sandboxing::SandboxPermissions;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::sandboxing::SandboxablePreference;
use crate::truncate::TruncationPolicy;
use crate::truncate::truncate_text;

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
}

#[derive(Clone, Debug)]
pub struct JsExecResult {
    pub output: String,
    pub content_items: Vec<FunctionCallOutputContentItem>,
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

pub struct JsReplManager {
    node_path: Option<PathBuf>,
    node_module_dirs: Vec<PathBuf>,
    tmp_dir: tempfile::TempDir,
    kernel: Arc<Mutex<Option<KernelState>>>,
    exec_lock: Arc<tokio::sync::Semaphore>,
    exec_tool_calls: Arc<Mutex<HashMap<String, ExecToolCalls>>>,
}

impl JsReplManager {
    async fn new(
        node_path: Option<PathBuf>,
        node_module_dirs: Vec<PathBuf>,
    ) -> Result<Arc<Self>, FunctionCallError> {
        let tmp_dir = tempfile::tempdir().map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to create js_repl temp dir: {err}"))
        })?;

        let manager = Arc::new(Self {
            node_path,
            node_module_dirs,
            tmp_dir,
            kernel: Arc::new(Mutex::new(None)),
            exec_lock: Arc::new(tokio::sync::Semaphore::new(1)),
            exec_tool_calls: Arc::new(Mutex::new(HashMap::new())),
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

    pub async fn reset(&self) -> Result<(), FunctionCallError> {
        let _permit = self.exec_lock.clone().acquire_owned().await.map_err(|_| {
            FunctionCallError::RespondToModel("js_repl execution unavailable".to_string())
        })?;
        self.reset_kernel().await;
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
                let dependency_env = session.dependency_env().await;
                let state = self
                    .start_kernel(
                        Arc::clone(&turn),
                        &dependency_env,
                        Some(session.conversation_id),
                    )
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
                self.reset_kernel().await;
                self.wait_for_exec_tool_calls(&req_id).await;
                self.exec_tool_calls.lock().await.clear();
                return Err(FunctionCallError::RespondToModel(
                    "js_repl execution timed out; kernel reset, rerun your request".to_string(),
                ));
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
            ExecResultMessage::Err { message } => Err(FunctionCallError::RespondToModel(message)),
        }
    }

    async fn start_kernel(
        &self,
        turn: Arc<TurnContext>,
        dependency_env: &HashMap<String, String>,
        thread_id: Option<ThreadId>,
    ) -> Result<KernelState, String> {
        let node_path = resolve_compatible_node(self.node_path.as_deref()).await?;

        let kernel_path = self
            .write_kernel_script()
            .await
            .map_err(|err| err.to_string())?;

        let mut env = create_env(&turn.shell_environment_policy, thread_id);
        if !dependency_env.is_empty() {
            env.extend(dependency_env.clone());
        }
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
        let has_managed_network_requirements = turn
            .config
            .config_layer_stack
            .requirements_toml()
            .network
            .is_some();
        let sandbox_type = sandbox.select_initial(
            &turn.file_system_sandbox_policy,
            turn.network_sandbox_policy,
            SandboxablePreference::Auto,
            turn.windows_sandbox_level,
            has_managed_network_requirements,
        );
        let exec_env = sandbox
            .transform(crate::sandboxing::SandboxTransformRequest {
                spec,
                policy: &turn.sandbox_policy,
                file_system_policy: &turn.file_system_sandbox_policy,
                network_policy: turn.network_sandbox_policy,
                sandbox: sandbox_type,
                enforce_managed_network: has_managed_network_requirements,
                network: None,
                sandbox_policy_cwd: &turn.cwd,
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions: None,
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
            Arc::clone(&self.kernel),
            Arc::clone(&recent_stderr),
            Arc::clone(&pending_execs),
            Arc::clone(&exec_contexts),
            Arc::clone(&self.exec_tool_calls),
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

    async fn write_kernel_script(&self) -> Result<PathBuf, std::io::Error> {
        let dir = self.tmp_dir.path();
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
        manager_kernel: Arc<Mutex<Option<KernelState>>>,
        recent_stderr: Arc<Mutex<VecDeque<String>>>,
        pending_execs: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<ExecResultMessage>>>>,
        exec_contexts: Arc<Mutex<HashMap<String, ExecContext>>>,
        exec_tool_calls: Arc<Mutex<HashMap<String, ExecToolCalls>>>,
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
                KernelToHost::ExecResult {
                    id,
                    ok,
                    output,
                    error,
                } => {
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
                                    output,
                                    content_items,
                                ),
                            }
                        } else {
                            ExecResultMessage::Err {
                                message: error
                                    .unwrap_or_else(|| "js_repl execution failed".to_string()),
                            }
                        };
                        let _ = tx.send(payload);
                    }
                    exec_contexts.lock().await.remove(&id);
                    JsReplManager::clear_exec_tool_calls_map(&exec_tool_calls, &id).await;
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
                            JsReplManager::kernel_debug_snapshot(&child, &recent_stderr).await;
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

        {
            let mut kernel = manager_kernel.lock().await;
            let should_clear = kernel
                .as_ref()
                .is_some_and(|state| Arc::ptr_eq(&state.child, &child));
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

        if !matches!(end_reason, KernelStreamEnd::Shutdown) {
            let mut pending_exec_ids = pending_exec_ids;
            pending_exec_ids.sort_unstable();
            let snapshot = Self::kernel_debug_snapshot(&child, &recent_stderr).await;
            warn!(
                reason = %end_reason.reason(),
                stream_error = %end_reason.error().unwrap_or(""),
                kernel_pid = ?snapshot.pid,
                kernel_status = %snapshot.status,
                pending_exec_count = pending_exec_ids.len(),
                pending_exec_ids = ?Self::truncate_id_list(&pending_exec_ids),
                kernel_stderr_tail = %snapshot.stderr_tail,
                "js_repl kernel terminated unexpectedly"
            );
        }
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

fn emitted_image_content_item(
    turn: &TurnContext,
    image_url: String,
    detail: Option<ImageDetail>,
) -> FunctionCallOutputContentItem {
    FunctionCallOutputContentItem::InputImage {
        image_url,
        detail: normalize_output_image_detail(turn.features.get(), &turn.model_info, detail),
    }
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
    matches!(name, "js_repl" | "js_repl_reset")
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum KernelToHost {
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

    fn set_danger_full_access(turn: &mut crate::codex::TurnContext) {
        turn.sandbox_policy
            .set(SandboxPolicy::DangerFullAccess)
            .expect("test setup should allow updating sandbox policy");
        turn.file_system_sandbox_policy =
            crate::protocol::FileSystemSandboxPolicy::from(turn.sandbox_policy.get());
        turn.network_sandbox_policy =
            crate::protocol::NetworkSandboxPolicy::from(turn.sandbox_policy.get());
    }

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
    async fn emitted_image_content_item_drops_unsupported_explicit_detail() {
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
                detail: None,
            }
        );
    }

    #[tokio::test]
    async fn emitted_image_content_item_does_not_force_original_when_enabled() {
        let (_session, mut turn) = make_session_and_context().await;
        Arc::make_mut(&mut turn.config)
            .features
            .enable(Feature::ImageDetailOriginal)
            .expect("test config should allow feature update");
        turn.features
            .enable(Feature::ImageDetailOriginal)
            .expect("test turn features should allow feature update");
        turn.model_info.supports_image_detail_original = true;

        let content_item =
            emitted_image_content_item(&turn, "data:image/png;base64,AAA".to_string(), None);

        assert_eq!(
            content_item,
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: None,
            }
        );
    }

    #[tokio::test]
    async fn emitted_image_content_item_allows_explicit_original_detail_when_enabled() {
        let (_session, mut turn) = make_session_and_context().await;
        Arc::make_mut(&mut turn.config)
            .features
            .enable(Feature::ImageDetailOriginal)
            .expect("test config should allow feature update");
        turn.features
            .enable(Feature::ImageDetailOriginal)
            .expect("test turn features should allow feature update");
        turn.model_info.supports_image_detail_original = true;

        let content_item = emitted_image_content_item(
            &turn,
            "data:image/png;base64,AAA".to_string(),
            Some(ImageDetail::Original),
        );

        assert_eq!(
            content_item,
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: Some(ImageDetail::Original),
            }
        );
    }

    #[tokio::test]
    async fn emitted_image_content_item_drops_explicit_original_detail_when_disabled() {
        let (_session, turn) = make_session_and_context().await;

        let content_item = emitted_image_content_item(
            &turn,
            "data:image/png;base64,AAA".to_string(),
            Some(ImageDetail::Original),
        );

        assert_eq!(
            content_item,
            FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: None,
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
                },
            )
            .await?;

        let child = {
            let guard = manager.kernel.lock().await;
            let state = guard.as_ref().expect("kernel should exist after warmup");
            Arc::clone(&state.child)
        };
        JsReplManager::kill_kernel_child(&child, "test_crash").await;
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let cleared = {
                    let guard = manager.kernel.lock().await;
                    guard
                        .as_ref()
                        .is_none_or(|state| !Arc::ptr_eq(&state.child, &child))
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
                },
            )
            .await?;

        let child = {
            let guard = manager.kernel.lock().await;
            let state = guard.as_ref().expect("kernel should exist after warmup");
            Arc::clone(&state.child)
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
                let exited = {
                    let mut child = child.lock().await;
                    child.try_wait()?.is_some()
                };
                if exited {
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
                        .is_none_or(|state| !Arc::ptr_eq(&state.child, &child))
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
        set_danger_full_access(&mut turn);

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
        set_danger_full_access(&mut turn);

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
        set_danger_full_access(&mut turn);

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
                },
            )
            .await
            .expect_err("invalid detail should fail");
        assert!(
            err.to_string()
                .contains("only supports detail \"original\"")
        );
        assert!(session.get_pending_input().await.is_empty());

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn js_repl_emit_image_treats_null_detail_as_omitted() -> anyhow::Result<()> {
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
await codex.emitImage({ bytes: png, mimeType: "image/png", detail: null });
"#;

        let result = manager
            .execute(
                Arc::clone(&session),
                turn,
                tracker,
                JsReplArgs {
                    code: code.to_string(),
                    timeout_ms: Some(15_000),
                },
            )
            .await?;
        assert_eq!(
            result.content_items.as_slice(),
            [FunctionCallOutputContentItem::InputImage {
                image_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==".to_string(),
                detail: None,
            }]
            .as_slice()
        );
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
                },
            )
            .await?;
        assert!(result.output.contains("env"));
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
        let expected_home_dir = serde_json::to_string("/tmp/codex-home")?;
        write_js_repl_test_module(
            cwd_dir.path(),
            "globals.js",
            &format!(
                "const expectedHomeDir = {expected_home_dir};\nconsole.log(`tmp:${{codex.tmpDir === tmpDir}}`);\nconsole.log(`cwd:${{typeof codex.cwd}}:${{codex.cwd.length > 0}}`);\nconsole.log(`home:${{codex.homeDir === expectedHomeDir}}`);\nconsole.log(`tool:${{typeof codex.tool}}`);\nconsole.log(\"local-file-console-ok\");\n"
            ),
        )?;

        let (session, mut turn) = make_session_and_context().await;
        session
            .set_dependency_env(HashMap::from([(
                "HOME".to_string(),
                "/tmp/codex-home".to_string(),
            )]))
            .await;
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
                },
            )
            .await?;
        assert!(result.output.contains("tmp:true"));
        assert!(result.output.contains("cwd:string:true"));
        assert!(result.output.contains("home:true"));
        assert!(result.output.contains("tool:function"));
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
                },
            )
            .await
            .expect_err("expected parent node_modules lookup to be rejected");
        assert!(err.to_string().contains("repl_probe"));
        Ok(())
    }
}
