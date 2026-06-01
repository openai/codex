use std::future::Future;
use std::pin::Pin;

use codex_tools::ToolName;

use crate::ExtensionData;

/// Future returned by one tool-lifecycle callback.
pub type ToolLifecycleFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Host-visible source for a model tool call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ToolCallSource {
    /// The model invoked the tool directly.
    Direct,
    /// Code mode invoked the tool while executing a runtime cell.
    CodeMode {
        /// Runtime cell that issued the nested tool request.
        cell_id: String,
        /// Code-mode's per-cell tool invocation id.
        runtime_tool_call_id: String,
    },
}

/// Extension-facing outcome for a finished tool call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCallOutcome {
    /// The tool returned a normal output.
    Completed {
        /// The tool output's own success marker for telemetry/logging.
        success: bool,
    },
    /// The tool was blocked by host policy before the handler ran.
    Blocked,
    /// The tool did not produce a normal output.
    Failed {
        /// Whether the host reached the tool handler before the failure.
        handler_executed: bool,
    },
    /// The host cancelled the tool before normal completion. Cancellation can
    /// win before the dispatch path accepts the call, so contributors should not
    /// assume a matching start callback exists.
    Aborted,
}

/// Input supplied when the host starts executing one tool call.
pub struct ToolStartInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
    /// Current turn submission id.
    pub turn_id: &'a str,
    /// Model-visible tool call id.
    pub call_id: &'a str,
    /// Tool name as routed by the host.
    pub tool_name: &'a ToolName,
    /// Source that issued the tool call.
    pub source: ToolCallSource,
}

/// Input supplied when the host finishes executing one tool call.
pub struct ToolFinishInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
    /// Store scoped to this turn runtime.
    pub turn_store: &'a ExtensionData,
    /// Current turn submission id.
    pub turn_id: &'a str,
    /// Model-visible tool call id.
    pub call_id: &'a str,
    /// Tool name as routed by the host.
    pub tool_name: &'a ToolName,
    /// Source that issued the tool call.
    pub source: ToolCallSource,
    /// Host-observed result of the tool call.
    pub outcome: ToolCallOutcome,
}
