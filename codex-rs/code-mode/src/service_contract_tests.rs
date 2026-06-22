use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
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
    NotificationCancelled,
    NotificationFinished,
    ToolStarted,
    ToolCancelled,
    CellClosed(CellId),
}

fn record_cell_closed(events_tx: &mpsc::UnboundedSender<DelegateEvent>, cell_id: &CellId) {
    let _ = events_tx.send(DelegateEvent::CellClosed(cell_id.clone()));
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

struct ReleasableNotificationDelegate {
    events_tx: mpsc::UnboundedSender<DelegateEvent>,
    notification_release: Semaphore,
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

impl ReleasableNotificationDelegate {
    fn new() -> (Arc<Self>, mpsc::UnboundedReceiver<DelegateEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        (
            Arc::new(Self {
                events_tx,
                notification_release: Semaphore::new(/*permits*/ 0),
            }),
            events_rx,
        )
    }

    fn release_notification(&self) {
        self.notification_release.add_permits(/*permits*/ 1);
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
        record_cell_closed(&self.events_tx, cell_id);
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

    fn cell_closed(&self, cell_id: &CellId) {
        record_cell_closed(&self.events_tx, cell_id);
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
        record_cell_closed(&self.events_tx, cell_id);
    }
}

fn cell_id(value: &str) -> CellId {
    CellId::new(value.to_string())
}

fn execute_request(source: &str) -> CreateCellRequest {
    CreateCellRequest {
        cell_id: cell_id("1"),
        tool_call_id: "call-1".to_string(),
        enabled_tools: Vec::new(),
        source: source.to_string(),
    }
}

async fn execute(service: &CodeModeService, request: CreateCellRequest) -> RuntimeResponse {
    execute_with_yield_time(service, request, /*yield_time_ms*/ 1).await
}

async fn execute_with_yield_time(
    service: &CodeModeService,
    request: CreateCellRequest,
    yield_time_ms: u64,
) -> RuntimeResponse {
    let cell_id = service.create_cell(request).await.unwrap();
    service
        .observe(ObserveRequest {
            cell_id,
            generation: ObservationGeneration::INITIAL,
            yield_time_ms,
        })
        .await
        .unwrap()
        .into()
}

async fn create_and_observe_to_pending(
    service: &CodeModeService,
    request: CreateCellRequest,
) -> Result<PendingOutcome, String> {
    let cell_id = service.create_pausable_cell(request).await?;
    match service
        .observe_to_pending(ObserveToPendingRequest { cell_id })
        .await?
    {
        ObserveToPendingOutcome::LiveCell(outcome) => Ok(outcome),
        ObserveToPendingOutcome::MissingCell(response) => Ok(PendingOutcome::Completed(response)),
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
async fn create_retry_returns_the_cell_from_the_original_ambiguous_request() {
    let service = CodeModeService::new();
    let request = execute_request("await new Promise(() => {});");

    let original_cell_id = service.create_cell(request.clone()).await.unwrap();
    let retry_cell_id = service.create_cell(request).await.unwrap();

    assert_eq!(retry_cell_id, original_cell_id);
    service.terminate(original_cell_id).await.unwrap();
}

#[tokio::test]
async fn cancelled_observation_is_replayed_by_its_generation() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = Arc::new(CodeModeService::with_delegate(delegate.clone()));
    let created_cell_id = service
        .create_cell(CreateCellRequest {
            cell_id: cell_id("1"),
            enabled_tools: vec![blocking_tool()],
            source: r#"await tools.block({}); text("done");"#.to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();
    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    let request = ObserveRequest {
        cell_id: created_cell_id,
        generation: ObservationGeneration::INITIAL,
        yield_time_ms: 60_000,
    };

    let first_attempt = tokio::spawn({
        let service = Arc::clone(&service);
        let request = request.clone();
        async move { service.observe(request).await }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if service
                .observations
                .lock()
                .await
                .contains_key(&cell_id("1"))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("observation registration timed out");
    first_attempt.abort();
    assert!(first_attempt.await.unwrap_err().is_cancelled());
    delegate.release_tool();

    assert_eq!(
        service.observe(request).await,
        Ok(ObserveOutcome::Completed {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        })
    );
}

#[tokio::test]
async fn next_observation_generation_evicts_the_previous_result() {
    let service = CodeModeService::new();
    let first = execute_with_yield_time(
        &service,
        CreateCellRequest {
            source: r#"text("before"); yield_control(); text("after");"#.to_string(),
            ..execute_request("")
        },
        /*yield_time_ms*/ 60_000,
    )
    .await;
    assert_eq!(
        first,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            }],
        }
    );

    assert_eq!(
        service
            .observe(ObserveRequest {
                cell_id: cell_id("1"),
                generation: ObservationGeneration::new(/*value*/ 1),
                yield_time_ms: 60_000,
            })
            .await
            .unwrap(),
        ObserveOutcome::Completed {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            }],
            error_text: None,
        }
    );

    {
        let observations = service.observations.lock().await;
        assert_eq!(observations.len(), 1);
        assert_eq!(
            observations
                .get(&cell_id("1"))
                .map(|record| record.request.generation),
            Some(ObservationGeneration::new(/*value*/ 1))
        );
    }
    assert_eq!(
        service
            .observe(ObserveRequest {
                cell_id: cell_id("1"),
                generation: ObservationGeneration::INITIAL,
                yield_time_ms: 60_000,
            })
            .await
            .unwrap_err(),
        "expected observation generation 2 for cell 1, got 0"
    );
}

#[tokio::test]
async fn yields_and_resumes() {
    let service = CodeModeService::new();
    let cell = execute_with_yield_time(
        &service,
        CreateCellRequest {
            source: r#"text("before"); yield_control(); text("after");"#.to_string(),
            ..execute_request("")
        },
        /*yield_time_ms*/ 60_000,
    )
    .await;

    assert_eq!(
        cell,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            }],
        }
    );
    assert_eq!(
        service
            .observe(ObserveRequest {
                cell_id: cell_id("1"),
                generation: ObservationGeneration::new(/*value*/ 1),
                yield_time_ms: 60_000,
            })
            .await
            .unwrap(),
        ObserveOutcome::Completed {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn yield_before_first_observation_preserves_its_output_boundary() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let cell_id = service
        .create_cell(CreateCellRequest {
            enabled_tools: vec![blocking_tool()],
            source: r#"
text("before");
yield_control();
text("after");
await tools.block({});
"#
            .to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();
    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);

    assert_eq!(
        service
            .observe(ObserveRequest {
                cell_id: cell_id.clone(),
                generation: ObservationGeneration::INITIAL,
                yield_time_ms: 60_000,
            })
            .await
            .unwrap(),
        ObserveOutcome::Yielded {
            cell_id: cell_id.clone(),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "before".to_string(),
            }],
        }
    );

    delegate.release_tool();
    assert_eq!(
        service
            .observe(ObserveRequest {
                cell_id: cell_id.clone(),
                generation: ObservationGeneration::new(/*value*/ 1),
                yield_time_ms: 60_000,
            })
            .await
            .unwrap(),
        ObserveOutcome::Completed {
            cell_id,
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn background_completion_notifies_the_delegate_without_another_observation() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate);
    let created_cell_id = service
        .create_cell(execute_request(
            r#"await new Promise(resolve => setTimeout(resolve, 100)); text("done");"#,
        ))
        .await
        .unwrap();
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            service.observe(ObserveRequest {
                cell_id: created_cell_id.clone(),
                generation: ObservationGeneration::INITIAL,
                yield_time_ms: 1,
            }),
        )
        .await
        .expect("initial observation should yield while the cell is still running")
        .unwrap(),
        ObserveOutcome::Yielded {
            cell_id: created_cell_id.clone(),
            content_items: Vec::new(),
        }
    );

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(created_cell_id)
    );
}

#[tokio::test]
async fn returns_and_resumes_from_the_pending_frontier() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());

    assert_eq!(
        create_and_observe_to_pending(
            &service,
            CreateCellRequest {
                enabled_tools: vec![blocking_tool()],
                source: r#"
await tools.block({});
text("after");
"#
                .to_string(),
                ..execute_request("")
            },
        )
        .await
        .unwrap(),
        PendingOutcome::Pending {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            pending_tool_call_ids: vec!["tool-1".to_string()],
        }
    );

    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    delegate.release_tool();

    assert_eq!(
        service
            .observe_to_pending(ObserveToPendingRequest {
                cell_id: cell_id("1"),
            })
            .await
            .unwrap(),
        ObserveToPendingOutcome::LiveCell(PendingOutcome::Completed(RuntimeResponse::Result {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "after".to_string(),
            }],
            error_text: None,
        }))
    );
}

#[tokio::test]
async fn observed_natural_completion_wins_over_termination() {
    let service = CodeModeService::new();
    let mut probe_generation = 0;
    let cell = execute(
        &service,
        execute_request(r#"yield_control(); store("finished", true); text("done");"#),
    )
    .await;

    assert_eq!(
        cell,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            probe_generation += 1;
            // Observe the synchronous probe through completion so scheduler load
            // cannot turn its short yield deadline into a false lifecycle failure.
            let response = create_and_observe_to_pending(
                &service,
                CreateCellRequest {
                    cell_id: cell_id(&format!("completion-probe-{probe_generation}")),
                    ..execute_request(r#"text(String(load("finished")));"#)
                },
            )
            .await
            .unwrap();
            let PendingOutcome::Completed(RuntimeResponse::Result { content_items, .. }) = response
            else {
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
        TerminateOutcome::Completed {
            cell_id: cell_id("1"),
            content_items: vec![FunctionCallOutputContentItem::InputText {
                text: "done".to_string(),
            }],
            error_text: None,
        }
    );
}

#[tokio::test]
async fn termination_cancels_pending_callbacks_before_responding() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let cell = execute(
        &service,
        execute_request(r#"notify("pending"); await new Promise(() => {});"#),
    )
    .await;

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
        TerminateOutcome::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
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
async fn termination_discards_stored_writes_before_the_next_cell_can_load_them() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let created_cell_id = service
        .create_cell(CreateCellRequest {
            enabled_tools: vec![blocking_tool()],
            source: r#"
store("candidate", "leaked");
await tools.block({});
"#
            .to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();

    // Reaching the delegate proves that the store ran before execution became gated.
    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    assert_eq!(
        service.terminate(created_cell_id).await.unwrap(),
        TerminateOutcome::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
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

    assert_eq!(
        execute_with_yield_time(
            &service,
            CreateCellRequest {
                cell_id: cell_id("2"),
                ..execute_request(r#"text(String(load("candidate")));"#)
            },
            /*yield_time_ms*/ 60_000,
        )
        .await,
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
async fn shutdown_cancels_notifications_while_natural_completion_is_draining() {
    let (delegate, mut events_rx) = HeldNotificationDelegate::new();
    let service = Arc::new(CodeModeService::with_delegate(delegate.clone()));
    service
        .create_cell(execute_request(r#"notify("pending");"#))
        .await
        .unwrap();

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationStarted
    );

    let shutdown_service = Arc::clone(&service);
    let shutdown = tokio::spawn(async move { shutdown_service.shutdown().await });

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationCancelled
    );
    delegate.release_notification();

    assert_eq!(shutdown.await.unwrap(), Ok(()));
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}

#[tokio::test]
async fn repeated_termination_is_rejected_while_callback_cleanup_is_pending() {
    let (delegate, mut events_rx) = HeldNotificationDelegate::new();
    let service = Arc::new(CodeModeService::with_delegate(delegate.clone()));
    let cell = execute(
        &service,
        execute_request(r#"notify("pending"); await new Promise(() => {});"#),
    )
    .await;

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
        TerminateOutcome::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}

#[tokio::test]
async fn create_cell_returns_before_natural_completion() {
    let (delegate, mut events_rx) = ReleasableNotificationDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let created_cell_id = service
        .create_cell(execute_request(r#"notify("pending");"#))
        .await
        .unwrap();
    assert_eq!(created_cell_id, cell_id("1"));
    let mut observation = Box::pin(service.observe(ObserveRequest {
        cell_id: created_cell_id,
        generation: ObservationGeneration::INITIAL,
        yield_time_ms: 60_000,
    }));

    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationStarted
    );
    std::future::poll_fn(|context| match observation.as_mut().poll(context) {
        std::task::Poll::Pending => std::task::Poll::Ready(()),
        std::task::Poll::Ready(result) => {
            panic!("observation returned while the notification was blocked: {result:?}")
        }
    })
    .await;

    delegate.release_notification();

    assert_eq!(
        observation.await.unwrap(),
        ObserveOutcome::Completed {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
            error_text: None,
        }
    );
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::NotificationFinished
    );
    assert_eq!(
        next_event(&mut events_rx).await,
        DelegateEvent::CellClosed(cell_id("1"))
    );
}

#[tokio::test]
async fn created_cell_can_be_terminated_before_observation() {
    let service = CodeModeService::new();
    let created_cell_id = service
        .create_cell(CreateCellRequest {
            source: "await new Promise(() => {});".to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(created_cell_id, cell_id("1"));
    assert_eq!(
        service.terminate(created_cell_id.clone()).await.unwrap(),
        TerminateOutcome::Terminated {
            cell_id: created_cell_id,
            content_items: Vec::new(),
        }
    );
}

#[tokio::test]
async fn second_observer_is_rejected_without_displacing_the_first() {
    let service = CodeModeService::new();
    let cell = execute(&service, execute_request("await new Promise(() => {});")).await;

    assert_eq!(
        cell,
        RuntimeResponse::Yielded {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );

    let first_observer = service
        .begin_observe(ObserveRequest {
            cell_id: cell_id("1"),
            generation: ObservationGeneration::new(/*value*/ 1),
            yield_time_ms: 60_000,
        })
        .await;
    assert_eq!(
        service
            .observe(ObserveRequest {
                cell_id: cell_id("1"),
                generation: ObservationGeneration::new(/*value*/ 1),
                yield_time_ms: 60_000,
            })
            .await
            .unwrap_err(),
        "exec cell 1 already has an active observer"
    );

    let terminated = TerminateOutcome::Terminated {
        cell_id: cell_id("1"),
        content_items: Vec::new(),
    };
    assert_eq!(service.terminate(cell_id("1")).await.unwrap(), terminated);
    assert_eq!(
        first_observer.await.unwrap(),
        ObserveOutcome::Terminated {
            cell_id: cell_id("1"),
            content_items: Vec::new(),
        }
    );
}

#[tokio::test]
async fn natural_completion_cleans_up_callbacks_before_responding() {
    let (delegate, mut events_rx) = BlockingDelegate::new();
    let service = CodeModeService::with_delegate(delegate.clone());
    let created_cell_id = service
        .create_cell(CreateCellRequest {
            enabled_tools: vec![blocking_tool()],
            source: concat!(
                "tools.block({});",
                "await new Promise(resolve => setTimeout(resolve, 100));",
                "text('done');",
            )
            .to_string(),
            ..execute_request("")
        })
        .await
        .unwrap();

    assert_eq!(next_event(&mut events_rx).await, DelegateEvent::ToolStarted);
    assert_eq!(
        service
            .observe(ObserveRequest {
                cell_id: created_cell_id,
                generation: ObservationGeneration::INITIAL,
                yield_time_ms: 60_000,
            })
            .await
            .unwrap(),
        ObserveOutcome::Completed {
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
