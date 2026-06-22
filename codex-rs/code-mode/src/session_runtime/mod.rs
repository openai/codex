mod types;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

pub(crate) use self::types::Cell;
pub(crate) use self::types::CellEvent;
pub(crate) use self::types::CellExecutionPolicy;
pub(crate) use self::types::CellId;
pub(crate) use self::types::CellKind;
pub(crate) use self::types::CreateCellRequest;
pub(crate) use self::types::Error;
pub(crate) use self::types::ImageDetail;
pub(crate) use self::types::NestedToolCall;
pub(crate) use self::types::OutputItem;
pub(crate) use self::types::PausableCell;
pub(crate) use self::types::PausableCellEvent;
pub(crate) use self::types::PendingFrontier;
pub(crate) use self::types::PendingGeneration;
pub(crate) use self::types::ResumeOutcome;
pub(crate) use self::types::SessionRuntimeDelegate;
pub(crate) use self::types::ToolDefinition;
pub(crate) use self::types::ToolKind;
pub(crate) use self::types::ToolName;
use crate::cell_actor::ActorEvent;
use crate::cell_actor::CellActor;
use crate::cell_actor::CellError;
use crate::cell_actor::CellEventFuture;
use crate::cell_actor::CellHandle;
use crate::cell_actor::CellHost;
use crate::cell_actor::CellState;
use crate::cell_actor::CellToolCall;
use crate::cell_actor::CompletionCommit;
use crate::cell_actor::ObserveMode;

type RuntimeEventFuture = Pin<Box<dyn Future<Output = Result<CellEvent, Error>> + Send + 'static>>;
type PausableRuntimeEventFuture =
    Pin<Box<dyn Future<Output = Result<PausableCellEvent, Error>> + Send + 'static>>;

/// Owns all cells and shared state for one transport-neutral code-mode session.
pub(crate) struct SessionRuntime<D: SessionRuntimeDelegate> {
    inner: Arc<Inner<D>>,
}

struct Inner<D: SessionRuntimeDelegate> {
    stored_values: Mutex<HashMap<String, JsonValue>>,
    cells: Mutex<HashMap<CellId, RegisteredCell>>,
    cell_tasks: TaskTracker,
    shutdown_token: CancellationToken,
    delegate: Arc<D>,
    next_cell_id: AtomicU64,
}

#[derive(Clone)]
enum RegisteredCell {
    Continuing(CellHandle),
    Pausable(CellHandle),
}

impl RegisteredCell {
    fn kind(&self) -> CellKind {
        match self {
            Self::Continuing(_) => CellKind::Continuing,
            Self::Pausable(_) => CellKind::Pausable,
        }
    }

    fn handle(&self) -> &CellHandle {
        match self {
            Self::Continuing(handle) | Self::Pausable(handle) => handle,
        }
    }
}

impl<D: SessionRuntimeDelegate> SessionRuntime<D> {
    pub(crate) fn new(delegate: Arc<D>) -> Self {
        Self {
            inner: Arc::new(Inner {
                stored_values: Mutex::new(HashMap::new()),
                cells: Mutex::new(HashMap::new()),
                cell_tasks: TaskTracker::new(),
                shutdown_token: CancellationToken::new(),
                delegate,
                next_cell_id: AtomicU64::new(1),
            }),
        }
    }

    pub(crate) fn is_alive(&self) -> bool {
        !self.inner.shutdown_token.is_cancelled()
    }

    pub(crate) async fn create_cell(&self, request: CreateCellRequest) -> Result<Cell, Error> {
        let id = self
            .start_cell(
                request,
                CellExecutionPolicy::ContinueWhenUnblocked,
                CellKind::Continuing,
            )
            .await?;
        Ok(Cell::new(id))
    }

    pub(crate) async fn create_pausable_cell(
        &self,
        request: CreateCellRequest,
    ) -> Result<PausableCell, Error> {
        let id = self
            .start_cell(
                request,
                CellExecutionPolicy::PauseAtPendingFrontier,
                CellKind::Pausable,
            )
            .await?;
        Ok(PausableCell::new(id))
    }

    #[cfg(test)]
    pub(crate) async fn wait(
        &self,
        cell: &Cell,
        yield_after: Duration,
    ) -> Result<CellEvent, Error> {
        self.begin_wait(cell, yield_after).await?.event().await
    }

    pub(crate) async fn begin_wait(
        &self,
        cell: &Cell,
        yield_after: Duration,
    ) -> Result<PendingEvent, Error> {
        let handle = self.continuing_handle(cell.id()).await?;
        Ok(PendingEvent {
            event: map_cell_event(
                cell.id().clone(),
                handle.observe(ObserveMode::YieldAfter(yield_after)),
            ),
        })
    }

    pub(crate) async fn wait_to_pending(
        &self,
        cell: &PausableCell,
    ) -> Result<PausableCellEvent, Error> {
        let handle = self.pausable_handle(cell.id()).await?;
        map_pausable_event(
            cell.id().clone(),
            handle.observe(ObserveMode::PendingFrontier),
        )
        .await
    }

    pub(crate) async fn resume(
        &self,
        cell: &PausableCell,
        generation: PendingGeneration,
    ) -> Result<ResumeOutcome, Error> {
        let handle = self.pausable_handle(cell.id()).await?;
        handle
            .resume(generation)
            .await
            .map_err(|error| actor_error(cell.id(), error))
    }

    pub(crate) async fn cell(&self, cell_id: &CellId) -> Result<Cell, Error> {
        self.continuing_handle(cell_id).await?;
        Ok(Cell::new(cell_id.clone()))
    }

    pub(crate) async fn pausable_cell(&self, cell_id: &CellId) -> Result<PausableCell, Error> {
        self.pausable_handle(cell_id).await?;
        Ok(PausableCell::new(cell_id.clone()))
    }

    pub(crate) async fn terminate(&self, cell_id: &CellId) -> Result<CellEvent, Error> {
        let cell = self
            .inner
            .cells
            .lock()
            .await
            .get(cell_id)
            .cloned()
            .ok_or_else(|| Error::MissingCell(cell_id.clone()))?;
        cell.handle()
            .terminate()
            .await
            .map(map_terminal_event)
            .map_err(|error| actor_error(cell_id, error))
    }

    pub(crate) async fn shutdown(&self) -> Result<(), Error> {
        self.begin_shutdown();
        // Taking the registry lock ensures every cell that passed the shutdown
        // check has registered its actor with the tracker before we wait.
        let cells = self.inner.cells.lock().await;
        self.inner.cell_tasks.close();
        drop(cells);
        self.inner.cell_tasks.wait().await;
        Ok(())
    }

    fn allocate_cell_id(&self) -> CellId {
        CellId::new(
            self.inner
                .next_cell_id
                .fetch_add(1, Ordering::Relaxed)
                .to_string(),
        )
    }

    async fn start_cell(
        &self,
        request: CreateCellRequest,
        execution_policy: CellExecutionPolicy,
        kind: CellKind,
    ) -> Result<CellId, Error> {
        if self.inner.shutdown_token.is_cancelled() {
            return Err(Error::ShuttingDown);
        }
        let cell_id = self.allocate_cell_id();
        let stored_values = self.inner.stored_values.lock().await.clone();
        let host = Arc::new(RuntimeCellHost {
            cell_id: cell_id.clone(),
            inner: Arc::clone(&self.inner),
        });
        let mut cells = self.inner.cells.lock().await;
        if self.inner.shutdown_token.is_cancelled() {
            return Err(Error::ShuttingDown);
        }
        if cells.contains_key(&cell_id) {
            return Err(Error::DuplicateCell(cell_id));
        }
        let cell_state = Arc::new(CellState::new(self.inner.shutdown_token.child_token()));
        let (handle, task) =
            CellActor::prepare(request, stored_values, host, cell_state, execution_policy)
                .map_err(Error::Runtime)?;
        let registered = match kind {
            CellKind::Continuing => RegisteredCell::Continuing(handle),
            CellKind::Pausable => RegisteredCell::Pausable(handle),
        };
        cells.insert(cell_id.clone(), registered);
        self.inner.cell_tasks.spawn(task);
        drop(cells);
        Ok(cell_id)
    }

    async fn continuing_handle(&self, cell_id: &CellId) -> Result<CellHandle, Error> {
        self.handle_for_kind(cell_id, CellKind::Continuing).await
    }

    async fn pausable_handle(&self, cell_id: &CellId) -> Result<CellHandle, Error> {
        self.handle_for_kind(cell_id, CellKind::Pausable).await
    }

    async fn handle_for_kind(
        &self,
        cell_id: &CellId,
        expected: CellKind,
    ) -> Result<CellHandle, Error> {
        let cells = self.inner.cells.lock().await;
        let cell = cells
            .get(cell_id)
            .ok_or_else(|| Error::MissingCell(cell_id.clone()))?;
        let actual = cell.kind();
        if actual != expected {
            return Err(Error::WrongCellKind {
                cell_id: cell_id.clone(),
                expected,
                actual,
            });
        }
        Ok(cell.handle().clone())
    }

    fn begin_shutdown(&self) {
        self.inner.shutdown_token.cancel();
        self.inner.cell_tasks.close();
    }
}

impl<D: SessionRuntimeDelegate> Drop for SessionRuntime<D> {
    fn drop(&mut self) {
        self.begin_shutdown();
    }
}

/// An admitted cell event that has not reached its requested frontier yet.
pub(crate) struct PendingEvent {
    event: RuntimeEventFuture,
}

impl PendingEvent {
    pub(crate) async fn event(self) -> Result<CellEvent, Error> {
        self.event.await
    }
}

struct RuntimeCellHost<D: SessionRuntimeDelegate> {
    cell_id: CellId,
    inner: Arc<Inner<D>>,
}

impl<D: SessionRuntimeDelegate> CellHost for RuntimeCellHost<D> {
    async fn invoke_tool(
        &self,
        invocation: CellToolCall,
        cancellation_token: CancellationToken,
    ) -> Result<JsonValue, String> {
        self.inner
            .delegate
            .invoke_tool(
                NestedToolCall {
                    cell_id: self.cell_id.clone(),
                    runtime_tool_call_id: invocation.id,
                    tool_name: invocation.name,
                    tool_kind: invocation.kind,
                    input: invocation.input,
                },
                cancellation_token,
            )
            .await
    }

    async fn notify(
        &self,
        call_id: String,
        text: String,
        cancellation_token: CancellationToken,
    ) -> Result<(), String> {
        self.inner
            .delegate
            .notify(call_id, self.cell_id.clone(), text, cancellation_token)
            .await
    }

    async fn commit_completion(
        &self,
        stored_value_writes: HashMap<String, JsonValue>,
        event: ActorEvent,
        pending_initial_yield_items: Option<Vec<OutputItem>>,
        cell_state: Arc<CellState>,
    ) -> CompletionCommit {
        let cancellation_token = cell_state.cancellation_token();
        let mut stored_values = tokio::select! {
            biased;
            _ = cancellation_token.cancelled() => {
                return CompletionCommit::rejected(event, pending_initial_yield_items);
            }
            stored_values = self.inner.stored_values.lock() => stored_values,
        };
        cell_state.commit_completion(event, pending_initial_yield_items, || {
            stored_values.extend(stored_value_writes);
        })
    }

    async fn closed(&self) {
        self.inner.cells.lock().await.remove(&self.cell_id);
        self.inner.delegate.cell_closed(&self.cell_id);
    }
}

fn map_cell_event(cell_id: CellId, event: CellEventFuture) -> RuntimeEventFuture {
    Box::pin(async move {
        match event.await.map_err(|error| actor_error(&cell_id, error))? {
            ActorEvent::Yielded { content_items } => Ok(CellEvent::Yielded { content_items }),
            ActorEvent::Completed {
                content_items,
                error_text,
            } => Ok(CellEvent::Completed {
                content_items,
                error_text,
            }),
            ActorEvent::Terminated { content_items } => Ok(CellEvent::Terminated { content_items }),
            ActorEvent::Pending(_) => Err(Error::Runtime(format!(
                "continuing cell {cell_id} unexpectedly reached a visible pending frontier"
            ))),
        }
    })
}

fn map_pausable_event(cell_id: CellId, event: CellEventFuture) -> PausableRuntimeEventFuture {
    Box::pin(async move {
        match event.await.map_err(|error| actor_error(&cell_id, error))? {
            ActorEvent::Pending(frontier) => Ok(PausableCellEvent::Pending(frontier)),
            ActorEvent::Completed {
                content_items,
                error_text,
            } => Ok(PausableCellEvent::Completed {
                content_items,
                error_text,
            }),
            ActorEvent::Terminated { content_items } => {
                Ok(PausableCellEvent::Terminated { content_items })
            }
            ActorEvent::Yielded { .. } => Err(Error::Runtime(format!(
                "pausable cell {cell_id} unexpectedly yielded"
            ))),
        }
    })
}

fn map_terminal_event(event: ActorEvent) -> CellEvent {
    match event {
        ActorEvent::Completed {
            content_items,
            error_text,
        } => CellEvent::Completed {
            content_items,
            error_text,
        },
        ActorEvent::Terminated { content_items } => CellEvent::Terminated { content_items },
        ActorEvent::Yielded { .. } | ActorEvent::Pending(_) => {
            panic!("termination returned a non-terminal cell event")
        }
    }
}

fn actor_error(cell_id: &CellId, error: CellError) -> Error {
    match error {
        CellError::Busy => Error::BusyObserver(cell_id.clone()),
        CellError::AlreadyTerminating => Error::AlreadyTerminating(cell_id.clone()),
        CellError::Closed => Error::ClosedCell(cell_id.clone()),
        CellError::InvalidGeneration { requested, latest } => Error::InvalidGeneration {
            cell_id: cell_id.clone(),
            requested,
            latest,
        },
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
