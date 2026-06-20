use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc as std_mpsc;
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

#[derive(Default)]
struct RecordingHost {
    committed: AtomicBool,
    notified: AtomicBool,
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

    async fn commit_completion(
        &self,
        _stored_value_writes: HashMap<String, JsonValue>,
        event: CellEvent,
        pending_initial_yield_items: Option<Vec<OutputItem>>,
        cell_state: Arc<CellState>,
    ) -> CompletionCommit {
        cell_state.commit_completion(event, pending_initial_yield_items, || {})
    }

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
        self.notified.store(true, Ordering::Release);
        Ok(())
    }

    async fn commit_completion(
        &self,
        _stored_value_writes: HashMap<String, JsonValue>,
        event: CellEvent,
        pending_initial_yield_items: Option<Vec<OutputItem>>,
        cell_state: Arc<CellState>,
    ) -> CompletionCommit {
        cell_state.commit_completion(event, pending_initial_yield_items, || {
            self.committed.store(true, Ordering::Release);
        })
    }

    async fn closed(&self) {}
}

struct CellActorHarness {
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    handle: CellHandle,
    task: tokio::task::JoinHandle<()>,
    runtime_tx: std_mpsc::Sender<RuntimeCommand>,
    runtime_control_rx: std_mpsc::Receiver<RuntimeControlCommand>,
    _runtime_pause_tx: std_mpsc::Sender<RuntimeControlCommand>,
    _runtime_event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
}

fn spawn_cell_actor_harness() -> CellActorHarness {
    spawn_cell_actor_harness_with_host(Arc::new(TestHost))
}

fn spawn_cell_actor_harness_with_host<H: CellHost>(host: Arc<H>) -> CellActorHarness {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (runtime_event_tx, runtime_event_rx) = mpsc::unbounded_channel();
    let (runtime_tx, runtime_pause_tx, runtime_terminate_handle) = spawn_runtime(
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
    let (runtime_control_tx, runtime_control_rx) = std_mpsc::channel();
    let cell_state = Arc::new(CellState::new(CancellationToken::new()));
    let handle = CellHandle::new(command_tx, Arc::clone(&cell_state));
    let task = tokio::spawn(run_cell(
        host,
        CellContext {
            runtime_tx: runtime_tx.clone(),
            runtime_control_tx,
            runtime_terminate_handle,
            cell_state,
        },
        event_rx,
        command_rx,
    ));

    CellActorHarness {
        event_tx,
        handle,
        task,
        runtime_tx,
        runtime_control_rx,
        _runtime_pause_tx: runtime_pause_tx,
        _runtime_event_rx: runtime_event_rx,
    }
}

async fn wait_for_notification(host: &RecordingHost) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !host.notified.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("notification barrier timed out");
}

#[tokio::test]
async fn completion_and_output_are_buffered_until_the_first_observation() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "before observation".to_string(),
            },
        ))
        .unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::new(),
            error_text: None,
        })
        .unwrap();
    while !host.committed.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }

    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::ZERO))
            .await,
        Ok(CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "before observation".to_string(),
            }],
            error_text: None,
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn pending_frontier_is_buffered_while_runtime_commands_are_queued() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    harness.event_tx.send(RuntimeEvent::Pending).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "notify-1".to_string(),
            text: "pending processed".to_string(),
        })
        .unwrap();
    while !host.notified.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    harness
        .runtime_tx
        .send(RuntimeCommand::ToolResponse {
            id: "tool-1".to_string(),
            result: JsonValue::Null,
        })
        .unwrap();

    assert!(matches!(
        harness.runtime_control_rx.try_recv(),
        Err(std_mpsc::TryRecvError::Empty)
    ));
    assert_eq!(
        harness.handle.observe(ObserveMode::PendingFrontier).await,
        Ok(CellEvent::Pending {
            content_items: Vec::new(),
            pending_tool_call_ids: Vec::new(),
        })
    );
    assert!(matches!(
        harness.runtime_control_rx.try_recv(),
        Err(std_mpsc::TryRecvError::Empty)
    ));

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
async fn buffered_yield_observation_resumes_an_unobserved_pending_frontier() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    harness.event_tx.send(RuntimeEvent::Pending).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "notify-1".to_string(),
            text: "pending processed".to_string(),
        })
        .unwrap();
    while !host.notified.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }

    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::from_secs(60)))
            .await,
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );
    loop {
        match harness.runtime_control_rx.try_recv() {
            Ok(RuntimeControlCommand::Continue) => break,
            Ok(command) => panic!("expected continue, got {command:?}"),
            Err(std_mpsc::TryRecvError::Empty) => tokio::task::yield_now().await,
            Err(std_mpsc::TryRecvError::Disconnected) => {
                panic!("runtime control channel disconnected")
            }
        }
    }

    host.notified.store(false, Ordering::Release);
    harness.event_tx.send(RuntimeEvent::Pending).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "notify-2".to_string(),
            text: "later pending processed".to_string(),
        })
        .unwrap();
    while !host.notified.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    assert!(matches!(
        harness.runtime_control_rx.try_recv(),
        Ok(RuntimeControlCommand::Continue)
    ));

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
async fn first_observation_preserves_a_yield_that_raced_with_creation() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            },
        ))
        .unwrap();
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            },
        ))
        .unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "after-initial-yield".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(1),
            harness
                .handle
                .observe(ObserveMode::YieldAfter(Duration::from_secs(60))),
        )
        .await
        .expect("initial yield was not preserved"),
        Ok(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "before".to_string(),
            }],
        })
    );
    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::ZERO))
            .await,
        Ok(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "after".to_string(),
            }],
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
async fn dropped_pending_observer_preserves_pre_observation_yield() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            },
        ))
        .unwrap();
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            },
        ))
        .unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "before-pending-observer".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;
    host.notified.store(false, Ordering::Release);

    let dropped_observation = harness.handle.observe(ObserveMode::PendingFrontier);
    assert_eq!(
        harness.handle.observe(ObserveMode::PendingFrontier).await,
        Err(CellError::Busy)
    );
    drop(dropped_observation);
    harness.event_tx.send(RuntimeEvent::Pending).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "after-dropped-pending".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(1),
            harness
                .handle
                .observe(ObserveMode::YieldAfter(Duration::from_secs(60))),
        )
        .await
        .expect("initial yield was not preserved after failed pending delivery"),
        Ok(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "before".to_string(),
            }],
        })
    );

    let termination = harness.handle.terminate();
    drop(harness.event_tx);
    assert_eq!(
        termination.await,
        Ok(CellEvent::Terminated {
            content_items: vec![OutputItem::Text {
                text: "after".to_string(),
            }],
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn dropped_pending_observer_preserves_pre_observation_yield_at_completion() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            },
        ))
        .unwrap();
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            },
        ))
        .unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "before-pending-observer".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;

    let dropped_observation = harness.handle.observe(ObserveMode::PendingFrontier);
    assert_eq!(
        harness.handle.observe(ObserveMode::PendingFrontier).await,
        Err(CellError::Busy)
    );
    drop(dropped_observation);
    harness
        .event_tx
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::new(),
            error_text: None,
        })
        .unwrap();
    while !host.committed.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(1),
            harness
                .handle
                .observe(ObserveMode::YieldAfter(Duration::from_secs(60))),
        )
        .await
        .expect("initial yield was not preserved after failed completion delivery"),
        Ok(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "before".to_string(),
            }],
        })
    );
    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::ZERO))
            .await,
        Ok(CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "after".to_string(),
            }],
            error_text: None,
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn unattached_yield_after_the_first_observation_is_a_no_op() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::ZERO))
            .await,
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "before ignored yield".to_string(),
            },
        ))
        .unwrap();
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "after-ignored-yield".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;

    let observation = harness
        .handle
        .observe(ObserveMode::YieldAfter(Duration::from_secs(60)));
    tokio::pin!(observation);
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 20), &mut observation)
            .await
            .is_err()
    );

    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "after observer attached".to_string(),
            },
        ))
        .unwrap();
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    assert_eq!(
        observation.await,
        Ok(CellEvent::Yielded {
            content_items: vec![
                OutputItem::Text {
                    text: "before ignored yield".to_string(),
                },
                OutputItem::Text {
                    text: "after observer attached".to_string(),
                },
            ],
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
async fn yield_timer_preempts_buffered_runtime_output() {
    let harness = spawn_cell_actor_harness();
    let initial_event = harness
        .handle
        .observe(ObserveMode::YieldAfter(Duration::ZERO));
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
        initial_event.await,
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
    let harness = spawn_cell_actor_harness();
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
    assert_eq!(termination.await, terminated);
    harness.task.await.unwrap();
}

#[tokio::test]
async fn observation_dropped_before_dequeue_does_not_consume_output() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));

    drop(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::from_secs(60))),
    );
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "survives pre-dequeue cancellation".to_string(),
            },
        ))
        .unwrap();
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "after-dropped-command".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;

    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(1),
            harness
                .handle
                .observe(ObserveMode::YieldAfter(Duration::from_secs(60))),
        )
        .await
        .expect("initial yield was not preserved"),
        Ok(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "survives pre-dequeue cancellation".to_string(),
            }],
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
async fn dropped_yield_observer_preserves_output_for_the_next_observation() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::ZERO))
            .await,
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    let dropped_observation = harness
        .handle
        .observe(ObserveMode::YieldAfter(Duration::from_secs(60)));
    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::ZERO))
            .await,
        Err(CellError::Busy)
    );
    drop(dropped_observation);
    harness
        .event_tx
        .send(RuntimeEvent::ContentItem(
            FunctionCallOutputContentItem::InputText {
                text: "survives active cancellation".to_string(),
            },
        ))
        .unwrap();
    harness.event_tx.send(RuntimeEvent::YieldRequested).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "after-dropped-observer".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;

    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::ZERO))
            .await,
        Ok(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "survives active cancellation".to_string(),
            }],
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
async fn dropped_pending_observer_preserves_the_frontier_for_the_next_observation() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(Arc::clone(&host));
    assert_eq!(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::ZERO))
            .await,
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    let dropped_observation = harness.handle.observe(ObserveMode::PendingFrontier);
    assert_eq!(
        harness.handle.observe(ObserveMode::PendingFrontier).await,
        Err(CellError::Busy)
    );
    drop(dropped_observation);
    harness
        .event_tx
        .send(RuntimeEvent::ToolCall {
            id: "tool-1".to_string(),
            name: codex_protocol::ToolName {
                name: "echo".to_string(),
                namespace: None,
            },
            kind: codex_code_mode_protocol::CodeModeToolKind::Function,
            input: Some(serde_json::json!({})),
        })
        .unwrap();
    harness.event_tx.send(RuntimeEvent::Pending).unwrap();
    harness
        .event_tx
        .send(RuntimeEvent::Notify {
            call_id: "after-dropped-pending".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;

    assert_eq!(
        harness.handle.observe(ObserveMode::PendingFrontier).await,
        Ok(CellEvent::Pending {
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string()],
        })
    );
    assert!(matches!(
        harness.runtime_control_rx.try_recv(),
        Err(std_mpsc::TryRecvError::Empty)
    ));

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
async fn only_the_first_termination_claims_a_buffered_completion() {
    let cell_state = CellState::new(CancellationToken::new());
    let completion = CellEvent::Completed {
        content_items: Vec::new(),
        error_text: None,
    };
    assert_eq!(
        cell_state.commit_completion(
            completion.clone(),
            /*pending_initial_yield_items*/ None,
            || {}
        ),
        CompletionCommit::Committed
    );
    assert!(matches!(
        cell_state.deliver_completion(/*observer*/ None),
        CompletionDelivery::Buffered
    ));

    let first_termination = cell_state.request_termination();
    assert_eq!(
        cell_state.request_termination().await,
        Err(CellError::AlreadyTerminating)
    );
    assert_eq!(first_termination.await, Ok(completion.clone()));
    assert_eq!(
        cell_state.finish_termination(CellEvent::Terminated {
            content_items: Vec::new(),
        }),
        Some(completion)
    );
}

#[tokio::test]
async fn termination_claim_prevents_stored_value_commit() {
    let cell_state = CellState::new(CancellationToken::new());
    let termination = cell_state.request_termination();
    let mut commit_ran = false;
    let completion = CellEvent::Completed {
        content_items: vec![OutputItem::Text {
            text: "after yield".to_string(),
        }],
        error_text: None,
    };

    assert_eq!(
        cell_state.commit_completion(
            completion,
            Some(vec![OutputItem::Text {
                text: "before yield".to_string(),
            }]),
            || commit_ran = true
        ),
        CompletionCommit::Rejected(CellEvent::Completed {
            content_items: vec![
                OutputItem::Text {
                    text: "before yield".to_string(),
                },
                OutputItem::Text {
                    text: "after yield".to_string(),
                },
            ],
            error_text: None,
        })
    );
    assert!(!commit_ran);

    let terminated = CellEvent::Terminated {
        content_items: Vec::new(),
    };
    assert_eq!(
        cell_state.finish_termination(terminated.clone()),
        Some(terminated.clone())
    );
    assert_eq!(termination.await, Ok(terminated));
}

#[test]
fn failed_completion_delivery_rebuffers_the_event() {
    let cell_state = CellState::new(CancellationToken::new());
    let event = CellEvent::Completed {
        content_items: Vec::new(),
        error_text: None,
    };
    assert_eq!(
        cell_state.commit_completion(
            event.clone(),
            /*pending_initial_yield_items*/ None,
            || {}
        ),
        CompletionCommit::Committed
    );
    let (response_tx, response_rx) = oneshot::channel();
    drop(response_rx);
    assert!(matches!(
        cell_state.deliver_completion(Some(
            (ObserveMode::YieldAfter(Duration::ZERO), response_tx,)
        )),
        CompletionDelivery::Buffered
    ));
    assert!(cell_state.accepting_observations());

    let (response_tx, mut response_rx) = oneshot::channel();
    assert!(matches!(
        cell_state.route_observation(ObserveMode::YieldAfter(Duration::ZERO), response_tx),
        ObservationDelivery::Delivered
    ));
    assert_eq!(response_rx.try_recv(), Ok(Ok(event)));
}

#[test]
fn buffered_initial_yield_precedes_buffered_completion_for_yield_observer() {
    let cell_state = CellState::new(CancellationToken::new());
    let completion = CellEvent::Completed {
        content_items: vec![OutputItem::Text {
            text: "after".to_string(),
        }],
        error_text: None,
    };
    assert_eq!(
        cell_state.commit_completion(
            completion.clone(),
            Some(vec![OutputItem::Text {
                text: "before".to_string(),
            }]),
            || {}
        ),
        CompletionCommit::Committed
    );
    assert!(matches!(
        cell_state.deliver_completion(/*observer*/ None),
        CompletionDelivery::Buffered
    ));

    let (response_tx, mut response_rx) = oneshot::channel();
    assert!(matches!(
        cell_state.route_observation(ObserveMode::YieldAfter(Duration::ZERO), response_tx),
        ObservationDelivery::Buffered
    ));
    assert_eq!(
        response_rx.try_recv(),
        Ok(Ok(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "before".to_string(),
            }],
        }))
    );

    let (response_tx, mut response_rx) = oneshot::channel();
    assert!(matches!(
        cell_state.route_observation(ObserveMode::YieldAfter(Duration::ZERO), response_tx),
        ObservationDelivery::Delivered
    ));
    assert_eq!(response_rx.try_recv(), Ok(Ok(completion)));
}

#[test]
fn pending_observer_merges_initial_yield_and_completion_output() {
    let cell_state = CellState::new(CancellationToken::new());
    assert_eq!(
        cell_state.commit_completion(
            CellEvent::Completed {
                content_items: vec![OutputItem::Text {
                    text: "after".to_string(),
                }],
                error_text: None,
            },
            Some(vec![OutputItem::Text {
                text: "before".to_string(),
            }]),
            || {}
        ),
        CompletionCommit::Committed
    );
    assert!(matches!(
        cell_state.deliver_completion(/*observer*/ None),
        CompletionDelivery::Buffered
    ));

    let (response_tx, mut response_rx) = oneshot::channel();
    assert!(matches!(
        cell_state.route_observation(ObserveMode::PendingFrontier, response_tx),
        ObservationDelivery::Delivered
    ));
    assert_eq!(
        response_rx.try_recv(),
        Ok(Ok(CellEvent::Completed {
            content_items: vec![
                OutputItem::Text {
                    text: "before".to_string(),
                },
                OutputItem::Text {
                    text: "after".to_string(),
                },
            ],
            error_text: None,
        }))
    );
}

#[test]
fn dropped_pending_observation_preserves_the_initial_yield_boundary() {
    let cell_state = CellState::new(CancellationToken::new());
    let completion = CellEvent::Completed {
        content_items: vec![OutputItem::Text {
            text: "after".to_string(),
        }],
        error_text: None,
    };
    assert_eq!(
        cell_state.commit_completion(
            completion.clone(),
            Some(vec![OutputItem::Text {
                text: "before".to_string(),
            }]),
            || {}
        ),
        CompletionCommit::Committed
    );
    assert!(matches!(
        cell_state.deliver_completion(/*observer*/ None),
        CompletionDelivery::Buffered
    ));

    let (response_tx, response_rx) = oneshot::channel();
    drop(response_rx);
    assert!(matches!(
        cell_state.route_observation(ObserveMode::PendingFrontier, response_tx),
        ObservationDelivery::Buffered
    ));

    let (response_tx, mut response_rx) = oneshot::channel();
    assert!(matches!(
        cell_state.route_observation(ObserveMode::YieldAfter(Duration::ZERO), response_tx),
        ObservationDelivery::Buffered
    ));
    assert_eq!(
        response_rx.try_recv(),
        Ok(Ok(CellEvent::Yielded {
            content_items: vec![OutputItem::Text {
                text: "before".to_string(),
            }],
        }))
    );

    let (response_tx, mut response_rx) = oneshot::channel();
    assert!(matches!(
        cell_state.route_observation(ObserveMode::YieldAfter(Duration::ZERO), response_tx),
        ObservationDelivery::Delivered
    ));
    assert_eq!(response_rx.try_recv(), Ok(Ok(completion)));
}
