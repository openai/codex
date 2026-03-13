#![allow(clippy::module_inception)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StreamOutput;
use crate::exec::is_likely_sandbox_denied;
use crate::truncate::TruncationPolicy;
use crate::truncate::formatted_truncate_text;
use codex_utils_pty::ExecCommandSession;
use codex_utils_pty::SpawnedPty;

use super::UNIFIED_EXEC_OUTPUT_MAX_TOKENS;
use super::UnifiedExecError;
use super::head_tail_buffer::HeadTailBuffer;

pub(crate) trait SpawnLifecycle: std::fmt::Debug + Send + Sync {
    fn after_spawn(&mut self) {}
}

pub(crate) type SpawnLifecycleHandle = Box<dyn SpawnLifecycle>;

#[derive(Debug, Default)]
pub(crate) struct NoopSpawnLifecycle;

impl SpawnLifecycle for NoopSpawnLifecycle {}

pub(crate) type OutputBuffer = Arc<Mutex<HeadTailBuffer>>;
pub(crate) struct OutputHandles {
    pub(crate) output_buffer: OutputBuffer,
    pub(crate) output_notify: Arc<Notify>,
    pub(crate) output_closed: Arc<AtomicBool>,
    pub(crate) output_closed_notify: Arc<Notify>,
    pub(crate) cancellation_token: CancellationToken,
}

#[derive(Debug)]
pub(crate) struct UnifiedExecProcess {
    process_handle: ExecCommandSession,
    output_tx: broadcast::Sender<Vec<u8>>,
    output_buffer: OutputBuffer,
    output_notify: Arc<Notify>,
    output_closed: Arc<AtomicBool>,
    output_closed_notify: Arc<Notify>,
    cancellation_token: CancellationToken,
    output_drained: Arc<Notify>,
    output_task: JoinHandle<()>,
    sandbox_type: SandboxType,
    _spawn_lifecycle: SpawnLifecycleHandle,
}

pub(crate) struct ManagedSplitProcess {
    pub(crate) process: UnifiedExecProcess,
    pub(crate) stdin: mpsc::Sender<Vec<u8>>,
    pub(crate) stdout_rx: mpsc::Receiver<Vec<u8>>,
    pub(crate) stderr_rx: mpsc::Receiver<Vec<u8>>,
}

impl UnifiedExecProcess {
    pub(crate) fn new(
        process_handle: ExecCommandSession,
        initial_output_rx: mpsc::Receiver<Vec<u8>>,
        sandbox_type: SandboxType,
        spawn_lifecycle: SpawnLifecycleHandle,
    ) -> Self {
        let output_buffer = Arc::new(Mutex::new(HeadTailBuffer::default()));
        let output_notify = Arc::new(Notify::new());
        let output_closed = Arc::new(AtomicBool::new(false));
        let output_closed_notify = Arc::new(Notify::new());
        let cancellation_token = CancellationToken::new();
        let output_drained = Arc::new(Notify::new());
        let (output_tx, _) = broadcast::channel::<Vec<u8>>(256);
        let mut receiver = initial_output_rx;
        let buffer_clone = Arc::clone(&output_buffer);
        let notify_clone = Arc::clone(&output_notify);
        let output_closed_clone = Arc::clone(&output_closed);
        let output_closed_notify_clone = Arc::clone(&output_closed_notify);
        let output_tx_clone = output_tx.clone();
        let output_task = tokio::spawn(async move {
            while let Some(chunk) = receiver.recv().await {
                let _ = output_tx_clone.send(chunk.clone());
                let mut guard = buffer_clone.lock().await;
                guard.push_chunk(chunk);
                drop(guard);
                notify_clone.notify_waiters();
            }
            output_closed_clone.store(true, Ordering::Release);
            output_closed_notify_clone.notify_waiters();
        });

        Self {
            process_handle,
            output_tx,
            output_buffer,
            output_notify,
            output_closed,
            output_closed_notify,
            cancellation_token,
            output_drained,
            output_task,
            sandbox_type,
            _spawn_lifecycle: spawn_lifecycle,
        }
    }

    pub(crate) fn writer_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.process_handle.writer_sender()
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

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub(super) fn output_drained_notify(&self) -> Arc<Notify> {
        Arc::clone(&self.output_drained)
    }

    pub(crate) fn has_exited(&self) -> bool {
        self.process_handle.has_exited()
    }

    pub(crate) fn exit_code(&self) -> Option<i32> {
        self.process_handle.exit_code()
    }

    pub(crate) fn terminate(&self) {
        self.output_closed.store(true, Ordering::Release);
        self.output_closed_notify.notify_waiters();
        self.process_handle.terminate();
        self.cancellation_token.cancel();
        self.output_task.abort();
    }

    pub(crate) fn request_terminate(&self) {
        self.process_handle.request_terminate();
    }

    pub(crate) fn pid(&self) -> Option<u32> {
        self.process_handle.pid()
    }

    async fn snapshot_output(&self) -> Vec<Vec<u8>> {
        let guard = self.output_buffer.lock().await;
        guard.snapshot_chunks()
    }

    pub(crate) fn sandbox_type(&self) -> SandboxType {
        self.sandbox_type
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
            output_rx,
            exit_rx,
        } = spawned;
        Self::from_process_parts(
            process_handle,
            output_rx,
            exit_rx,
            sandbox_type,
            spawn_lifecycle,
        )
        .await
    }

    pub(crate) async fn from_process_parts(
        process_handle: ExecCommandSession,
        output_rx: mpsc::Receiver<Vec<u8>>,
        mut exit_rx: oneshot::Receiver<i32>,
        sandbox_type: SandboxType,
        spawn_lifecycle: SpawnLifecycleHandle,
    ) -> Result<Self, UnifiedExecError> {
        let managed = Self::new(process_handle, output_rx, sandbox_type, spawn_lifecycle);

        let exit_ready = matches!(exit_rx.try_recv(), Ok(_) | Err(TryRecvError::Closed));

        if exit_ready {
            managed.signal_exit();
            managed.check_for_sandbox_denial().await?;
            return Ok(managed);
        }

        if tokio::time::timeout(Duration::from_millis(150), &mut exit_rx)
            .await
            .is_ok()
        {
            managed.signal_exit();
            managed.check_for_sandbox_denial().await?;
            return Ok(managed);
        }

        tokio::spawn({
            let cancellation_token = managed.cancellation_token.clone();
            async move {
                let _ = exit_rx.await;
                cancellation_token.cancel();
            }
        });

        Ok(managed)
    }

    fn signal_exit(&self) {
        self.cancellation_token.cancel();
    }
}

impl Drop for UnifiedExecProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}
