use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::CodeModeNestedToolCall;
use crate::ExecuteRequest;
use crate::RuntimeResponse;
use crate::WaitOutcome;
use crate::WaitRequest;

pub type CodeModeSessionResultFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;
pub type CodeModeSessionProviderFuture<'a> =
    CodeModeSessionResultFuture<'a, Arc<dyn CodeModeSession>>;
pub type ToolInvocationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<JsonValue, String>> + Send + 'a>>;
pub type NotificationFuture<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

/// Optional resource limits shared by every cell in one code-mode session.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodeModeSessionCellExecutionLimits {
    pub max_yield_time_ms: Option<u64>,
    pub max_heap_size_bytes: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
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

pub struct StartedCell {
    pub cell_id: CellId,
    initial_response: CodeModeSessionResultFuture<'static, RuntimeResponse>,
}

impl StartedCell {
    pub fn new(cell_id: CellId, initial_response_rx: oneshot::Receiver<RuntimeResponse>) -> Self {
        Self::from_future(cell_id, async move {
            initial_response_rx
                .await
                .map_err(|_| "exec runtime ended unexpectedly".to_string())
        })
    }

    pub fn from_result_receiver(
        cell_id: CellId,
        initial_response_rx: oneshot::Receiver<Result<RuntimeResponse, String>>,
    ) -> Self {
        Self::from_future(cell_id, async move {
            initial_response_rx
                .await
                .map_err(|_| "exec runtime ended unexpectedly".to_string())?
        })
    }

    pub fn from_future(
        cell_id: CellId,
        initial_response: impl Future<Output = Result<RuntimeResponse, String>> + Send + 'static,
    ) -> Self {
        Self {
            cell_id,
            initial_response: Box::pin(initial_response),
        }
    }

    pub async fn initial_response(self) -> Result<RuntimeResponse, String> {
        self.initial_response.await
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

    /// Releases delegate state associated with a cell after it reaches a terminal state.
    fn cell_closed(&self, cell_id: &CellId);
}

/// A session delegate for clients that do not expose nested tools or notifications.
pub struct NoopCodeModeSessionDelegate;

impl CodeModeSessionDelegate for NoopCodeModeSessionDelegate {
    fn invoke_tool<'a>(
        &'a self,
        _invocation: CodeModeNestedToolCall,
        cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        Box::pin(async move {
            cancellation_token.cancelled().await;
            Err("code mode nested tools are unavailable".to_string())
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

    fn cell_closed(&self, _cell_id: &CellId) {}
}

/// A durable code-mode session owned by one Codex thread.
///
/// Cells executed in the same session share stored values. Separate sessions
/// must keep those values isolated. Implementations may execute cells
/// in-process or remotely.
pub trait CodeModeSession: Send + Sync {
    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
    ) -> CodeModeSessionResultFuture<'a, StartedCell>;

    fn wait<'a>(&'a self, request: WaitRequest) -> CodeModeSessionResultFuture<'a, WaitOutcome>;

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, WaitOutcome>;

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()>;
}

/// Creates code-mode sessions for Codex threads.
///
/// Implementations may share a remote host process across all sessions created
/// by one provider.
pub trait CodeModeSessionProvider: Send + Sync {
    /// Reports whether this provider can execute code without starting its host.
    fn availability(&self) -> Result<(), String> {
        Ok(())
    }

    fn create_session<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
    ) -> CodeModeSessionProviderFuture<'a>;

    /// Creates a session whose cells share the supplied execution limits.
    ///
    /// Existing providers remain compatible with unlimited sessions, but must
    /// explicitly implement this method before accepting non-default limits.
    fn create_session_with_limits<'a>(
        &'a self,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> CodeModeSessionProviderFuture<'a> {
        if limits == CodeModeSessionCellExecutionLimits::default() {
            self.create_session(delegate)
        } else {
            Box::pin(async {
                Err("code-mode session provider does not support resource limits".to_string())
            })
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
