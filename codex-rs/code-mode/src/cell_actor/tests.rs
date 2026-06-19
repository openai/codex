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
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::session_runtime::OutputItem;

struct TestHost;

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
    let cancellation_token = CancellationToken::new();
    let handle = CellHandle::new(command_tx.clone());
    let task = tokio::spawn(run_cell(
        Arc::new(TestHost),
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
