mod callbacks;
mod conversions;
mod types;

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use self::callbacks::CallbackCompletion;
use self::callbacks::finish_callbacks;
use self::callbacks::log_task_result;
use self::callbacks::spawn_notification;
use self::callbacks::spawn_tool;
use self::conversions::cell_tool_kind;
use self::conversions::output_item;
use self::conversions::runtime_request;
use self::types::CellCommand;
pub(crate) use self::types::CellError;
pub(crate) use self::types::CellEventFuture;
pub(crate) use self::types::CellHandle;
pub(crate) use self::types::CellHost;
pub(crate) use self::types::CellState;
pub(crate) use self::types::CellToolCall;
pub(crate) use self::types::CompletionCommit;
use self::types::CompletionDelivery;
use self::types::ObservationDelivery;
use crate::runtime::PendingRuntimeMode;
use crate::runtime::RuntimeCommand;
use crate::runtime::RuntimeControlCommand;
use crate::runtime::RuntimeEvent;
use crate::runtime::spawn_runtime;
use crate::session_runtime::CellEvent;
use crate::session_runtime::CreateCellRequest as CellRequest;
use crate::session_runtime::ObserveMode;
use crate::session_runtime::OutputItem;
use crate::session_runtime::ToolName as CellToolName;

pub(crate) struct CellActor;

impl CellActor {
    pub(crate) fn prepare<H: CellHost>(
        request: CellRequest,
        stored_values: HashMap<String, JsonValue>,
        host: Arc<H>,
        initial_observe_mode: ObserveMode,
        cell_state: Arc<CellState>,
    ) -> Result<
        (
            CellHandle,
            CellEventFuture,
            impl Future<Output = ()> + Send + 'static,
        ),
        String,
    > {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (initial_response_tx, initial_response_rx) = oneshot::channel();
        let (runtime_tx, runtime_control_tx, runtime_terminate_handle) = spawn_runtime(
            stored_values,
            runtime_request(request),
            event_tx,
            PendingRuntimeMode::PauseUntilResumed,
        )?;
        let handle = CellHandle::new(command_tx, Arc::clone(&cell_state));
        let task = run_cell(
            host,
            CellContext {
                runtime_tx,
                runtime_control_tx,
                runtime_terminate_handle,
                cell_state,
            },
            event_rx,
            command_rx,
            Observer {
                mode: initial_observe_mode,
                response_tx: initial_response_tx,
            },
        );
        let initial_response =
            Box::pin(async move { initial_response_rx.await.unwrap_or(Err(CellError::Closed)) });
        Ok((handle, initial_response, task))
    }
}

struct CellContext {
    runtime_tx: std::sync::mpsc::Sender<RuntimeCommand>,
    runtime_control_tx: std::sync::mpsc::Sender<RuntimeControlCommand>,
    runtime_terminate_handle: v8::IsolateHandle,
    cell_state: Arc<CellState>,
}

struct Observer {
    mode: ObserveMode,
    response_tx: oneshot::Sender<Result<CellEvent, CellError>>,
}

async fn run_cell<H: CellHost>(
    host: Arc<H>,
    context: CellContext,
    mut event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
    command_rx: mpsc::UnboundedReceiver<CellCommand>,
    initial_observer: Observer,
) {
    let CellContext {
        runtime_tx,
        runtime_control_tx,
        runtime_terminate_handle,
        cell_state,
    } = context;
    let cancellation_token = cell_state.cancellation_token();
    let callback_cancellation_token = cancellation_token.child_token();
    let mut content_items = Vec::new();
    let mut pending_tool_call_ids = Vec::new();
    let mut observer = Some(initial_observer);
    let mut termination = false;
    let mut runtime_closed = false;
    let mut runtime_paused = false;
    let mut yield_timer: Option<std::pin::Pin<Box<tokio::time::Sleep>>> = None;
    let mut notification_tasks = JoinSet::new();
    let mut tool_tasks = JoinSet::new();
    let mut command_rx = Some(command_rx);
    loop {
        let yield_deadline_elapsed = yield_timer
            .as_ref()
            .is_some_and(|yield_timer| yield_timer.deadline() <= tokio::time::Instant::now());
        tokio::select! {
            biased;
            _ = cancellation_token.cancelled(), if !termination => {
                termination = true;
                yield_timer = None;
                drop(command_rx.take());
                begin_termination(
                    &runtime_tx,
                    &runtime_control_tx,
                    &runtime_terminate_handle,
                    &cancellation_token,
                );
                if runtime_closed {
                    finish_callbacks(
                        &callback_cancellation_token,
                        &mut notification_tasks,
                        &mut tool_tasks,
                        CallbackCompletion::Cancel,
                    ).await;
                    finish_termination(
                        &cell_state,
                        observer.take().map(|observer| observer.response_tx),
                        CellEvent::Terminated {
                            content_items: std::mem::take(&mut content_items),
                        },
                    );
                    break;
                }
            }
            maybe_command = async {
                match command_rx.as_mut() {
                    Some(command_rx) => command_rx.recv().await,
                    None => std::future::pending::<Option<CellCommand>>().await,
                }
            } => {
                let Some(CellCommand::Observe { mode, response_tx }) = maybe_command else {
                    cancellation_token.cancel();
                    continue;
                };
                let response_tx = match cell_state.route_observation(response_tx) {
                    ObservationDelivery::Running(response_tx) => response_tx,
                    ObservationDelivery::Delivered => break,
                    ObservationDelivery::Buffered | ObservationDelivery::Closed => continue,
                };
                if observer
                    .as_ref()
                    .is_some_and(|observer| observer.response_tx.is_closed())
                {
                    observer = None;
                    yield_timer = None;
                }
                if observer.is_some() || termination {
                    let _ = response_tx.send(Err(CellError::Busy));
                    continue;
                }
                observer = Some(Observer { mode, response_tx });
                yield_timer = observer.as_ref().and_then(observer_timer);
                resume_for_observation(
                    mode,
                    &mut runtime_paused,
                    &runtime_tx,
                    &runtime_control_tx,
                );
            }
            _ = async {
                if let Some(yield_timer) = yield_timer.as_mut() {
                    yield_timer.await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                yield_timer = None;
                send_observer_event(
                    observer.take(),
                    CellEvent::Yielded {
                        content_items: std::mem::take(&mut content_items),
                    },
                );
            }
            maybe_event = async {
                if runtime_closed {
                    std::future::pending::<Option<RuntimeEvent>>().await
                } else {
                    event_rx.recv().await
                }
            }, if !yield_deadline_elapsed => {
                let Some(event) = maybe_event else {
                    runtime_closed = true;
                    if termination || cancellation_token.is_cancelled() {
                        finish_callbacks(
                            &callback_cancellation_token,
                            &mut notification_tasks,
                            &mut tool_tasks,
                            CallbackCompletion::Cancel,
                        ).await;
                        finish_termination(
                            &cell_state,
                            observer.take().map(|observer| observer.response_tx),
                            CellEvent::Terminated {
                                content_items: std::mem::take(&mut content_items),
                            },
                        );
                        break;
                    }
                    finish_callbacks(
                        &callback_cancellation_token,
                        &mut notification_tasks,
                        &mut tool_tasks,
                        CallbackCompletion::DrainNotifications,
                    )
                    .await;
                    let event = CellEvent::Completed {
                        content_items: std::mem::take(&mut content_items),
                        error_text: Some("exec runtime ended unexpectedly".to_string()),
                    };
                    let rejected_event = match host
                        .commit_completion(
                            HashMap::new(),
                            event,
                            Arc::clone(&cell_state),
                        )
                        .await
                    {
                        CompletionCommit::Committed => None,
                        CompletionCommit::Rejected(event) => Some(event),
                    };
                    match cell_state.deliver_completion(
                        observer.take().map(|observer| observer.response_tx),
                    ) {
                        CompletionDelivery::Delivered => break,
                        CompletionDelivery::Buffered => {}
                        CompletionDelivery::Rejected(response_tx) => {
                            finish_termination(
                                &cell_state,
                                response_tx,
                                CellEvent::Terminated {
                                    content_items: rejected_completion_content(rejected_event),
                                },
                            );
                            break;
                        }
                    }
                    continue;
                };
                match event {
                    RuntimeEvent::Started => {
                        yield_timer = observer.as_ref().and_then(observer_timer);
                    }
                    RuntimeEvent::Pending => {
                        runtime_paused = true;
                        if matches!(
                            observer.as_ref().map(|observer| observer.mode),
                            Some(ObserveMode::PendingFrontier)
                        ) {
                            yield_timer = None;
                            send_observer_event(
                                observer.take(),
                                CellEvent::Pending {
                                    content_items: std::mem::take(&mut content_items),
                                    pending_tool_call_ids: std::mem::take(
                                        &mut pending_tool_call_ids,
                                    ),
                                },
                            );
                        } else {
                            pending_tool_call_ids.clear();
                            let _ = runtime_control_tx.send(RuntimeControlCommand::Continue);
                            runtime_paused = false;
                        }
                    }
                    RuntimeEvent::ContentItem(item) => content_items.push(output_item(item)),
                    RuntimeEvent::YieldRequested => {
                        if matches!(
                            observer.as_ref().map(|observer| observer.mode),
                            Some(ObserveMode::YieldAfter(_))
                        ) {
                            yield_timer = None;
                            send_observer_event(
                                observer.take(),
                                CellEvent::Yielded {
                                    content_items: std::mem::take(&mut content_items),
                                },
                            );
                        }
                    }
                    RuntimeEvent::Notify { call_id, text } => {
                        spawn_notification(
                            &mut notification_tasks,
                            Arc::clone(&host),
                            call_id,
                            text,
                            callback_cancellation_token.child_token(),
                        );
                    }
                    RuntimeEvent::ToolCall { id, name, kind, input } => {
                        pending_tool_call_ids.push(id.clone());
                        spawn_tool(
                            &mut tool_tasks,
                            Arc::clone(&host),
                            CellToolCall {
                                id,
                                name: CellToolName {
                                    name: name.name,
                                    namespace: name.namespace,
                                },
                                kind: cell_tool_kind(kind),
                                input,
                            },
                            runtime_tx.clone(),
                            callback_cancellation_token.child_token(),
                        );
                    }
                    RuntimeEvent::Result { stored_value_writes, error_text } => {
                        runtime_closed = true;
                        yield_timer = None;
                        if termination || cancellation_token.is_cancelled() {
                            finish_callbacks(
                                &callback_cancellation_token,
                                &mut notification_tasks,
                                &mut tool_tasks,
                                CallbackCompletion::Cancel,
                            ).await;
                            finish_termination(
                                &cell_state,
                                observer.take().map(|observer| observer.response_tx),
                                CellEvent::Terminated {
                                    content_items: std::mem::take(&mut content_items),
                                },
                            );
                            break;
                        }
                        finish_callbacks(
                            &callback_cancellation_token,
                            &mut notification_tasks,
                            &mut tool_tasks,
                            CallbackCompletion::DrainNotifications,
                        )
                        .await;
                        let event = CellEvent::Completed {
                            content_items: std::mem::take(&mut content_items),
                            error_text,
                        };
                        let rejected_event = match host
                            .commit_completion(
                                stored_value_writes,
                                event,
                                Arc::clone(&cell_state),
                            )
                            .await
                        {
                            CompletionCommit::Committed => None,
                            CompletionCommit::Rejected(event) => Some(event),
                        };
                        match cell_state.deliver_completion(
                            observer.take().map(|observer| observer.response_tx),
                        ) {
                            CompletionDelivery::Delivered => break,
                            CompletionDelivery::Buffered => {}
                            CompletionDelivery::Rejected(response_tx) => {
                                finish_termination(
                                    &cell_state,
                                    response_tx,
                                    CellEvent::Terminated {
                                        content_items: rejected_completion_content(rejected_event),
                                    },
                                );
                                break;
                            }
                        }
                    }
                }
            }
            task_result = notification_tasks.join_next(), if !notification_tasks.is_empty() => {
                log_task_result(task_result, "notification");
            }
            task_result = tool_tasks.join_next(), if !tool_tasks.is_empty() => {
                log_task_result(task_result, "tool");
            }
        }
    }
    // Reject requests that arrive while asynchronous terminal cleanup runs.
    cell_state.tombstone();
    drop(command_rx.take());
    begin_termination(
        &runtime_tx,
        &runtime_control_tx,
        &runtime_terminate_handle,
        &cancellation_token,
    );
    finish_callbacks(
        &callback_cancellation_token,
        &mut notification_tasks,
        &mut tool_tasks,
        CallbackCompletion::Cancel,
    )
    .await;
    host.closed().await;
}

fn send_observer_event(observer: Option<Observer>, event: CellEvent) {
    if let Some(observer) = observer {
        let _ = observer.response_tx.send(Ok(event));
    }
}

fn rejected_completion_content(event: Option<CellEvent>) -> Vec<OutputItem> {
    match event {
        Some(CellEvent::Completed { content_items, .. }) => content_items,
        None => Vec::new(),
        Some(event) => panic!("completion commit rejected an unexpected event: {event:?}"),
    }
}

fn finish_termination(
    cell_state: &CellState,
    observer_tx: Option<oneshot::Sender<Result<CellEvent, CellError>>>,
    event: CellEvent,
) {
    if let Some(event) = cell_state.finish_termination(event)
        && let Some(observer_tx) = observer_tx
    {
        let _ = observer_tx.send(Ok(event));
    }
}

fn observer_timer(observer: &Observer) -> Option<std::pin::Pin<Box<tokio::time::Sleep>>> {
    match observer.mode {
        ObserveMode::YieldAfter(duration) => Some(Box::pin(tokio::time::sleep(duration))),
        ObserveMode::PendingFrontier => None,
    }
}

fn resume_for_observation(
    mode: ObserveMode,
    runtime_paused: &mut bool,
    runtime_tx: &std::sync::mpsc::Sender<RuntimeCommand>,
    runtime_control_tx: &std::sync::mpsc::Sender<RuntimeControlCommand>,
) {
    if *runtime_paused {
        let control = match mode {
            ObserveMode::YieldAfter(_) => RuntimeControlCommand::Continue,
            ObserveMode::PendingFrontier => RuntimeControlCommand::Resume,
        };
        let _ = runtime_control_tx.send(control);
        *runtime_paused = false;
    } else if matches!(mode, ObserveMode::PendingFrontier) {
        let _ = runtime_tx.send(RuntimeCommand::ObservePendingFrontier);
    }
}

fn begin_termination(
    runtime_tx: &std::sync::mpsc::Sender<RuntimeCommand>,
    runtime_control_tx: &std::sync::mpsc::Sender<RuntimeControlCommand>,
    runtime_terminate_handle: &v8::IsolateHandle,
    cancellation_token: &CancellationToken,
) {
    cancellation_token.cancel();
    let _ = runtime_tx.send(RuntimeCommand::Terminate);
    let _ = runtime_control_tx.send(RuntimeControlCommand::Terminate);
    let _ = runtime_terminate_handle.terminate_execution();
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
