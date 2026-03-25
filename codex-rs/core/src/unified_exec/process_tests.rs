use super::process::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecError;
use async_trait::async_trait;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecServerError;
use codex_exec_server::ProcessId;
use codex_exec_server::StartedExecProcess;
use codex_exec_server::WriteResponse;
use codex_exec_server::WriteStatus;
use codex_sandboxing::SandboxType;
use std::sync::Arc;
use tokio::sync::mpsc;

struct MockExecProcess {
    process_id: ProcessId,
    write_response: WriteResponse,
}

#[async_trait]
impl ExecProcess for MockExecProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    async fn write(&self, _chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError> {
        Ok(self.write_response.clone())
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        Ok(())
    }
}

fn remote_process(write_status: WriteStatus) -> UnifiedExecProcess {
    let (_events_tx, events_rx) = mpsc::channel(1);
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "test-process".to_string().into(),
            write_response: WriteResponse {
                status: write_status,
            },
        }),
        events: events_rx,
    };

    UnifiedExecProcess::from((started, SandboxType::None))
}

#[tokio::test]
async fn remote_write_unknown_process_marks_process_exited() {
    let process = remote_process(WriteStatus::UnknownProcess);

    let err = process
        .write(b"hello")
        .await
        .expect_err("expected write failure");

    assert!(matches!(err, UnifiedExecError::WriteToStdin));
    assert!(process.has_exited());
}

#[tokio::test]
async fn remote_write_closed_stdin_marks_process_exited() {
    let process = remote_process(WriteStatus::StdinClosed);

    let err = process
        .write(b"hello")
        .await
        .expect_err("expected write failure");

    assert!(matches!(err, UnifiedExecError::WriteToStdin));
    assert!(process.has_exited());
}
