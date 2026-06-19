mod types;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

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
use crate::cell_actor::CellState;
use crate::cell_actor::CellToolCall;

type RuntimeEventFuture = Pin<Box<dyn Future<Output = Result<CellEvent, Error>> + Send + 'static>>;

/// Owns all cells and shared state for one transport-neutral code-mode session.
pub struct SessionRuntime<D: SessionRuntimeDelegate> {
    inner: Arc<Inner<D>>,
}

struct Inner<D: SessionRuntimeDelegate> {
    stored_values: Mutex<HashMap<String, JsonValue>>,
    cells: Mutex<HashMap<CellId, CellEntry>>,
    // Tracks actor cleanup only. CellPhase remains the semantic lifecycle.
    cell_tasks: TaskTracker,
    shutdown_token: CancellationToken,
    delegate: Arc<D>,
}

enum CellEntry {
    Active(CellHandle),
    Tombstone,
}

impl<D: SessionRuntimeDelegate> SessionRuntime<D> {
    pub fn new(delegate: Arc<D>) -> Self {
        Self {
            inner: Arc::new(Inner {
                stored_values: Mutex::new(HashMap::new()),
                cells: Mutex::new(HashMap::new()),
                cell_tasks: TaskTracker::new(),
                shutdown_token: CancellationToken::new(),
                delegate,
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
    ) -> Result<CellEvent, Error> {
        if self.inner.shutdown_token.is_cancelled() {
            return Err(Error::ShuttingDown);
        }
        self.start_cell(request, initial_observe_mode).await?.await
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
            let Some(CellEntry::Active(handle)) = cells.get(cell_id) else {
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
            let Some(CellEntry::Active(handle)) = cells.get(cell_id) else {
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
        // Taking the registry lock ensures every cell that passed the shutdown
        // check has registered its actor with the tracker before we wait.
        let cells = self.inner.cells.lock().await;
        self.inner.cell_tasks.close();
        drop(cells);
        self.inner.cell_tasks.wait().await;
        Ok(())
    }

    async fn start_cell(
        &self,
        request: ExecuteRequest,
        initial_observe_mode: ObserveMode,
    ) -> Result<RuntimeEventFuture, Error> {
        let cell_id = request.cell_id.clone();
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
        let (handle, initial_event, task) = CellActor::prepare(
            request,
            stored_values,
            host,
            initial_observe_mode,
            cell_state,
        )
        .map_err(Error::Runtime)?;
        cells.insert(cell_id.clone(), CellEntry::Active(handle));
        self.inner.cell_tasks.spawn(task);
        drop(cells);
        Ok(map_actor_event(cell_id, initial_event))
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

/// An admitted observation that has not reached its requested frontier yet.
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
        event: CellEvent,
        cell_state: Arc<CellState>,
    ) -> bool {
        let mut stored_values = self.inner.stored_values.lock().await;
        cell_state.commit_completion(event, || {
            stored_values.extend(stored_value_writes);
        })
    }

    async fn closed(&self) {
        let mut cells = self.inner.cells.lock().await;
        if let Some(entry) = cells.get_mut(&self.cell_id) {
            *entry = CellEntry::Tombstone;
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
