use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::Weak;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::codex::ExecCommandContext;
use crate::codex::Session;
use crate::exec::ExecParams;
use crate::exec::SandboxType;
use crate::function_tool::FunctionCallError;
use crate::landlock::spawn_command_under_linux_sandbox;
use crate::protocol::ReviewDecision;
use crate::protocol::SandboxPolicy;
use crate::safety::SafetyCheck;
use crate::safety::assess_command_safety;
use crate::seatbelt::spawn_command_under_seatbelt;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use codex_otel::otel_event_manager::ToolDecisionSource;

const LOG_CAP_BYTES: usize = 512 * 1024; // 512 KiB cap per process
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(200);
const BACKGROUND_TOOL_NAME: &str = "background_process";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BackgroundProcessState {
    Running,
    Exited {
        exit_code: Option<i32>,
        signal: Option<i32>,
        finished_at: SystemTime,
    },
    Failed {
        message: String,
        finished_at: SystemTime,
    },
}

#[derive(Debug)]
struct LogEntry {
    stream: LogStream,
    text: String,
}

impl Clone for LogEntry {
    fn clone(&self) -> Self {
        Self {
            stream: self.stream,
            text: self.text.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogStream {
    Stdout,
    Stderr,
}

impl LogStream {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[derive(Debug, Default)]
struct ProcessLog {
    entries: VecDeque<LogEntry>,
    total_bytes: usize,
}

impl ProcessLog {
    fn append(&mut self, stream: LogStream, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(chunk).into_owned();
        self.total_bytes = self.total_bytes.saturating_add(text.len());
        self.entries.push_back(LogEntry { stream, text });

        while self.total_bytes > LOG_CAP_BYTES {
            if let Some(front) = self.entries.pop_front() {
                self.total_bytes = self.total_bytes.saturating_sub(front.text.len());
            } else {
                break;
            }
        }
    }

    fn snapshot(&self) -> Vec<LogEntry> {
        self.entries.iter().cloned().collect()
    }
}

struct ManagedBackgroundProcess {
    id: String,
    command_for_display: Vec<String>,
    cwd: PathBuf,
    started_at: SystemTime,
    sandbox_type: SandboxType,
    child: Arc<AsyncMutex<Child>>,
    state: Arc<RwLock<BackgroundProcessState>>,
    log: Arc<AsyncMutex<ProcessLog>>,
    stdout_task: JoinHandle<()>,
    stderr_task: JoinHandle<()>,
    monitor_task: JoinHandle<()>,
}

impl ManagedBackgroundProcess {
    async fn summary(&self) -> BackgroundProcessSummary {
        let state = self.state.read().await.clone();
        BackgroundProcessSummary {
            id: self.id.clone(),
            command: self.command_for_display.clone(),
            cwd: self.cwd.clone(),
            started_at: self.started_at,
            state,
            sandbox_type: self.sandbox_type,
        }
    }

    async fn logs(&self) -> Vec<BackgroundProcessLogEntry> {
        let log = self.log.lock().await;
        log.snapshot()
            .into_iter()
            .map(|entry| BackgroundProcessLogEntry {
                stream: entry.stream.as_str().to_string(),
                text: entry.text,
            })
            .collect()
    }

    async fn kill(&self) -> Result<(), std::io::Error> {
        let mut child = self.child.lock().await;
        match child.start_kill() {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
            Err(err) => Err(err),
        }
    }
}

impl Drop for ManagedBackgroundProcess {
    fn drop(&mut self) {
        self.stdout_task.abort();
        self.stderr_task.abort();
        self.monitor_task.abort();
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BackgroundProcessSummary {
    pub(crate) id: String,
    pub(crate) command: Vec<String>,
    pub(crate) cwd: PathBuf,
    pub(crate) started_at: SystemTime,
    pub(crate) state: BackgroundProcessState,
    pub(crate) sandbox_type: SandboxType,
}

#[derive(Debug, Clone)]
pub(crate) struct BackgroundProcessLogEntry {
    pub(crate) stream: String,
    pub(crate) text: String,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct StartProcessResponse {
    pub(crate) process_id: String,
}

pub(crate) struct BackgroundProcessManager {
    next_id: AtomicU64,
    processes: AsyncMutex<HashMap<String, Arc<ManagedBackgroundProcess>>>,
    running_count: Arc<AtomicU64>,
    session_handle: Arc<StdMutex<Option<Weak<Session>>>>,
}

impl BackgroundProcessManager {
    pub(crate) fn new() -> Self {
        Self {
            next_id: AtomicU64::new(0),
            processes: AsyncMutex::new(HashMap::new()),
            running_count: Arc::new(AtomicU64::new(0)),
            session_handle: Arc::new(StdMutex::new(None)),
        }
    }

    pub(crate) fn set_session(&self, session: Weak<Session>) {
        let mut guard = self
            .session_handle
            .lock()
            .expect("background process session_handle lock poisoned");
        *guard = Some(session);
    }

    pub(crate) async fn start(
        &self,
        session: &Session,
        turn_context: &crate::codex::TurnContext,
        exec_context: ExecCommandContext,
        exec_params: ExecParams,
        approved_commands: HashSet<Vec<String>>,
        codex_linux_sandbox_exe: Option<PathBuf>,
    ) -> Result<StartProcessResponse, FunctionCallError> {
        let id_num = self.next_id.fetch_add(1, Ordering::SeqCst) + 1;
        let process_id = format!("bg-{id_num}");
        let otel_event_manager = turn_context.client.get_otel_event_manager();
        let command_for_display = exec_context.command_for_display.clone();

        let safety = assess_command_safety(
            &exec_params.command,
            turn_context.approval_policy,
            &turn_context.sandbox_policy,
            &approved_commands,
            exec_params.with_escalated_permissions.unwrap_or(false),
        );

        let sandbox_type = match safety {
            SafetyCheck::AutoApprove {
                sandbox_type,
                user_explicitly_approved,
            } => {
                otel_event_manager.tool_decision(
                    BACKGROUND_TOOL_NAME,
                    exec_context.call_id.as_str(),
                    ReviewDecision::Approved,
                    if user_explicitly_approved {
                        ToolDecisionSource::User
                    } else {
                        ToolDecisionSource::Config
                    },
                );
                sandbox_type
            }
            SafetyCheck::AskUser => {
                let decision = session
                    .request_command_approval(
                        exec_context.sub_id.clone(),
                        exec_context.call_id.clone(),
                        exec_params.command.clone(),
                        exec_params.cwd.clone(),
                        exec_params.justification.clone(),
                    )
                    .await;

                otel_event_manager.tool_decision(
                    BACKGROUND_TOOL_NAME,
                    exec_context.call_id.as_str(),
                    decision,
                    ToolDecisionSource::User,
                );

                match decision {
                    ReviewDecision::Approved => SandboxType::None,
                    ReviewDecision::ApprovedForSession => {
                        session
                            .add_approved_command(exec_params.command.clone())
                            .await;
                        SandboxType::None
                    }
                    ReviewDecision::Denied => {
                        return Err(FunctionCallError::RespondToModel(
                            "background process rejected by user".to_string(),
                        ));
                    }
                    ReviewDecision::Abort => {
                        return Err(FunctionCallError::RespondToModel(
                            "background process aborted by user".to_string(),
                        ));
                    }
                }
            }
            SafetyCheck::Reject { reason } => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "background process rejected: {reason}"
                )));
            }
        };

        let mut child = spawn_background_child(
            &exec_params,
            sandbox_type,
            &turn_context.sandbox_policy,
            &turn_context.cwd,
            codex_linux_sandbox_exe.as_ref(),
        )
        .await?;

        let stdout = child.stdout.take().ok_or_else(|| {
            FunctionCallError::RespondToModel("failed to capture stdout".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            FunctionCallError::RespondToModel("failed to capture stderr".to_string())
        })?;

        let child = Arc::new(AsyncMutex::new(child));
        let state = Arc::new(RwLock::new(BackgroundProcessState::Running));
        let log = Arc::new(AsyncMutex::new(ProcessLog::default()));

        let stdout_task =
            spawn_log_task(Arc::clone(&log), BufReader::new(stdout), LogStream::Stdout);
        let stderr_task =
            spawn_log_task(Arc::clone(&log), BufReader::new(stderr), LogStream::Stderr);
        let monitor_task = spawn_monitor_task(
            Arc::clone(&child),
            Arc::clone(&state),
            Arc::clone(&self.running_count),
            Arc::clone(&self.session_handle),
        );

        let managed = Arc::new(ManagedBackgroundProcess {
            id: process_id.clone(),
            command_for_display,
            cwd: exec_params.cwd.clone(),
            started_at: SystemTime::now(),
            sandbox_type,
            child,
            state,
            log,
            stdout_task,
            stderr_task,
            monitor_task,
        });

        {
            let mut processes = self.processes.lock().await;
            processes.insert(process_id.clone(), managed);
        }

        self.running_count.fetch_add(1, Ordering::SeqCst);
        notify_running_count(&self.session_handle, &self.running_count).await;

        Ok(StartProcessResponse { process_id })
    }

    pub(crate) async fn list(&self) -> Vec<BackgroundProcessSummary> {
        let processes: Vec<Arc<ManagedBackgroundProcess>> = {
            let guard = self.processes.lock().await;
            guard.values().cloned().collect()
        };

        let mut summaries = Vec::with_capacity(processes.len());
        for process in processes {
            summaries.push(process.summary().await);
        }
        summaries
    }

    pub(crate) async fn logs(
        &self,
        process_id: &str,
    ) -> Result<Vec<BackgroundProcessLogEntry>, FunctionCallError> {
        let process = {
            let processes = self.processes.lock().await;
            processes.get(process_id).cloned()
        };
        let process = process.ok_or_else(|| {
            FunctionCallError::RespondToModel(format!("unknown background process: {process_id}"))
        })?;
        Ok(process.logs().await)
    }

    pub(crate) async fn kill(&self, process_id: &str) -> Result<(), FunctionCallError> {
        let process = {
            let processes = self.processes.lock().await;
            processes.get(process_id).cloned()
        };
        let process = process.ok_or_else(|| {
            FunctionCallError::RespondToModel(format!("unknown background process: {process_id}"))
        })?;

        process
            .kill()
            .await
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
    }
}

async fn spawn_background_child(
    params: &ExecParams,
    sandbox_type: SandboxType,
    sandbox_policy: &SandboxPolicy,
    sandbox_cwd: &Path,
    codex_linux_sandbox_exe: Option<&PathBuf>,
) -> Result<Child, FunctionCallError> {
    match sandbox_type {
        SandboxType::None => {
            let (program, args) = params.command.split_first().ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "background process command must not be empty".to_string(),
                )
            })?;
            spawn_child_async(
                PathBuf::from(program),
                args.to_vec(),
                None,
                params.cwd.clone(),
                sandbox_policy,
                StdioPolicy::RedirectForShellTool,
                params.env.clone(),
            )
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to spawn background process: {err}"
                ))
            })
        }
        SandboxType::MacosSeatbelt => spawn_command_under_seatbelt(
            params.command.clone(),
            params.cwd.clone(),
            sandbox_policy,
            sandbox_cwd,
            StdioPolicy::RedirectForShellTool,
            params.env.clone(),
        )
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to spawn background process in seatbelt: {err}"
            ))
        }),
        SandboxType::LinuxSeccomp => {
            let Some(exe) = codex_linux_sandbox_exe else {
                return Err(FunctionCallError::RespondToModel(
                    "codex-linux-sandbox executable not configured".to_string(),
                ));
            };
            spawn_command_under_linux_sandbox(
                exe,
                params.command.clone(),
                params.cwd.clone(),
                sandbox_policy,
                sandbox_cwd,
                StdioPolicy::RedirectForShellTool,
                params.env.clone(),
            )
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to spawn background process in linux sandbox: {err}"
                ))
            })
        }
    }
}

fn spawn_log_task<R>(
    log: Arc<AsyncMutex<ProcessLog>>,
    mut reader: BufReader<R>,
    stream: LogStream,
) -> JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let mut log = log.lock().await;
                    log.append(stream, &buf[..n]);
                }
                Err(_) => break,
            }
        }
    })
}

fn spawn_monitor_task(
    child: Arc<AsyncMutex<Child>>,
    state: Arc<RwLock<BackgroundProcessState>>,
    running_count: Arc<AtomicU64>,
    session_handle: Arc<StdMutex<Option<Weak<Session>>>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let mut guard = child.lock().await;
            match guard.try_wait() {
                Ok(Some(status)) => {
                    let finished_at = SystemTime::now();
                    let exit_code = status.code();
                    #[cfg(unix)]
                    let signal = status.signal();
                    #[cfg(not(unix))]
                    let signal = None;
                    drop(guard);
                    {
                        let mut state_guard = state.write().await;
                        *state_guard = BackgroundProcessState::Exited {
                            exit_code,
                            signal,
                            finished_at,
                        };
                    }
                    break;
                }
                Ok(None) => {
                    drop(guard);
                }
                Err(err) => {
                    drop(guard);
                    {
                        let mut state_guard = state.write().await;
                        *state_guard = BackgroundProcessState::Failed {
                            message: err.to_string(),
                            finished_at: SystemTime::now(),
                        };
                    }
                    break;
                }
            }
            tokio::time::sleep(WAIT_POLL_INTERVAL).await;
        }

        let _ = running_count.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_sub(1)
        });
        notify_running_count(&session_handle, &running_count).await;
    })
}

async fn notify_running_count(
    session_handle: &Arc<StdMutex<Option<Weak<Session>>>>,
    running_count: &Arc<AtomicU64>,
) {
    let weak_session = {
        let guard = session_handle
            .lock()
            .expect("background process session_handle lock poisoned");
        guard.clone()
    };

    if let Some(weak) = weak_session
        && let Some(session) = weak.upgrade()
    {
        let running = running_count.load(Ordering::SeqCst);
        session.notify_background_process_count(running).await;
    }
}

pub(crate) fn make_exec_context_for_background(
    sub_id: String,
    call_id: String,
    command_for_display: Vec<String>,
    cwd: PathBuf,
) -> ExecCommandContext {
    ExecCommandContext {
        sub_id,
        call_id,
        command_for_display,
        cwd,
        apply_patch: None,
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct BackgroundProcessInvocation {
    pub(crate) action: BackgroundProcessAction,
    #[serde(default)]
    pub(crate) command: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) cwd: Option<String>,
    #[serde(default)]
    pub(crate) env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub(crate) process_id: Option<String>,
    #[serde(default)]
    pub(crate) with_escalated_permissions: Option<bool>,
    #[serde(default)]
    pub(crate) justification: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackgroundProcessAction {
    Start,
    List,
    Logs,
    Kill,
}

pub(crate) fn system_time_to_unix_millis(time: SystemTime) -> Option<u128> {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .map(|dur| dur.as_millis())
}

pub(crate) fn background_state_to_json(state: &BackgroundProcessState) -> serde_json::Value {
    match state {
        BackgroundProcessState::Running => serde_json::json!({
            "status": "running",
        }),
        BackgroundProcessState::Exited {
            exit_code,
            signal,
            finished_at,
        } => serde_json::json!({
            "status": "exited",
            "exit_code": exit_code,
            "signal": signal,
            "finished_at_ms": system_time_to_unix_millis(*finished_at),
        }),
        BackgroundProcessState::Failed {
            message,
            finished_at,
        } => serde_json::json!({
            "status": "failed",
            "message": message,
            "finished_at_ms": system_time_to_unix_millis(*finished_at),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn system_time_converts_to_millis() {
        let ts = SystemTime::UNIX_EPOCH + Duration::from_millis(1234);
        assert_eq!(system_time_to_unix_millis(ts), Some(1234));
    }

    #[test]
    fn running_state_serializes() {
        let value = background_state_to_json(&BackgroundProcessState::Running);
        assert_eq!(value, serde_json::json!({"status": "running"}));
    }

    #[test]
    fn exited_state_serializes_with_metadata() {
        let finished_at = SystemTime::UNIX_EPOCH + Duration::from_secs(42);
        let value = background_state_to_json(&BackgroundProcessState::Exited {
            exit_code: Some(0),
            signal: None,
            finished_at,
        });
        assert_eq!(
            value,
            serde_json::json!({
                "status": "exited",
                "exit_code": 0,
                "signal": null,
                "finished_at_ms": Some(42_000),
            })
        );
    }

    #[test]
    fn failed_state_serializes() {
        let finished_at = SystemTime::UNIX_EPOCH + Duration::from_secs(5);
        let value = background_state_to_json(&BackgroundProcessState::Failed {
            message: "boom".to_string(),
            finished_at,
        });
        assert_eq!(
            value,
            serde_json::json!({
                "status": "failed",
                "message": "boom",
                "finished_at_ms": Some(5_000),
            })
        );
    }
}
