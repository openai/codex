mod types;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::Value as JsonValue;
use sha2::Digest;
use sha2::Sha256;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

pub use self::types::Cell;
pub use self::types::CellEvent;
pub(crate) use self::types::CellExecutionPolicy;
pub use self::types::CellId;
pub use self::types::CellKind;
pub use self::types::CreateCellRequest;
pub use self::types::Error;
pub use self::types::ImageDetail;
pub use self::types::NestedToolCall;
pub use self::types::OutputItem;
pub use self::types::PausableCell;
pub use self::types::PausableCellEvent;
pub use self::types::PendingFrontier;
pub use self::types::PendingGeneration;
pub use self::types::ResumeOutcome;
pub use self::types::SessionRuntimeDelegate;
pub use self::types::ToolDefinition;
pub use self::types::ToolKind;
pub use self::types::ToolName;
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

const CELL_ID_ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
const CELL_ID_LENGTH: usize = 16;

/// Owns all cells and shared state for one transport-neutral code-mode session.
pub struct SessionRuntime<D: SessionRuntimeDelegate> {
    inner: Arc<Inner<D>>,
}

struct Inner<D: SessionRuntimeDelegate> {
    stored_values: Mutex<HashMap<String, JsonValue>>,
    cells: Mutex<HashMap<CellId, RegisteredCell>>,
    created_cells: Mutex<HashMap<String, Arc<OnceCell<IdempotentCell>>>>,
    cell_tasks: TaskTracker,
    shutdown_token: CancellationToken,
    delegate: Arc<D>,
    cell_id_namespace: CellIdNamespace,
    next_cell_id: AtomicU64,
}

#[derive(Clone)]
struct IdempotentCell {
    id: CellId,
    kind: CellKind,
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

enum CellIdNamespace {
    Runtime(uuid::Uuid),
    #[cfg(test)]
    Unscoped,
}

impl<D: SessionRuntimeDelegate> SessionRuntime<D> {
    pub fn new(delegate: Arc<D>) -> Self {
        Self::with_cell_id_namespace(delegate, CellIdNamespace::Runtime(uuid::Uuid::new_v4()))
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(delegate: Arc<D>) -> Self {
        Self::with_cell_id_namespace(delegate, CellIdNamespace::Unscoped)
    }

    fn with_cell_id_namespace(delegate: Arc<D>, cell_id_namespace: CellIdNamespace) -> Self {
        Self {
            inner: Arc::new(Inner {
                stored_values: Mutex::new(HashMap::new()),
                cells: Mutex::new(HashMap::new()),
                created_cells: Mutex::new(HashMap::new()),
                cell_tasks: TaskTracker::new(),
                shutdown_token: CancellationToken::new(),
                delegate,
                cell_id_namespace,
                next_cell_id: AtomicU64::new(1),
            }),
        }
    }

    pub fn is_alive(&self) -> bool {
        !self.inner.shutdown_token.is_cancelled()
    }

    pub async fn create_cell(&self, request: CreateCellRequest) -> Result<Cell, Error> {
        let id = self
            .create_idempotent_cell(
                request,
                CellExecutionPolicy::ContinueWhenUnblocked,
                CellKind::Continuing,
            )
            .await?;
        Ok(Cell::new(id))
    }

    pub async fn create_pausable_cell(
        &self,
        request: CreateCellRequest,
    ) -> Result<PausableCell, Error> {
        let id = self
            .create_idempotent_cell(
                request,
                CellExecutionPolicy::PauseAtPendingFrontier,
                CellKind::Pausable,
            )
            .await?;
        Ok(PausableCell::new(id))
    }

    pub async fn wait(&self, cell: &Cell, yield_after: Duration) -> Result<CellEvent, Error> {
        self.begin_wait(cell, yield_after).await?.event().await
    }

    pub async fn begin_wait(
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

    pub async fn wait_to_pending(&self, cell: &PausableCell) -> Result<PausableCellEvent, Error> {
        let handle = self.pausable_handle(cell.id()).await?;
        map_pausable_event(
            cell.id().clone(),
            handle.observe(ObserveMode::PendingFrontier),
        )
        .await
    }

    pub async fn resume(
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

    #[cfg(test)]
    pub(crate) async fn pausable_cell(&self, cell_id: &CellId) -> Result<PausableCell, Error> {
        self.pausable_handle(cell_id).await?;
        Ok(PausableCell::new(cell_id.clone()))
    }

    pub async fn terminate(&self, cell_id: &CellId) -> Result<CellEvent, Error> {
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

    pub async fn shutdown(&self) -> Result<(), Error> {
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
        let sequence = self.inner.next_cell_id.fetch_add(1, Ordering::Relaxed);
        let value = match &self.inner.cell_id_namespace {
            CellIdNamespace::Runtime(runtime_id) => {
                let digest = Sha256::new()
                    .chain_update(runtime_id.as_bytes())
                    .chain_update(sequence.to_be_bytes())
                    .finalize();
                let mut encoded = String::with_capacity(CELL_ID_LENGTH);
                for chunk in digest[..10].chunks_exact(5) {
                    let chunk_bits = chunk
                        .iter()
                        .fold(0_u64, |value, byte| (value << 8) | u64::from(*byte));
                    for shift in (0..40).step_by(5).rev() {
                        let index = ((chunk_bits >> shift) & 0x1f) as usize;
                        encoded.push(char::from(CELL_ID_ALPHABET[index]));
                    }
                }
                encoded
            }
            #[cfg(test)]
            CellIdNamespace::Unscoped => sequence.to_string(),
        };
        CellId::new(value)
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

    async fn create_idempotent_cell(
        &self,
        request: CreateCellRequest,
        execution_policy: CellExecutionPolicy,
        kind: CellKind,
    ) -> Result<CellId, Error> {
        let key = request.idempotency_key.clone();
        let created_cell = {
            let mut created_cells = self.inner.created_cells.lock().await;
            Arc::clone(
                created_cells
                    .entry(key)
                    .or_insert_with(|| Arc::new(OnceCell::new())),
            )
        };
        let existing = created_cell
            .get_or_try_init(|| async move {
                let id = self.start_cell(request, execution_policy, kind).await?;
                Ok::<IdempotentCell, Error>(IdempotentCell { id, kind })
            })
            .await?;
        if existing.kind != kind {
            return Err(Error::WrongCellKind {
                cell_id: existing.id.clone(),
                expected: kind,
                actual: existing.kind,
            });
        }
        Ok(existing.id.clone())
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
pub struct PendingEvent {
    event: RuntimeEventFuture,
}

impl PendingEvent {
    pub async fn event(self) -> Result<CellEvent, Error> {
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
