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
#[cfg(test)]
pub(crate) use self::types::CellEvent;
pub(crate) use self::types::CellEvent as ActorEvent;
pub(crate) use self::types::CellEventFuture;
pub(crate) use self::types::CellHandle;
pub(crate) use self::types::CellHost;
pub(crate) use self::types::CellState;
pub(crate) use self::types::CellToolCall;
pub(crate) use self::types::CompletionCommit;
use self::types::CompletionDelivery;
use self::types::ObservationDelivery;
pub(crate) use self::types::ObserveMode;
use crate::runtime::PendingRuntimeMode;
use crate::runtime::RuntimeCommand;
use crate::runtime::RuntimeControlCommand;
use crate::runtime::RuntimeEvent;
use crate::runtime::spawn_runtime;
use crate::session_runtime::CellExecutionPolicy;
use crate::session_runtime::CreateCellRequest as CellRequest;
use crate::session_runtime::OutputItem;
use crate::session_runtime::PendingFrontier;
use crate::session_runtime::PendingGeneration;
use crate::session_runtime::ResumeOutcome;
use crate::session_runtime::ToolName as CellToolName;

pub(crate) struct CellActor;

impl CellActor {
    pub(crate) fn prepare<H: CellHost>(
        request: CellRequest,
        stored_values: HashMap<String, JsonValue>,
        host: Arc<H>,
        cell_state: Arc<CellState>,
        execution_policy: CellExecutionPolicy,
    ) -> Result<(CellHandle, impl Future<Output = ()> + Send + 'static), String> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (command_tx, command_rx) = mpsc::unbounded_channel();
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
            execution_policy,
        );
        Ok((handle, task))
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
    response_tx: oneshot::Sender<Result<ActorEvent, CellError>>,
}

async fn run_cell<H: CellHost>(
    host: Arc<H>,
    context: CellContext,
    mut event_rx: mpsc::UnboundedReceiver<RuntimeEvent>,
    command_rx: mpsc::UnboundedReceiver<CellCommand>,
    execution_policy: CellExecutionPolicy,
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
    let mut pending_initial_yield_items: Option<Vec<OutputItem>> = None;
    let mut pending_frontier: Option<PendingFrontier> = None;
    let mut pending_frontier_observed = false;
    let mut next_pending_generation = 1;
    let mut last_resumed_generation = None;
    let mut observer: Option<Observer> = None;
    let mut has_been_observed = false;
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
                        ActorEvent::Terminated {
                            content_items: take_termination_content(
                                &mut pending_frontier,
                                pending_frontier_observed,
                                &mut pending_initial_yield_items,
                                &mut content_items,
                            ),
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
                let Some(command) = maybe_command else {
                    cancellation_token.cancel();
                    continue;
                };
                let (mode, response_tx) = match command {
                    CellCommand::Observe { mode, response_tx } => (mode, response_tx),
                    CellCommand::Resume { generation, response_tx } => {
                        let result = if termination {
                            Err(CellError::Closed)
                        } else if let Some(frontier) = pending_frontier.as_ref() {
                            let current = frontier.generation;
                            match generation.cmp(&current) {
                                std::cmp::Ordering::Less => Ok(ResumeOutcome::AlreadyRunning),
                                std::cmp::Ordering::Greater => {
                                    Err(CellError::InvalidGeneration {
                                        requested: generation,
                                        latest: Some(current),
                                    })
                                }
                                std::cmp::Ordering::Equal => {
                                    pending_frontier = None;
                                    pending_frontier_observed = false;
                                    last_resumed_generation = Some(generation);
                                    runtime_paused = false;
                                    let _ = runtime_control_tx
                                        .send(RuntimeControlCommand::Continue);
                                    Ok(ResumeOutcome::Resumed)
                                }
                            }
                        } else {
                            let latest = last_resumed_generation;
                            if latest.is_some_and(|latest| generation <= latest) {
                                Ok(ResumeOutcome::AlreadyRunning)
                            } else {
                                Err(CellError::InvalidGeneration {
                                    requested: generation,
                                    latest,
                                })
                            }
                        };
                        let _ = response_tx.send(result);
                        continue;
                    }
                };
                if response_tx.is_closed() {
                    continue;
                }
                let response_tx = match cell_state.route_observation(mode, response_tx) {
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
                has_been_observed = true;
                if matches!(mode, ObserveMode::YieldAfter(_))
                    && let Some(yielded_items) = pending_initial_yield_items.take()
                {
                    let delivered = match send_cell_event(
                        response_tx,
                        ActorEvent::Yielded {
                            content_items: yielded_items,
                        },
                    ) {
                        Ok(()) => true,
                        Err(ActorEvent::Yielded { content_items }) => {
                            pending_initial_yield_items = Some(content_items);
                            has_been_observed = false;
                            false
                        }
                        Err(event) => {
                            panic!("initial yield delivery returned an unexpected event: {event:?}")
                        }
                    };
                    if delivered && runtime_paused {
                        pending_frontier = None;
                        pending_frontier_observed = false;
                        let _ = runtime_control_tx.send(RuntimeControlCommand::Continue);
                        runtime_paused = false;
                    }
                    continue;
                }
                if matches!(mode, ObserveMode::PendingFrontier)
                    && let Some(frontier) = pending_frontier.as_ref()
                {
                    if send_cell_event(response_tx, ActorEvent::Pending(frontier.clone())).is_ok() {
                        pending_frontier_observed = true;
                    }
                    continue;
                }
                observer = Some(Observer { mode, response_tx });
                yield_timer = observer.as_ref().and_then(observer_timer);
                if runtime_paused && matches!(mode, ObserveMode::YieldAfter(_)) {
                    pending_frontier = None;
                    pending_frontier_observed = false;
                    let _ = runtime_control_tx.send(RuntimeControlCommand::Continue);
                    runtime_paused = false;
                }
            }
            _ = async {
                if let Some(yield_timer) = yield_timer.as_mut() {
                    yield_timer.await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                yield_timer = None;
                restore_undelivered_yield(
                    send_observer_event(
                        observer.take(),
                        ActorEvent::Yielded {
                            content_items: std::mem::take(&mut content_items),
                        },
                    ),
                    &mut content_items,
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
                        let termination_content_items = take_termination_content(
                            &mut pending_frontier,
                            pending_frontier_observed,
                            &mut pending_initial_yield_items,
                            &mut content_items,
                        );
                        finish_callbacks(
                            &callback_cancellation_token,
                            &mut notification_tasks,
                            &mut tool_tasks,
                            CallbackCompletion::Cancel,
                        ).await;
                        finish_termination(
                            &cell_state,
                            observer.take().map(|observer| observer.response_tx),
                            ActorEvent::Terminated {
                                content_items: termination_content_items,
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
                    let event = ActorEvent::Completed {
                        content_items: take_termination_content(
                            &mut pending_frontier,
                            pending_frontier_observed,
                            &mut pending_initial_yield_items,
                            &mut content_items,
                        ),
                        error_text: Some("exec runtime ended unexpectedly".to_string()),
                    };
                    let rejected_event = match host
                        .commit_completion(
                            HashMap::new(),
                            event,
                            /*pending_initial_yield_items*/ None,
                            Arc::clone(&cell_state),
                        )
                        .await
                    {
                        CompletionCommit::Committed => None,
                        CompletionCommit::Rejected(event) => Some(event),
                    };
                    match cell_state.deliver_completion(
                        observer
                            .take()
                            .map(|observer| (observer.mode, observer.response_tx)),
                    ) {
                        CompletionDelivery::Delivered => break,
                        CompletionDelivery::Buffered => {}
                        CompletionDelivery::Rejected(response_tx) => {
                            finish_termination(
                                &cell_state,
                                response_tx,
                                ActorEvent::Terminated {
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
                    RuntimeEvent::Pending {
                        pending_tool_call_ids,
                    } => {
                        runtime_paused = true;
                        if matches!(
                            execution_policy,
                            CellExecutionPolicy::ContinueWhenUnblocked
                        ) {
                            pending_frontier = None;
                            let _ = runtime_control_tx.send(RuntimeControlCommand::Continue);
                            runtime_paused = false;
                        } else {
                            if pending_frontier.is_none() {
                                pending_frontier_observed = false;
                            }
                            let frontier = pending_frontier.get_or_insert_with(|| {
                                let generation = PendingGeneration::new(next_pending_generation);
                                next_pending_generation += 1;
                                PendingFrontier {
                                    generation,
                                    content_items: take_all_content(
                                        &mut pending_initial_yield_items,
                                        &mut content_items,
                                    ),
                                    pending_tool_call_ids,
                                }
                            });
                            if let Some(observer) = observer.take_if(|observer| {
                                observer.mode == ObserveMode::PendingFrontier
                            }) {
                                yield_timer = None;
                                if send_cell_event(
                                    observer.response_tx,
                                    ActorEvent::Pending(frontier.clone()),
                                )
                                .is_ok()
                                {
                                    pending_frontier_observed = true;
                                }
                            }
                        }
                    }
                    RuntimeEvent::ContentItem(item) => content_items.push(output_item(item)),
                    RuntimeEvent::YieldRequested => {
                        // An unattached yield is normally a no-op. Preserve only the first
                        // pre-observation yield so create followed by its initial observe/wait
                        // retains the former execute initial-response behavior. After an
                        // observation attaches, later unattached yields do not affect a future
                        // observation.
                        let yield_observer = matches!(
                            observer.as_ref().map(|observer| observer.mode),
                            Some(ObserveMode::YieldAfter(_))
                        );
                        if yield_observer {
                            yield_timer = None;
                            restore_undelivered_yield(
                                send_observer_event(
                                    observer.take(),
                                    ActorEvent::Yielded {
                                        content_items: std::mem::take(&mut content_items),
                                    },
                                ),
                                &mut content_items,
                            );
                        } else if observer.is_none()
                            && !has_been_observed
                            && pending_initial_yield_items.is_none()
                        {
                            pending_initial_yield_items = Some(std::mem::take(&mut content_items));
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
                            let termination_content_items = take_termination_content(
                                &mut pending_frontier,
                                pending_frontier_observed,
                                &mut pending_initial_yield_items,
                                &mut content_items,
                            );
                            finish_callbacks(
                                &callback_cancellation_token,
                                &mut notification_tasks,
                                &mut tool_tasks,
                                CallbackCompletion::Cancel,
                            ).await;
                            finish_termination(
                                &cell_state,
                                observer.take().map(|observer| observer.response_tx),
                                ActorEvent::Terminated {
                                    content_items: termination_content_items,
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
                        let event = ActorEvent::Completed {
                            content_items: std::mem::take(&mut content_items),
                            error_text,
                        };
                        let rejected_event = match host
                            .commit_completion(
                                stored_value_writes,
                                event,
                                pending_initial_yield_items.take(),
                                Arc::clone(&cell_state),
                            )
                            .await
                        {
                            CompletionCommit::Committed => None,
                            CompletionCommit::Rejected(event) => Some(event),
                        };
                        match cell_state.deliver_completion(
                            observer
                                .take()
                                .map(|observer| (observer.mode, observer.response_tx)),
                        ) {
                            CompletionDelivery::Delivered => break,
                            CompletionDelivery::Buffered => {}
                            CompletionDelivery::Rejected(response_tx) => {
                                finish_termination(
                                    &cell_state,
                                    response_tx,
                                    ActorEvent::Terminated {
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

fn send_observer_event(observer: Option<Observer>, event: ActorEvent) -> Result<(), ActorEvent> {
    let Some(observer) = observer else {
        return Err(event);
    };
    send_cell_event(observer.response_tx, event)
}

fn send_cell_event(
    response_tx: oneshot::Sender<Result<ActorEvent, CellError>>,
    event: ActorEvent,
) -> Result<(), ActorEvent> {
    match response_tx.send(Ok(event)) {
        Ok(()) => Ok(()),
        Err(Ok(event)) => Err(event),
        Err(Err(error)) => panic!("cell event delivery returned an actor error: {error:?}"),
    }
}

fn restore_undelivered_yield(
    delivery: Result<(), ActorEvent>,
    content_items: &mut Vec<OutputItem>,
) {
    match delivery {
        Ok(()) => {}
        Err(ActorEvent::Yielded {
            content_items: mut undelivered_items,
        }) => {
            undelivered_items.append(content_items);
            *content_items = undelivered_items;
        }
        Err(event) => panic!("yield delivery returned an unexpected event: {event:?}"),
    }
}

fn rejected_completion_content(event: Option<ActorEvent>) -> Vec<OutputItem> {
    match event {
        Some(ActorEvent::Completed { content_items, .. }) => content_items,
        None => Vec::new(),
        Some(event) => panic!("completion commit rejected an unexpected event: {event:?}"),
    }
}

fn take_all_content(
    pending_initial_yield_items: &mut Option<Vec<OutputItem>>,
    content_items: &mut Vec<OutputItem>,
) -> Vec<OutputItem> {
    let Some(mut yielded_items) = pending_initial_yield_items.take() else {
        return std::mem::take(content_items);
    };
    yielded_items.append(content_items);
    yielded_items
}

fn take_termination_content(
    pending_frontier: &mut Option<PendingFrontier>,
    pending_frontier_observed: bool,
    pending_initial_yield_items: &mut Option<Vec<OutputItem>>,
    content_items: &mut Vec<OutputItem>,
) -> Vec<OutputItem> {
    let mut termination_content = match pending_frontier.take() {
        Some(_) if pending_frontier_observed => Vec::new(),
        Some(frontier) => frontier.content_items,
        None => Vec::new(),
    };
    termination_content.append(&mut take_all_content(
        pending_initial_yield_items,
        content_items,
    ));
    termination_content
}

fn finish_termination(
    cell_state: &CellState,
    observer_tx: Option<oneshot::Sender<Result<ActorEvent, CellError>>>,
    event: ActorEvent,
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
