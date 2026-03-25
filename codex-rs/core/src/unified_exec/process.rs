#![allow(clippy::module_inception)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::broadcast;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::exec::ExecToolCallOutput;
use crate::exec::StreamOutput;
use crate::exec::is_likely_sandbox_denied;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecSessionEvent;
use codex_exec_server::StartedExecProcess;
use codex_exec_server::WriteStatus;
use codex_protocol::protocol::TruncationPolicy;
use codex_sandboxing::SandboxType;
use codex_utils_output_truncation::formatted_truncate_text;
use codex_utils_pty::ExecCommandSession;
use codex_utils_pty::SpawnedPty;

use super::UNIFIED_EXEC_OUTPUT_MAX_TOKENS;
use super::UnifiedExecError;
use super::head_tail_buffer::HeadTailBuffer;
use super::process_state::ProcessState;

pub(crate) trait SpawnLifecycle: std::fmt::Debug + Send + Sync {
    /// Returns file descriptors that must stay open across the child `exec()`.
    ///
    /// The returned descriptors must already be valid in the parent process and
    /// stay valid until `after_spawn()` runs, which is the first point where
    /// the parent may release its copies.
    fn inherited_fds(&self) -> Vec<i32> {
        Vec::new()
    }

    fn after_spawn(&mut self) {}
}

pub(crate) type SpawnLifecycleHandle = Box<dyn SpawnLifecycle>;

#[derive(Debug, Default)]
/// Spawn lifecycle that performs no extra setup around process launch.
pub(crate) struct NoopSpawnLifecycle;

impl SpawnLifecycle for NoopSpawnLifecycle {}

pub(crate) type OutputBuffer = Arc<Mutex<HeadTailBuffer>>;
/// Shared output state exposed to polling and streaming consumers.
pub(crate) struct OutputHandles {
    pub(crate) output_buffer: OutputBuffer,
    pub(crate) output_notify: Arc<Notify>,
    pub(crate) output_closed: Arc<AtomicBool>,
    pub(crate) output_closed_notify: Arc<Notify>,
    pub(crate) cancellation_token: CancellationToken,
}

/// Transport-specific process handle used by unified exec.
enum ProcessHandle {
    Local(Box<ExecCommandSession>),
    Remote(Arc<dyn ExecProcess>),
}

/// Unified wrapper over local PTY sessions and exec-server-backed processes.
pub(crate) struct UnifiedExecProcess {
    process_handle: ProcessHandle,
    output_tx: broadcast::Sender<Vec<u8>>,
    output_buffer: OutputBuffer,
    output_notify: Arc<Notify>,
    output_closed: Arc<AtomicBool>,
    output_closed_notify: Arc<Notify>,
    cancellation_token: CancellationToken,
    output_drained: Arc<Notify>,
    state_tx: watch::Sender<ProcessState>,
    state_rx: watch::Receiver<ProcessState>,
    output_task: Option<JoinHandle<()>>,
    sandbox_type: SandboxType,
    _spawn_lifecycle: Option<SpawnLifecycleHandle>,
}

impl std::fmt::Debug for UnifiedExecProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnifiedExecProcess")
            .field("has_exited", &self.has_exited())
            .field("exit_code", &self.exit_code())
            .field("sandbox_type", &self.sandbox_type)
            .finish_non_exhaustive()
    }
}

impl UnifiedExecProcess {
    fn new(
        process_handle: ProcessHandle,
        sandbox_type: SandboxType,
        spawn_lifecycle: Option<SpawnLifecycleHandle>,
    ) -> Self {
        let output_buffer = Arc::new(Mutex::new(HeadTailBuffer::default()));
        let output_notify = Arc::new(Notify::new());
        let output_closed = Arc::new(AtomicBool::new(false));
        let output_closed_notify = Arc::new(Notify::new());
        let cancellation_token = CancellationToken::new();
        let output_drained = Arc::new(Notify::new());
        let (output_tx, _) = broadcast::channel(64);
        let (state_tx, state_rx) = watch::channel(ProcessState::default());

        Self {
            process_handle,
            output_tx,
            output_buffer,
            output_notify,
            output_closed,
            output_closed_notify,
            cancellation_token,
            output_drained,
            state_tx,
            state_rx,
            output_task: None,
            sandbox_type,
            _spawn_lifecycle: spawn_lifecycle,
        }
    }

    pub(super) async fn write(&self, data: &[u8]) -> Result<(), UnifiedExecError> {
        match &self.process_handle {
            ProcessHandle::Local(process_handle) => process_handle
                .writer_sender()
                .send(data.to_vec())
                .await
                .map(|()| ())
                .map_err(|_| UnifiedExecError::WriteToStdin),
            ProcessHandle::Remote(process_handle) => {
                match process_handle.write(data.to_vec()).await {
                    Ok(response) => match response.status {
                        WriteStatus::Accepted => Ok(()),
                        WriteStatus::UnknownProcess | WriteStatus::StdinClosed => {
                            let state = self.state_rx.borrow().clone();
                            let _ = self.state_tx.send_replace(state.exited(state.exit_code));
                            self.cancellation_token.cancel();
                            Err(UnifiedExecError::WriteToStdin)
                        }
                        WriteStatus::Starting => Err(UnifiedExecError::WriteToStdin),
                    },
                    Err(err) => Err(UnifiedExecError::process_failed(err.to_string())),
                }
            }
        }
    }

    pub(super) fn output_handles(&self) -> OutputHandles {
        OutputHandles {
            output_buffer: Arc::clone(&self.output_buffer),
            output_notify: Arc::clone(&self.output_notify),
            output_closed: Arc::clone(&self.output_closed),
            output_closed_notify: Arc::clone(&self.output_closed_notify),
            cancellation_token: self.cancellation_token.clone(),
        }
    }

    pub(super) fn output_receiver(&self) -> tokio::sync::broadcast::Receiver<Vec<u8>> {
        self.output_tx.subscribe()
    }

    pub(super) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub(super) fn output_drained_notify(&self) -> Arc<Notify> {
        Arc::clone(&self.output_drained)
    }

    pub(super) fn has_exited(&self) -> bool {
        let state = self.state_rx.borrow().clone();
        match &self.process_handle {
            ProcessHandle::Local(process_handle) => state.has_exited || process_handle.has_exited(),
            ProcessHandle::Remote(_) => state.has_exited,
        }
    }

    pub(super) fn exit_code(&self) -> Option<i32> {
        let state = self.state_rx.borrow().clone();
        match &self.process_handle {
            ProcessHandle::Local(process_handle) => {
                state.exit_code.or_else(|| process_handle.exit_code())
            }
            ProcessHandle::Remote(_) => state.exit_code,
        }
    }

    pub(super) fn terminate(&self) {
        self.output_closed.store(true, Ordering::Release);
        self.output_closed_notify.notify_waiters();
        match &self.process_handle {
            ProcessHandle::Local(process_handle) => process_handle.terminate(),
            ProcessHandle::Remote(process_handle) => {
                let process_handle = Arc::clone(process_handle);
                tokio::spawn(async move {
                    let _ = process_handle.terminate().await;
                });
            }
        }
        self.cancellation_token.cancel();
        if let Some(output_task) = &self.output_task {
            output_task.abort();
        }
    }

    async fn snapshot_output(&self) -> Vec<Vec<u8>> {
        let guard = self.output_buffer.lock().await;
        guard.snapshot_chunks()
    }

    pub(crate) fn sandbox_type(&self) -> SandboxType {
        self.sandbox_type
    }

    pub(super) fn failure_message(&self) -> Option<String> {
        self.state_rx.borrow().failure_message.clone()
    }

    pub(super) async fn check_for_sandbox_denial(&self) -> Result<(), UnifiedExecError> {
        let _ =
            tokio::time::timeout(Duration::from_millis(20), self.output_notify.notified()).await;

        let collected_chunks = self.snapshot_output().await;
        let mut aggregated: Vec<u8> = Vec::new();
        for chunk in collected_chunks {
            aggregated.extend_from_slice(&chunk);
        }
        let aggregated_text = String::from_utf8_lossy(&aggregated).to_string();
        self.check_for_sandbox_denial_with_text(&aggregated_text)
            .await?;

        Ok(())
    }

    pub(super) async fn check_for_sandbox_denial_with_text(
        &self,
        text: &str,
    ) -> Result<(), UnifiedExecError> {
        let sandbox_type = self.sandbox_type();
        if sandbox_type == SandboxType::None || !self.has_exited() {
            return Ok(());
        }

        let exit_code = self.exit_code().unwrap_or(-1);
        let exec_output = ExecToolCallOutput {
            exit_code,
            stderr: StreamOutput::new(text.to_string()),
            aggregated_output: StreamOutput::new(text.to_string()),
            ..Default::default()
        };
        if is_likely_sandbox_denied(sandbox_type, &exec_output) {
            let snippet = formatted_truncate_text(
                text,
                TruncationPolicy::Tokens(UNIFIED_EXEC_OUTPUT_MAX_TOKENS),
            );
            let message = if snippet.is_empty() {
                format!("Process exited with code {exit_code}")
            } else {
                snippet
            };
            return Err(UnifiedExecError::sandbox_denied(message, exec_output));
        }
        Ok(())
    }

    pub(super) async fn from_spawned(
        spawned: SpawnedPty,
        sandbox_type: SandboxType,
        spawn_lifecycle: SpawnLifecycleHandle,
    ) -> Result<Self, UnifiedExecError> {
        let SpawnedPty {
            session: process_handle,
            stdout_rx,
            stderr_rx,
            mut exit_rx,
        } = spawned;
        let output_rx = codex_utils_pty::combine_output_receivers(stdout_rx, stderr_rx);
        let mut managed = Self::new(
            ProcessHandle::Local(Box::new(process_handle)),
            sandbox_type,
            Some(spawn_lifecycle),
        );
        managed.output_task = Some(Self::spawn_local_output_task(
            output_rx,
            Arc::clone(&managed.output_buffer),
            Arc::clone(&managed.output_notify),
            Arc::clone(&managed.output_closed),
            Arc::clone(&managed.output_closed_notify),
            managed.output_tx.clone(),
        ));

        match exit_rx.try_recv() {
            Ok(exit_code) => {
                managed.signal_exit(Some(exit_code));
                managed.check_for_sandbox_denial().await?;
                return Ok(managed);
            }
            Err(TryRecvError::Closed) => {
                managed.signal_exit(/*exit_code*/ None);
                managed.check_for_sandbox_denial().await?;
                return Ok(managed);
            }
            Err(TryRecvError::Empty) => {}
        }

        if let Ok(exit_result) =
            tokio::time::timeout(Duration::from_millis(150), &mut exit_rx).await
        {
            managed.signal_exit(exit_result.ok());
            managed.check_for_sandbox_denial().await?;
            return Ok(managed);
        }

        tokio::spawn({
            let state_tx = managed.state_tx.clone();
            let cancellation_token = managed.cancellation_token.clone();
            async move {
                let exit_code = exit_rx.await.ok();
                let state = state_tx.borrow().clone();
                let _ = state_tx.send_replace(state.exited(exit_code));
                cancellation_token.cancel();
            }
        });

        Ok(managed)
    }

    fn spawn_remote_output_task(
        started: StartedExecProcess,
        output_buffer: OutputBuffer,
        output_notify: Arc<Notify>,
        output_closed: Arc<AtomicBool>,
        output_closed_notify: Arc<Notify>,
        output_tx: broadcast::Sender<Vec<u8>>,
        state_tx: watch::Sender<ProcessState>,
        cancellation_token: CancellationToken,
    ) -> JoinHandle<()> {
        let StartedExecProcess { mut events, .. } = started;
        tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Some(ExecSessionEvent::Output { chunk, .. }) => {
                        let mut guard = output_buffer.lock().await;
                        guard.push_chunk(chunk.clone());
                        drop(guard);
                        let _ = output_tx.send(chunk);
                        output_notify.notify_waiters();
                    }
                    Some(ExecSessionEvent::Exited { exit_code, .. }) => {
                        let state = state_tx.borrow().clone();
                        let _ = state_tx.send_replace(state.exited(Some(exit_code)));
                        cancellation_token.cancel();
                    }
                    Some(ExecSessionEvent::Closed { .. }) => {
                        let state = state_tx.borrow().clone();
                        let _ = state_tx.send_replace(state.exited(state.exit_code));
                        output_closed.store(true, Ordering::Release);
                        output_closed_notify.notify_waiters();
                        cancellation_token.cancel();
                        break;
                    }
                    Some(ExecSessionEvent::Failed { message }) => {
                        let state = state_tx.borrow().clone();
                        let _ = state_tx.send_replace(state.failed(message));
                        output_closed.store(true, Ordering::Release);
                        output_closed_notify.notify_waiters();
                        cancellation_token.cancel();
                        break;
                    }
                    None => {
                        let state = state_tx.borrow().clone();
                        let _ = state_tx.send_replace(state.exited(state.exit_code));
                        output_closed.store(true, Ordering::Release);
                        output_closed_notify.notify_waiters();
                        cancellation_token.cancel();
                        break;
                    }
                }
            }
        })
    }

    fn spawn_local_output_task(
        mut receiver: tokio::sync::broadcast::Receiver<Vec<u8>>,
        buffer: OutputBuffer,
        output_notify: Arc<Notify>,
        output_closed: Arc<AtomicBool>,
        output_closed_notify: Arc<Notify>,
        output_tx: broadcast::Sender<Vec<u8>>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Ok(chunk) => {
                        let mut guard = buffer.lock().await;
                        guard.push_chunk(chunk.clone());
                        drop(guard);
                        let _ = output_tx.send(chunk);
                        output_notify.notify_waiters();
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        output_closed.store(true, Ordering::Release);
                        output_closed_notify.notify_waiters();
                        break;
                    }
                };
            }
        })
    }

    fn signal_exit(&self, exit_code: Option<i32>) {
        let state = self.state_rx.borrow().clone();
        let _ = self.state_tx.send_replace(state.exited(exit_code));
        self.cancellation_token.cancel();
    }
}

impl From<(StartedExecProcess, SandboxType)> for UnifiedExecProcess {
    fn from((started, sandbox_type): (StartedExecProcess, SandboxType)) -> Self {
        let process_handle = ProcessHandle::Remote(Arc::clone(&started.process));
        let mut managed = Self::new(process_handle, sandbox_type, /*spawn_lifecycle*/ None);
        managed.output_task = Some(Self::spawn_remote_output_task(
            started,
            Arc::clone(&managed.output_buffer),
            Arc::clone(&managed.output_notify),
            Arc::clone(&managed.output_closed),
            Arc::clone(&managed.output_closed_notify),
            managed.output_tx.clone(),
            managed.state_tx.clone(),
            managed.cancellation_token.clone(),
        ));
        managed
    }
}

impl Drop for UnifiedExecProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}
