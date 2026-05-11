use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_tools::ToolName;
use codex_tools::ToolSpec;

use crate::ToolCall;
use crate::ToolError;
use crate::ToolOutput;

/// Future returned by one executable-tool invocation.
pub type ToolFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Box<dyn ToolOutput>, ToolError>> + Send + 'a>>;

/// Future returned by one mutability probe.
pub type BoolFuture<'a> = Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

/// Model-visible definition plus executable implementation for one tool.
#[derive(Clone)]
pub struct ToolBundle<C> {
    definition: ToolDefinition,
    executor: Arc<dyn ToolExecutor<C>>,
}

impl<C> ToolBundle<C> {
    /// Creates one executable tool bundle.
    pub fn new(name: ToolName, spec: ToolSpec, executor: Arc<dyn ToolExecutor<C>>) -> Self {
        Self {
            definition: ToolDefinition {
                name,
                spec,
                supports_parallel_tool_calls: false,
            },
            executor,
        }
    }

    /// Marks this tool as safe for the host to run in parallel with peers.
    #[must_use]
    pub fn allow_parallel_calls(mut self) -> Self {
        self.definition.supports_parallel_tool_calls = true;
        self
    }

    /// Returns the model-visible tool definition.
    pub fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    /// Returns the executable implementation.
    pub fn executor(&self) -> Arc<dyn ToolExecutor<C>> {
        Arc::clone(&self.executor)
    }
}

/// Model-visible metadata owned by an executable tool bundle.
#[derive(Clone)]
pub struct ToolDefinition {
    pub name: ToolName,
    pub spec: ToolSpec,
    pub supports_parallel_tool_calls: bool,
}

/// Executable behavior for one contributed tool.
///
/// Implementations should keep host-specific needs inside `C`; tool owners that
/// do not require host state can implement the trait for any `C`.
pub trait ToolExecutor<C>: Send + Sync {
    fn execute<'a>(&'a self, call: ToolCall<C>) -> ToolFuture<'a>;

    /// Returns whether the call may mutate user state.
    ///
    /// Hosts can use this conservative signal for serialization or approval
    /// policy. Context-free read tools should keep the default.
    fn is_mutating<'a>(&'a self, _call: &'a ToolCall<C>) -> BoolFuture<'a> {
        Box::pin(async { false })
    }
}
