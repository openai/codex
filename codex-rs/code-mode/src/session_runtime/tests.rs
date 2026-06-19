use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::*;

struct RecordingDelegate;

struct BlockingNotificationDelegate {
    notification_started_tx: mpsc::UnboundedSender<()>,
}

impl BlockingNotificationDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<()>) {
        let (notification_started_tx, notification_started_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                notification_started_tx,
            }),
            notification_started_rx,
        )
    }
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
}

impl SessionRuntimeDelegate for BlockingNotificationDelegate {
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
        cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        self.notification_started_tx
            .send(())
            .map_err(|_| "test did not receive notification start".to_string())?;
        cancellation_token.cancelled().await;
        Ok(())
    }
}

fn execute_request(source: &str) -> ExecuteRequest {
    execute_request_with_id("1", source)
}

fn execute_request_with_id(cell_id: &str, source: &str) -> ExecuteRequest {
    ExecuteRequest {
        cell_id: CellId::new(cell_id),
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
    }
}

#[tokio::test]
#[expect(
    clippy::await_holding_invalid_type,
    reason = "test holds the registry lock to observe terminal routing before removal"
)]
async fn terminal_cells_are_unrouted_before_they_are_unregistered() {
    let runtime = Arc::new(SessionRuntime::new(Arc::new(RecordingDelegate)));
    let cell_id = CellId::new("1");
    assert_eq!(
        runtime
            .execute(
                execute_request("while (true) {}"),
                ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
            )
            .await,
        Ok(CellEvent::Yielded {
            content_items: Vec::new(),
        })
    );

    let mut cell_count = runtime.inner.cell_count_tx.subscribe();
    let cells = runtime.inner.cells.lock().await;
    let handle = cells.get(&cell_id).unwrap().clone();
    assert_eq!(
        handle.terminate().await,
        Ok(CellEvent::Terminated {
            content_items: Vec::new(),
        })
    );
    assert!(cells.contains_key(&cell_id));
    assert_eq!(
        handle.observe(ObserveMode::PendingFrontier).await,
        Err(CellError::Closed)
    );
    assert_eq!(handle.terminate().await, Err(CellError::Closed));
    drop(cells);

    cell_count.changed().await.unwrap();
    assert_eq!(*cell_count.borrow_and_update(), 0);
    assert_eq!(runtime.shutdown().await, Ok(()));
}

#[tokio::test]
#[expect(
    clippy::await_holding_invalid_type,
    reason = "test holds the registry lock to force admission ahead of shutdown"
)]
async fn shutdown_rejects_cell_admission_queued_before_the_registry_lock() {
    let runtime = Arc::new(SessionRuntime::new(Arc::new(RecordingDelegate)));
    let cells = runtime.inner.cells.lock().await;

    let execution = runtime.execute(
        execute_request("while (true) {}"),
        ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
    );
    tokio::pin!(execution);
    std::future::poll_fn(|context| match execution.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(Ok(_)) => panic!("execution completed before the registry lock was released"),
        Poll::Ready(Err(error)) => {
            panic!("execution failed before the registry lock was released: {error}")
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
    assert!(matches!(execution.await, Err(Error::ShuttingDown)));
    assert_eq!(shutdown.await, Ok(()));
}

#[tokio::test]
#[expect(
    clippy::await_holding_invalid_type,
    reason = "test holds the registry lock to force shutdown ahead of admission"
)]
async fn shutdown_rejects_cell_admission_queued_after_the_registry_lock() {
    let runtime = Arc::new(SessionRuntime::new(Arc::new(RecordingDelegate)));
    let cells = runtime.inner.cells.lock().await;

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

    let execution = runtime.execute(
        execute_request("text('late');"),
        ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
    );
    tokio::pin!(execution);
    std::future::poll_fn(|context| match execution.as_mut().poll(context) {
        Poll::Pending => panic!("execution waited after shutdown cancellation"),
        Poll::Ready(Ok(_)) => panic!("execution started after shutdown cancellation"),
        Poll::Ready(Err(Error::ShuttingDown)) => Poll::Ready(()),
        Poll::Ready(Err(error)) => panic!("execution failed with an unexpected error: {error}"),
    })
    .await;

    drop(cells);

    assert_eq!(shutdown.await, Ok(()));
}

#[tokio::test]
#[expect(
    clippy::await_holding_invalid_type,
    reason = "test holds the registry lock while cancelling a blocked shutdown future"
)]
async fn cancelling_shutdown_while_waiting_for_the_registry_lock_keeps_the_session_closed() {
    let runtime = SessionRuntime::new(Arc::new(RecordingDelegate));
    let cells = runtime.inner.cells.lock().await;

    let mut shutdown = Box::pin(runtime.shutdown());
    std::future::poll_fn(|context| match shutdown.as_mut().poll(context) {
        Poll::Pending => Poll::Ready(()),
        Poll::Ready(Ok(())) => panic!("shutdown completed before acquiring the registry lock"),
        Poll::Ready(Err(error)) => {
            panic!("shutdown failed before acquiring the registry lock: {error}")
        }
    })
    .await;
    assert!(!runtime.is_alive());
    drop(shutdown);

    assert!(matches!(
        runtime
            .execute(
                execute_request("text('late');"),
                ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
            )
            .await,
        Err(Error::ShuttingDown)
    ));
    drop(cells);
}

#[tokio::test]
async fn drop_terminates_cells_when_the_registry_is_locked() {
    let runtime = SessionRuntime::new(Arc::new(RecordingDelegate));
    assert_eq!(
        runtime
            .execute(
                execute_request("while (true) {}"),
                ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
            )
            .await
            .unwrap(),
        CellEvent::Yielded {
            content_items: Vec::new(),
        }
    );

    let inner = Arc::clone(&runtime.inner);
    let mut cell_count = inner.cell_count_tx.subscribe();
    let cells = inner.cells.lock().await;
    drop(runtime);
    drop(cells);

    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), cell_count.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*cell_count.borrow_and_update(), 0);
}

#[tokio::test]
async fn drop_cancels_notifications_during_natural_completion_when_registry_is_locked() {
    let (delegate, mut events_rx) = BlockingNotificationDelegate::new();
    let runtime = SessionRuntime::new(Arc::clone(&delegate));
    assert_eq!(
        runtime
            .execute(
                execute_request(r#"yield_control(); notify("pending");"#),
                ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
            )
            .await
            .unwrap(),
        CellEvent::Yielded {
            content_items: Vec::new(),
        }
    );

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), events_rx.recv())
            .await
            .unwrap(),
        Some(())
    );

    let inner = Arc::clone(&runtime.inner);
    let mut cell_count = inner.cell_count_tx.subscribe();
    let cells = inner.cells.lock().await;
    drop(runtime);
    drop(cells);

    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), cell_count.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*cell_count.borrow_and_update(), 0);
}

#[tokio::test]
async fn client_owned_cell_id_is_preserved_and_active_duplicates_are_rejected() {
    let runtime = SessionRuntime::new(Arc::new(RecordingDelegate));
    let cell_id = CellId::new("client-generated");

    assert_eq!(
        runtime
            .execute(
                execute_request_with_id(cell_id.as_str(), "await new Promise(() => {});"),
                ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
            )
            .await
            .unwrap(),
        CellEvent::Yielded {
            content_items: Vec::new(),
        }
    );
    assert_eq!(
        runtime
            .execute(
                execute_request_with_id(cell_id.as_str(), "text('duplicate');"),
                ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
            )
            .await,
        Err(Error::DuplicateCell(cell_id.clone()))
    );
    assert_eq!(
        runtime.terminate(&cell_id).await,
        Ok(CellEvent::Terminated {
            content_items: Vec::new(),
        })
    );
}
