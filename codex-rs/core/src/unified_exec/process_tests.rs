#![cfg(test)]

use super::process::OutputDrainPolicy;
use super::process::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecError;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEventReceiver;
use codex_exec_server::ExecProcessFuture;
use codex_exec_server::ExecServerError;
use codex_exec_server::ProcessId;
use codex_exec_server::ProcessSignal;
use codex_exec_server::ReadResponse;
use codex_exec_server::StartedExecProcess;
use codex_exec_server::WriteResponse;
use codex_exec_server::WriteStatus;
use pretty_assertions::assert_eq;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;

struct MockExecProcess {
    process_id: ProcessId,
    write_response: WriteResponse,
    read_responses: Mutex<VecDeque<ReadResponse>>,
    terminate_error: Option<String>,
    wake_tx: watch::Sender<u64>,
}

impl MockExecProcess {
    async fn read(&self) -> Result<ReadResponse, ExecServerError> {
        Ok(self
            .read_responses
            .lock()
            .await
            .pop_front()
            .unwrap_or(ReadResponse {
                chunks: Vec::new(),
                next_seq: 1,
                exited: false,
                exit_code: None,
                closed: false,
                failure: None,
                sandbox_denied: false,
            }))
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        if let Some(message) = &self.terminate_error {
            return Err(ExecServerError::Protocol(message.clone()));
        }
        Ok(())
    }
}

impl ExecProcess for MockExecProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    fn subscribe_wake(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    fn subscribe_events(&self) -> ExecProcessEventReceiver {
        ExecProcessEventReceiver::empty()
    }

    fn read(
        &self,
        _after_seq: Option<u64>,
        _max_bytes: Option<usize>,
        _wait_ms: Option<u64>,
    ) -> ExecProcessFuture<'_, ReadResponse> {
        Box::pin(MockExecProcess::read(self))
    }

    fn write(&self, _chunk: Vec<u8>) -> ExecProcessFuture<'_, WriteResponse> {
        Box::pin(async { Ok(self.write_response.clone()) })
    }

    fn signal(&self, _signal: ProcessSignal) -> ExecProcessFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn terminate(&self) -> ExecProcessFuture<'_, ()> {
        Box::pin(MockExecProcess::terminate(self))
    }
}

pub(super) async fn remote_process(
    write_status: WriteStatus,
    terminate_error: Option<String>,
    sandbox_type: impl Into<Option<codex_sandboxing::SandboxType>>,
) -> UnifiedExecProcess {
    let (wake_tx, _wake_rx) = watch::channel(0);
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "test-process".to_string().into(),
            write_response: WriteResponse {
                status: write_status,
            },
            read_responses: Mutex::new(VecDeque::new()),
            terminate_error,
            wake_tx,
        }),
        sandbox_type: sandbox_type.into(),
    };

    UnifiedExecProcess::from_exec_server_started(started, OutputDrainPolicy::WaitForOutputClosure)
        .await
        .expect("remote process should start")
}

#[tokio::test]
async fn remote_write_unknown_process_marks_process_exited() {
    let process = remote_process(
        WriteStatus::UnknownProcess,
        /*terminate_error*/ None,
        codex_sandboxing::SandboxType::None,
    )
    .await;

    let err = process
        .write(b"hello")
        .await
        .expect_err("expected write failure");

    assert!(matches!(err, UnifiedExecError::WriteToStdin));
    assert!(process.has_exited());
}

#[tokio::test]
async fn remote_write_closed_stdin_marks_process_exited() {
    let process = remote_process(
        WriteStatus::StdinClosed,
        /*terminate_error*/ None,
        codex_sandboxing::SandboxType::None,
    )
    .await;

    let err = process
        .write(b"hello")
        .await
        .expect_err("expected write failure");

    assert!(matches!(err, UnifiedExecError::WriteToStdin));
    assert!(process.has_exited());
}

#[tokio::test]
async fn fail_and_terminate_preserves_failure_message() {
    let process = remote_process(
        WriteStatus::Accepted,
        /*terminate_error*/ None,
        codex_sandboxing::SandboxType::None,
    )
    .await;

    process.fail_and_terminate("network denied".to_string());
    process.fail_and_terminate("second failure".to_string());

    assert!(process.has_exited());
    assert_eq!(
        process.failure_message(),
        Some("network denied".to_string())
    );
}

#[tokio::test]
async fn remote_terminate_confirmed_updates_state_on_success_only() {
    let process = remote_process(
        WriteStatus::Accepted,
        Some("terminate unavailable".to_string()),
        codex_sandboxing::SandboxType::None,
    )
    .await;

    let err = process
        .terminate_confirmed()
        .await
        .expect_err("expected terminate failure");

    assert!(matches!(err, UnifiedExecError::ProcessFailed { .. }));
    assert!(!process.has_exited());

    let process = remote_process(
        WriteStatus::Accepted,
        /*terminate_error*/ None,
        codex_sandboxing::SandboxType::None,
    )
    .await;

    process
        .terminate_confirmed()
        .await
        .expect("terminate should succeed");

    assert!(process.has_exited());
}

#[tokio::test]
async fn remote_process_preserves_executor_sandbox_type() {
    use codex_sandboxing::SandboxType;
    for sandbox_type in [
        None,
        Some(SandboxType::None),
        Some(SandboxType::LinuxSeccomp),
    ] {
        let process = remote_process(
            WriteStatus::Accepted,
            /*terminate_error*/ None,
            sandbox_type,
        )
        .await;
        assert_eq!(process.sandbox_type(), sandbox_type);
    }
}

#[tokio::test(start_paused = true)]
async fn closed_output_is_classified_without_waiting_for_more_output() {
    use codex_sandboxing::SandboxType;
    for sandbox_type in [SandboxType::None, SandboxType::LinuxSeccomp] {
        let process = remote_process(
            WriteStatus::Accepted,
            /*terminate_error*/ None,
            sandbox_type,
        )
        .await;
        let output = process.output_handles();
        output
            .output_buffer
            .lock()
            .await
            .push_chunk(ExecOutputStream::Stderr, b"permission denied");
        let closed = output.output_closed_notify.notified();
        tokio::pin!(closed);
        closed.as_mut().enable();
        process.terminate_confirmed().await.expect("terminate");
        closed.await;

        let start = tokio::time::Instant::now();
        let result = process.check_for_sandbox_denial().await;
        assert_eq!(start.elapsed(), std::time::Duration::ZERO);
        if sandbox_type == SandboxType::None {
            result.expect("unsandboxed failure is not a sandbox denial");
        } else {
            assert!(matches!(
                result,
                Err(UnifiedExecError::SandboxDenied { .. })
            ));
        }
    }
}

#[tokio::test(start_paused = true)]
async fn output_closing_during_denial_check_wakes_without_timeout() {
    use codex_sandboxing::SandboxType;
    for sandbox_type in [SandboxType::None, SandboxType::LinuxSeccomp] {
        let process = remote_process(
            WriteStatus::Accepted,
            /*terminate_error*/ None,
            sandbox_type,
        )
        .await;
        process
            .output_handles()
            .output_buffer
            .lock()
            .await
            .push_chunk(ExecOutputStream::Stderr, b"permission denied");

        let check = process.check_for_sandbox_denial();
        tokio::pin!(check);
        let start = tokio::time::Instant::now();
        assert!(futures::poll!(check.as_mut()).is_pending());
        process.terminate_confirmed().await.expect("terminate");

        let result = check.await;
        assert_eq!(start.elapsed(), std::time::Duration::ZERO);
        if sandbox_type == SandboxType::None {
            result.expect("unsandboxed failure is not a sandbox denial");
        } else {
            assert!(matches!(
                result,
                Err(UnifiedExecError::SandboxDenied { .. })
            ));
        }
    }
}

#[tokio::test(start_paused = true)]
async fn open_output_preserves_denial_context_timeout() {
    let process = remote_process(
        WriteStatus::Accepted,
        /*terminate_error*/ None,
        codex_sandboxing::SandboxType::None,
    )
    .await;

    let start = tokio::time::Instant::now();
    process
        .check_for_sandbox_denial()
        .await
        .expect("running process is not a sandbox denial");
    assert_eq!(start.elapsed(), std::time::Duration::from_millis(20));
}
