use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::time::Duration;

use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::cell_actor::CompletionCommit;

struct RecordingDelegate;

struct ImmediateToolDelegate {
    invocations_tx: mpsc::UnboundedSender<String>,
}

struct BlockingToolDelegate {
    invocations_tx: mpsc::UnboundedSender<String>,
    release: Arc<Semaphore>,
}

impl SessionRuntimeDelegate for RecordingDelegate {
    async fn invoke_tool(
        &self,
        _invocation: NestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        Ok(JsonValue::Null)
    }

    async fn notify(
        &self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        Ok(())
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

impl SessionRuntimeDelegate for ImmediateToolDelegate {
    async fn invoke_tool(
        &self,
        invocation: NestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        let _ = self.invocations_tx.send(invocation.tool_name.name);
        Ok(JsonValue::Null)
    }

    async fn notify(
        &self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        Ok(())
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

impl SessionRuntimeDelegate for BlockingToolDelegate {
    async fn invoke_tool(
        &self,
        invocation: NestedToolCall,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        let _ = self.invocations_tx.send(invocation.tool_name.name);
        let permit = tokio::select! {
            permit = self.release.acquire() => permit.map_err(|error| error.to_string())?,
            () = cancellation_token.cancelled() => return Err("cancelled".to_string()),
        };
        permit.forget();
        Ok(JsonValue::Null)
    }

    async fn notify(
        &self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        Ok(())
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

fn tool_definition(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        tool_name: ToolName {
            name: name.to_string(),
            namespace: None,
        },
        description: String::new(),
        kind: ToolKind::Function,
    }
}

#[tokio::test]
async fn default_policy_resolves_tools_before_the_first_observation() {
    let (invocations_tx, mut invocations_rx) = mpsc::unbounded_channel();
    let runtime = SessionRuntime::new(Arc::new(ImmediateToolDelegate { invocations_tx }));
    let cell = runtime
        .create_cell(CreateCellRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: vec![tool_definition("first"), tool_definition("second")],
            source: r#"
await tools.first({});
await tools.second({});
text("done");
"#
            .to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), invocations_rx.recv())
            .await
            .expect("first tool invocation timed out"),
        Some("first".to_string())
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), invocations_rx.recv())
            .await
            .expect("second tool invocation timed out"),
        Some("second".to_string())
    );
    assert_eq!(
        runtime
            .observe(&cell, ObserveMode::YieldAfter(Duration::from_secs(1)))
            .await,
        Ok(CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn pausable_cell_supports_a_synchronous_host_driver() {
    let (invocations_tx, mut invocations_rx) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let runtime = SessionRuntime::new(Arc::new(BlockingToolDelegate {
        invocations_tx,
        release: Arc::clone(&release),
    }));
    let cell = runtime
        .create_pausable_cell(CreateCellRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: vec![tool_definition("first"), tool_definition("second")],
            source: r#"
await tools.first({});
await tools.second({});
text("done");
"#
            .to_string(),
        })
        .await
        .unwrap();

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), invocations_rx.recv())
            .await
            .expect("first tool invocation timed out"),
        Some("first".to_string())
    );
    let first = runtime.wait_to_pending(&cell).await.unwrap();
    let CellEvent::Pending(first_frontier) = &first else {
        panic!("expected the first pending frontier, got {first:?}");
    };
    assert_eq!(
        first_frontier.generation,
        PendingGeneration::new(/*value*/ 1)
    );
    assert_eq!(runtime.wait_to_pending(&cell).await, Ok(first.clone()));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), invocations_rx.recv())
            .await
            .is_err()
    );

    let (first_resume, duplicate_resume) = tokio::join!(
        runtime.resume(&cell, first_frontier.generation),
        runtime.resume(&cell, first_frontier.generation),
    );
    assert!(matches!(
        (first_resume, duplicate_resume),
        (
            Ok(ResumeOutcome::Resumed),
            Ok(ResumeOutcome::AlreadyRunning)
        ) | (
            Ok(ResumeOutcome::AlreadyRunning),
            Ok(ResumeOutcome::Resumed)
        )
    ));
    release.add_permits(1);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), invocations_rx.recv())
            .await
            .expect("second tool invocation timed out"),
        Some("second".to_string())
    );

    let second = runtime.wait_to_pending(&cell).await.unwrap();
    let CellEvent::Pending(second_frontier) = &second else {
        panic!("expected the second pending frontier, got {second:?}");
    };
    assert_eq!(
        second_frontier.generation,
        PendingGeneration::new(/*value*/ 2)
    );
    assert_eq!(
        runtime.resume(&cell, first_frontier.generation).await,
        Ok(ResumeOutcome::AlreadyRunning)
    );
    assert_eq!(
        runtime
            .resume(&cell, PendingGeneration::new(/*value*/ 3))
            .await,
        Err(Error::InvalidGeneration {
            cell_id: cell.clone(),
            requested: PendingGeneration::new(/*value*/ 3),
            latest: Some(PendingGeneration::new(/*value*/ 2)),
        })
    );
    assert_eq!(
        runtime.resume(&cell, second_frontier.generation).await,
        Ok(ResumeOutcome::Resumed)
    );
    release.add_permits(1);
    assert_eq!(
        runtime.wait_to_pending(&cell).await,
        Ok(CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn pending_frontier_reports_only_authoritatively_outstanding_parallel_tools() {
    let (invocations_tx, mut invocations_rx) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let runtime = SessionRuntime::new(Arc::new(BlockingToolDelegate {
        invocations_tx,
        release: Arc::clone(&release),
    }));
    let cell = runtime
        .create_pausable_cell(CreateCellRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: vec![tool_definition("first"), tool_definition("second")],
            source: r#"
await Promise.all([tools.first({}), tools.second({})]);
text("done");
"#
            .to_string(),
        })
        .await
        .unwrap();

    let mut invocations = vec![
        tokio::time::timeout(Duration::from_secs(1), invocations_rx.recv())
            .await
            .expect("first tool invocation timed out")
            .expect("first tool invocation channel closed"),
        tokio::time::timeout(Duration::from_secs(1), invocations_rx.recv())
            .await
            .expect("second tool invocation timed out")
            .expect("second tool invocation channel closed"),
    ];
    invocations.sort();
    assert_eq!(invocations, vec!["first".to_string(), "second".to_string()]);

    let CellEvent::Pending(first_frontier) = runtime.wait_to_pending(&cell).await.unwrap() else {
        panic!("expected first pending frontier");
    };
    assert_eq!(
        first_frontier.pending_tool_call_ids,
        vec!["tool-1".to_string(), "tool-2".to_string()]
    );

    assert_eq!(
        runtime.resume(&cell, first_frontier.generation).await,
        Ok(ResumeOutcome::Resumed)
    );
    release.add_permits(1);

    let CellEvent::Pending(second_frontier) = runtime.wait_to_pending(&cell).await.unwrap() else {
        panic!("expected second pending frontier");
    };
    assert_eq!(second_frontier.pending_tool_call_ids.len(), 1);
    assert!(
        first_frontier
            .pending_tool_call_ids
            .contains(&second_frontier.pending_tool_call_ids[0])
    );

    assert_eq!(
        runtime.resume(&cell, second_frontier.generation).await,
        Ok(ResumeOutcome::Resumed)
    );
    release.add_permits(1);
    assert_eq!(
        runtime.wait_to_pending(&cell).await,
        Ok(CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn pending_observation_waits_for_resumed_work_to_reach_a_new_frontier() {
    let (invocations_tx, mut invocations_rx) = mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let runtime = SessionRuntime::new(Arc::new(BlockingToolDelegate {
        invocations_tx,
        release: Arc::clone(&release),
    }));
    let cell = runtime
        .create_pausable_cell(CreateCellRequest {
            tool_call_id: "call-1".to_string(),
            enabled_tools: vec![tool_definition("blocked")],
            source: r#"
await tools.blocked({});
text("done");
"#
            .to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), invocations_rx.recv())
            .await
            .expect("tool invocation timed out"),
        Some("blocked".to_string())
    );
    let CellEvent::Pending(frontier) = runtime.wait_to_pending(&cell).await.unwrap() else {
        panic!("expected a pending frontier");
    };

    assert_eq!(
        runtime.resume(&cell, frontier.generation).await,
        Ok(ResumeOutcome::Resumed)
    );
    let next_event = runtime.wait_to_pending(&cell);
    tokio::pin!(next_event);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut next_event)
            .await
            .is_err()
    );
    release.add_permits(1);
    assert_eq!(
        next_event.await,
        Ok(CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn termination_rejects_a_waiting_store_commit_before_the_next_cell_can_load_it() {
    let runtime = SessionRuntime::new(Arc::new(RecordingDelegate));
    let cell_state = Arc::new(CellState::new(CancellationToken::new()));
    let host = RuntimeCellHost {
        cell_id: CellId::new("terminating-writer"),
        inner: Arc::clone(&runtime.inner),
    };
    let completion = CellEvent::Completed {
        content_items: vec![OutputItem::Text {
            text: "uncommitted output".to_string(),
        }],
        error_text: None,
    };

    let stored_values = runtime.inner.stored_values.lock().await;
    let commit = host.commit_completion(
        HashMap::from([(
            "candidate".to_string(),
            JsonValue::String("lost".to_string()),
        )]),
        completion.clone(),
        /*pending_initial_yield_items*/ None,
        Arc::clone(&cell_state),
    );
    tokio::pin!(commit);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(commit.as_mut().poll(&mut context), Poll::Pending));

    let termination = cell_state.request_termination();
    drop(stored_values);
    assert_eq!(commit.await, CompletionCommit::Rejected(completion));
    let terminated = CellEvent::Terminated {
        content_items: Vec::new(),
    };
    assert_eq!(
        cell_state.finish_termination(terminated.clone()),
        Some(terminated.clone())
    );
    assert_eq!(termination.await, Ok(terminated));
    assert!(
        !runtime
            .inner
            .stored_values
            .lock()
            .await
            .contains_key("candidate")
    );

    let reader = runtime
        .create_cell(CreateCellRequest {
            tool_call_id: "reader".to_string(),
            enabled_tools: Vec::new(),
            source: r#"text(String(load("candidate")));"#.to_string(),
        })
        .await
        .unwrap();
    assert_eq!(
        runtime
            .observe(&reader, ObserveMode::YieldAfter(Duration::from_secs(1)))
            .await,
        Ok(CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "undefined".to_string(),
            }],
            error_text: None,
        })
    );
    runtime.shutdown().await.unwrap();
}

fn execute_request(source: &str) -> CreateCellRequest {
    CreateCellRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
    }
}

#[tokio::test]
#[expect(
    clippy::await_holding_invalid_type,
    reason = "test holds the registry lock to force admission ahead of shutdown"
)]
async fn shutdown_rejects_cell_admission_queued_before_the_registry_lock() {
    let runtime = Arc::new(SessionRuntime::new(Arc::new(RecordingDelegate)));
    let cells = runtime.inner.cells.lock().await;

    let creation = runtime.create_cell(execute_request("while (true) {}"));
    tokio::pin!(creation);
    std::future::poll_fn(|context| match creation.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(Ok(_)) => panic!("creation completed before the registry lock was released"),
        Poll::Ready(Err(error)) => {
            panic!("creation failed before the registry lock was released: {error}")
        }
    })
    .await;

    let shutdown = runtime.shutdown();
    tokio::pin!(shutdown);
    std::future::poll_fn(|context| match shutdown.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(Ok(())) => panic!("shutdown completed before acquiring the registry lock"),
        Poll::Ready(Err(error)) => {
            panic!("shutdown failed before acquiring the registry lock: {error}")
        }
    })
    .await;

    assert!(!runtime.is_alive());
    drop(cells);
    assert!(matches!(creation.await, Err(Error::ShuttingDown)));
    assert_eq!(shutdown.await, Ok(()));
}

#[tokio::test]
async fn drop_terminates_cells_when_the_registry_is_locked() {
    let runtime = SessionRuntime::new(Arc::new(RecordingDelegate));
    let cell = runtime
        .create_cell(execute_request("while (true) {}"))
        .await
        .unwrap();
    assert_eq!(cell, CellId::new("1"));
    assert_eq!(
        runtime
            .observe(
                &cell,
                ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
            )
            .await,
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    let inner = Arc::clone(&runtime.inner);
    let cells = inner.cells.lock().await;
    drop(runtime);
    drop(cells);

    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), inner.cell_tasks.wait())
        .await
        .unwrap();
    assert!(inner.cell_tasks.is_empty());
}
