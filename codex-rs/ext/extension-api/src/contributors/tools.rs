use codex_tools::JsonToolOutput;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;

/// Model-facing output returned by extension-owned tools.
pub type ExtensionToolOutput = JsonToolOutput;

/// Thin alias for extension-owned executable tools.
///
/// Extensions implement the shared `ToolExecutor<ToolCall>` contract directly;
/// the marker keeps contributor signatures readable while preserving one
/// executable-tool abstraction across host and extension tools.
pub trait ExtensionToolExecutor: ToolExecutor<ToolCall, Output = ExtensionToolOutput> {}

impl<T> ExtensionToolExecutor for T where T: ToolExecutor<ToolCall, Output = ExtensionToolOutput> {}
