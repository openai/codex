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
use crate::TerminateRequest;
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

/// The owner of the output budget for each exec or wait observation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CodeModeOutputTruncation {
    /// Return raw output; the caller applies its own budget and formatting.
    #[default]
    Client,
    /// The host applies the exec, wait, or terminate observation's budget before
    /// transmitting output (10,000 tokens by default). Clients must not truncate
    /// it again or append the script error: status and error metadata remain separate.
    ///
    /// Between observations, the host retains at most
    /// [`crate::host::BOUNDED_OUTPUT_MAX_BYTES`] of output payload. It also
    /// bounds the storage for retained items separately, without charging it
    /// against the payload or observation token budgets.
    /// Ordinary text notices report overflow, even for a zero-token budget.
    /// Media are indivisible and can be omitted if they exceed the collector.
    /// Without overflow, formatting matches Core's client policy.
    Server,
}

/// Settings shared by every cell in one code-mode session.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodeModeSessionOptions {
    pub cell_execution_limits: CodeModeSessionCellExecutionLimits,
    pub output_truncation: CodeModeOutputTruncation,
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

/// Host callbacks owned by one code-mode execution.
///
/// The session retains the supplied delegate while starting and running the cell,
/// including across yields, and releases it through its existing close/cancel paths.
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
    /// Identifies who applies output truncation for this session's responses.
    /// Providers must not change modes when reconnecting to a new host.
    fn output_truncation(&self) -> CodeModeOutputTruncation {
        CodeModeOutputTruncation::Client
    }

    /// Executes a cell, yielding its foreground observation if `preempt` is signaled.
    /// The cell continues running after its observation yields.
    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        preempt: Option<CancellationToken>,
    ) -> CodeModeSessionResultFuture<'a, StartedCell>;

    /// Waits for a cell, yielding the observation if `preempt` is signaled while the cell continues.
    /// Server mode applies the request's output limit; Client mode ignores it.
    fn wait<'a>(
        &'a self,
        request: WaitRequest,
        preempt: Option<CancellationToken>,
    ) -> CodeModeSessionResultFuture<'a, WaitOutcome>;

    /// Terminates a cell, applying the request's limit to its final observation
    /// in Server mode; Client mode ignores the limit.
    fn terminate<'a>(
        &'a self,
        request: TerminateRequest,
    ) -> CodeModeSessionResultFuture<'a, WaitOutcome>;

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

    /// Observes a host without starting a session. Process-owned providers must
    /// not start or replace a host. Network providers may connect or reconnect
    /// according to their transport policy. Sampling failures and cancellation
    /// must not explicitly reset the shared execution connection.
    fn sample_process_metrics(
        &self,
    ) -> CodeModeSessionResultFuture<'_, crate::CodeModeProcessMetricsSample> {
        Box::pin(async { Ok(crate::CodeModeProcessMetricsSample::Unsupported) })
    }

    fn create_session(&self) -> CodeModeSessionProviderFuture<'_>;

    /// Creates a session whose cells share the supplied execution limits.
    ///
    /// Existing providers remain compatible with unlimited sessions, but must
    /// explicitly implement this method before accepting non-default limits.
    fn create_session_with_limits<'a>(
        &'a self,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> CodeModeSessionProviderFuture<'a> {
        if limits == CodeModeSessionCellExecutionLimits::default() {
            self.create_session()
        } else {
            Box::pin(async {
                Err("code-mode session provider does not support resource limits".to_string())
            })
        }
    }

    /// Requests output handling for one session, without changing other sessions
    /// created by this provider. Server mode requires host support.
    fn create_session_with_options<'a>(
        &'a self,
        options: CodeModeSessionOptions,
    ) -> CodeModeSessionProviderFuture<'a> {
        match options.output_truncation {
            CodeModeOutputTruncation::Client => {
                self.create_session_with_limits(options.cell_execution_limits)
            }
            CodeModeOutputTruncation::Server => Box::pin(async {
                Err(
                    "code-mode session provider does not support server output truncation"
                        .to_string(),
                )
            }),
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
