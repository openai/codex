use super::process::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecError;
use async_trait::async_trait;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_exec_server::ExecApprovalRequest;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecServerError;
use codex_exec_server::ProcessId;
use codex_exec_server::ReadResponse;
use codex_exec_server::ResolveExecApprovalResponse;
use codex_exec_server::StartedExecProcess;
use codex_exec_server::WriteResponse;
use codex_exec_server::WriteStatus;
use codex_sandboxing::SandboxType;
use pretty_assertions::assert_eq;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio::time::Duration;

struct MockExecProcess {
    process_id: ProcessId,
    write_response: WriteResponse,
    read_responses: Mutex<VecDeque<ReadResponse>>,
    wake_tx: watch::Sender<u64>,
}

#[async_trait]
impl ExecProcess for MockExecProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    fn subscribe_wake(&self) -> watch::Receiver<u64> {
        self.wake_tx.subscribe()
    }

    async fn read(
        &self,
        _after_seq: Option<u64>,
        _max_bytes: Option<usize>,
        _wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError> {
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
                exec_approval: None,
            }))
    }

    async fn write(&self, _chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError> {
        Ok(self.write_response.clone())
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        Ok(())
    }

    async fn resolve_exec_approval(
        &self,
        _approval_id: String,
        _decision: CommandExecutionApprovalDecision,
    ) -> Result<ResolveExecApprovalResponse, ExecServerError> {
        Ok(ResolveExecApprovalResponse { accepted: true })
    }
}

async fn remote_process(write_status: WriteStatus) -> UnifiedExecProcess {
    let (wake_tx, _wake_rx) = watch::channel(0);
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "test-process".to_string().into(),
            write_response: WriteResponse {
                status: write_status,
            },
            read_responses: Mutex::new(VecDeque::new()),
            wake_tx,
        }),
    };

    UnifiedExecProcess::from_remote_started(started, SandboxType::None)
        .await
        .expect("remote process should start")
}

#[tokio::test]
async fn remote_write_unknown_process_marks_process_exited() {
    let process = remote_process(WriteStatus::UnknownProcess).await;

    let err = process
        .write(b"hello")
        .await
        .expect_err("expected write failure");

    assert!(matches!(err, UnifiedExecError::WriteToStdin));
    assert!(process.has_exited());
}

#[tokio::test]
async fn remote_write_closed_stdin_marks_process_exited() {
    let process = remote_process(WriteStatus::StdinClosed).await;

    let err = process
        .write(b"hello")
        .await
        .expect_err("expected write failure");

    assert!(matches!(err, UnifiedExecError::WriteToStdin));
    assert!(process.has_exited());
}

#[tokio::test]
async fn remote_process_waits_for_early_exit_event() {
    let (wake_tx, _wake_rx) = watch::channel(0);
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "test-process".to_string().into(),
            write_response: WriteResponse {
                status: WriteStatus::Accepted,
            },
            read_responses: Mutex::new(VecDeque::from([ReadResponse {
                chunks: Vec::new(),
                next_seq: 2,
                exited: true,
                exit_code: Some(17),
                closed: true,
                failure: None,
                exec_approval: None,
            }])),
            wake_tx: wake_tx.clone(),
        }),
    };

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = wake_tx.send(1);
    });

    let process = UnifiedExecProcess::from_remote_started(started, SandboxType::None)
        .await
        .expect("remote process should observe early exit");

    assert!(process.has_exited());
    assert_eq!(process.exit_code(), Some(17));
}

#[tokio::test]
async fn remote_process_surfaces_pending_exec_approval() {
    let approval = ExecApprovalRequest {
        call_id: "call-1".to_string(),
        approval_id: Some("approval-1".to_string()),
        turn_id: "turn-1".to_string(),
        command: vec!["git".to_string(), "status".to_string()],
        cwd: "/tmp".into(),
        reason: Some("approval required".to_string()),
        additional_permissions: None,
        proposed_execpolicy_amendment: None,
        available_decisions: Some(vec![CommandExecutionApprovalDecision::Accept]),
    };
    let (wake_tx, _wake_rx) = watch::channel(0);
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "test-process".to_string().into(),
            write_response: WriteResponse {
                status: WriteStatus::Accepted,
            },
            read_responses: Mutex::new(VecDeque::from([ReadResponse {
                chunks: Vec::new(),
                next_seq: 2,
                exited: false,
                exit_code: None,
                closed: false,
                failure: None,
                exec_approval: Some(approval.clone()),
            }])),
            wake_tx: wake_tx.clone(),
        }),
    };

    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = wake_tx.send(1);
    });

    let process = UnifiedExecProcess::from_remote_started(started, SandboxType::None)
        .await
        .expect("remote process should start");

    assert_eq!(process.take_pending_exec_approval().await, Some(approval));
}
