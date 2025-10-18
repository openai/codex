use portable_pty::CommandBuilder;
use portable_pty::PtySize;
use portable_pty::native_pty_system;
use rand::Rng;
use rand::distr::Alphanumeric;
use serde_json::Map as JsonMap;
use serde_json::Value as JsonValue;
use serde_json::json;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::io::ErrorKind;
use std::io::Read;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::Instant;

use crate::exec_command::ExecCommandSession;
use crate::exec_command::ExecCommandSessionParams;
use crate::truncate::truncate_middle;

mod errors;

pub use errors::UnifiedExecError;

const MIN_YIELD_TIME_MS: u64 = 250;
const MAX_YIELD_TIME_MS: u64 = 30_000;
const DEFAULT_EXEC_YIELD_TIME_MS: u64 = 10_000;
const DEFAULT_WRITE_YIELD_TIME_MS: u64 = 250;
const DEFAULT_MAX_OUTPUT_TOKENS: usize = 10_000;
const PIPE_READ_LIMIT: usize = 1024 * 1024;
const POST_WRITE_SETTLE_MS: u64 = 100;

#[derive(Debug, Clone)]
pub enum UnifiedExecMode<'a> {
    Start {
        cmd: Cow<'a, str>,
        yield_time_ms: Option<u64>,
        max_output_tokens: Option<usize>,
        shell: Option<&'a str>,
        login: Option<bool>,
        cwd: Option<&'a str>,
    },
    Write {
        session_id: i32,
        chars: &'a str,
        yield_time_ms: Option<u64>,
        max_output_tokens: Option<usize>,
    },
}

#[derive(Debug, Clone)]
pub struct UnifiedExecRequest<'a> {
    pub mode: UnifiedExecMode<'a>,
    pub output_chunk_id: Option<bool>,
    pub output_wall_time: Option<bool>,
    pub output_json: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct UnifiedExecResult {
    pub content: UnifiedExecContent,
    pub metadata: UnifiedExecMetadata,
}

#[derive(Debug, Clone)]
pub enum UnifiedExecContent {
    Text(String),
    Json(String),
}

impl UnifiedExecContent {
    pub fn into_string(self) -> String {
        match self {
            Self::Text(s) | Self::Json(s) => s,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Text(s) | Self::Json(s) => s,
        }
    }

    pub fn is_json(&self) -> bool {
        matches!(self, Self::Json(_))
    }
}

#[derive(Debug, Clone)]
pub struct UnifiedExecMetadata {
    pub chunk_id: String,
    pub session_id: Option<i32>,
    pub exit_code: Option<i32>,
    pub wall_time_seconds: f64,
    pub original_token_count: Option<u64>,
    pub exec_cmd: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct OutputPreferences {
    chunk_id: bool,
    wall_time: bool,
    json: bool,
}

impl OutputPreferences {
    fn from_request(request: &UnifiedExecRequest<'_>) -> Self {
        Self {
            chunk_id: request.output_chunk_id.unwrap_or(true),
            wall_time: request.output_wall_time.unwrap_or(true),
            json: request.output_json.unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StartSessionParams<'a> {
    cmd: &'a str,
    yield_time_ms: Option<u64>,
    max_output_tokens: Option<usize>,
    shell: Option<&'a str>,
    login: Option<bool>,
    cwd: Option<&'a str>,
    preferences: OutputPreferences,
}

#[derive(Debug)]
struct BuildResultParams {
    process_id: i32,
    exit_code: Option<i32>,
    truncated_tokens: Option<u64>,
    output: String,
    wall_time: f64,
    preferences: OutputPreferences,
    keep_session: bool,
    exec_cmd: Option<String>,
}

#[derive(Debug)]
pub struct UnifiedExecSessionManager {
    sessions: Mutex<HashMap<i32, Arc<ManagedUnifiedExecSession>>>,
}

impl Default for UnifiedExecSessionManager {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug)]
struct ManagedUnifiedExecSession {
    session: ExecCommandSession,
    output_rx: Mutex<broadcast::Receiver<Vec<u8>>>,
    process_id: i32,
}

impl ManagedUnifiedExecSession {
    fn new(
        session: ExecCommandSession,
        initial_output_rx: broadcast::Receiver<Vec<u8>>,
        process_id: i32,
    ) -> Self {
        Self {
            session,
            output_rx: Mutex::new(initial_output_rx),
            process_id,
        }
    }

    fn writer(&self) -> mpsc::Sender<Vec<u8>> {
        self.session.writer_sender()
    }

    fn exit_code(&self) -> Option<i32> {
        self.session.exit_code()
    }

    fn has_exited(&self) -> bool {
        self.session.has_exited()
    }
}

impl UnifiedExecSessionManager {
    pub async fn handle_request(
        &self,
        request: UnifiedExecRequest<'_>,
    ) -> Result<UnifiedExecResult, UnifiedExecError> {
        let preferences = OutputPreferences::from_request(&request);
        match request.mode {
            UnifiedExecMode::Start {
                cmd,
                yield_time_ms,
                max_output_tokens,
                shell,
                login,
                cwd,
            } => {
                self.start_session(StartSessionParams {
                    cmd: cmd.as_ref(),
                    yield_time_ms,
                    max_output_tokens,
                    shell,
                    login,
                    cwd,
                    preferences,
                })
                .await
            }
            UnifiedExecMode::Write {
                session_id,
                chars,
                yield_time_ms,
                max_output_tokens,
            } => {
                self.write_to_session(
                    session_id,
                    chars,
                    yield_time_ms,
                    max_output_tokens,
                    preferences,
                )
                .await
            }
        }
    }

    async fn start_session(
        &self,
        params: StartSessionParams<'_>,
    ) -> Result<UnifiedExecResult, UnifiedExecError> {
        let StartSessionParams {
            cmd,
            yield_time_ms,
            max_output_tokens,
            shell,
            login,
            cwd,
            preferences,
        } = params;
        if cmd.trim().is_empty() {
            return Err(UnifiedExecError::MissingCommandLine);
        }

        let yield_ms = clamp_yield_time(yield_time_ms, DEFAULT_EXEC_YIELD_TIME_MS);
        let max_tokens = normalize_max_output_tokens(max_output_tokens);
        let shell = shell.unwrap_or("/bin/bash");
        let login = login.unwrap_or(true);

        let (session, initial_output_rx, process_id) =
            create_unified_exec_session(cmd, shell, login, cwd).await?;
        let managed = Arc::new(ManagedUnifiedExecSession::new(
            session,
            initial_output_rx,
            process_id,
        ));

        let output_start = Instant::now();
        let (output, truncated_tokens) = collect_output(&managed, yield_ms, max_tokens).await;
        let wall_time = output_start.elapsed().as_secs_f64();
        let exit_code = managed.exit_code();
        let should_keep_session = exit_code.is_none();

        if should_keep_session {
            self.sessions
                .lock()
                .await
                .insert(managed.process_id, managed.clone());
        }

        Ok(build_result(BuildResultParams {
            process_id: managed.process_id,
            exit_code,
            truncated_tokens,
            output,
            wall_time,
            preferences,
            keep_session: should_keep_session,
            exec_cmd: Some(cmd.to_string()),
        }))
    }

    async fn write_to_session(
        &self,
        session_id: i32,
        chars: &str,
        yield_time_ms: Option<u64>,
        max_output_tokens: Option<usize>,
        preferences: OutputPreferences,
    ) -> Result<UnifiedExecResult, UnifiedExecError> {
        let managed = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(&session_id)
                .cloned()
                .ok_or(UnifiedExecError::UnknownSessionId { session_id })?
        };

        if managed.has_exited() {
            let exit_code = managed.exit_code();
            self.sessions.lock().await.remove(&session_id);
            return Err(UnifiedExecError::SessionExited {
                session_id,
                exit_code,
            });
        }

        if !chars.is_empty()
            && managed
                .writer()
                .send(chars.as_bytes().to_vec())
                .await
                .is_err()
        {
            self.sessions.lock().await.remove(&session_id);
            return Err(UnifiedExecError::WriteToStdin { session_id });
        }

        if !chars.is_empty() {
            tokio::time::sleep(Duration::from_millis(POST_WRITE_SETTLE_MS)).await;
        }

        let yield_ms = clamp_yield_time(yield_time_ms, DEFAULT_WRITE_YIELD_TIME_MS);
        let max_tokens = normalize_max_output_tokens(max_output_tokens);
        let output_start = Instant::now();
        let (output, truncated_tokens) = collect_output(&managed, yield_ms, max_tokens).await;
        let wall_time = output_start.elapsed().as_secs_f64();
        let exit_code = managed.exit_code();
        let should_keep_session = exit_code.is_none();

        if !should_keep_session {
            self.sessions.lock().await.remove(&session_id);
        }

        Ok(build_result(BuildResultParams {
            process_id: managed.process_id,
            exit_code,
            truncated_tokens,
            output,
            wall_time,
            preferences,
            keep_session: should_keep_session,
            exec_cmd: None,
        }))
    }
}

fn clamp_yield_time(value: Option<u64>, default_value: u64) -> u64 {
    let requested = value.unwrap_or(default_value);
    requested.clamp(MIN_YIELD_TIME_MS, MAX_YIELD_TIME_MS)
}

fn normalize_max_output_tokens(value: Option<usize>) -> usize {
    let requested = value.unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS);
    requested.max(1)
}

fn random_chunk_id() -> String {
    let mut rng = rand::rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric) as char)
        .take(6)
        .collect()
}

async fn collect_output(
    session: &ManagedUnifiedExecSession,
    yield_time_ms: u64,
    max_output_tokens: usize,
) -> (String, Option<u64>) {
    let deadline = Instant::now() + Duration::from_millis(yield_time_ms);
    let mut collected: Vec<u8> = Vec::with_capacity(4096);
    let mut receiver = session.output_rx.lock().await;

    let mut observed_output = false;
    loop {
        if collected.len() >= PIPE_READ_LIMIT {
            break;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, receiver.recv()).await {
            Ok(Ok(chunk)) => {
                push_limited(&mut collected, &chunk, PIPE_READ_LIMIT);
                observed_output = true;
            }
            Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                continue;
            }
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => {
                break;
            }
        }

        break;
    }

    if observed_output {
        while collected.len() < PIPE_READ_LIMIT {
            match receiver.try_recv() {
                Ok(chunk) => push_limited(&mut collected, &chunk, PIPE_READ_LIMIT),
                Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Closed) => break,
            }
        }
    }

    drop(receiver);

    let output = String::from_utf8_lossy(&collected).to_string();
    let cap_bytes = (max_output_tokens as u64)
        .saturating_mul(4)
        .min(PIPE_READ_LIMIT as u64) as usize;
    truncate_middle(&output, cap_bytes)
}

fn push_limited(buffer: &mut Vec<u8>, chunk: &[u8], limit: usize) {
    if buffer.len() >= limit {
        return;
    }
    let available = limit - buffer.len();
    if available == 0 {
        return;
    }
    if chunk.len() <= available {
        buffer.extend_from_slice(chunk);
    } else {
        buffer.extend_from_slice(&chunk[..available]);
    }
}

fn build_result(params: BuildResultParams) -> UnifiedExecResult {
    let BuildResultParams {
        process_id,
        exit_code,
        truncated_tokens,
        output,
        wall_time,
        preferences,
        keep_session,
        exec_cmd,
    } = params;
    let chunk_id = random_chunk_id();
    let content = if preferences.json {
        UnifiedExecContent::Json(build_json_body(
            &chunk_id,
            process_id,
            exit_code,
            truncated_tokens,
            wall_time,
            &output,
            preferences,
        ))
    } else {
        UnifiedExecContent::Text(build_text_body(
            &chunk_id,
            process_id,
            exit_code,
            truncated_tokens,
            wall_time,
            &output,
            preferences,
        ))
    };

    let metadata = UnifiedExecMetadata {
        chunk_id,
        session_id: if keep_session { Some(process_id) } else { None },
        exit_code,
        wall_time_seconds: wall_time,
        original_token_count: truncated_tokens,
        exec_cmd,
    };

    UnifiedExecResult { content, metadata }
}

fn build_text_body(
    chunk_id: &str,
    process_id: i32,
    exit_code: Option<i32>,
    truncated_tokens: Option<u64>,
    wall_time: f64,
    output: &str,
    preferences: OutputPreferences,
) -> String {
    let mut parts = Vec::new();
    if preferences.chunk_id {
        parts.push(format!("Chunk ID: {chunk_id}\n"));
    }
    if preferences.wall_time {
        parts.push(format!("Wall time: {wall_time:.3} seconds\n"));
    }
    match exit_code {
        Some(code) => parts.push(format!("Process exited with code {code}\n")),
        None => parts.push(format!("Process running with session ID {process_id}\n")),
    }
    if let Some(tokens) = truncated_tokens {
        parts.push(format!(
            "Warning: truncated output (original token count: {tokens})\n"
        ));
    }
    parts.push("Output:\n".to_string());
    parts.push(output.to_string());
    parts.concat()
}

fn build_json_body(
    chunk_id: &str,
    process_id: i32,
    exit_code: Option<i32>,
    truncated_tokens: Option<u64>,
    wall_time: f64,
    output: &str,
    preferences: OutputPreferences,
) -> String {
    let mut map: JsonMap<String, JsonValue> = JsonMap::new();

    if preferences.chunk_id {
        map.insert(
            "chunk_id".to_string(),
            JsonValue::String(chunk_id.to_string()),
        );
    }

    if preferences.wall_time {
        let rounded = (wall_time * 1000.0).round() / 1000.0;
        map.insert("wall_time".to_string(), json!(rounded));
    }

    if let Some(code) = exit_code {
        map.insert("exit_code".to_string(), json!(code));
    } else {
        map.insert("session_id".to_string(), json!(process_id));
    }

    if let Some(tokens) = truncated_tokens {
        map.insert("original_token_count".to_string(), json!(tokens));
    }

    let lines: JsonMap<String, JsonValue> = output
        .lines()
        .enumerate()
        .map(|(idx, line)| ((idx + 1).to_string(), JsonValue::String(line.to_string())))
        .collect();
    map.insert("output".to_string(), JsonValue::Object(lines));

    JsonValue::Object(map).to_string()
}

async fn create_unified_exec_session(
    cmd: &str,
    shell: &str,
    login: bool,
    cwd: Option<&str>,
) -> Result<(ExecCommandSession, broadcast::Receiver<Vec<u8>>, i32), UnifiedExecError> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(UnifiedExecError::create_session)?;

    let mut command_builder = CommandBuilder::new(shell);
    command_builder.arg(if login { "-lc" } else { "-c" });
    command_builder.arg(cmd);
    command_builder.env("NO_COLOR", "1");
    command_builder.env("TERM", "dumb");
    command_builder.env("LANG", "C.UTF-8");
    command_builder.env("LC_CTYPE", "C.UTF-8");
    command_builder.env("LC_ALL", "C.UTF-8");
    command_builder.env("COLORTERM", "");
    command_builder.env("PAGER", "cat");
    command_builder.env("GIT_PAGER", "cat");
    if let Some(dir) = cwd {
        command_builder.cwd(dir);
    }

    let mut child = pair
        .slave
        .spawn_command(command_builder)
        .map_err(UnifiedExecError::create_session)?;
    let killer = child.clone_killer();
    let raw_pid = child
        .process_id()
        .ok_or(UnifiedExecError::MissingProcessId)?;
    let process_id = i32::try_from(raw_pid).map_err(|_| UnifiedExecError::ProcessIdOverflow {
        process_id: raw_pid,
    })?;

    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(UnifiedExecError::create_session)?;
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

    let writer = pair
        .master
        .take_writer()
        .map_err(UnifiedExecError::create_session)?;
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

    let exit_status = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let wait_exit_status = Arc::clone(&exit_status);
    let exit_code = Arc::new(StdMutex::new(None));
    let exit_code_handle = Arc::clone(&exit_code);
    let wait_handle = tokio::task::spawn_blocking(move || {
        let result = child.wait();
        wait_exit_status.store(true, Ordering::SeqCst);
        let code = result
            .ok()
            .and_then(|status| i32::try_from(status.exit_code()).ok())
            .unwrap_or(-1);
        if let Ok(mut guard) = exit_code_handle.lock() {
            *guard = Some(code);
        }
    });

    let (session, initial_output_rx) = ExecCommandSession::new(ExecCommandSessionParams {
        writer_tx,
        output_tx,
        killer,
        reader_handle,
        writer_handle,
        wait_handle,
        exit_status,
        exit_code,
    });

    Ok((session, initial_output_rx, process_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use core_test_support::skip_if_sandbox;

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn start_and_poll_session() -> Result<(), UnifiedExecError> {
        skip_if_sandbox!(Ok(()));

        let manager = UnifiedExecSessionManager::default();
        let result = manager
            .handle_request(UnifiedExecRequest {
                mode: UnifiedExecMode::Start {
                    cmd: Cow::Borrowed("printf 'ready\\n' && read dummy"),
                    yield_time_ms: Some(1_000),
                    max_output_tokens: Some(1_000),
                    shell: Some("/bin/bash"),
                    login: Some(false),
                    cwd: None,
                },
                output_chunk_id: Some(true),
                output_wall_time: Some(true),
                output_json: Some(false),
            })
            .await?;

        assert!(result.metadata.session_id.is_some());
        assert!(result.content.as_str().contains("ready"));

        let session_id = result.metadata.session_id.unwrap();
        let poll = manager
            .handle_request(UnifiedExecRequest {
                mode: UnifiedExecMode::Write {
                    session_id,
                    chars: "",
                    yield_time_ms: Some(500),
                    max_output_tokens: Some(1_000),
                },
                output_chunk_id: Some(false),
                output_wall_time: Some(false),
                output_json: Some(true),
            })
            .await?;

        assert!(poll.content.is_json());
        assert!(poll.metadata.session_id.is_some());

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn session_cleans_up_after_exit() -> Result<(), UnifiedExecError> {
        skip_if_sandbox!(Ok(()));

        let manager = UnifiedExecSessionManager::default();
        let result = manager
            .handle_request(UnifiedExecRequest {
                mode: UnifiedExecMode::Start {
                    cmd: Cow::Borrowed("echo done"),
                    yield_time_ms: Some(1_000),
                    max_output_tokens: Some(1_000),
                    shell: Some("/bin/bash"),
                    login: Some(false),
                    cwd: None,
                },
                output_chunk_id: None,
                output_wall_time: None,
                output_json: Some(false),
            })
            .await?;

        if let Some(session_id) = result.metadata.session_id {
            tokio::time::sleep(Duration::from_millis(100)).await;
            match manager
                .handle_request(UnifiedExecRequest {
                    mode: UnifiedExecMode::Write {
                        session_id,
                        chars: "",
                        yield_time_ms: Some(250),
                        max_output_tokens: Some(1_000),
                    },
                    output_chunk_id: None,
                    output_wall_time: None,
                    output_json: Some(false),
                })
                .await
            {
                Ok(poll) => {
                    assert!(poll.metadata.session_id.is_none());
                    assert!(poll.content.into_string().contains("done"));
                }
                Err(UnifiedExecError::SessionExited { exit_code, .. }) => {
                    assert_eq!(exit_code, Some(0));
                }
                Err(other) => return Err(other),
            }
        } else {
            assert!(result.content.into_string().contains("done"));
        }

        Ok(())
    }
}
