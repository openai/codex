use std::sync::Arc;
use std::time::Duration;

use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::CodeModeToolKind;
use crate::ToolDefinition;

#[derive(Debug, PartialEq)]
enum DelegateEvent {
    NotificationStarted,
    NotificationFinished,
    ToolStarted,
}

struct BlockingDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
    tool_release: Notify,
}

struct NeverResolvingNotificationDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
}

struct ReleasableNotificationDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
    notification_release: Semaphore,
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

impl ReleasableNotificationDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<DelegateEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                events_tx,
                notification_release: Semaphore::new(/* permits */ 0),
            }),
            events_rx,
        )
    }

    fn release_notification(&self) {
        self.notification_release.add_permits(/* permits */ 1);
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
}

impl CodeModeSessionDelegate for ReleasableNotificationDelegate {
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
            tokio::select! {
                _ = self.notification_release.acquire() => {
                    let _ = self.events_tx.send(DelegateEvent::NotificationFinished);
                    Ok(())
                }
                _ = cancellation_token.cancelled() => {
                    Err("cancelled".to_string())
                }
            }
        })
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
}

impl BlockingDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<DelegateEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                events_tx,
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
}

fn cell_id(value: &str) -> CellId {
    CellId::new(value.to_string())
}

fn execute_request(source: &str) -> ExecuteRequest {
    ExecuteRequest {
        cell_id: cell_id("1"),
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
            source: r#"
text("before");
yield_control();
store("finished", true);
text("after");
"#
            .to_string(),
            yield_time_ms: Some(60_000),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(
        cell,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            }],
        }
    );
    tokio::time::timeout(Duration::from_secs(/*secs*/ 1), async {
        for cell_number in 2.. {
            let response = service
                .execute(ExecuteRequest {
                    cell_id: cell_id(&cell_number.to_string()),
                    ..execute_request(r#"text(String(load("finished")));"#)
                })
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
    assert_eq!(
        service
            .wait(WaitRequest {
                cell_id: cell_id("1"),
                yield_time_ms: 60_000,
            })
            .await
            .unwrap(),
        WaitOutcome::MissingCell(RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            error_text: Some("exec cell 1 not found".to_string()),
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
        cell,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
    tokio::time::timeout(Duration::from_secs(1), async {
        for cell_number in 2.. {
            let response = service
                .execute(ExecuteRequest {
                    cell_id: cell_id(&cell_number.to_string()),
                    ..execute_request(r#"text(String(load("finished")));"#)
                })
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
        cell,
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
}

#[tokio::test]
async fn shutdown_does_not_await_notifications_during_natural_completion() {
    let (delegate, mut events_rx) = NeverResolvingNotificationDelegate::new();
    let service = Arc::new(CodeModeService::with_delegate(delegate));
    service
        .execute(execute_request(r#"yield_control(); notify("pending");"#))
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
}

#[tokio::test]
async fn termination_does_not_await_notifications_during_natural_completion() {
    let (delegate, mut events_rx) = NeverResolvingNotificationDelegate::new();
    let service = CodeModeService::with_delegate(delegate);
    service
        .execute(execute_request(r#"yield_control(); notify("pending");"#))
        .await
        .unwrap();

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationStarted
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
}

#[tokio::test]
async fn natural_completion_waits_for_notifications_before_responding() {
    let (delegate, mut events_rx) = ReleasableNotificationDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let mut initial_response = Box::pin(service.execute(execute_request(r#"notify("pending");"#)));
    std::future::poll_fn(|context| match initial_response.as_mut().poll(context) {
        std::task::Poll::Pending => std::task::Poll::Ready(()),
        std::task::Poll::Ready(result) => {
            panic!("execute returned before the notification was released: {result:?}")
        }
    })
    .await;

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationStarted
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut initial_response)
            .await
            .is_err()
    );

    delegate.release_notification();

    assert_eq!(
        initial_response.await.unwrap(),
        RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            error_text: None,
        }
    );
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationFinished
    );
}

#[tokio::test]
async fn termination_discards_pending_tools_before_responding() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let cell = service
        .execute(ExecuteRequest {
            enabled_tools: vec![blocking_tool()],
            source: r#"await tools.block({});"#.to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    assert_eq!(
        cell,
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
}

#[tokio::test]
async fn shutdown_discards_pending_tools_before_returning() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = Arc::new(CodeModeService::with_delegate(delegate.clone()));
    service
        .execute(ExecuteRequest {
            enabled_tools: vec![blocking_tool()],
            source: r#"await tools.block({});"#.to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);

    let shutdown_service = Arc::clone(&service);
    assert_eq!(shutdown_service.shutdown().await, Ok(()));
}

#[tokio::test]
async fn shutdown_completes_when_a_nested_tool_ignores_cancellation() {
    let (delegate, mut events_rx) = NeverResolvingToolDelegate::new();
    let service = Arc::new(CodeModeService::with_delegate(delegate));
    service
        .execute(ExecuteRequest {
            enabled_tools: vec![blocking_tool()],
            source: r#"await tools.block({});"#.to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);

    let shutdown_service = Arc::clone(&service);
    tokio::time::timeout(Duration::from_millis(/*millis*/ 100), async move {
        shutdown_service.shutdown().await
    })
    .await
    .expect("shutdown should not await a non-cooperative nested tool")
    .unwrap();
}

#[tokio::test]
async fn second_observer_is_rejected_without_displacing_the_first() {
    let service = CodeModeService::new();
    let cell = service
        .execute(execute_request("await new Promise(() => {});"))
        .await
        .unwrap();

    assert_eq!(
        cell,
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
async fn dropping_a_wait_allows_a_later_wait_to_observe_the_cell() {
    let service = CodeModeService::new();
    let cell = service
        .execute(execute_request(
            "yield_control(); await new Promise(() => {});",
        ))
        .await
        .unwrap();

    assert_eq!(
        cell,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );

    let abandoned_wait = service
        .begin_wait(WaitRequest {
            cell_id: cell_id("1"),
            yield_time_ms: 60_000,
        })
        .await;
    drop(abandoned_wait);

    assert_eq!(
        service
            .wait(WaitRequest {
                cell_id: cell_id("1"),
                yield_time_ms: 0,
            })
            .await
            .unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
    assert_eq!(
        service.terminate(cell_id("1")).await.unwrap(),
        WaitOutcome::LiveCell(RuntimeResponse::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        })
    );
}
