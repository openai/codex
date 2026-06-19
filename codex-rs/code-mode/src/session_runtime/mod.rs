mod types;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::warn;

pub use self::types::CellEvent;
pub use self::types::CellId;
pub use self::types::Error;
pub use self::types::ExecuteRequest;
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
use crate::cell_actor::CellToolCall;

type RuntimeEventFuture = Pin<Box<dyn Future<Output = Result<CellEvent, Error>> + Send + 'static>>;

/// Owns all cells and shared state for one transport-neutral code-mode session.
pub struct SessionRuntime<D: SessionRuntimeDelegate> {
    inner: Arc<Inner<D>>,
}

struct Inner<D: SessionRuntimeDelegate> {
    stored_values: Mutex<HashMap<String, JsonValue>>,
    cells: Mutex<HashMap<CellId, CellState>>,
    cell_count_tx: watch::Sender<usize>,
    shutdown_token: CancellationToken,
    delegate: Arc<D>,
    next_cell_id: AtomicU64,
}

enum CellState {
    Live(CellHandle),
    Closing,
}

impl<D: SessionRuntimeDelegate> SessionRuntime<D> {
    pub fn new(delegate: Arc<D>) -> Self {
        let (cell_count_tx, _) = watch::channel(/*init*/ 0);
        Self {
            inner: Arc::new(Inner {
                stored_values: Mutex::new(HashMap::new()),
                cells: Mutex::new(HashMap::new()),
                cell_count_tx,
                shutdown_token: CancellationToken::new(),
                delegate,
                next_cell_id: AtomicU64::new(1),
            }),
        }
    }

    pub fn is_alive(&self) -> bool {
        !self.inner.shutdown_token.is_cancelled()
    }

    pub async fn execute(
        &self,
        request: ExecuteRequest,
        initial_observe_mode: ObserveMode,
    ) -> Result<StartedCell, Error> {
        if self.inner.shutdown_token.is_cancelled() {
            return Err(Error::ShuttingDown);
        }
        let cell_id = self.allocate_cell_id();
        let initial_event = self
            .start_cell(cell_id.clone(), request, initial_observe_mode)
            .await?;
        Ok(StartedCell {
            cell_id,
            initial_event,
        })
    }

    pub async fn observe(&self, cell_id: &CellId, mode: ObserveMode) -> Result<CellEvent, Error> {
        self.begin_observe(cell_id, mode).await?.event().await
    }

    pub async fn begin_observe(
        &self,
        cell_id: &CellId,
        mode: ObserveMode,
    ) -> Result<PendingEvent, Error> {
        let handle = {
            let cells = self.inner.cells.lock().await;
            let Some(CellState::Live(handle)) = cells.get(cell_id) else {
                return Err(Error::MissingCell(cell_id.clone()));
            };
            handle.clone()
        };
        Ok(PendingEvent {
            event: map_actor_event(cell_id.clone(), handle.observe(mode)),
        })
    }

    pub async fn terminate(&self, cell_id: &CellId) -> Result<CellEvent, Error> {
        let handle = {
            let cells = self.inner.cells.lock().await;
            let Some(CellState::Live(handle)) = cells.get(cell_id) else {
                return Err(Error::MissingCell(cell_id.clone()));
            };
            handle.clone()
        };
        handle
            .terminate()
            .await
            .map_err(|error| actor_error(cell_id, error))
    }

    pub async fn shutdown(&self) -> Result<(), Error> {
        self.begin_shutdown();
        // Serialize with cell admission before taking the count snapshot.
        let cells = self.inner.cells.lock().await;
        let mut cell_count = self.inner.cell_count_tx.subscribe();
        drop(cells);
        while *cell_count.borrow_and_update() != 0 {
            if cell_count.changed().await.is_err() {
                break;
            }
        }
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
        cell_id: CellId,
        request: ExecuteRequest,
        initial_observe_mode: ObserveMode,
    ) -> Result<RuntimeEventFuture, Error> {
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
        let (handle, initial_event, task) = CellActor::prepare(
            request,
            stored_values,
            host,
            initial_observe_mode,
            self.inner.shutdown_token.clone(),
        )
        .map_err(Error::Runtime)?;
        cells.insert(cell_id.clone(), CellState::Live(handle));
        self.inner.cell_count_tx.send_replace(cells.len());
        drop(cells);
        tokio::spawn(task);
        Ok(map_actor_event(cell_id, initial_event))
    }

    fn begin_shutdown(&self) {
        self.inner.shutdown_token.cancel();
    }
}

impl<D: SessionRuntimeDelegate> Drop for SessionRuntime<D> {
    fn drop(&mut self) {
        self.begin_shutdown();
    }
}

/// A cell admitted by [`SessionRuntime::execute`].
pub struct StartedCell {
    pub cell_id: CellId,
    initial_event: RuntimeEventFuture,
}

/// An admitted observation that has not reached its requested frontier yet.
pub struct PendingEvent {
    event: RuntimeEventFuture,
}

impl PendingEvent {
    pub async fn event(self) -> Result<CellEvent, Error> {
        self.event.await
    }
}

impl StartedCell {
    pub async fn initial_event(self) -> Result<CellEvent, Error> {
        self.initial_event.await
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

    async fn commit_stored_values(&self, stored_value_writes: HashMap<String, JsonValue>) {
        self.inner
            .stored_values
            .lock()
            .await
            .extend(stored_value_writes);
    }

    async fn closed(&self) {
        self.inner
            .cells
            .lock()
            .await
            .insert(self.cell_id.clone(), CellState::Closing);
        if let Err(err) = self.inner.delegate.cell_closed(&self.cell_id).await {
            warn!(
                "failed to close code mode delegate state for cell {}: {err}",
                self.cell_id
            );
        }
        let mut cells = self.inner.cells.lock().await;
        if cells.remove(&self.cell_id).is_some() {
            self.inner.cell_count_tx.send_replace(cells.len());
        }
    }
}

fn map_actor_event(cell_id: CellId, event: CellEventFuture) -> RuntimeEventFuture {
    Box::pin(async move { event.await.map_err(|error| actor_error(&cell_id, error)) })
}

fn actor_error(cell_id: &CellId, error: CellError) -> Error {
    match error {
        CellError::Busy => Error::BusyObserver(cell_id.clone()),
        CellError::AlreadyTerminating => Error::AlreadyTerminating(cell_id.clone()),
        CellError::Closed => Error::MissingCell(cell_id.clone()),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
