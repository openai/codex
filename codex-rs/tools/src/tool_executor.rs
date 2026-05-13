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

    fn handle(
        &self,
        invocation: Invocation,
    ) -> impl Future<Output = Result<Self::Output, FunctionCallError>> + Send;
}
