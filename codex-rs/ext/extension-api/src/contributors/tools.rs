use std::future::Future;
use std::pin::Pin;

use codex_tools::FunctionCallError;
use codex_tools::JsonToolOutput;
use codex_tools::ToolCall;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

/// Model-facing output returned by extension-owned tools.
pub type ExtensionToolOutput = JsonToolOutput;

/// Future returned by extension-owned tool execution.
pub type ExtensionToolFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ExtensionToolOutput, FunctionCallError>> + Send + 'a>>;

/// Object-safe runtime contract for extension-owned model-visible tools.
///
/// Implementations keep an extension tool's model-visible spec attached to the
/// executable runtime that handles calls for that tool.
pub trait ExtensionToolExecutor: Send + Sync {
    /// The concrete tool name handled by this extension runtime.
    fn tool_name(&self) -> ToolName;

    /// The model-visible spec for this extension tool.
    fn spec(&self) -> Option<ToolSpec> {
        None
    }

    /// Execute one extension tool invocation.
    fn handle(&self, call: ToolCall) -> ExtensionToolFuture<'_>;
}
