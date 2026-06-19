use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::session_runtime::OutputItem;

struct TestHost;

#[derive(Default)]
struct RecordingHost {
    committed: AtomicBool,
    closed: AtomicBool,
}

struct BlockingCommitHost {
    commit_started_tx: mpsc::UnboundedSender<()>,
    commit_release: Semaphore,
}

impl CellHost for TestHost {
    async fn invoke_tool(
        &self,
        _invocation: CellToolCall,
        _cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        Err("unexpected tool call".to_string())
    }

    async fn notify(
        &self,
        _call_id: String,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn commit_stored_values(&self, _stored_value_writes: HashMap<String, JsonValue>) {}

    async fn closed(&self) {}
}

impl CellHost for RecordingHost {
    async fn invoke_tool(
        &self,
        _invocation: CellToolCall,
        _cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        Err("unexpected tool call".to_string())
    }

    async fn notify(
        &self,
        _call_id: String,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn commit_stored_values(&self, _stored_value_writes: HashMap<String, JsonValue>) {
        self.committed.store(true, Ordering::Release);
    }

    async fn closed(&self) {
        self.closed.store(true, Ordering::Release);
    }
}

impl BlockingCommitHost {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<()>) {
        let (commit_started_tx, commit_started_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                commit_started_tx,
                commit_release: Semaphore::new(/*permits*/ 0),
            }),
            commit_started_rx,
        )
    }
}

impl CellHost for BlockingCommitHost {
    async fn invoke_tool(
        &self,
        _invocation: CellToolCall,
        _cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        Err("unexpected tool call".to_string())
    }

    async fn notify(
        &self,
        _call_id: String,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        Ok(())
    }

    async fn commit_stored_values(&self, _stored_value_writes: HashMap<String, JsonValue>) {
        self.commit_started_tx
            .send(())
            .expect("test did not receive commit start");
        self.commit_release
            .acquire()
            .await
            .expect("test did not release commit")
            .forget();
    }

    async fn closed(&self) {}
}

struct CellActorHarness {
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    command_tx: mpsc::UnboundedSender<CellCommand>,
    handle: CellHandle,
    initial_event_rx: oneshot::Receiver<Result<CellEvent, CellError>>,
    session_shutdown_token: CancellationToken,
    task: tokio::task::JoinHandle<()>,
    _runtime_event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
}

fn spawn_cell_actor_harness(initial_observe_mode: ObserveMode) -> CellActorHarness {
    spawn_cell_actor_harness_with_host(initial_observe_mode, Arc::new(TestHost))
}

fn spawn_cell_actor_harness_with_host<H: CellHost>(
    initial_observe_mode: ObserveMode,
    host: Arc<H>,
) -> CellActorHarness {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (initial_event_tx, initial_event_rx) = oneshot::channel();
    let (runtime_event_tx, runtime_event_rx) = mpsc::unbounded_channel();
    let (runtime_tx, runtime_control_tx, runtime_terminate_handle) = spawn_runtime(
        HashMap::new(),
        ExecuteRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: Vec::new(),
            source: "await new Promise(() => {});".to_string(),
            yield_time_ms: None,
            max_output_tokens: None,
        },
        runtime_event_tx,
        PendingRuntimeMode::PauseUntilResumed,
    )
    .unwrap();
    let session_shutdown_token = CancellationToken::new();
    let cancellation_token = session_shutdown_token.child_token();
    let handle = CellHandle::new(command_tx.clone());
    let task = tokio::spawn(run_cell(
        host,
        CellContext {
            runtime_tx,
            runtime_control_tx,
            runtime_terminate_handle,
            cancellation_token,
            session_shutdown_token: session_shutdown_token.clone(),
        },
        event_rx,
        command_rx,
        Observer {
            mode: initial_observe_mode,
            response_tx: initial_event_tx,
        },
    ));

    CellActorHarness {
        event_tx,
        command_tx,
        handle,
        initial_event_rx,
        session_shutdown_token,
        task,
        _runtime_event_rx: runtime_event_rx,
    }
}

#[tokio::test]
async fn yield_timer_preempts_buffered_runtime_output() {
    let harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::ZERO));
    harness.event_tx.send(RuntimeEvent::Started).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "queued output".to_string(),
            },
        ))
        .unwrap();

    assert_eq!(
        harness.initial_event_rx.await.unwrap(),
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    let termination = harness.handle.terminate();
    drop(harness.event_tx);
    assert_eq!(
        termination.await,
        Ok(CellEvent::Terminated {
            content_items: vec![OutputItem::Text {
                text: "queued output".to_string(),
            }],
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn queued_termination_preempts_unobserved_runtime_completion() {
    let harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::from_secs(60)));
    harness
        .event_tx
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::new(),
            error_text: None,
        })
        .unwrap();
    let termination = harness.handle.terminate();

    let terminated = Ok(CellEvent::Terminated {
        content_items: Vec::new(),
    });
    assert_eq!(termination.await, terminated.clone());
    assert_eq!(harness.initial_event_rx.await.unwrap(), terminated);
    harness.task.await.unwrap();
}

#[tokio::test]
async fn termination_preempts_result_without_committing_stored_values() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(
        ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        Arc::clone(&host),
    );
    harness
        .event_tx
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::from([("key".to_string(), JsonValue::Bool(true))]),
            error_text: None,
        })
        .unwrap();
    let termination = harness.handle.terminate();

    let terminated = Ok(CellEvent::Terminated {
        content_items: Vec::new(),
    });
    assert_eq!(termination.await, terminated.clone());
    assert_eq!(harness.initial_event_rx.await.unwrap(), terminated);
    harness.task.await.unwrap();
    assert!(!host.committed.load(Ordering::Acquire));
    assert!(host.closed.load(Ordering::Acquire));
}

#[tokio::test]
async fn observer_receives_a_buffered_completion_after_commit_finishes() {
    let (host, mut commit_started_rx) = BlockingCommitHost::new();
    let harness = spawn_cell_actor_harness_with_host(
        ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        Arc::clone(&host),
    );
    harness.event_tx.send(RuntimeEvent::Started).unwrap();
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    assert_eq!(
        harness.initial_event_rx.await.unwrap(),
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    harness
        .event_tx
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::new(),
            error_text: None,
        })
        .unwrap();
    assert_eq!(commit_started_rx.recv().await, Some(()));

    let completion = harness
        .handle
        .observe(ObserveMode::YieldAfter(Duration::ZERO));
    host.commit_release.add_permits(/*n*/ 1);

    assert_eq!(
        completion.await,
        Ok(CellEvent::Completed {
            content_items: Vec::new(),
            error_text: None,
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn dropped_observer_does_not_block_the_next_observer() {
    let harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::from_secs(
        /*secs*/ 60,
    )));
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    assert_eq!(
        harness.initial_event_rx.await.unwrap(),
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    let abandoned_observer = harness
        .handle
        .observe(ObserveMode::YieldAfter(Duration::ZERO));
    drop(abandoned_observer);
    let next_observer = harness
        .handle
        .observe(ObserveMode::YieldAfter(Duration::ZERO));
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();

    assert_eq!(
        next_observer.await,
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    let termination = harness.handle.terminate();
    drop(harness.event_tx);
    assert_eq!(
        termination.await,
        Ok(CellEvent::Terminated {
            content_items: Vec::new(),
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn session_shutdown_terminates_the_active_observer() {
    let harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::from_secs(
        /*secs*/ 60,
    )));
    harness.session_shutdown_token.cancel();
    drop(harness.event_tx);

    assert_eq!(
        harness.initial_event_rx.await.unwrap(),
        Ok(CellEvent::Terminated {
            content_items: Vec::new(),
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn unexpected_runtime_close_reports_a_completed_error_and_closes_the_actor() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(
        ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        Arc::clone(&host),
    );
    drop(harness.event_tx);

    assert_eq!(
        harness.initial_event_rx.await.unwrap(),
        Ok(CellEvent::Completed {
            content_items: Vec::new(),
            error_text: Some("exec runtime ended unexpectedly".to_string()),
        })
    );
    harness.task.await.unwrap();
    assert!(host.closed.load(Ordering::Acquire));
}

#[tokio::test]
async fn termination_wins_when_the_runtime_channel_closes() {
    let harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::from_secs(
        /*secs*/ 60,
    )));
    let termination = harness.handle.terminate();
    drop(harness.event_tx);

    let terminated = Ok(CellEvent::Terminated {
        content_items: Vec::new(),
    });
    assert_eq!(termination.await, terminated.clone());
    assert_eq!(harness.initial_event_rx.await.unwrap(), terminated);
    harness.task.await.unwrap();
}

#[tokio::test]
async fn session_shutdown_preempts_continuous_command_traffic() {
    let mut harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::from_secs(
        /*secs*/ 60,
    )));
    let keep_sending = Arc::new(AtomicBool::new(true));
    let producer_keep_sending = Arc::clone(&keep_sending);
    let command_tx = harness.command_tx.clone();
    let (producer_started_tx, producer_started_rx) = oneshot::channel();
    let producer = thread::spawn(move || {
        let mut producer_started_tx = Some(producer_started_tx);
        while producer_keep_sending.load(Ordering::Relaxed) {
            let (response_tx, _response_rx) = oneshot::channel();
            if command_tx
                .send(CellCommand::Observe {
                    mode: ObserveMode::PendingFrontier,
                    response_tx,
                })
                .is_err()
            {
                break;
            }
            if let Some(producer_started_tx) = producer_started_tx.take() {
                let _ = producer_started_tx.send(());
            }
        }
    });
    producer_started_rx.await.unwrap();

    harness.session_shutdown_token.cancel();
    drop(harness.event_tx);

    let shutdown_result =
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut harness.task).await;

    keep_sending.store(false, Ordering::Relaxed);
    producer.join().unwrap();
    if shutdown_result.is_err() {
        harness.task.abort();
        let _ = harness.task.await;
    }

    match shutdown_result {
        Ok(task_result) => task_result.unwrap(),
        Err(_) => panic!("session shutdown did not finish while commands were queued"),
    }
}
