use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use codex_code_mode_protocol::CodeModeOutputTruncation;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::host::BOUNDED_OUTPUT_MAX_IMAGE_BYTES;
use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::runtime::RuntimeEventSender;
use crate::session_runtime::OutputItem;

#[derive(Default)]
struct TestHost {
    commit_started: CancellationToken,
    resume_commit: Option<CancellationToken>,
}

#[derive(Default)]
struct RecordingHost {
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
        _stored_value_writes: HashMap<String, Arc<JsonValue>>,
        event: impl FnOnce() -> CellEvent + Send,
        cell_state: Arc<CellState>,
    ) -> CompletionCommit {
        self.commit_started.cancel();
        if let Some(resume) = &self.resume_commit {
            resume.cancelled().await;
        }
        cell_state.commit_completion(event, || {})
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
        _stored_value_writes: HashMap<String, Arc<JsonValue>>,
        event: impl FnOnce() -> CellEvent + Send,
        cell_state: Arc<CellState>,
    ) -> CompletionCommit {
        cell_state.commit_completion(event, || {})
    }

    async fn closed(&self) {}
}

struct CellActorHarness {
    event_tx: RuntimeEventSender,
    handle: CellHandle,
    yield_signal: CancellationToken,
    initial_event_rx: oneshot::Receiver<Result<CellEvent, CellError>>,
    task: tokio::task::JoinHandle<()>,
    runtime_control_rx: std_mpsc::Receiver<RuntimeControlCommand>,
    _runtime_event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
}

fn spawn_cell_actor_harness(initial_observe_mode: ObserveMode) -> CellActorHarness {
    spawn_cell_actor_harness_with_host(initial_observe_mode, Arc::new(TestHost::default()))
}

fn spawn_cell_actor_harness_with_host<H: CellHost>(
    initial_observe_mode: ObserveMode,
    host: Arc<H>,
) -> CellActorHarness {
    spawn_cell_actor_harness_with_host_and_failure_handler(
        initial_observe_mode,
        host,
        CodeModeOutputTruncation::Client,
        /*task_failure_handler*/ None,
    )
}

fn spawn_cell_actor_harness_with_host_and_failure_handler<H: CellHost>(
    initial_observe_mode: ObserveMode,
    host: Arc<H>,
    output_truncation: CodeModeOutputTruncation,
    task_failure_handler: Option<TaskFailureHandler>,
) -> CellActorHarness {
    let (event_tx, event_rx) = runtime_event_channel(output_truncation);
    let output = OutputCollector::new(output_truncation);
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (initial_event_tx, initial_event_rx) = oneshot::channel();
    let (runtime_event_tx, runtime_event_rx) = mpsc::unbounded_channel();
    let (runtime_tx, _runtime_control_tx, runtime_terminate_handle) = spawn_runtime(
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
        /*task_failure_handler*/ None,
    )
    .unwrap();
    let (runtime_control_tx, runtime_control_rx) = std_mpsc::channel();
    let cell_state = Arc::new(CellState::new(CancellationToken::new()));
    let handle = CellHandle::new(command_tx, Arc::clone(&cell_state));
    let yield_signal = CancellationToken::new();
    let task = tokio::spawn(run_cell(
        host,
        CellContext {
            runtime_tx,
            runtime_control_tx,
            runtime_terminate_handle,
            cell_state,
            output,
        },
        event_rx,
        command_rx,
        Observer {
            observation: initial_observe_mode.with_yield_signal(yield_signal.clone()),
            response_tx: initial_event_tx,
        },
        task_failure_handler,
    ));

    CellActorHarness {
        event_tx,
        handle,
        yield_signal,
        initial_event_rx,
        task,
        runtime_control_rx,
        _runtime_event_rx: runtime_event_rx,
    }
}

#[tokio::test]
async fn termination_drains_a_send_already_blocked_by_backpressure() {
    let harness = spawn_cell_actor_harness_with_host_and_failure_handler(
        ObserveMode::YieldAfter(Duration::from_secs(60)),
        Arc::new(TestHost::default()),
        CodeModeOutputTruncation::Server,
        /*task_failure_handler*/ None,
    );
    let (termination, capacity) = {
        let RuntimeEventSender::Bounded(tx) = &harness.event_tx else {
            panic!("server runtime must use a bounded sender");
        };
        // This test owns the V8-side events. The current-thread actor cannot run
        // before our first await; fill its actual FIFO and register a sender.
        for _ in 0..tx.max_capacity() {
            tx.try_send(RuntimeEvent::ContentItem(OutputItem::InputText {
                text: "queued".to_string(),
            }))
            .unwrap();
        }
        let mut send = std::pin::pin!(tx.send(RuntimeEvent::ContentItem(OutputItem::InputText {
            text: "blocked".to_string(),
        })));
        assert!(futures::poll!(send.as_mut()).is_pending());
        // terminate() synchronously cancels. No event was consumed between the
        // Pending poll and cancellation; run_cell must drain while terminating.
        let termination = harness.handle.terminate();
        tokio::time::timeout(Duration::from_secs(1), send)
            .await
            .expect("termination must release the registered sender")
            .expect("actor must drain the blocked item");
        (termination, tx.max_capacity())
    };
    drop(harness.event_tx);
    let event = tokio::time::timeout(Duration::from_secs(1), termination)
        .await
        .expect("termination must finish after draining")
        .expect("termination event");
    let mut expected = OutputCollector::new(CodeModeOutputTruncation::Server);
    for _ in 0..capacity {
        expected.push(OutputItem::InputText {
            text: "queued".to_string(),
        });
    }
    expected.push(OutputItem::InputText {
        text: "blocked".to_string(),
    });
    assert_eq!(
        event,
        CellEvent::Terminated {
            content_items: expected,
        }
    );
    assert_eq!(harness.initial_event_rx.await.unwrap(), Ok(event));
    tokio::time::timeout(Duration::from_secs(1), harness.task)
        .await
        .expect("actor must close")
        .unwrap();
}

#[tokio::test]
async fn unexpected_runtime_thread_exit_is_reported_to_the_session_owner() {
    let (failure_tx, mut failure_rx) = mpsc::unbounded_channel();
    let harness = spawn_cell_actor_harness_with_host_and_failure_handler(
        ObserveMode::YieldAfter(Duration::from_secs(60)),
        Arc::new(TestHost::default()),
        CodeModeOutputTruncation::Client,
        Some(Arc::new(move |reason| {
            let _ = failure_tx.send(reason);
        })),
    );
    drop(harness.event_tx);

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), failure_rx.recv())
            .await
            .expect("runtime failure timeout")
            .expect("runtime failure"),
        "code-mode V8 runtime thread ended unexpectedly"
    );
    assert!(
        harness
            .initial_event_rx
            .await
            .expect("initial event")
            .is_ok()
    );
    harness.task.await.expect("cell task");
}

#[tokio::test]
async fn termination_during_completion_commit_preserves_output_and_discards_losing_error() {
    for mode in [
        CodeModeOutputTruncation::Client,
        CodeModeOutputTruncation::Server,
    ] {
        // The actor has received Result but cannot commit until the host releases
        // the session store. Termination must decide which output is published.
        let resume_commit = CancellationToken::new();
        let host = Arc::new(TestHost {
            resume_commit: Some(resume_commit.clone()),
            ..TestHost::default()
        });
        let harness = spawn_cell_actor_harness_with_host_and_failure_handler(
            ObserveMode::YieldAfter(Duration::from_secs(60)),
            Arc::clone(&host),
            mode,
            /*task_failure_handler*/ None,
        );
        // Simulate V8's dedicated thread, where server-mode sends can block
        // while the event queue is full.
        let event_tx = harness.event_tx.clone();
        tokio::task::spawn_blocking(move || {
            event_tx
                .blocking_send(RuntimeEvent::ContentItem(OutputItem::InputText {
                    text: "output that must survive termination".to_string(),
                }))
                .expect("output");
            event_tx
                .blocking_send(RuntimeEvent::Result {
                    stored_value_writes: HashMap::new(),
                    error_text: Some("uncommitted error".repeat(128 * 1024)),
                })
                .expect("completion");
        })
        .await
        .expect("runtime producer");
        tokio::time::timeout(Duration::from_secs(5), host.commit_started.cancelled())
            .await
            .expect("commit reached");
        let termination = harness.handle.terminate();
        resume_commit.cancel();
        let mut expected_output = OutputCollector::new(mode);
        expected_output.push(OutputItem::InputText {
            text: "output that must survive termination".to_string(),
        });
        let expected = Ok(CellEvent::Terminated {
            content_items: expected_output,
        });
        assert_eq!(termination.await, expected, "{mode:?}");
        assert_eq!(harness.initial_event_rx.await.expect("observer"), expected);
        harness.task.await.expect("cell task");
    }
}

#[tokio::test]
async fn runtime_thread_panic_remains_a_cell_error_without_owner_supervision() {
    let harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::from_secs(60)));
    harness
        .event_tx
        .blocking_send(RuntimeEvent::ThreadPanicked)
        .expect("runtime panic event");
    drop(harness.event_tx);

    assert_eq!(
        harness.initial_event_rx.await.expect("initial event"),
        Ok(CellEvent::Completed {
            content_items: OutputCollector::Client(Vec::new()),
            result_buffered_for: Duration::ZERO,
            error_text: Some("exec runtime ended unexpectedly".to_string()),
        })
    );
    harness.task.await.expect("cell task");
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
async fn yield_timer_preempts_buffered_runtime_output() {
    let harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::ZERO));
    harness
        .event_tx
        .blocking_send(RuntimeEvent::Started)
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::ContentItem(OutputItem::InputText {
            text: "queued output".to_string(),
        }))
        .unwrap();

    assert_eq!(
        harness.initial_event_rx.await.unwrap(),
        Ok(CellEvent::Yielded {
            content_items: OutputCollector::Client(Vec::new()),
        })
    );

    let termination = harness.handle.terminate();
    drop(harness.event_tx);
    assert_eq!(
        termination.await,
        Ok(CellEvent::Terminated {
            content_items: OutputCollector::Client(vec![OutputItem::InputText {
                text: "queued output".to_string(),
            }]),
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn queued_termination_preempts_unobserved_runtime_completion() {
    let harness = spawn_cell_actor_harness(ObserveMode::YieldAfter(Duration::ZERO));
    harness
        .event_tx
        .blocking_send(RuntimeEvent::Started)
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::YieldRequested)
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::ContentItem(OutputItem::InputText {
            text: "queued output".to_string(),
        }))
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::Result {
            stored_value_writes: HashMap::new(),
            error_text: None,
        })
        .unwrap();
    harness.yield_signal.cancel();
    let termination = harness.handle.terminate();

    let terminated = Ok(CellEvent::Terminated {
        content_items: OutputCollector::Client(vec![OutputItem::InputText {
            text: "queued output".to_string(),
        }]),
    });
    assert_eq!(termination.await, terminated.clone());
    assert_eq!(harness.initial_event_rx.await.unwrap(), terminated);
    harness.task.await.unwrap();
}

#[tokio::test]
async fn observation_dropped_before_dequeue_does_not_consume_output() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(
        ObserveMode::YieldAfter(Duration::from_secs(60)),
        Arc::clone(&host),
    );
    harness
        .event_tx
        .blocking_send(RuntimeEvent::YieldRequested)
        .unwrap();
    assert!(harness.initial_event_rx.await.unwrap().is_ok());

    drop(
        harness
            .handle
            .observe(ObserveMode::YieldAfter(Duration::from_secs(60))),
    );
    harness
        .event_tx
        .blocking_send(RuntimeEvent::ContentItem(OutputItem::InputText {
            text: "survives pre-dequeue cancellation".to_string(),
        }))
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::YieldRequested)
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::Notify {
            call_id: "after-dropped-command".to_string(),
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
            content_items: OutputCollector::Client(vec![OutputItem::InputText {
                text: "survives pre-dequeue cancellation".to_string(),
            }]),
        })
    );

    let termination = harness.handle.terminate();
    drop(harness.event_tx);
    assert_eq!(
        termination.await,
        Ok(CellEvent::Terminated {
            content_items: OutputCollector::Client(Vec::new()),
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn dropped_yield_observer_preserves_output_for_the_next_observation() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(
        ObserveMode::YieldAfter(Duration::from_secs(60)),
        Arc::clone(&host),
    );
    harness
        .event_tx
        .blocking_send(RuntimeEvent::YieldRequested)
        .unwrap();
    assert!(harness.initial_event_rx.await.unwrap().is_ok());

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
        .blocking_send(RuntimeEvent::ContentItem(OutputItem::InputText {
            text: "survives active cancellation".to_string(),
        }))
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::YieldRequested)
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::Notify {
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
            content_items: OutputCollector::Client(vec![OutputItem::InputText {
                text: "survives active cancellation".to_string(),
            }]),
        })
    );

    let termination = harness.handle.terminate();
    drop(harness.event_tx);
    assert_eq!(
        termination.await,
        Ok(CellEvent::Terminated {
            content_items: OutputCollector::Client(Vec::new()),
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test]
async fn dropped_pending_observer_preserves_the_frontier_for_the_next_observation() {
    let host = Arc::new(RecordingHost::default());
    let harness = spawn_cell_actor_harness_with_host(
        ObserveMode::YieldAfter(Duration::from_secs(60)),
        Arc::clone(&host),
    );
    harness
        .event_tx
        .blocking_send(RuntimeEvent::YieldRequested)
        .unwrap();
    assert!(harness.initial_event_rx.await.unwrap().is_ok());

    let dropped_observation = harness.handle.observe(ObserveMode::PendingFrontier);
    assert_eq!(
        harness.handle.observe(ObserveMode::PendingFrontier).await,
        Err(CellError::Busy)
    );
    drop(dropped_observation);
    let item = OutputItem::InputText {
        text: "before frontier".to_owned(),
    };
    harness
        .event_tx
        .blocking_send(RuntimeEvent::ContentItem(item.clone()))
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::ToolCall {
            id: "tool-1".to_string(),
            name: codex_protocol::ToolName {
                name: "echo".to_string(),
                namespace: None,
            },
            kind: codex_code_mode_protocol::CodeModeToolKind::Function,
            input: Some(serde_json::json!({})),
        })
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::Pending)
        .unwrap();
    harness
        .event_tx
        .blocking_send(RuntimeEvent::Notify {
            call_id: "after-dropped-pending".to_string(),
            text: "barrier".to_string(),
        })
        .unwrap();
    wait_for_notification(&host).await;

    assert_eq!(
        harness.handle.observe(ObserveMode::PendingFrontier).await,
        Ok(CellEvent::Pending {
            content_items: OutputCollector::Client(vec![item]),
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
            content_items: OutputCollector::Client(Vec::new()),
        })
    );
    harness.task.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn only_the_first_termination_claims_a_buffered_completion() {
    let cell_state = CellState::new(CancellationToken::new());
    let completion = CellEvent::Completed {
        content_items: OutputCollector::Client(Vec::new()),
        result_buffered_for: Duration::ZERO,
        error_text: None,
    };
    assert_eq!(
        cell_state.commit_completion(|| completion.clone(), || {}),
        CompletionCommit::Committed
    );
    assert!(matches!(
        cell_state.deliver_completion(/*response_tx*/ None),
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
            content_items: OutputCollector::Client(Vec::new()),
        }),
        Some(completion)
    );
}

#[tokio::test]
async fn termination_claim_prevents_stored_value_commit() {
    let cell_state = CellState::new(CancellationToken::new());
    let termination = cell_state.request_termination();
    let mut commit_ran = false;
    assert_eq!(
        cell_state.commit_completion(
            || panic!("terminated cell must not construct completion output"),
            || commit_ran = true
        ),
        CompletionCommit::Rejected
    );
    assert!(!commit_ran);

    let terminated = CellEvent::Terminated {
        content_items: OutputCollector::Client(Vec::new()),
    };
    assert_eq!(
        cell_state.finish_termination(terminated.clone()),
        Some(terminated.clone())
    );
    assert_eq!(termination.await, Ok(terminated));
}

#[tokio::test(start_paused = true)]
async fn failed_completion_delivery_rebuffers_the_event() {
    let cell_state = CellState::new(CancellationToken::new());
    let completion = |result_buffered_for| CellEvent::Completed {
        content_items: OutputCollector::Client(vec![OutputItem::InputText {
            text: "done".to_string(),
        }]),
        result_buffered_for,
        error_text: None,
    };
    assert_eq!(
        cell_state.commit_completion(|| completion(Duration::ZERO), || {}),
        CompletionCommit::Committed
    );
    let (response_tx, response_rx) = oneshot::channel();
    drop(response_rx);
    assert!(matches!(
        cell_state.deliver_completion(Some(response_tx)),
        CompletionDelivery::Buffered
    ));
    assert!(cell_state.accepting_observations());

    let interval = Duration::from_secs(5);
    tokio::time::advance(interval).await;
    let (response_tx, response_rx) = oneshot::channel();
    drop(response_rx);
    assert!(matches!(
        cell_state.route_observation(response_tx),
        ObservationDelivery::Buffered
    ));

    tokio::time::advance(interval).await;
    let (response_tx, mut response_rx) = oneshot::channel();
    assert!(matches!(
        cell_state.route_observation(response_tx),
        ObservationDelivery::Delivered
    ));
    assert_eq!(response_rx.try_recv(), Ok(Ok(completion(2 * interval))));
}

#[test]
fn a_failed_yield_restores_image_allocation_and_omission_order() {
    let mut output = OutputCollector::new(CodeModeOutputTruncation::Server);
    let image_url = "i".repeat(BOUNDED_OUTPUT_MAX_IMAGE_BYTES);
    let pointer = image_url.as_ptr();
    output.push(OutputItem::InputImage {
        image_url,
        detail: None,
    });
    output.push(OutputItem::InputImage {
        image_url: "excess".to_string(),
        detail: None,
    });
    let (tx, rx) = oneshot::channel();
    drop(rx);
    send_yield(&mut output, tx);

    output.push(OutputItem::InputText {
        text: "after".to_string(),
    });
    let (tx, mut rx) = oneshot::channel();
    send_yield(&mut output, tx);
    let CellEvent::Yielded { content_items } = rx.try_recv().unwrap().unwrap() else {
        panic!("expected a yield");
    };
    let items = crate::output::render_output(
        content_items.into_collected_output(),
        /*max_output_tokens*/ None,
    );
    let [
        OutputItem::InputImage { image_url, .. },
        OutputItem::InputText { text: notice },
        OutputItem::InputText { text },
    ] = items.as_slice()
    else {
        panic!("expected image, omission notice, and subsequent text");
    };
    assert_eq!(image_url.as_ptr(), pointer);
    assert!(notice.contains("1 images"), "{notice}");
    assert_eq!(text, "after");
    assert_eq!(
        output.take(),
        OutputCollector::new(CodeModeOutputTruncation::Server)
    );
}
