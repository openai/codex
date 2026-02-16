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
use tracing::warn;
use uuid::Uuid;

use crate::client_common::tools::ToolSpec;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecExpiration;
use crate::exec_env::create_env;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::SandboxManager;
use crate::sandboxing::SandboxPermissions;
use crate::tools::ToolRouter;
use crate::tools::context::SharedTurnDiffTracker;
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
}

#[derive(Clone, Debug)]
pub struct JsExecResult {
    pub output: String,
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
    codex_home: PathBuf,
    tmp_dir: tempfile::TempDir,
    kernel: Mutex<Option<KernelState>>,
    exec_lock: Arc<tokio::sync::Semaphore>,
    exec_tool_calls: Arc<Mutex<HashMap<String, ExecToolCalls>>>,
}

impl JsReplManager {
    async fn new(
        node_path: Option<PathBuf>,
        codex_home: PathBuf,
    ) -> Result<Arc<Self>, FunctionCallError> {
        let tmp_dir = tempfile::tempdir().map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to create js_repl temp dir: {err}"))
        })?;

        let manager = Arc::new(Self {
            node_path,
            codex_home,
            tmp_dir,
            kernel: Mutex::new(None),
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

    async fn wait_for_all_exec_tool_calls(&self) {
        loop {
            let notified = {
                let calls = self.exec_tool_calls.lock().await;
                calls
                    .values()
                    .find(|state| state.in_flight > 0)
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
    ) -> bool {
        let mut calls = exec_tool_calls.lock().await;
        let Some(state) = calls.get_mut(exec_id) else {
            return false;
        };
        state.in_flight += 1;
        true
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
            state.notify.notify_waiters();
        }
    }

    pub async fn reset(&self) -> Result<(), FunctionCallError> {
        self.reset_kernel().await;
        self.wait_for_all_exec_tool_calls().await;
        self.exec_tool_calls.lock().await.clear();
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
                self.reset().await?;
                return Err(FunctionCallError::RespondToModel(
                    "js_repl execution timed out; kernel reset, rerun your request".to_string(),
                ));
            }
        };

        match response {
            ExecResultMessage::Ok { output } => Ok(JsExecResult { output }),
            ExecResultMessage::Err { message } => Err(FunctionCallError::RespondToModel(message)),
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

        let kernel_path = self
            .write_kernel_script()
            .await
            .map_err(|err| err.to_string())?;

        let mut env = create_env(&turn.shell_environment_policy, thread_id);
        env.insert(
            "CODEX_JS_TMP_DIR".to_string(),
            self.tmp_dir.path().to_string_lossy().to_string(),
        );
        env.insert(
            "CODEX_JS_REPL_HOME".to_string(),
            self.codex_home
                .join("js_repl")
                .to_string_lossy()
                .to_string(),
        );

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
                    let mut pending = pending_execs.lock().await;
                    if let Some(tx) = pending.remove(&id) {
                        let payload = if ok {
                            ExecResultMessage::Ok { output }
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
                KernelToHost::RunTool(req) => {
                    if !JsReplManager::begin_exec_tool_call(&exec_tool_calls, &req.exec_id).await {
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
                    }
                    let stdin_clone = Arc::clone(&stdin);
                    let exec_contexts = Arc::clone(&exec_contexts);
                    let exec_tool_calls = Arc::clone(&exec_tool_calls);
                    let recent_stderr = Arc::clone(&recent_stderr);
                    tokio::spawn(async move {
                        let exec_id = req.exec_id.clone();
                        let tool_call_id = req.id.clone();
                        let tool_name = req.tool_name.clone();
                        let context = { exec_contexts.lock().await.get(&exec_id).cloned() };
                        let result = match context {
                            Some(ctx) => JsReplManager::run_tool_request(ctx, req).await,
                            None => RunToolResult {
                                id: req.id.clone(),
                                ok: false,
                                response: None,
                                error: Some("js_repl exec context not found".to_string()),
                            },
                        };
                        JsReplManager::finish_exec_tool_call(&exec_tool_calls, &exec_id).await;
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

    if let Ok(path) = which::which("node") {
        return Some(path);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context;
    use crate::protocol::AskForApproval;
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
            assert!(JsReplManager::begin_exec_tool_call(&exec_tool_calls, &exec_id).await);

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
}
