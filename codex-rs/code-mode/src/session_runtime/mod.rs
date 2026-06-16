mod types;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use serde_json::Value as JsonValue;
use sha2::Digest;
use sha2::Sha256;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

pub use self::types::CellEvent;
pub use self::types::CellId;
pub use self::types::CreateCellRequest;
pub use self::types::Error;
pub use self::types::ImageDetail;
pub use self::types::NestedToolCall;
pub use self::types::ObserveMode;
pub use self::types::OutputItem;
pub use self::types::SessionRuntimeDelegate;
pub use self::types::ToolDefinition;
pub use self::types::ToolKind;
pub use self::types::ToolName;
use crate::cell_actor::CellActor;
use crate::cell_actor::CellError;
use crate::cell_actor::CellEventFuture;
use crate::cell_actor::CellHandle;
use crate::cell_actor::CellHost;
use crate::cell_actor::CellState;
use crate::cell_actor::CellToolCall;
use crate::cell_actor::CompletionCommit;

type RuntimeEventFuture = Pin<Box<dyn Future<Output = Result<CellEvent, Error>> + Send + 'static>>;

const CELL_ID_ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
const CELL_ID_LENGTH: usize = 16;

/// Owns all cells and shared state for one transport-neutral code-mode session.
pub struct SessionRuntime<D: SessionRuntimeDelegate> {
    inner: Arc<Inner<D>>,
}

struct Inner<D: SessionRuntimeDelegate> {
    stored_values: Mutex<HashMap<String, JsonValue>>,
    cells: Mutex<HashMap<CellId, CellHandle>>,
    cell_tasks: TaskTracker,
    shutdown_token: CancellationToken,
    delegate: Arc<D>,
    cell_id_namespace: CellIdNamespace,
    next_cell_id: AtomicU64,
}

enum CellIdNamespace {
    Runtime(uuid::Uuid),
}

impl<D: SessionRuntimeDelegate> SessionRuntime<D> {
    pub fn new(delegate: Arc<D>) -> Self {
        Self::with_cell_id_namespace(delegate, CellIdNamespace::Runtime(uuid::Uuid::new_v4()))
    }

    fn with_cell_id_namespace(delegate: Arc<D>, cell_id_namespace: CellIdNamespace) -> Self {
        Self {
            inner: Arc::new(Inner {
                stored_values: Mutex::new(HashMap::new()),
                cells: Mutex::new(HashMap::new()),
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

    pub async fn create_cell(&self, request: CreateCellRequest) -> Result<CellId, Error> {
        if self.inner.shutdown_token.is_cancelled() {
            return Err(Error::ShuttingDown);
        }
        let cell_id = self.allocate_cell_id();
        self.start_cell(cell_id.clone(), request).await?;
        Ok(cell_id)
    }

    pub async fn observe(&self, cell_id: &CellId, mode: ObserveMode) -> Result<CellEvent, Error> {
        self.begin_observe(cell_id, mode).await?.event().await
    }

    pub async fn begin_observe(
        &self,
        cell_id: &CellId,
        mode: ObserveMode,
    ) -> Result<PendingEvent, Error> {
        let handle = self
            .inner
            .cells
            .lock()
            .await
            .get(cell_id)
            .cloned()
            .ok_or_else(|| Error::MissingCell(cell_id.clone()))?;
        Ok(PendingEvent {
            event: map_actor_event(cell_id.clone(), handle.observe(mode)),
        })
    }

    pub async fn terminate(&self, cell_id: &CellId) -> Result<CellEvent, Error> {
        let handle = self
            .inner
            .cells
            .lock()
            .await
            .get(cell_id)
            .cloned()
            .ok_or_else(|| Error::MissingCell(cell_id.clone()))?;
        handle
            .terminate()
            .await
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
        };
        CellId::new(value)
    }

    async fn start_cell(&self, cell_id: CellId, request: CreateCellRequest) -> Result<(), Error> {
        let stored_values = self.inner.stored_values.lock().await.clone();
        let host = Arc::new(RuntimeCellHost {
            cell_id: cell_id.clone(),
            inner: Arc::clone(&self.inner),
            terminal_notified: AtomicBool::new(false),
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
            CellActor::prepare(request, stored_values, host, cell_state).map_err(Error::Runtime)?;
        cells.insert(cell_id.clone(), handle);
        self.inner.cell_tasks.spawn(task);
        drop(cells);
        Ok(())
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
    terminal_notified: AtomicBool,
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
        event: CellEvent,
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

    fn terminal(&self) {
        if !self.terminal_notified.swap(true, Ordering::AcqRel) {
            self.inner.delegate.cell_closed(&self.cell_id);
        }
    }

    async fn closed(&self) {
        self.inner.cells.lock().await.remove(&self.cell_id);
        self.terminal();
    }
}

fn map_actor_event(cell_id: CellId, event: CellEventFuture) -> RuntimeEventFuture {
    Box::pin(async move { event.await.map_err(|error| actor_error(&cell_id, error)) })
}

fn actor_error(cell_id: &CellId, error: CellError) -> Error {
    match error {
        CellError::Busy => Error::BusyObserver(cell_id.clone()),
        CellError::AlreadyTerminating => Error::AlreadyTerminating(cell_id.clone()),
        CellError::Closed => Error::ClosedCell(cell_id.clone()),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
