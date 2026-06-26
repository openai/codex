use super::process::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecError;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::ExecProcessEventReceiver;
use codex_exec_server::ExecProcessFuture;
use codex_exec_server::ExecServerError;
use codex_exec_server::ProcessId;
use codex_exec_server::ProcessOutputChunk;
use codex_exec_server::ProcessSignal;
use codex_exec_server::ReadResponse;
use codex_exec_server::StartedExecProcess;
use codex_exec_server::WriteResponse;
use codex_exec_server::WriteStatus;
use pretty_assertions::assert_eq;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::watch;

use codex_exec_server::ExecOutputStream;

struct MockExecProcess {
    process_id: ProcessId,
    write_response: WriteResponse,
    read_responses: Mutex<VecDeque<ReadResponse>>,
    terminate_error: Option<String>,
    wake_tx: watch::Sender<u64>,
    events: Vec<ExecProcessEvent>,
    read_count: Arc<AtomicUsize>,
}

impl MockExecProcess {
    async fn read(&self) -> Result<ReadResponse, ExecServerError> {
        self.read_count.fetch_add(1, Ordering::Relaxed);
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
        ExecProcessEventReceiver::from_events(self.events.clone())
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

async fn remote_process(
    write_status: WriteStatus,
    terminate_error: Option<String>,
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
            events: Vec::new(),
            read_count: Arc::new(AtomicUsize::new(0)),
        }),
    };

    UnifiedExecProcess::from_exec_server_started(started)
        .await
        .expect("remote process should start")
}

#[tokio::test]
async fn remote_write_unknown_process_marks_process_exited() {
    let process = remote_process(WriteStatus::UnknownProcess, /*terminate_error*/ None).await;

    let err = process
        .write(b"hello")
        .await
        .expect_err("expected write failure");

    assert!(matches!(err, UnifiedExecError::WriteToStdin));
    assert!(process.has_exited());
}

#[tokio::test]
async fn remote_write_closed_stdin_marks_process_exited() {
    let process = remote_process(WriteStatus::StdinClosed, /*terminate_error*/ None).await;

    let err = process
        .write(b"hello")
        .await
        .expect_err("expected write failure");

    assert!(matches!(err, UnifiedExecError::WriteToStdin));
    assert!(process.has_exited());
}

#[tokio::test]
async fn fail_and_terminate_preserves_failure_message() {
    let process = remote_process(WriteStatus::Accepted, /*terminate_error*/ None).await;

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
    )
    .await;

    let err = process
        .terminate_confirmed()
        .await
        .expect_err("expected terminate failure");

    assert!(matches!(err, UnifiedExecError::ProcessFailed { .. }));
    assert!(!process.has_exited());

    let process = remote_process(WriteStatus::Accepted, /*terminate_error*/ None).await;

    process
        .terminate_confirmed()
        .await
        .expect("terminate should succeed");

    assert!(process.has_exited());
}

#[tokio::test]
async fn remote_process_waits_for_early_exit_event() {
    let (wake_tx, _wake_rx) = watch::channel(0);
    let read_count = Arc::new(AtomicUsize::new(0));
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "test-process".to_string().into(),
            write_response: WriteResponse {
                status: WriteStatus::Accepted,
            },
            read_responses: Mutex::new(VecDeque::new()),
            terminate_error: None,
            wake_tx,
            events: vec![
                ExecProcessEvent::Exited {
                    seq: 1,
                    exit_code: 17,
                    sandbox_denied: Some(false),
                },
                ExecProcessEvent::Closed { seq: 2 },
            ],
            read_count: Arc::clone(&read_count),
        }),
    };

    let process = UnifiedExecProcess::from_exec_server_started(started)
        .await
        .expect("remote process should observe early exit");

    assert!(process.has_exited());
    assert_eq!(process.exit_code(), Some(17));
    assert_eq!(read_count.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn remote_process_preserves_sandbox_denial_before_closed_event() {
    let (wake_tx, _wake_rx) = watch::channel(0);
    let read_count = Arc::new(AtomicUsize::new(0));
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "sandbox-denied".to_string().into(),
            write_response: WriteResponse {
                status: WriteStatus::Accepted,
            },
            read_responses: Mutex::new(VecDeque::new()),
            terminate_error: None,
            wake_tx,
            events: vec![ExecProcessEvent::Exited {
                seq: 1,
                exit_code: 1,
                sandbox_denied: Some(true),
            }],
            read_count: Arc::clone(&read_count),
        }),
    };

    let error = UnifiedExecProcess::from_exec_server_started(started)
        .await
        .expect_err("sandbox denial should be preserved");

    assert!(matches!(error, UnifiedExecError::SandboxDenied { .. }));
    assert_eq!(read_count.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn remote_process_reads_legacy_exit_event_for_sandbox_denial() {
    let (wake_tx, _wake_rx) = watch::channel(0);
    let read_count = Arc::new(AtomicUsize::new(0));
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "legacy-sandbox-denied".to_string().into(),
            write_response: WriteResponse {
                status: WriteStatus::Accepted,
            },
            read_responses: Mutex::new(VecDeque::from([ReadResponse {
                chunks: Vec::new(),
                next_seq: 2,
                exited: true,
                exit_code: Some(1),
                closed: false,
                failure: None,
                sandbox_denied: true,
            }])),
            terminate_error: None,
            wake_tx,
            events: vec![ExecProcessEvent::Exited {
                seq: 1,
                exit_code: 1,
                sandbox_denied: None,
            }],
            read_count: Arc::clone(&read_count),
        }),
    };

    let error = UnifiedExecProcess::from_exec_server_started(started)
        .await
        .expect_err("legacy exit should recover executor sandbox denial");

    assert!(matches!(error, UnifiedExecError::SandboxDenied { .. }));
    assert_eq!(read_count.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn remote_process_recovers_output_missing_from_event_replay() {
    let (wake_tx, _wake_rx) = watch::channel(0);
    let read_count = Arc::new(AtomicUsize::new(0));
    let recovered_chunks = vec![
        ProcessOutputChunk {
            seq: 1,
            stream: ExecOutputStream::Stdout,
            chunk: b"one".to_vec().into(),
        },
        ProcessOutputChunk {
            seq: 2,
            stream: ExecOutputStream::Stdout,
            chunk: b"two".to_vec().into(),
        },
    ];
    let started = StartedExecProcess {
        process: Arc::new(MockExecProcess {
            process_id: "truncated-replay".to_string().into(),
            write_response: WriteResponse {
                status: WriteStatus::Accepted,
            },
            read_responses: Mutex::new(VecDeque::from([ReadResponse {
                chunks: recovered_chunks,
                next_seq: 5,
                exited: true,
                exit_code: Some(0),
                closed: true,
                failure: None,
                sandbox_denied: false,
            }])),
            terminate_error: None,
            wake_tx,
            // A bounded event replay can begin after earlier events were evicted.
            events: vec![ExecProcessEvent::Output(ProcessOutputChunk {
                seq: 2,
                stream: ExecOutputStream::Stdout,
                chunk: b"two".to_vec().into(),
            })],
            read_count: Arc::clone(&read_count),
        }),
    };

    let process = UnifiedExecProcess::from_exec_server_started(started)
        .await
        .expect("missing replay events should recover through process/read");

    let output_handles = process.output_handles();
    assert_eq!(
        output_handles.output_buffer.lock().await.snapshot_chunks(),
        vec![b"one".to_vec(), b"two".to_vec()]
    );
    assert_eq!(process.exit_code(), Some(0));
    assert_eq!(read_count.load(Ordering::Relaxed), 1);
}
