use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::FunctionCallOutputContentItem;
use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::*;

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
    handle: CellHandle,
    initial_event_rx: oneshot::Receiver<Result<CellEvent, CellError>>,
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
    let handle = CellHandle::new(command_tx);
    let task = tokio::spawn(run_cell(
        Arc::new(TestHost),
        CellContext {
            runtime_tx,
            runtime_control_tx,
            runtime_terminate_handle,
            cancellation_token: CancellationToken::new(),
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
        handle,
        initial_event_rx,
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
            content_items: vec![CellOutputItem::Text {
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
