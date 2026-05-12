use std::future::Future;

use crate::FunctionCallError;
use crate::ToolName;
use crate::ToolOutput;
use crate::ToolSpec;

/// Shared runtime contract for model-visible tools.
///
/// Implementations keep the model-visible spec tied to the executable runtime.
/// Host crates can layer routing, hooks, telemetry, or other orchestration on
/// top without reopening the spec/runtime split.
pub trait ToolExecutor<Invocation>: Send + Sync {
    type Output: ToolOutput + 'static;

    /// The concrete tool name handled by this runtime instance.
    fn tool_name(&self) -> ToolName;

    fn spec(&self) -> Option<ToolSpec> {
        None
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        false
    }

    /// Returns `true` if the invocation might mutate the user's environment.
    ///
    /// Implementations should remain defensive and return `true` whenever the
    /// exact effect of an invocation is uncertain.
    fn is_mutating(&self, _invocation: &Invocation) -> impl Future<Output = bool> + Send {
        async { false }
    }

    fn handle(
        &self,
        invocation: Invocation,
    ) -> impl Future<Output = Result<Self::Output, FunctionCallError>> + Send;
}
