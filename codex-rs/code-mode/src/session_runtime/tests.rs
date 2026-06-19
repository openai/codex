use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Poll;
use std::time::Duration;

use pretty_assertions::assert_eq;
use serde_json::Value as JsonValue;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::*;

enum FirstCloseOutcome {
    Succeeds,
    Fails,
}

#[derive(Debug, PartialEq)]
enum NotificationEvent {
    Started,
    Cancelled,
    Closed(CellId),
}

struct BlockingCloseDelegate {
    close_started_tx: mpsc::UnboundedSender<CellId>,
    close_release: Semaphore,
    fail_first_close: AtomicBool,
}

struct BlockingNotificationDelegate {
    events_tx: mpsc::UnboundedSender<NotificationEvent>,
}

impl BlockingCloseDelegate {
    fn new(first_close_outcome: FirstCloseOutcome) -> (Arc<Self>, mpsc::UnboundedReceiver<CellId>) {
        let (close_started_tx, close_started_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                close_started_tx,
                close_release: Semaphore::new(/*permits*/ 0),
                fail_first_close: AtomicBool::new(matches!(
                    first_close_outcome,
                    FirstCloseOutcome::Fails
                )),
            }),
            close_started_rx,
        )
    }
}

impl BlockingNotificationDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<NotificationEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (Arc::new(Self { events_tx }), events_rx)
    }
}

impl SessionRuntimeDelegate for BlockingCloseDelegate {
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

    async fn cell_closed(&self, cell_id: &CellId) -> Result<(), String> {
        self.close_started_tx
            .send(cell_id.clone())
            .map_err(|_| "test did not receive cell close".to_string())?;
        self.close_release
            .acquire()
            .await
            .map_err(|_| "test did not release cell close".to_string())?
            .forget();
        if self.fail_first_close.swap(false, Ordering::AcqRel) {
            return Err("test close failure".to_string());
        }
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
        self.events_tx
            .send(NotificationEvent::Started)
            .map_err(|_| "test did not receive notification start".to_string())?;
        cancellation_token.cancelled().await;
        self.events_tx
            .send(NotificationEvent::Cancelled)
            .map_err(|_| "test did not receive notification cancellation".to_string())
    }

    async fn cell_closed(&self, cell_id: &CellId) -> Result<(), String> {
        self.events_tx
            .send(NotificationEvent::Closed(cell_id.clone()))
            .map_err(|_| "test did not receive cell close".to_string())
    }
}

fn execute_request(source: &str) -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
    }
}

#[tokio::test]
async fn closing_cells_reject_requests_while_delegate_cleanup_runs() {
    let (delegate, mut close_started_rx) = BlockingCloseDelegate::new(FirstCloseOutcome::Succeeds);
    let runtime = Arc::new(SessionRuntime::new(Arc::clone(&delegate)));
    let started = runtime
        .execute(
            execute_request(r#"text("done");"#),
            ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        )
        .await
        .unwrap();
    let cell_id = started.cell_id.clone();

    assert_eq!(
        started.initial_event().await.unwrap(),
        CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        }
    );
    assert_eq!(close_started_rx.recv().await, Some(cell_id.clone()));

    assert_eq!(
        runtime
            .observe(&cell_id, ObserveMode::PendingFrontier)
            .await,
        Err(Error::MissingCell(cell_id.clone()))
    );
    assert_eq!(
        runtime.terminate(&cell_id).await,
        Err(Error::MissingCell(cell_id.clone()))
    );

    let shutdown_runtime = Arc::clone(&runtime);
    let mut shutdown = tokio::spawn(async move { shutdown_runtime.shutdown().await });
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut shutdown)
            .await
            .is_err()
    );

    delegate.close_release.add_permits(/*n*/ 1);

    assert_eq!(shutdown.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn shutdown_waits_for_each_cell_to_finish_delegate_cleanup() {
    let (delegate, mut close_started_rx) = BlockingCloseDelegate::new(FirstCloseOutcome::Succeeds);
    let runtime = Arc::new(SessionRuntime::new(Arc::clone(&delegate)));

    let completed = runtime
        .execute(
            execute_request(r#"text("done");"#),
            ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        )
        .await
        .unwrap();
    assert_eq!(
        completed.initial_event().await.unwrap(),
        CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        }
    );
    let closing_cell_id = close_started_rx.recv().await.unwrap();

    let live = runtime
        .execute(
            execute_request("while (true) {}"),
            ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
        )
        .await
        .unwrap();
    assert_eq!(
        live.initial_event().await.unwrap(),
        CellEvent::Yielded {
            content_items: Vec::new(),
        }
    );

    let shutdown_runtime = Arc::clone(&runtime);
    let mut shutdown = tokio::spawn(async move { shutdown_runtime.shutdown().await });
    let terminated_cell_id = close_started_rx.recv().await.unwrap();
    assert_ne!(closing_cell_id, terminated_cell_id);

    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut shutdown)
            .await
            .is_err()
    );

    delegate.close_release.add_permits(/*n*/ 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut shutdown)
            .await
            .is_err()
    );

    delegate.close_release.add_permits(/*n*/ 1);
    assert_eq!(shutdown.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn shutdown_waits_for_remaining_cells_when_delegate_cleanup_fails() {
    let (delegate, mut close_started_rx) = BlockingCloseDelegate::new(FirstCloseOutcome::Fails);
    let runtime = Arc::new(SessionRuntime::new(Arc::clone(&delegate)));

    let completed = runtime
        .execute(
            execute_request(r#"text("done");"#),
            ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        )
        .await
        .unwrap();
    assert_eq!(
        completed.initial_event().await.unwrap(),
        CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        }
    );
    let closing_cell_id = close_started_rx.recv().await.unwrap();

    let live = runtime
        .execute(
            execute_request("while (true) {}"),
            ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
        )
        .await
        .unwrap();
    assert_eq!(
        live.initial_event().await.unwrap(),
        CellEvent::Yielded {
            content_items: Vec::new(),
        }
    );

    let shutdown_runtime = Arc::clone(&runtime);
    let mut shutdown = tokio::spawn(async move { shutdown_runtime.shutdown().await });
    let terminated_cell_id = close_started_rx.recv().await.unwrap();
    assert_ne!(closing_cell_id, terminated_cell_id);

    delegate.close_release.add_permits(/*n*/ 1);
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut shutdown)
            .await
            .is_err()
    );

    delegate.close_release.add_permits(/*n*/ 1);
    assert_eq!(shutdown.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn concurrent_shutdowns_wait_for_the_same_cell_cleanup() {
    let (delegate, mut close_started_rx) = BlockingCloseDelegate::new(FirstCloseOutcome::Succeeds);
    let runtime = Arc::new(SessionRuntime::new(Arc::clone(&delegate)));

    let completed = runtime
        .execute(
            execute_request(r#"text("done");"#),
            ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        )
        .await
        .unwrap();
    let cell_id = completed.cell_id.clone();
    assert_eq!(
        completed.initial_event().await.unwrap(),
        CellEvent::Completed {
            content_items: vec![OutputItem::Text {
                text: "done".to_string(),
            }],
            error_text: None,
        }
    );
    assert_eq!(close_started_rx.recv().await, Some(cell_id));

    let first_runtime = Arc::clone(&runtime);
    let mut first_shutdown = tokio::spawn(async move { first_runtime.shutdown().await });
    let second_runtime = Arc::clone(&runtime);
    let mut second_shutdown = tokio::spawn(async move { second_runtime.shutdown().await });

    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut first_shutdown)
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut second_shutdown)
            .await
            .is_err()
    );

    delegate.close_release.add_permits(/*n*/ 1);
    assert_eq!(first_shutdown.await.unwrap(), Ok(()));
    assert_eq!(second_shutdown.await.unwrap(), Ok(()));
}

#[tokio::test]
async fn shutdown_waits_for_cell_admission_already_queued_for_the_registry_lock() {
    let (delegate, mut close_started_rx) = BlockingCloseDelegate::new(FirstCloseOutcome::Succeeds);
    let runtime = Arc::new(SessionRuntime::new(Arc::clone(&delegate)));
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

    drop(cells);

    let started = execution.await.unwrap();
    let cell_id = started.cell_id;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), async {
            tokio::select! {
                close_started = close_started_rx.recv() => close_started,
                result = &mut shutdown => {
                    panic!("shutdown completed before cell delegate cleanup: {result:?}");
                }
            }
        })
        .await
        .unwrap(),
        Some(cell_id)
    );

    delegate.close_release.add_permits(/*n*/ 1);
    assert_eq!(shutdown.await, Ok(()));
}

#[tokio::test]
async fn drop_terminates_cells_when_the_registry_is_locked() {
    let (delegate, mut close_started_rx) = BlockingCloseDelegate::new(FirstCloseOutcome::Succeeds);
    let runtime = SessionRuntime::new(Arc::clone(&delegate));
    let started = runtime
        .execute(
            execute_request("while (true) {}"),
            ObserveMode::YieldAfter(Duration::from_millis(/*millis*/ 1)),
        )
        .await
        .unwrap();
    let cell_id = started.cell_id.clone();
    assert_eq!(
        started.initial_event().await.unwrap(),
        CellEvent::Yielded {
            content_items: Vec::new(),
        }
    );

    let inner = Arc::clone(&runtime.inner);
    let mut cell_count = inner.cell_count_tx.subscribe();
    let cells = inner.cells.lock().await;
    drop(runtime);
    drop(cells);

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), close_started_rx.recv())
            .await
            .unwrap(),
        Some(cell_id)
    );

    delegate.close_release.add_permits(/*n*/ 1);
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
    let started = runtime
        .execute(
            execute_request(r#"notify("pending");"#),
            ObserveMode::YieldAfter(Duration::from_secs(/*secs*/ 60)),
        )
        .await
        .unwrap();
    let cell_id = started.cell_id;

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), events_rx.recv())
            .await
            .unwrap(),
        Some(NotificationEvent::Started)
    );

    let inner = Arc::clone(&runtime.inner);
    let mut cell_count = inner.cell_count_tx.subscribe();
    let cells = inner.cells.lock().await;
    drop(runtime);
    drop(cells);

    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), events_rx.recv())
            .await
            .unwrap(),
        Some(NotificationEvent::Cancelled)
    );
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(/*secs*/ 1), events_rx.recv())
            .await
            .unwrap(),
        Some(NotificationEvent::Closed(cell_id))
    );
    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), cell_count.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*cell_count.borrow_and_update(), 0);
}
