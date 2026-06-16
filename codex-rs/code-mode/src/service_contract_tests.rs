use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::CodeModeToolKind;
use crate::ToolDefinition;

#[derive(Debug, PartialEq)]
enum DelegateEvent {
    NotificationStarted,
    NotificationCancelled,
    ToolStarted,
    ToolCancelled,
    CellClosed(CellId),
}

struct BlockingDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
    notification_finished: AtomicBool,
    tool_finished: AtomicBool,
    tool_release: Notify,
}

struct HeldNotificationDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
    notification_release: Notify,
}

impl HeldNotificationDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<DelegateEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                events_tx,
                notification_release: Notify::new(),
            }),
            events_rx,
        )
    }

    fn release_notification(&self) {
        self.notification_release.notify_one();
    }
}

impl CodeModeSessionDelegate for HeldNotificationDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            cancellation_token.cancelled().await;
            Err("cancelled".to_string())
        })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            let _ = self.events_tx.send(DelegateEvent::NotificationStarted);
            cancellation_token.cancelled().await;
            let _ = self.events_tx.send(DelegateEvent::NotificationCancelled);
            self.notification_release.notified().await;
            Ok(())
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        let _ = self
            .events_tx
            .send(DelegateEvent::CellClosed(cell_id.clone()));
    }
}

impl BlockingDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<DelegateEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                events_tx,
                notification_finished: AtomicBool::new(false),
                tool_finished: AtomicBool::new(false),
                tool_release: Notify::new(),
            }),
            events_rx,
        )
    }

    fn release_tool(&self) {
        self.tool_release.notify_one();
    }
}

impl CodeModeSessionDelegate for BlockingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            let _ = self.events_tx.send(DelegateEvent::ToolStarted);
            tokio::select! {
                _ = self.tool_release.notified() => {
                    self.tool_finished.store(true, Ordering::Release);
                    Ok(serde_json::Value::Null)
                }
                _ = cancellation_token.cancelled() => {
                    self.tool_finished.store(true, Ordering::Release);
                    let _ = self.events_tx.send(DelegateEvent::ToolCancelled);
                    Err("cancelled".to_string())
                }
            }
        })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            let _ = self.events_tx.send(DelegateEvent::NotificationStarted);
            cancellation_token.cancelled().await;
            self.notification_finished.store(true, Ordering::Release);
            let _ = self.events_tx.send(DelegateEvent::NotificationCancelled);
            Err("cancelled".to_string())
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        let _ = self
            .events_tx
            .send(DelegateEvent::CellClosed(cell_id.clone()));
    }
}

fn cell_id(value: &str) -> CellId {
    CellId::new(value.to_string())
}

fn execute_request(source: &str) -> ExecuteRequest {
    ExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
        yield_time_ms: Some(1),
        max_output_tokens: None,
    }
}

fn blocking_tool() -> ToolDefinition {
    ToolDefinition {
        name: "block".to_string(),
        tool_name: ToolName::plain("block"),
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        output_schema: None,
    }
}

async fn next_event(events_rx: &mut mpsc::UnboundedReceiver<DelegateEvent>) -> DelegateEvent {
    tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
        .await
        .expect("delegate event timeout")
        .expect("delegate event channel closed")
}

#[tokio::test]
async fn yields_and_resumes() {
    let service = CodeModeService::new();
    let cell = service
        .execute(ExecuteRequest {
            source: r#"text("before"); yield_control(); text("after");"#.to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(
        cell.initial_response().await.unwrap(),
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            }],
        }
    );
    assert_eq!(
        service
            .wait(WaitRequest {
                cell_id: cell_id("1"),
                yield_time_ms: 60_000,
            })
            .await
            .unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            }],
            error_text: None,
        })
    );
}

#[tokio::test]
async fn returns_and_resumes_from_the_pending_frontier() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());

    assert_eq!(
        service
            .execute_to_pending(ExecuteRequest {
                enabled_tools: vec![blocking_tool()],
                source: r#"
await tools.block({});
text("after");
"#
                .to_string(),
                yield_time_ms: Some(60_000),
                ..execute_request("")
            })
            .await
            .unwrap(),
        ExecuteToPendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string()],
        }
    );

    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    delegate.release_tool();

    assert_eq!(
        service
            .wait_to_pending(WaitToPendingRequest {
                cell_id: cell_id("1"),
            })
            .await
            .unwrap(),
        WaitToPendingOutcome::LiveCell(ExecuteToPendingOutcome::Completed(
            RuntimeResponse::Result {
                cell_id: cell_id("1"),
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: "after".to_string(),
                }],
                error_text: None,
            }
        ))
    );
}

#[tokio::test]
async fn observed_natural_completion_wins_over_termination() {
    let service = CodeModeService::new();
    let cell = service
        .execute(execute_request(
            r#"yield_control(); store("finished", true); text("done");"#,
        ))
        .await
        .unwrap();

    assert_eq!(
        cell.initial_response().await.unwrap(),
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if service.inner.stored_values.lock().await.get("finished")
                == Some(&serde_json::json!(true))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        service.terminate(cell_id("1")).await.unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
}

#[tokio::test]
async fn termination_cancels_pending_callbacks_before_responding() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let cell = service
        .execute(execute_request(
            r#"notify("pending"); await new Promise(() => {});"#,
        ))
        .await
        .unwrap();

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationStarted
    );
    assert_eq!(
        cell.initial_response().await.unwrap(),
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
    assert_eq!(
        service.terminate(cell_id("1")).await.unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
    assert!(delegate.notification_finished.load(Ordering::Acquire));
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationCancelled
    );
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}

#[tokio::test]
async fn repeated_termination_is_rejected_while_callback_cleanup_is_pending() {
    let (delegate, mut events_rx) = HeldNotificationDelegate::new();
    let service = Arc::new(CodeModeService::with_delegate(delegate.clone()));
    let cell = service
        .execute(execute_request(
            r#"notify("pending"); await new Promise(() => {});"#,
        ))
        .await
        .unwrap();

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationStarted
    );
    assert_eq!(
        cell.initial_response().await.unwrap(),
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );

    let terminating_service = Arc::clone(&service);
    let first_termination =
        tokio::spawn(async move { terminating_service.terminate(cell_id("1")).await });
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationCancelled
    );

    let repeated_termination = service.terminate(cell_id("1")).await;
    delegate.release_notification();

    assert_eq!(
        repeated_termination.unwrap_err(),
        "exec cell 1 is already terminating"
    );
    assert_eq!(
        first_termination.await.unwrap().unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}

#[tokio::test]
async fn second_observer_is_rejected_without_displacing_the_first() {
    let service = CodeModeService::new();
    let cell = service
        .execute(execute_request("await new Promise(() => {});"))
        .await
        .unwrap();

    assert_eq!(
        cell.initial_response().await.unwrap(),
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );

    let first_observer = service
        .begin_wait(WaitRequest {
            cell_id: cell_id("1"),
            yield_time_ms: 60_000,
        })
        .await;
    assert_eq!(
        service
            .wait(WaitRequest {
                cell_id: cell_id("1"),
                yield_time_ms: 60_000,
            })
            .await
            .unwrap_err(),
        "exec cell 1 already has an active observer"
    );

    let terminated = RuntimeResponse::Terminated {
        cell_id: cell_id("1"),
        content_items: Vec::new(),
    };
    assert_eq!(
        service.terminate(cell_id("1")).await.unwrap(),
        WaitOutcome::LiveCell(terminated.clone())
    );
    assert_eq!(
        first_observer.await.unwrap(),
        WaitOutcome::LiveCell(terminated)
    );
}

#[tokio::test]
async fn natural_completion_cleans_up_callbacks_before_responding() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let cell = service
        .execute(ExecuteRequest {
            enabled_tools: vec![blocking_tool()],
            source: r#"tools.block({}); text("done");"#.to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    assert_eq!(
        cell.initial_response().await.unwrap(),
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        }
    );
    assert!(delegate.tool_finished.load(Ordering::Acquire));
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::ToolCancelled
    );
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}
