use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use codex_code_mode_protocol::CodeModeToolKind;
use codex_code_mode_protocol::DEFAULT_IMAGE_DETAIL;
use codex_code_mode_protocol::ToolDefinition;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::sync::mpsc;

use super::*;
use crate::runtime::RuntimeEvent;
use crate::runtime::spawn_runtime_thread;

struct CellHarness {
    handle: CellHandle,
    events: mpsc::UnboundedReceiver<CellEvent>,
    task: tokio::task::JoinHandle<Result<(), ActorError>>,
}

fn spawn_cell(source: &str, enabled_tools: Vec<ToolDefinition>) -> CellHarness {
    let request = CellRequest::new("call-1", enabled_tools, source).expect("valid cell request");
    let (handle, events, task) = CellActor::prepare(request, HashMap::new()).expect("prepare cell");
    CellHarness {
        handle,
        events,
        task: tokio::spawn(task),
    }
}

fn echo_tool() -> ToolDefinition {
    ToolDefinition {
        name: "echo".to_string(),
        tool_name: ToolName::plain("echo"),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    }
}

async fn next_event(events: &mut mpsc::UnboundedReceiver<CellEvent>) -> CellEvent {
    tokio::time::timeout(Duration::from_secs(2), events.recv())
        .await
        .expect("cell event timeout")
        .expect("cell event channel closed")
}

async fn assert_closed(mut harness: CellHarness) {
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), harness.events.recv())
            .await
            .expect("cell close timeout"),
        None
    );
    tokio::time::timeout(Duration::from_secs(2), harness.task)
        .await
        .expect("cell task timeout")
        .expect("cell task panicked")
        .expect("cell actor failed");
}

struct ControlledCell {
    handle: CellHandle,
    events: mpsc::UnboundedReceiver<CellEvent>,
    runtime_events: mpsc::UnboundedSender<RuntimeEvent>,
    runtime_commands: Option<std_mpsc::Receiver<RuntimeCommand>>,
    _runtime_controls: std_mpsc::Receiver<RuntimeControlCommand>,
    release_runtime: std_mpsc::Sender<()>,
    runtime_active: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<Result<(), ActorError>>,
}

fn controlled_cell(shutdown_timeout: Duration) -> ControlledCell {
    let (runtime_events, runtime_event_rx) = mpsc::unbounded_channel();
    let (runtime_command_tx, runtime_commands) = std_mpsc::channel();
    let (runtime_control_tx, runtime_controls) = std_mpsc::channel();
    let (release_runtime, release_runtime_rx) = std_mpsc::channel();
    let (runtime_started_tx, runtime_started_rx) = std_mpsc::sync_channel(/*bound*/ 1);
    let runtime_active = Arc::new(AtomicBool::new(false));
    let thread_runtime_active = Arc::clone(&runtime_active);
    let runtime_thread = spawn_runtime_thread(move || {
        thread_runtime_active.store(true, Ordering::Release);
        runtime_started_tx.send(()).expect("report runtime start");
        let _ = release_runtime_rx.recv();
        thread_runtime_active.store(false, Ordering::Release);
    });
    runtime_started_rx.recv().expect("runtime thread started");
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let (event_tx, events) = mpsc::unbounded_channel();
    let handle = CellHandle { command_tx };
    let task = tokio::spawn(run_cell(
        RuntimeHandle {
            command_tx: runtime_command_tx,
            control_tx: runtime_control_tx,
            terminator: RuntimeTerminator::Noop,
            thread: Some(runtime_thread),
        },
        runtime_event_rx,
        command_rx,
        event_tx,
        shutdown_timeout,
    ));
    ControlledCell {
        handle,
        events,
        runtime_events,
        runtime_commands: Some(runtime_commands),
        _runtime_controls: runtime_controls,
        release_runtime,
        runtime_active,
        task,
    }
}

async fn start_controlled(cell: &mut ControlledCell) {
    cell.runtime_events
        .send(RuntimeEvent::Started)
        .expect("queue runtime start");
    assert_eq!(next_event(&mut cell.events).await, CellEvent::Started);
}

async fn actor_result(
    task: tokio::task::JoinHandle<Result<(), ActorError>>,
) -> Result<(), ActorError> {
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("cell task timeout")
        .expect("cell task panicked")
}

#[tokio::test]
async fn synchronous_execution_emits_granular_events_in_actor_order() {
    let mut harness = spawn_cell(
        r#"
text("before");
image("data:image/png;base64,AAA");
notify("notice");
yield_control();
yield_control();
text("after");
store("answer", 42);
"#,
        Vec::new(),
    );

    let mut events = Vec::new();
    loop {
        let event = next_event(&mut harness.events).await;
        let terminal = matches!(event, CellEvent::Completed { .. } | CellEvent::Terminated);
        events.push(event);
        if terminal {
            break;
        }
    }
    assert_eq!(
        events,
        vec![
            CellEvent::Started,
            CellEvent::OutputText {
                text: "before".to_string(),
            },
            CellEvent::OutputImage {
                image_url: "data:image/png;base64,AAA".to_string(),
                detail: DEFAULT_IMAGE_DETAIL,
            },
            CellEvent::Notification {
                call_id: "call-1".to_string(),
                text: "notice".to_string(),
            },
            CellEvent::YieldRequested,
            CellEvent::YieldRequested,
            CellEvent::OutputText {
                text: "after".to_string(),
            },
            CellEvent::Completed {
                stored_value_writes: HashMap::from([("answer".to_string(), json!(42))]),
                error_text: None,
            },
        ]
    );
    assert_closed(harness).await;
}

#[tokio::test]
async fn termination_queued_before_actor_start_emits_direct_terminal() {
    let request = CellRequest::new("call-1", Vec::new(), "await new Promise(() => {});")
        .expect("valid cell request");
    let (handle, mut events, task) =
        CellActor::prepare(request, HashMap::new()).expect("prepare cell");
    handle.terminate().expect("queue termination before start");
    let task = tokio::spawn(task);

    assert_eq!(next_event(&mut events).await, CellEvent::Terminated);
    assert_closed(CellHarness {
        handle,
        events,
        task,
    })
    .await;
}

#[tokio::test]
async fn termination_before_started_uses_the_cleanup_deadline() {
    let mut cell = controlled_cell(Duration::ZERO);
    cell.handle
        .terminate()
        .expect("queue termination during startup");

    let error = actor_result(cell.task)
        .await
        .expect_err("stalled cleanup should fault at the termination deadline");
    assert_eq!(error.kind(), ActorErrorKind::RuntimeCleanupTimedOut);
    assert!(cell.runtime_active.load(Ordering::Acquire));
    assert_eq!(cell.events.recv().await, None);

    drop(error);
    cell.release_runtime
        .send(())
        .expect("release detached runtime");
    tokio::time::timeout(Duration::from_secs(2), async {
        while cell.runtime_active.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("detached runtime did not exit");
}

#[tokio::test]
async fn completion_before_started_emits_direct_terminal() {
    let mut cell = controlled_cell(Duration::from_secs(2));
    cell.runtime_events
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::from([("committed".to_string(), json!(true))]),
            error_text: Some("initialization failed".to_string()),
        })
        .expect("queue direct completion");
    cell.release_runtime
        .send(())
        .expect("release runtime thread");

    assert_eq!(
        next_event(&mut cell.events).await,
        CellEvent::Completed {
            stored_value_writes: HashMap::from([("committed".to_string(), json!(true))]),
            error_text: Some("initialization failed".to_string()),
        }
    );
    actor_result(cell.task)
        .await
        .expect("directly completed actor should close cleanly");
}

#[tokio::test]
async fn nonterminal_event_before_started_faults_the_actor() {
    let mut cell = controlled_cell(Duration::from_secs(2));
    cell.runtime_events
        .send(RuntimeEvent::YieldRequested)
        .expect("queue invalid pre-start event");
    cell.release_runtime
        .send(())
        .expect("release runtime thread");

    let error = actor_result(cell.task)
        .await
        .expect_err("pre-start nonterminal event should fault the actor");
    assert_eq!(error.kind(), ActorErrorKind::RuntimeClosedUnexpectedly);
    assert_eq!(cell.events.recv().await, None);
}

#[tokio::test]
async fn callback_send_failure_preserves_queued_natural_completion() {
    let mut cell = controlled_cell(Duration::from_secs(2));
    start_controlled(&mut cell).await;
    cell.runtime_events
        .send(RuntimeEvent::ToolCall {
            id: "tool-1".to_string(),
            name: ToolName::plain("echo"),
            kind: CodeModeToolKind::Function,
            input: Some(json!({ "value": "ignored" })),
        })
        .expect("queue tool call");
    assert!(matches!(
        next_event(&mut cell.events).await,
        CellEvent::ToolCallRequested { .. }
    ));

    drop(cell.runtime_commands.take());
    cell.handle
        .finish_tool_call("tool-1", ToolCallOutcome::Result(json!("late")))
        .expect("queue late tool result");
    cell.runtime_events
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::from([("committed".to_string(), json!(true))]),
            error_text: None,
        })
        .expect("queue natural completion");
    cell.release_runtime
        .send(())
        .expect("release runtime thread");

    assert_eq!(
        next_event(&mut cell.events).await,
        CellEvent::Completed {
            stored_value_writes: HashMap::from([("committed".to_string(), json!(true))]),
            error_text: None,
        }
    );
    actor_result(cell.task)
        .await
        .expect("naturally completed actor should close cleanly");
}

#[tokio::test]
async fn callback_send_failure_followed_by_runtime_close_faults_the_actor() {
    let mut cell = controlled_cell(Duration::from_secs(2));
    start_controlled(&mut cell).await;
    cell.runtime_events
        .send(RuntimeEvent::ToolCall {
            id: "tool-1".to_string(),
            name: ToolName::plain("echo"),
            kind: CodeModeToolKind::Function,
            input: None,
        })
        .expect("queue tool call");
    assert!(matches!(
        next_event(&mut cell.events).await,
        CellEvent::ToolCallRequested { .. }
    ));

    drop(cell.runtime_commands.take());
    cell.handle
        .finish_tool_call("tool-1", ToolCallOutcome::Result(json!("late")))
        .expect("queue late tool result");
    drop(cell.runtime_events);
    cell.release_runtime
        .send(())
        .expect("release runtime thread");

    let error = actor_result(cell.task)
        .await
        .expect_err("missing runtime terminal event should fault the actor");
    assert_eq!(error.kind(), ActorErrorKind::RuntimeClosedUnexpectedly);
    assert_eq!(cell.events.recv().await, None);
}

#[tokio::test]
async fn termination_and_completion_each_win_when_processed_first() {
    let mut terminated = controlled_cell(Duration::from_secs(2));
    start_controlled(&mut terminated).await;
    terminated.handle.terminate().expect("queue termination");
    terminated
        .runtime_events
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::from([("ignored".to_string(), json!(true))]),
            error_text: None,
        })
        .expect("queue completion after termination");
    terminated
        .release_runtime
        .send(())
        .expect("release terminated runtime");
    assert_eq!(
        next_event(&mut terminated.events).await,
        CellEvent::Terminated
    );
    actor_result(terminated.task)
        .await
        .expect("terminated actor should close cleanly");

    let mut completed = controlled_cell(Duration::from_secs(2));
    start_controlled(&mut completed).await;
    completed
        .runtime_events
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::from([("kept".to_string(), json!(true))]),
            error_text: None,
        })
        .expect("queue completion");
    tokio::time::timeout(Duration::from_secs(2), completed.runtime_events.closed())
        .await
        .expect("actor did not claim completion");
    assert!(matches!(completed.handle.terminate(), Err(CellClosed)));
    completed
        .release_runtime
        .send(())
        .expect("release completed runtime");
    assert_eq!(
        next_event(&mut completed.events).await,
        CellEvent::Completed {
            stored_value_writes: HashMap::from([("kept".to_string(), json!(true))]),
            error_text: None,
        }
    );
    actor_result(completed.task)
        .await
        .expect("completed actor should close cleanly");
}

#[tokio::test]
async fn expired_cleanup_deadline_preempts_backlog_and_retains_runtime_owner() {
    let mut cell = controlled_cell(Duration::ZERO);
    start_controlled(&mut cell).await;
    cell.runtime_events
        .send(RuntimeEvent::Result {
            stored_value_writes: HashMap::new(),
            error_text: None,
        })
        .expect("queue runtime completion");
    for _ in 0..10_000 {
        cell.runtime_events
            .send(RuntimeEvent::YieldRequested)
            .expect("queue runtime backlog");
    }
    let error = actor_result(cell.task)
        .await
        .expect_err("cleanup deadline should fault the actor");
    assert_eq!(error.kind(), ActorErrorKind::RuntimeCleanupTimedOut);
    assert!(cell.runtime_active.load(Ordering::Acquire));
    assert_eq!(cell.events.recv().await, None);

    drop(error);
    cell.release_runtime
        .send(())
        .expect("release retained runtime");
    tokio::time::timeout(Duration::from_secs(2), async {
        while cell.runtime_active.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("detached runtime did not exit");
}

#[tokio::test]
async fn concurrent_callbacks_accept_results_and_errors_in_any_completion_order() {
    let mut harness = spawn_cell(
        r#"
const [first, second] = await Promise.all([
  tools.echo({ value: "first" }).catch((error) => error),
  tools.echo({ value: "second" }),
]);
text(JSON.stringify([first, second]));
"#,
        vec![echo_tool()],
    );

    assert_eq!(next_event(&mut harness.events).await, CellEvent::Started);
    let first = next_event(&mut harness.events).await;
    let second = next_event(&mut harness.events).await;
    let CellEvent::ToolCallRequested {
        id: first_id,
        input: first_input,
        ..
    } = first
    else {
        panic!("expected first tool call");
    };
    let CellEvent::ToolCallRequested {
        id: second_id,
        input: second_input,
        ..
    } = second
    else {
        panic!("expected second tool call");
    };
    assert_eq!(
        [first_input, second_input],
        [
            Some(json!({ "value": "first" })),
            Some(json!({ "value": "second" })),
        ]
    );

    harness
        .handle
        .finish_tool_call(second_id, ToolCallOutcome::Result(json!("second")))
        .expect("finish second tool call");
    harness
        .handle
        .finish_tool_call(first_id, ToolCallOutcome::Error("first failed".to_string()))
        .expect("finish first tool call");

    assert_eq!(
        next_event(&mut harness.events).await,
        CellEvent::OutputText {
            text: r#"["first failed","second"]"#.to_string(),
        }
    );
    assert_eq!(
        next_event(&mut harness.events).await,
        CellEvent::Completed {
            stored_value_writes: HashMap::new(),
            error_text: None,
        }
    );
    assert_closed(harness).await;
}

#[tokio::test]
async fn termination_wins_after_yield_and_cleans_up_a_pending_timer() {
    let mut harness = spawn_cell(
        r#"
yield_control();
setTimeout(() => text("late"), 60_000);
await new Promise(() => {});
"#,
        Vec::new(),
    );

    assert_eq!(next_event(&mut harness.events).await, CellEvent::Started);
    assert_eq!(
        next_event(&mut harness.events).await,
        CellEvent::YieldRequested
    );
    harness.handle.terminate().expect("terminate cell");
    assert_eq!(next_event(&mut harness.events).await, CellEvent::Terminated);
    assert_closed(harness).await;
}

#[tokio::test]
async fn script_failure_preserves_writes_and_does_not_poison_the_next_isolate() {
    let mut failed = spawn_cell(
        r#"
store("beforeFailure", true);
throw new Error("boom");
"#,
        Vec::new(),
    );
    assert_eq!(next_event(&mut failed.events).await, CellEvent::Started);
    let CellEvent::Completed {
        stored_value_writes,
        error_text: Some(error_text),
    } = next_event(&mut failed.events).await
    else {
        panic!("expected failed completion");
    };
    assert_eq!(
        stored_value_writes,
        HashMap::from([("beforeFailure".to_string(), json!(true))])
    );
    assert!(error_text.contains("boom"));
    assert_closed(failed).await;

    let mut healthy = spawn_cell(r#"text("healthy");"#, Vec::new());
    assert_eq!(next_event(&mut healthy.events).await, CellEvent::Started);
    assert_eq!(
        next_event(&mut healthy.events).await,
        CellEvent::OutputText {
            text: "healthy".to_string(),
        }
    );
    assert_eq!(
        next_event(&mut healthy.events).await,
        CellEvent::Completed {
            stored_value_writes: HashMap::new(),
            error_text: None,
        }
    );
    assert_closed(healthy).await;
}
