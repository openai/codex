use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use tokio_util::sync::CancellationToken;

use crate::CodeModeNestedToolCall;
use crate::CreateCellRequest;
use crate::ObserveOutcome;
use crate::ObserveRequest;
use crate::ReleaseObservationRequest;
use crate::TerminateOutcome;

pub type CodeModeSessionResultFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;
pub type CodeModeSessionProviderFuture<'a> =
    CodeModeSessionResultFuture<'a, Arc<dyn CodeModeSession>>;
pub type ToolInvocationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<JsonValue, String>> + Send + 'a>>;
pub type NotificationFuture<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CellId(String);

impl CellId {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CellId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Host callbacks used by a code-mode session while cells are executing.
pub trait CodeModeSessionDelegate: Send + Sync {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a>;

    fn notify<'a>(
        &'a self,
        call_id: String,
        cell_id: CellId,
        text: String,
        cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a>;

    /// Reports that a cell has reached its terminal state and will issue no more callbacks.
    ///
    /// Implementations must keep this non-blocking. The session does not wait for an
    /// acknowledgement or retry delivery after a transport failure.
    fn cell_closed(&self, cell_id: &CellId);
}

/// A stateful code-mode session owned by one Codex thread.
///
/// Cells executed in the same session share stored values. Separate sessions
/// must keep those values isolated. Implementations may execute cells
/// in-process or remotely. Implementations should surface lost connections or
/// protocol desynchronization, but do not need to preserve cells or stored
/// values across process failure or restart.
pub trait CodeModeSession: Send + Sync {
    /// Returns whether the session can still accept requests.
    ///
    /// Remote implementations should return `false` after their underlying
    /// connection fails so callers can create a fresh session for later work.
    fn is_alive(&self) -> bool;

    fn create_cell<'a>(
        &'a self,
        request: CreateCellRequest,
    ) -> CodeModeSessionResultFuture<'a, CellId>;

    fn observe<'a>(
        &'a self,
        request: ObserveRequest,
    ) -> CodeModeSessionResultFuture<'a, ObserveOutcome>;

    fn release_observation<'a>(
        &'a self,
        request: ReleaseObservationRequest,
    ) -> CodeModeSessionResultFuture<'a, ()>;

    fn terminate<'a>(
        &'a self,
        cell_id: CellId,
    ) -> CodeModeSessionResultFuture<'a, TerminateOutcome>;

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()>;
}

/// Creates code-mode sessions for Codex threads.
///
/// Implementations may share a remote host process across all sessions created
/// by one provider.
pub trait CodeModeSessionProvider: Send + Sync {
    fn create_session<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a>;
}
