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
    ToolStarted,
    CellClosed(CellId),
}

struct BlockingDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
    tool_future_dropped: AtomicBool,
    tool_release: Notify,
}

struct DropFlag<'a>(&'a AtomicBool);

impl Drop for DropFlag<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

struct NeverResolvingNotificationDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
}

struct NeverResolvingToolDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
}

impl NeverResolvingNotificationDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<DelegateEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (Arc::new(Self { events_tx }), events_rx)
    }
}

impl NeverResolvingToolDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<DelegateEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (Arc::new(Self { events_tx }), events_rx)
    }
}

impl CodeModeSessionDelegate for NeverResolvingNotificationDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async { Err("unexpected tool call".to_string()) })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async move {
            let _ = self.events_tx.send(DelegateEvent::NotificationStarted);
            std::future::pending().await
        })
    }

    fn cell_closed(&self, cell_id: &CellId) {
        let _ = self
            .events_tx
            .send(DelegateEvent::CellClosed(cell_id.clone()));
    }
}

impl CodeModeSessionDelegate for NeverResolvingToolDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            let _ = self.events_tx.send(DelegateEvent::ToolStarted);
            std::future::pending().await
        })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async { Ok(()) })
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
                tool_future_dropped: AtomicBool::new(false),
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
        let drop_flag = DropFlag(&self.tool_future_dropped);
        Box::pin(async move {
            let _drop_flag = drop_flag;
            let _ = self.events_tx.send(DelegateEvent::ToolStarted);
            tokio::select! {
                _ = self.tool_release.notified() => {
                    Ok(serde_json::Value::Null)
                }
                _ = cancellation_token.cancelled() => {
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
            let response = service
                .execute(ExecuteRequest {
                    yield_time_ms: Some(60_000),
                    ..execute_request(r#"text(String(load("finished")));"#)
                })
                .await
                .unwrap()
                .initial_response()
                .await
                .unwrap();
            let RuntimeResponse::Result { content_items, .. } = response else {
                panic!("expected stored-value probe to complete");
            };
            if content_items
                == vec![FunctionCallOutputContentItem::InputText {
                    text: "true".to_string(),
                }]
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
async fn termination_discards_pending_callbacks_before_responding() {
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
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}

#[tokio::test]
async fn termination_discards_stored_writes_before_the_next_cell_can_load_them() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let cell = service
        .execute(ExecuteRequest {
            enabled_tools: vec![blocking_tool()],
            source: r#"
store("candidate", "leaked");
await tools.block({});
"#
            .to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    // Reaching the delegate proves that the store ran before execution became gated.
    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    assert_eq!(
        service.terminate(cell.cell_id).await.unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
    delegate.release_tool();
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );

    assert_eq!(
        service
            .execute(ExecuteRequest {
                yield_time_ms: Some(60_000),
                ..execute_request(r#"text(String(load("candidate")));"#)
            })
            .await
            .unwrap()
            .initial_response()
            .await
            .unwrap(),
        RuntimeResponse::Result {
            cell_id: cell_id("2"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "undefined".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn shutdown_does_not_await_notifications_during_natural_completion() {
    let (delegate, mut events_rx) = NeverResolvingNotificationDelegate::new();
    let service = Arc::new(CodeModeService::with_delegate(delegate));
    service
        .execute(execute_request(r#"notify("pending");"#))
        .await
        .unwrap();

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationStarted
    );

    let shutdown_service = Arc::clone(&service);
    tokio::time::timeout(Duration::from_millis(/*millis*/ 100), async move {
        shutdown_service.shutdown().await
    })
    .await
    .expect("shutdown should not await a non-cooperative notification")
    .unwrap();
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}

#[tokio::test]
async fn shutdown_does_not_await_a_non_cooperative_nested_tool() {
    let (delegate, mut events_rx) = NeverResolvingToolDelegate::new();
    let service = CodeModeService::with_delegate(delegate);
    let _started = service
        .execute(ExecuteRequest {
            enabled_tools: vec![blocking_tool()],
            source: r#"await tools.block({});"#.to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    tokio::time::timeout(Duration::from_millis(/*millis*/ 100), service.shutdown())
        .await
        .expect("shutdown should not await a non-cooperative nested tool")
        .unwrap();
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}

#[tokio::test]
async fn termination_does_not_await_a_non_cooperative_notification() {
    let (delegate, mut events_rx) = NeverResolvingNotificationDelegate::new();
    let service = CodeModeService::with_delegate(delegate);
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
        tokio::time::timeout(
            Duration::from_millis(/*millis*/ 100),
            service.terminate(cell_id("1")),
        )
        .await
        .expect("termination should not await a non-cooperative notification")
        .unwrap(),
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
            source: concat!(
                "tools.block({});",
                "await new Promise(resolve => setTimeout(resolve, 100));",
                "text('done');",
            )
            .to_string(),
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
    assert!(delegate.tool_future_dropped.load(Ordering::Acquire));
}
