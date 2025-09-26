use portable_pty::CommandBuilder;
use portable_pty::PtySize;
use portable_pty::native_pty_system;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::ErrorKind;
use std::io::Read;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio::time::Instant;

use crate::exec_command::ExecCommandSession;
use crate::truncate::truncate_middle;

mod errors;

pub(crate) use errors::UnifiedExecError;

const DEFAULT_TIMEOUT_MS: u64 = 1_000;
const MAX_TIMEOUT_MS: u64 = 60_000;
const UNIFIED_EXEC_OUTPUT_MAX_BYTES: usize = 128 * 1024; // 128 KiB

#[derive(Debug)]
pub(crate) struct UnifiedExecRequest<'a> {
    pub session_id: Option<i32>,
    pub input_chunks: &'a [String],
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UnifiedExecResult {
    pub session_id: Option<i32>,
    pub output: String,
}

#[derive(Debug, Default)]
pub(crate) struct UnifiedExecSessionManager {
    next_session_id: AtomicI32,
    sessions: Mutex<HashMap<i32, ManagedUnifiedExecSession>>,
}

#[derive(Debug)]
struct ManagedUnifiedExecSession {
    session: ExecCommandSession,
    output_buffer: OutputBuffer,
    /// Notifies waiters whenever new output has been appended to
    /// `output_buffer`, allowing clients to poll for fresh data.
    output_notify: Arc<Notify>,
    output_task: JoinHandle<()>,
}

#[derive(Debug, Default)]
struct OutputBufferState {
    chunks: VecDeque<Vec<u8>>,
    total_bytes: usize,
}

impl OutputBufferState {
    fn push_chunk(&mut self, chunk: Vec<u8>) {
        self.total_bytes = self.total_bytes.saturating_add(chunk.len());
        self.chunks.push_back(chunk);

        let mut excess = self
            .total_bytes
            .saturating_sub(UNIFIED_EXEC_OUTPUT_MAX_BYTES);

        while excess > 0 {
            match self.chunks.front_mut() {
                Some(front) if excess >= front.len() => {
                    excess -= front.len();
                    self.total_bytes = self.total_bytes.saturating_sub(front.len());
                    self.chunks.pop_front();
                }
                Some(front) => {
                    front.drain(..excess);
                    self.total_bytes = self.total_bytes.saturating_sub(excess);
                    break;
                }
                None => break,
            }
        }
    }

    fn drain(&mut self) -> Vec<Vec<u8>> {
        let drained: Vec<Vec<u8>> = self.chunks.drain(..).collect();
        self.total_bytes = 0;
        drained
    }
}

type OutputBuffer = Arc<Mutex<OutputBufferState>>;
type OutputHandles = (OutputBuffer, Arc<Notify>);

impl ManagedUnifiedExecSession {
    fn new(
        session: ExecCommandSession,
        initial_output_rx: tokio::sync::broadcast::Receiver<Vec<u8>>,
    ) -> Self {
        let output_buffer = Arc::new(Mutex::new(OutputBufferState::default()));
        let output_notify = Arc::new(Notify::new());
        let mut receiver = initial_output_rx;
        let buffer_clone = Arc::clone(&output_buffer);
        let notify_clone = Arc::clone(&output_notify);
        let output_task = tokio::spawn(async move {
            while let Ok(chunk) = receiver.recv().await {
                let mut guard = buffer_clone.lock().await;
                guard.push_chunk(chunk);
                drop(guard);
                notify_clone.notify_waiters();
            }
        });

        Self {
            session,
            output_buffer,
            output_notify,
            output_task,
        }
    }

    fn writer_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.session.writer_sender()
    }

    fn output_handles(&self) -> OutputHandles {
        (
            Arc::clone(&self.output_buffer),
            Arc::clone(&self.output_notify),
        )
    }

    fn has_exited(&self) -> bool {
        self.session.has_exited()
    }
}

impl Drop for ManagedUnifiedExecSession {
    fn drop(&mut self) {
        self.output_task.abort();
    }
}

impl UnifiedExecSessionManager {
    pub async fn handle_request(
        &self,
        request: UnifiedExecRequest<'_>,
    ) -> Result<UnifiedExecResult, UnifiedExecError> {
        let (timeout_ms, timeout_warning) = match request.timeout_ms {
            Some(requested) if requested > MAX_TIMEOUT_MS => (
                MAX_TIMEOUT_MS,
                Some(format!(
                    "Warning: requested timeout {requested}ms exceeds maximum of {MAX_TIMEOUT_MS}ms; clamping to {MAX_TIMEOUT_MS}ms.\n"
                )),
            ),
            Some(requested) => (requested, None),
            None => (DEFAULT_TIMEOUT_MS, None),
        };

        let mut new_session: Option<ManagedUnifiedExecSession> = None;
        let session_id;
        let writer_tx;
        let output_buffer;
        let output_notify;

        if let Some(existing_id) = request.session_id {
            let mut sessions = self.sessions.lock().await;
            match sessions.get(&existing_id) {
                Some(session) => {
                    if session.has_exited() {
                        sessions.remove(&existing_id);
                        return Err(UnifiedExecError::UnknownSessionId {
                            session_id: existing_id,
                        });
                    }
                    let (buffer, notify) = session.output_handles();
                    session_id = existing_id;
                    writer_tx = session.writer_sender();
                    output_buffer = buffer;
                    output_notify = notify;
                }
                None => {
                    return Err(UnifiedExecError::UnknownSessionId {
                        session_id: existing_id,
                    });
                }
            }
            drop(sessions);
        } else {
            let command = request.input_chunks.to_vec();
            let new_id = self.next_session_id.fetch_add(1, Ordering::SeqCst);
            let (session, initial_output_rx) = create_unified_exec_session(&command).await?;
            let managed_session = ManagedUnifiedExecSession::new(session, initial_output_rx);
            let (buffer, notify) = managed_session.output_handles();
            writer_tx = managed_session.writer_sender();
            output_buffer = buffer;
            output_notify = notify;
            session_id = new_id;
            new_session = Some(managed_session);
        };

        if request.session_id.is_some() {
            let joined_input = request.input_chunks.join(" ");
            if !joined_input.is_empty() && writer_tx.send(joined_input.into_bytes()).await.is_err()
            {
                return Err(UnifiedExecError::WriteToStdin);
            }
        }

        let mut collected: Vec<u8> = Vec::with_capacity(4096);
        let start = Instant::now();
        let deadline = start + Duration::from_millis(timeout_ms);

        loop {
            let drained_chunks;
            let mut wait_for_output = None;
            {
                let mut guard = output_buffer.lock().await;
                drained_chunks = guard.drain();
                if drained_chunks.is_empty() {
                    wait_for_output = Some(output_notify.notified());
                }
            }

            if drained_chunks.is_empty() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining == Duration::ZERO {
                    break;
                }

                let notified = wait_for_output.unwrap_or_else(|| output_notify.notified());
                tokio::pin!(notified);
                tokio::select! {
                    _ = &mut notified => {}
                    _ = tokio::time::sleep(remaining) => break,
                }
                continue;
            }

            for chunk in drained_chunks {
                collected.extend_from_slice(&chunk);
            }

            if Instant::now() >= deadline {
                break;
            }
        }

        let (output, _maybe_tokens) = truncate_middle(
            &String::from_utf8_lossy(&collected),
            UNIFIED_EXEC_OUTPUT_MAX_BYTES,
        );
        let output = if let Some(warning) = timeout_warning {
            format!("{warning}{output}")
        } else {
            output
        };

        let should_store_session = if let Some(session) = new_session.as_ref() {
            !session.has_exited()
        } else if request.session_id.is_some() {
            let mut sessions = self.sessions.lock().await;
            if let Some(existing) = sessions.get(&session_id) {
                if existing.has_exited() {
                    sessions.remove(&session_id);
                    false
                } else {
                    true
                }
            } else {
                false
            }
        } else {
            true
        };

        if should_store_session {
            if let Some(session) = new_session {
                self.sessions.lock().await.insert(session_id, session);
            }
            Ok(UnifiedExecResult {
                session_id: Some(session_id),
                output,
            })
        } else {
            Ok(UnifiedExecResult {
                session_id: None,
                output,
            })
        }
    }
}

async fn create_unified_exec_session(
    command: &[String],
) -> Result<
    (
        ExecCommandSession,
        tokio::sync::broadcast::Receiver<Vec<u8>>,
    ),
    UnifiedExecError,
> {
    if command.is_empty() {
        return Err(UnifiedExecError::MissingCommandLine);
    }

    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(UnifiedExecError::create_session)?;

    // Safe thanks to the check at the top of the function.
    let mut command_builder = CommandBuilder::new(command[0].clone());
    for arg in &command[1..] {
        command_builder.arg(arg);
    }

    let mut child = pair
        .slave
        .spawn_command(command_builder)
        .map_err(UnifiedExecError::create_session)?;
    let killer = child.clone_killer();

    let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(128);
    let (output_tx, _) = tokio::sync::broadcast::channel::<Vec<u8>>(256);

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

    let exit_status = Arc::new(AtomicBool::new(false));
    let wait_exit_status = Arc::clone(&exit_status);
    let wait_handle = tokio::task::spawn_blocking(move || {
        let _ = child.wait();
        wait_exit_status.store(true, Ordering::SeqCst);
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
    Ok((session, initial_output_rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use core_test_support::skip_if_sandbox;

    #[test]
    fn push_chunk_trims_only_excess_bytes() {
        let mut buffer = OutputBufferState::default();
        buffer.push_chunk(vec![b'a'; UNIFIED_EXEC_OUTPUT_MAX_BYTES]);
        buffer.push_chunk(vec![b'b']);
        buffer.push_chunk(vec![b'c']);

        assert_eq!(buffer.total_bytes, UNIFIED_EXEC_OUTPUT_MAX_BYTES);
        assert_eq!(buffer.chunks.len(), 3);
        assert_eq!(
            buffer.chunks.front().unwrap().len(),
            UNIFIED_EXEC_OUTPUT_MAX_BYTES - 2
        );
        assert_eq!(buffer.chunks.pop_back().unwrap(), vec![b'c']);
        assert_eq!(buffer.chunks.pop_back().unwrap(), vec![b'b']);
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unified_exec_persists_across_requests_jif() -> Result<(), UnifiedExecError> {
        skip_if_sandbox!(Ok(()));

        let manager = UnifiedExecSessionManager::default();

        let open_shell = manager
            .handle_request(UnifiedExecRequest {
                session_id: None,
                input_chunks: &["bash".to_string(), "-i".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;
        let session_id = open_shell.session_id.expect("expected session_id");

        manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_id),
                input_chunks: &[
                    "export".to_string(),
                    "CODEX_INTERACTIVE_SHELL_VAR=codex\n".to_string(),
                ],
                timeout_ms: Some(2_500),
            })
            .await?;

        let out_2 = manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_id),
                input_chunks: &["echo $CODEX_INTERACTIVE_SHELL_VAR\n".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;
        assert!(out_2.output.contains("codex"));

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn multi_unified_exec_sessions() -> Result<(), UnifiedExecError> {
        skip_if_sandbox!(Ok(()));

        let manager = UnifiedExecSessionManager::default();

        let shell_a = manager
            .handle_request(UnifiedExecRequest {
                session_id: None,
                input_chunks: &["/bin/bash".to_string(), "-i".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;
        let session_a = shell_a.session_id.expect("expected session id");

        manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_a),
                input_chunks: &["export CODEX_INTERACTIVE_SHELL_VAR=codex\n".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;

        let out_2 = manager
            .handle_request(UnifiedExecRequest {
                session_id: None,
                input_chunks: &[
                    "echo".to_string(),
                    "$CODEX_INTERACTIVE_SHELL_VAR\n".to_string(),
                ],
                timeout_ms: Some(2_500),
            })
            .await?;
        assert!(!out_2.output.contains("codex"));

        let out_3 = manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_a),
                input_chunks: &["echo $CODEX_INTERACTIVE_SHELL_VAR\n".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;
        assert!(out_3.output.contains("codex"));

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unified_exec_timeouts() -> Result<(), UnifiedExecError> {
        skip_if_sandbox!(Ok(()));

        let manager = UnifiedExecSessionManager::default();

        let open_shell = manager
            .handle_request(UnifiedExecRequest {
                session_id: None,
                input_chunks: &["bash".to_string(), "-i".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;
        let session_id = open_shell.session_id.expect("expected session id");

        manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_id),
                input_chunks: &[
                    "export".to_string(),
                    "CODEX_INTERACTIVE_SHELL_VAR=codex\n".to_string(),
                ],
                timeout_ms: Some(2_500),
            })
            .await?;

        let out_2 = manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_id),
                input_chunks: &["sleep 5 && echo $CODEX_INTERACTIVE_SHELL_VAR\n".to_string()],
                timeout_ms: Some(10),
            })
            .await?;
        assert!(!out_2.output.contains("codex"));

        tokio::time::sleep(Duration::from_secs(7)).await;

        let empty = Vec::new();
        let out_3 = manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_id),
                input_chunks: &empty,
                timeout_ms: Some(100),
            })
            .await?;

        assert!(out_3.output.contains("codex"));

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore] // Ignored while we have a better way to test this.
    async fn requests_with_large_timeout_are_capped() -> Result<(), UnifiedExecError> {
        let manager = UnifiedExecSessionManager::default();

        let result = manager
            .handle_request(UnifiedExecRequest {
                session_id: None,
                input_chunks: &["echo".to_string(), "codex".to_string()],
                timeout_ms: Some(120_000),
            })
            .await?;

        assert!(result.output.starts_with(
            "Warning: requested timeout 120000ms exceeds maximum of 60000ms; clamping to 60000ms.\n"
        ));
        assert!(result.output.contains("codex"));

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    #[ignore] // Ignored while we have a better way to test this.
    async fn completed_commands_do_not_persist_sessions() -> Result<(), UnifiedExecError> {
        let manager = UnifiedExecSessionManager::default();
        let result = manager
            .handle_request(UnifiedExecRequest {
                session_id: None,
                input_chunks: &["/bin/echo".to_string(), "codex".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;

        assert!(result.session_id.is_none());
        assert!(result.output.contains("codex"));

        assert!(manager.sessions.lock().await.is_empty());

        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn reusing_completed_session_returns_unknown_session() -> Result<(), UnifiedExecError> {
        skip_if_sandbox!(Ok(()));

        let manager = UnifiedExecSessionManager::default();

        let open_shell = manager
            .handle_request(UnifiedExecRequest {
                session_id: None,
                input_chunks: &["/bin/bash".to_string(), "-i".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;
        let session_id = open_shell.session_id.expect("expected session id");

        manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_id),
                input_chunks: &["exit\n".to_string()],
                timeout_ms: Some(2_500),
            })
            .await?;

        tokio::time::sleep(Duration::from_millis(200)).await;

        let err = manager
            .handle_request(UnifiedExecRequest {
                session_id: Some(session_id),
                input_chunks: &[],
                timeout_ms: Some(100),
            })
            .await
            .expect_err("expected unknown session error");

        match err {
            UnifiedExecError::UnknownSessionId { session_id: err_id } => {
                assert_eq!(err_id, session_id);
            }
            other => panic!("expected UnknownSessionId, got {other:?}"),
        }

        assert!(!manager.sessions.lock().await.contains_key(&session_id));

        Ok(())
    }
}
