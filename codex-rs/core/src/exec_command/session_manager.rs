use std::collections::HashMap;
use std::fmt;
use std::io::ErrorKind;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use chrono::Utc;
use dirs::cache_dir;
use portable_pty::CommandBuilder;
use portable_pty::PtySize;
use portable_pty::native_pty_system;
use sha2::Digest;
use sha2::Sha256;
use tokio::fs::File as TokioFile;
use tokio::fs::{self};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;

use crate::exec_command::exec_command_params::ExecCommandParams;
use crate::exec_command::exec_command_params::WriteStdinParams;
use crate::exec_command::exec_command_session::ExecCommandSession;
use crate::exec_command::session_id::SessionId;
use crate::truncate::truncate_middle;

use super::control::ExecControlAction;
use super::control::ExecControlParams;
use super::control::ExecControlResponse;
use super::control::ExecControlStatus;

const DEFAULT_IDLE_TIMEOUT_MS: u64 = 300_000;
const MIN_IDLE_TIMEOUT_MS: u64 = 1_000;
const MAX_IDLE_TIMEOUT_MS: u64 = 86_400_000; // 24h
const DEFAULT_HARD_TIMEOUT_MS: Option<u64> = Some(7_200_000); // 2h
const MIN_GRACE_MS: u64 = 500;
const MAX_GRACE_MS: u64 = 60_000;
const IDLE_WATCH_INTERVAL: Duration = Duration::from_secs(1);
const PRUNE_AFTER_MS: u64 = 600_000;

#[derive(Debug, Clone)]
pub struct SessionManager {
    inner: Arc<SessionRegistry>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(SessionRegistry {
                next_session_id: AtomicU32::new(0),
                sessions: Mutex::new(HashMap::new()),
            }),
        }
    }
}

impl SessionManager {
    pub async fn handle_exec_command_request(
        &self,
        params: ExecCommandParams,
    ) -> Result<ExecCommandOutput, String> {
        self.inner.prune_finished().await;

        let session_id = SessionId(self.inner.next_session_id.fetch_add(1, Ordering::SeqCst));

        let (session, mut output_rx, mut exit_rx) = create_exec_command_session(params.clone())
            .await
            .map_err(|err| {
                format!(
                    "failed to create exec command session for session id {}: {err}",
                    session_id.0
                )
            })?;

        let idle_timeout = params
            .idle_timeout_ms
            .map(|ms| clamp(ms, MIN_IDLE_TIMEOUT_MS, MAX_IDLE_TIMEOUT_MS))
            .unwrap_or(DEFAULT_IDLE_TIMEOUT_MS);
        let hard_timeout = params
            .hard_timeout_ms
            .or(DEFAULT_HARD_TIMEOUT_MS)
            .map(Duration::from_millis);
        let grace_period =
            Duration::from_millis(clamp(params.grace_period_ms, MIN_GRACE_MS, MAX_GRACE_MS));
        let log_threshold = params.log_threshold_bytes.clamp(1_024, 4 * 1024 * 1024) as usize;

        let managed_session = ManagedSession::new(
            session_id,
            params.cmd.clone(),
            session,
            Duration::from_millis(idle_timeout),
            hard_timeout,
            grace_period,
            log_threshold,
        );

        let managed_session = Arc::new(managed_session);
        self.inner
            .sessions
            .lock()
            .await
            .insert(session_id, Arc::clone(&managed_session));
        managed_session
            .start_watchdogs(Arc::clone(&self.inner))
            .await;

        // Collect output
        let cap_bytes_u64 = params.max_output_tokens.saturating_mul(4);
        let cap_bytes: usize = cap_bytes_u64.min(usize::MAX as u64) as usize;
        let mut collected: Vec<u8> = Vec::with_capacity(4096);

        let start_time = Instant::now();
        let deadline = start_time + Duration::from_millis(params.yield_time_ms);
        let mut exit_code: Option<i32> = None;

        loop {
            if Instant::now() >= deadline {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            tokio::select! {
                biased;
                exit = &mut exit_rx => {
                    exit_code = exit.ok();
                    let grace_deadline = Instant::now() + Duration::from_millis(25);
                    while Instant::now() < grace_deadline {
                        match timeout(Duration::from_millis(1), output_rx.recv()).await {
                            Ok(Ok(chunk)) => {
                                managed_session.record_activity().await;
                                if let Err(err) = managed_session.append_log(&chunk).await {
                                    tracing::error!("failed to append log: {err}");
                                }
                                collected.extend_from_slice(&chunk);
                            }
                            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                                continue;
                            }
                            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                            Err(_) => break,
                        }
                    }
                    break;
                }
                chunk = timeout(remaining, output_rx.recv()) => {
                    match chunk {
                        Ok(Ok(chunk)) => {
                            managed_session.record_activity().await;
                            if let Err(err) = managed_session.append_log(&chunk).await {
                                tracing::error!("failed to append log: {err}");
                            }
                            collected.extend_from_slice(&chunk);
                        }
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                            break;
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        let output = String::from_utf8_lossy(&collected).to_string();
        let (output, original_token_count) = truncate_middle(&output, cap_bytes);
        let wall_time = Instant::now().duration_since(start_time);
        managed_session.increment_output_bytes(collected.len() as u64);

        let snapshot = managed_session.log_snapshot().await;

        let exit_status = if let Some(code) = exit_code {
            managed_session
                .mark_terminated(TerminationReason::Completed { exit_code: code })
                .await;
            self.inner.remove_session(session_id).await;
            ExitStatus::Exited(code)
        } else {
            ExitStatus::Ongoing(session_id)
        };

        Ok(ExecCommandOutput {
            wall_time,
            exit_status,
            original_token_count,
            output,
            log_path: snapshot.log_path,
            log_sha256: snapshot.log_sha256,
            total_output_bytes: snapshot.total_bytes,
        })
    }

    pub async fn handle_write_stdin_request(
        &self,
        params: WriteStdinParams,
    ) -> Result<ExecCommandOutput, String> {
        self.inner.prune_finished().await;
        let session = {
            let sessions = self.inner.sessions.lock().await;
            sessions.get(&params.session_id).cloned()
        };

        let Some(session) = session else {
            return Err(format!("unknown session id {}", params.session_id.0));
        };

        let WriteStdinParams {
            session_id,
            chars,
            yield_time_ms,
            max_output_tokens,
        } = params;

        if !chars.is_empty() && session.write_to_stdin(chars.into_bytes()).await.is_err() {
            return Err("failed to write to stdin".to_string());
        }

        let cap_bytes_u64 = max_output_tokens.saturating_mul(4);
        let cap_bytes: usize = cap_bytes_u64.min(usize::MAX as u64) as usize;
        let mut collected: Vec<u8> = Vec::with_capacity(4096);
        let start_time = Instant::now();
        let deadline = start_time + Duration::from_millis(yield_time_ms);
        let mut output_rx = session.output_receiver();

        loop {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let remaining = deadline - now;
            match timeout(remaining, output_rx.recv()).await {
                Ok(Ok(chunk)) => {
                    session.record_activity().await;
                    if let Err(err) = session.append_log(&chunk).await {
                        tracing::error!("failed to append log: {err}");
                    }
                    collected.extend_from_slice(&chunk);
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
                Err(_) => break,
            }
        }

        session.increment_output_bytes(collected.len() as u64);

        let output = String::from_utf8_lossy(&collected).to_string();
        let (output, original_token_count) = truncate_middle(&output, cap_bytes);
        let wall_time = Instant::now().duration_since(start_time);
        let snapshot = session.log_snapshot().await;

        let exit_status = if session.session.has_exited() {
            session
                .mark_terminated(TerminationReason::Completed { exit_code: 0 })
                .await;
            self.inner.remove_session(session_id).await;
            ExitStatus::Exited(0)
        } else {
            ExitStatus::Ongoing(session_id)
        };

        Ok(ExecCommandOutput {
            wall_time,
            exit_status,
            original_token_count,
            output,
            log_path: snapshot.log_path,
            log_sha256: snapshot.log_sha256,
            total_output_bytes: snapshot.total_bytes,
        })
    }

    pub async fn handle_exec_control_request(
        &self,
        params: ExecControlParams,
    ) -> ExecControlResponse {
        self.inner.prune_finished().await;

        let session = {
            let sessions = self.inner.sessions.lock().await;
            sessions.get(&params.session_id).cloned()
        };

        let Some(session) = session else {
            return ExecControlResponse {
                session_id: params.session_id,
                status: ExecControlStatus::NoSuchSession,
                note: None,
            };
        };

        if session.is_terminated() {
            return ExecControlResponse {
                session_id: params.session_id,
                status: ExecControlStatus::AlreadyTerminated,
                note: session.termination_note().await,
            };
        }

        let status = match params.action {
            ExecControlAction::Keepalive { extend_timeout_ms } => {
                session.keepalive(extend_timeout_ms).await;
                ExecControlStatus::ack()
            }
            ExecControlAction::SendCtrlC => {
                if session.send_ctrl_c().await {
                    ExecControlStatus::ack()
                } else {
                    ExecControlStatus::reject("failed to send ctrl-c")
                }
            }
            ExecControlAction::Terminate => {
                session.mark_grace(TerminationReason::UserRequested).await;
                if session.send_ctrl_c().await {
                    ExecControlStatus::ack()
                } else {
                    ExecControlStatus::reject("failed to signal process")
                }
            }
            ExecControlAction::ForceKill => session
                .force_kill(TerminationReason::ForceKilled)
                .await
                .map_or_else(ExecControlStatus::reject, |_| ExecControlStatus::ack()),
            ExecControlAction::SetIdleTimeout { timeout_ms } => {
                session.set_idle_timeout(timeout_ms).await;
                ExecControlStatus::ack()
            }
        };

        ExecControlResponse {
            session_id: params.session_id,
            status,
            note: session.termination_note().await,
        }
    }

    pub async fn list_sessions(&self) -> Vec<ExecSessionSummary> {
        self.inner.prune_finished().await;
        let sessions = self.inner.sessions.lock().await;
        let mut summaries: Vec<_> = sessions.values().map(|session| session.summary()).collect();
        summaries.sort_by(|a, b| a.session_id.0.cmp(&b.session_id.0));
        summaries
    }
}

#[derive(Debug)]
pub struct ExecCommandOutput {
    wall_time: Duration,
    exit_status: ExitStatus,
    original_token_count: Option<u64>,
    output: String,
    log_path: Option<PathBuf>,
    log_sha256: Option<String>,
    total_output_bytes: u64,
}

impl ExecCommandOutput {
    pub(crate) fn to_text_output(&self) -> String {
        let wall_time_secs = self.wall_time.as_secs_f32();
        let termination_status = match self.exit_status {
            ExitStatus::Exited(code) => format!("Process exited with code {code}"),
            ExitStatus::Ongoing(session_id) => {
                format!("Process running with session ID {}", session_id.0)
            }
        };
        let truncation_status = match self.original_token_count {
            Some(tokens) => {
                format!("\nWarning: truncated output (original token count: {tokens})")
            }
            None => "".to_string(),
        };
        let log_status = match (&self.log_path, &self.log_sha256) {
            (Some(path), Some(hash)) => format!(
                "\nLog: {path:?} (sha256:{hash}, total_bytes:{})",
                self.total_output_bytes
            ),
            _ => String::new(),
        };
        format!(
            r#"Wall time: {wall_time_secs:.3} seconds
{termination_status}{truncation_status}{log_status}
Output:
{output}"#,
            output = self.output
        )
    }
}

#[derive(Debug)]
pub enum ExitStatus {
    Exited(i32),
    Ongoing(SessionId),
}

#[derive(Debug)]
struct SessionRegistry {
    next_session_id: AtomicU32,
    sessions: Mutex<HashMap<SessionId, Arc<ManagedSession>>>,
}

impl SessionRegistry {
    async fn remove_session(&self, session_id: SessionId) {
        self.sessions.lock().await.remove(&session_id);
    }

    async fn prune_finished(&self) {
        let mut sessions = self.sessions.lock().await;
        let now = Instant::now();
        sessions.retain(|_, session| !session.prunable(now));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLifecycle {
    Running,
    Grace,
    Terminated,
}

impl fmt::Display for SessionLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionLifecycle::Running => write!(f, "running"),
            SessionLifecycle::Grace => write!(f, "grace"),
            SessionLifecycle::Terminated => write!(f, "terminated"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecSessionSummary {
    pub session_id: SessionId,
    pub command_preview: String,
    pub state: SessionLifecycle,
    pub uptime_ms: u128,
    pub idle_remaining_ms: Option<u128>,
    pub total_output_bytes: u64,
    pub log_path: Option<PathBuf>,
}

#[derive(Debug)]
struct ManagedSession {
    session_id: SessionId,
    command: String,
    session: ExecCommandSession,
    writer_tx: mpsc::Sender<Vec<u8>>,
    created_at: Instant,
    last_activity: Mutex<Instant>,
    idle_timeout: Mutex<Duration>,
    hard_deadline: Mutex<Option<Instant>>,
    grace_period: Duration,
    state: AtomicU8,
    termination: Mutex<Option<TerminationRecord>>,
    log: Mutex<LogDescriptor>,
    output_bytes: AtomicU64,
    watchers: Mutex<Vec<JoinHandle<()>>>,
}

impl ManagedSession {
    fn new(
        session_id: SessionId,
        command: String,
        session: ExecCommandSession,
        idle_timeout: Duration,
        hard_deadline: Option<Duration>,
        grace_period: Duration,
        log_threshold: usize,
    ) -> Self {
        let now = Instant::now();
        let writer_tx = session.writer_sender();
        let hard_deadline_instant = hard_deadline.map(|d| now + d);
        Self {
            session_id,
            command,
            session,
            writer_tx,
            created_at: now,
            last_activity: Mutex::new(now),
            idle_timeout: Mutex::new(idle_timeout),
            hard_deadline: Mutex::new(hard_deadline_instant),
            grace_period,
            state: AtomicU8::new(SessionState::RUNNING),
            termination: Mutex::new(None),
            log: Mutex::new(LogDescriptor::new(log_threshold, session_id)),
            output_bytes: AtomicU64::new(0),
            watchers: Mutex::new(Vec::new()),
        }
    }

    async fn start_watchdogs(self: &Arc<Self>, registry: Arc<SessionRegistry>) {
        let idle_session = Arc::clone(self);
        let idle_registry = Arc::clone(&registry);
        let idle_handle = tokio::spawn(async move {
            loop {
                if idle_session.is_terminated() {
                    break;
                }
                let idle_timeout = idle_session.current_idle_timeout().await;
                let since_activity = idle_session.since_last_activity().await;
                if since_activity >= idle_timeout {
                    idle_session
                        .mark_grace(TerminationReason::IdleTimeout { idle_timeout })
                        .await;
                    let _ = idle_session.send_ctrl_c().await;
                    sleep(idle_session.grace_period).await;
                    if !idle_session.is_terminated() {
                        let _ = idle_session
                            .force_kill(TerminationReason::IdleTimeout { idle_timeout })
                            .await;
                    }
                    idle_registry.prune_finished().await;
                    break;
                }
                sleep(IDLE_WATCH_INTERVAL).await;
            }
        });

        let mut handles = self.watchers.lock().await;
        handles.push(idle_handle);

        if let Some(deadline) = *self.hard_deadline.lock().await {
            let hard_session = Arc::clone(self);
            let hard_registry = Arc::clone(&registry);
            handles.push(tokio::spawn(async move {
                let now = Instant::now();
                if deadline > now {
                    sleep(deadline - now).await;
                }
                if hard_session.is_terminated() {
                    return;
                }
                hard_session
                    .mark_grace(TerminationReason::HardTimeout)
                    .await;
                let _ = hard_session.send_ctrl_c().await;
                sleep(hard_session.grace_period).await;
                if !hard_session.is_terminated() {
                    let _ = hard_session
                        .force_kill(TerminationReason::HardTimeout)
                        .await;
                }
                hard_registry.prune_finished().await;
            }));
        }
    }

    async fn record_activity(&self) {
        let mut guard = self.last_activity.lock().await;
        *guard = Instant::now();
    }

    async fn since_last_activity(&self) -> Duration {
        let guard = self.last_activity.lock().await;
        Instant::now().saturating_duration_since(*guard)
    }

    async fn current_idle_timeout(&self) -> Duration {
        *self.idle_timeout.lock().await
    }

    async fn log_snapshot(&self) -> LogSnapshot {
        self.log.lock().await.snapshot()
    }

    async fn append_log(&self, chunk: &[u8]) -> std::io::Result<()> {
        self.log.lock().await.append(chunk).await
    }

    fn increment_output_bytes(&self, delta: u64) {
        self.output_bytes.fetch_add(delta, Ordering::SeqCst);
    }

    async fn mark_terminated(&self, reason: TerminationReason) {
        self.state.store(SessionState::TERMINATED, Ordering::SeqCst);
        let mut guard = self.termination.lock().await;
        *guard = Some(TerminationRecord {
            reason,
            at: Instant::now(),
        });
    }

    async fn mark_grace(&self, reason: TerminationReason) {
        let _ = self.state.compare_exchange(
            SessionState::RUNNING,
            SessionState::GRACE,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        let mut guard = self.termination.lock().await;
        *guard = Some(TerminationRecord {
            reason,
            at: Instant::now(),
        });
    }

    async fn write_to_stdin(&self, bytes: Vec<u8>) -> Result<(), ()> {
        self.writer_tx.send(bytes).await.map_err(|_| ())
    }

    fn output_receiver(&self) -> tokio::sync::broadcast::Receiver<Vec<u8>> {
        self.session.output_receiver()
    }

    async fn keepalive(&self, extend_timeout_ms: Option<u64>) {
        self.record_activity().await;
        if let Some(ms) = extend_timeout_ms {
            let ms = clamp(ms, MIN_IDLE_TIMEOUT_MS, MAX_IDLE_TIMEOUT_MS);
            let mut guard = self.idle_timeout.lock().await;
            *guard = Duration::from_millis(ms);
        }
    }

    async fn set_idle_timeout(&self, timeout_ms: u64) {
        let ms = clamp(timeout_ms, MIN_IDLE_TIMEOUT_MS, MAX_IDLE_TIMEOUT_MS);
        let mut guard = self.idle_timeout.lock().await;
        *guard = Duration::from_millis(ms);
    }

    async fn send_ctrl_c(&self) -> bool {
        self.writer_tx.send(vec![0x03]).await.is_ok()
    }

    async fn force_kill(&self, reason: TerminationReason) -> Result<(), String> {
        self.session.force_kill()?;
        self.mark_terminated(reason).await;
        Ok(())
    }

    fn is_terminated(&self) -> bool {
        self.state.load(Ordering::SeqCst) == SessionState::TERMINATED
    }

    fn summary(&self) -> ExecSessionSummary {
        let state = self.state();
        let uptime = Instant::now().saturating_duration_since(self.created_at);
        let idle_remaining = self.idle_timeout.try_lock().ok().and_then(|timeout| {
            self.last_activity
                .try_lock()
                .ok()
                .map(|last| timeout.saturating_sub(Instant::now().saturating_duration_since(*last)))
        });
        ExecSessionSummary {
            session_id: self.session_id,
            command_preview: preview_command(&self.command, 80),
            state,
            uptime_ms: uptime.as_millis(),
            idle_remaining_ms: idle_remaining.map(|d| d.as_millis()),
            total_output_bytes: self.output_bytes.load(Ordering::SeqCst),
            log_path: self
                .log
                .try_lock()
                .ok()
                .and_then(|log| log.snapshot().log_path),
        }
    }

    fn state(&self) -> SessionLifecycle {
        match self.state.load(Ordering::SeqCst) {
            SessionState::RUNNING => SessionLifecycle::Running,
            SessionState::GRACE => SessionLifecycle::Grace,
            _ => SessionLifecycle::Terminated,
        }
    }

    async fn termination_note(&self) -> Option<String> {
        let record = self.termination.lock().await;
        record.as_ref().map(|r| r.reason.to_string())
    }

    fn prunable(&self, now: Instant) -> bool {
        if let Ok(record) = self.termination.try_lock()
            && let Some(record) = &*record
        {
            return now
                .checked_duration_since(record.at)
                .map(|dur| dur.as_millis() as u64 > PRUNE_AFTER_MS)
                .unwrap_or(false);
        }
        false
    }
}

#[derive(Debug, Clone)]
struct TerminationRecord {
    reason: TerminationReason,
    at: Instant,
}

#[derive(Debug, Clone)]
enum TerminationReason {
    Completed { exit_code: i32 },
    IdleTimeout { idle_timeout: Duration },
    HardTimeout,
    UserRequested,
    ForceKilled,
}

impl std::fmt::Display for TerminationReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminationReason::Completed { exit_code } => {
                write!(f, "completed (exit_code={exit_code})")
            }
            TerminationReason::IdleTimeout { idle_timeout } => {
                write!(f, "idle_timeout (timeout={}s)", idle_timeout.as_secs())
            }
            TerminationReason::HardTimeout => write!(f, "hard_timeout"),
            TerminationReason::UserRequested => write!(f, "user_requested"),
            TerminationReason::ForceKilled => write!(f, "force_killed"),
        }
    }
}

#[derive(Debug, Default, Clone)]
struct LogSnapshot {
    log_path: Option<PathBuf>,
    log_sha256: Option<String>,
    total_bytes: u64,
}

#[derive(Debug)]
struct LogDescriptor {
    threshold: usize,
    buffer: Vec<u8>,
    file: Option<LogFile>,
    hasher: Sha256,
    total_bytes: u64,
    session_label: String,
}

impl LogDescriptor {
    fn new(threshold: usize, session_id: SessionId) -> Self {
        Self {
            threshold,
            buffer: Vec::with_capacity(threshold),
            file: None,
            hasher: Sha256::new(),
            total_bytes: 0,
            session_label: format!("session-{:08}", session_id.0),
        }
    }

    async fn append(&mut self, chunk: &[u8]) -> std::io::Result<()> {
        self.total_bytes = self.total_bytes.saturating_add(chunk.len() as u64);
        self.hasher.update(chunk);
        if let Some(file) = &mut self.file {
            file.write(chunk).await?;
            return Ok(());
        }

        if self.buffer.len() + chunk.len() <= self.threshold {
            self.buffer.extend_from_slice(chunk);
            return Ok(());
        }

        let mut file = LogFile::create(&self.session_label).await?;
        file.write(&self.buffer).await?;
        file.write(chunk).await?;
        self.file = Some(file);
        self.buffer.clear();
        Ok(())
    }

    fn snapshot(&self) -> LogSnapshot {
        let hasher = self.hasher.clone();
        let digest = hasher.finalize();
        let mut hash_hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write;
            let _ = write!(&mut hash_hex, "{byte:02x}");
        }
        LogSnapshot {
            log_path: self.file.as_ref().map(|file| file.path.clone()),
            log_sha256: if self.total_bytes > 0 {
                Some(hash_hex)
            } else {
                None
            },
            total_bytes: self.total_bytes,
        }
    }
}

#[derive(Debug)]
struct LogFile {
    path: PathBuf,
    file: TokioFile,
}

impl LogFile {
    async fn create(label: &str) -> std::io::Result<Self> {
        let base = log_base_dir().await?;
        let filename = format!("{label}-{}.ansi", Utc::now().format("%Y%m%dT%H%M%S%.3fZ"));
        let path = base.join(filename);
        let file = TokioFile::create(&path).await?;
        Ok(Self { path, file })
    }

    async fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.file.write_all(bytes).await
    }
}

async fn log_base_dir() -> std::io::Result<PathBuf> {
    let dir = cache_dir()
        .map(|p| p.join("codex").join("exec_logs"))
        .unwrap_or_else(|| std::env::temp_dir().join("codex-exec-logs"));
    fs::create_dir_all(&dir).await?;
    Ok(dir)
}

fn preview_command(cmd: &str, max: usize) -> String {
    if cmd.len() <= max {
        return cmd.to_string();
    }
    let keep = max / 2;
    format!("{}…{}", &cmd[..keep], &cmd[cmd.len() - keep..])
}

fn clamp(value: u64, min: u64, max: u64) -> u64 {
    value.min(max).max(min)
}

struct SessionState;
impl SessionState {
    const RUNNING: u8 = 0;
    const GRACE: u8 = 1;
    const TERMINATED: u8 = 2;
}

async fn create_exec_command_session(
    params: ExecCommandParams,
) -> anyhow::Result<(
    ExecCommandSession,
    tokio::sync::broadcast::Receiver<Vec<u8>>,
    oneshot::Receiver<i32>,
)> {
    let ExecCommandParams {
        cmd,
        yield_time_ms: _,
        max_output_tokens: _,
        shell,
        login,
        idle_timeout_ms: _,
        hard_timeout_ms: _,
        grace_period_ms: _,
        log_threshold_bytes: _,
    } = params;

    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut command_builder = CommandBuilder::new(shell);
    let shell_mode_opt = if login { "-lc" } else { "-c" };
    command_builder.arg(shell_mode_opt);
    command_builder.arg(cmd);

    let mut child = pair.slave.spawn_command(command_builder)?;
    let killer = child.clone_killer();

    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(256);

    let mut reader = pair.master.try_clone_reader()?;
    let output_tx_clone = output_tx.clone();
    let reader_handle = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = output_tx_clone.send(buf[..n].to_vec());
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(_) => break,
            }
        }
    });

    let writer = pair.master.take_writer()?;
    let writer = Arc::new(StdMutex::new(writer));
    let writer_handle = tokio::spawn({
        let writer = writer.clone();
        async move {
            while let Some(bytes) = writer_rx.recv().await {
                let writer = writer.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    if let Ok(mut guard) = writer.lock() {
                        use std::io::Write;
                        let _ = guard.write_all(&bytes);
                        let _ = guard.flush();
                    }
                })
                .await;
            }
        }
    });

    let (exit_tx, exit_rx) = oneshot::channel::<i32>();
    let exit_status = Arc::new(AtomicBool::new(false));
    let wait_exit_status = exit_status.clone();
    let wait_handle = tokio::task::spawn_blocking(move || {
        let code = match child.wait() {
            Ok(status) => status.exit_code() as i32,
            Err(_) => -1,
        };
        wait_exit_status.store(true, Ordering::SeqCst);
        let _ = exit_tx.send(code);
    });

    let (session, initial_output_rx) = ExecCommandSession::new(
        writer_tx,
        output_tx,
        killer,
        reader_handle,
        writer_handle,
        wait_handle,
        exit_status,
    );
    Ok((session, initial_output_rx, exit_rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec_command::ExecControlAction;
    use crate::exec_command::ExecControlParams;
    use crate::exec_command::ExecControlStatus;

    const TEST_SHELL: &str = "/bin/bash";

    fn base_params(cmd: &str) -> ExecCommandParams {
        ExecCommandParams {
            cmd: cmd.to_string(),
            yield_time_ms: 250,
            max_output_tokens: 1024,
            shell: TEST_SHELL.to_string(),
            login: false,
            idle_timeout_ms: Some(1_000),
            hard_timeout_ms: Some(2_000),
            grace_period_ms: 200,
            log_threshold_bytes: 1_024,
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn idle_timeout_terminates_session() {
        let manager = SessionManager::default();
        let params = base_params("sleep 5");
        let output = match manager.handle_exec_command_request(params).await {
            Ok(output) => output,
            Err(err) => panic!("exec start failed: {err}"),
        };
        let session_id = match output.exit_status {
            ExitStatus::Ongoing(id) => id,
            ExitStatus::Exited(code) => panic!("session exited early {code}"),
        };

        tokio::time::sleep(Duration::from_millis(1_600)).await;

        let list = manager.list_sessions().await;
        let summary = match list.into_iter().find(|s| s.session_id == session_id) {
            Some(summary) => summary,
            None => panic!("session summary not found"),
        };
        assert!(matches!(
            summary.state,
            SessionLifecycle::Grace | SessionLifecycle::Terminated
        ));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn keepalive_extends_session() {
        let manager = SessionManager::default();
        let params = ExecCommandParams {
            idle_timeout_ms: Some(1_000),
            hard_timeout_ms: Some(4_000),
            grace_period_ms: 200,
            ..base_params("sleep 6")
        };
        let output = match manager.handle_exec_command_request(params).await {
            Ok(output) => output,
            Err(err) => panic!("exec start failed: {err}"),
        };
        let session_id = match output.exit_status {
            ExitStatus::Ongoing(id) => id,
            ExitStatus::Exited(code) => panic!("session exited early {code}"),
        };

        tokio::time::sleep(Duration::from_millis(700)).await;
        let control = ExecControlParams {
            session_id,
            action: ExecControlAction::Keepalive {
                extend_timeout_ms: Some(2_000),
            },
        };
        let resp = manager.handle_exec_control_request(control).await;
        assert!(matches!(resp.status, ExecControlStatus::Ack));

        tokio::time::sleep(Duration::from_millis(1_500)).await;

        let list = manager.list_sessions().await;
        let summary = match list.into_iter().find(|s| s.session_id == session_id) {
            Some(summary) => summary,
            None => panic!("session summary not found"),
        };
        assert_eq!(summary.state, SessionLifecycle::Running);

        // Clean up to avoid lingering processes.
        let _ = manager
            .handle_exec_control_request(ExecControlParams {
                session_id,
                action: ExecControlAction::ForceKill,
            })
            .await;
    }
}
