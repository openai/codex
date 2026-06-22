use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use std::time::Duration;

use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::cell_actor::CompletionCommit;

struct RecordingDelegate;

struct ImmediateToolDelegate {
    invocations_tx: mpsc::UnboundedSender<String>,
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
async fn continuing_cell_resolves_tools_before_the_first_observation() {
    let (invocations_tx, mut invocations_rx) = mpsc::unbounded_channel();
    let runtime = SessionRuntime::new(Arc::new(ImmediateToolDelegate { invocations_tx }));
    let cell_id = runtime
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
            .observe(&cell_id, ObserveMode::YieldAfter(Duration::from_secs(1)))
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
    let cell_id = runtime
        .create_cell(execute_request("while (true) {}"))
        .await
        .unwrap();
    assert_eq!(cell_id, CellId::new("1"));
    assert_eq!(
        runtime
            .observe(
                &cell_id,
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
