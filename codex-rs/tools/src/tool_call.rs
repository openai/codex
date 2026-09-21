use crate::FunctionCallError;
use crate::ToolName;
use crate::ToolPayload;
use codex_extension_items::ExtensionItem;
use codex_file_system::ExecutorFileSystem;
use codex_file_system::FileSystemSandboxContext;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_output_truncation::TruncationPolicy;
use codex_utils_output_truncation::with_serialization_allowance;
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::LazyLock;

/// Raw response history snapshot available when an extension tool is invoked.
#[derive(Clone)]
pub struct ConversationHistory {
    items: Arc<LazyLock<Box<[ResponseItem]>, HistoryLoader>>,
}

type HistoryLoader = Box<dyn FnOnce() -> Box<[ResponseItem]> + Send + Sync>;

impl ConversationHistory {
    pub fn new(items: Vec<ResponseItem>) -> Self {
        Self::new_deferred(move || items)
    }

    /// Materializes an invocation-time snapshot only when a tool reads its history.
    /// The loader must capture that snapshot rather than querying mutable session state.
    pub fn new_deferred(load: impl FnOnce() -> Vec<ResponseItem> + Send + Sync + 'static) -> Self {
        Self {
            items: Arc::new(LazyLock::new(Box::new(move || load().into_boxed_slice()))),
        }
    }

    pub fn items(&self) -> &[ResponseItem] {
        &self.items
    }
}

impl Default for ConversationHistory {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl fmt::Debug for ConversationHistory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConversationHistory")
            .finish_non_exhaustive()
    }
}

/// Future returned when an extension tool emits a visible turn-item lifecycle event.
pub type TurnItemEmissionFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// Visible turn items that an extension may publish into the host lifecycle.
#[derive(Clone, Debug)]
pub struct ExtensionTurnItem {
    /// Canonical extension item plus compatibility events derived by its owner.
    ///
    /// Core intentionally does not inspect extension-owned payloads, so it
    /// cannot derive their legacy fanout. It emits the canonical lifecycle
    /// event first, then these extension-provided events. Core also skips
    /// global turn-item contributors here so extensions cannot mutate items
    /// owned by other extensions.
    pub item: ExtensionItem,
    pub legacy_events: Vec<EventMsg>,
}

/// Host-provided capability for extension tools to emit visible turn items.
///
/// Implementations route lifecycle events through the host's normal item event
/// pipeline and client delivery.
pub trait TurnItemEmitter: Send + Sync {
    /// Emits the beginning of one visible turn item.
    fn emit_started<'a>(&'a self, item: ExtensionTurnItem) -> TurnItemEmissionFuture<'a>;

    /// Emits one completed visible turn item.
    fn emit_completed<'a>(&'a self, item: ExtensionTurnItem) -> TurnItemEmissionFuture<'a>;
}

/// Host-owned turn environment summary visible to extension tools.
#[derive(Clone)]
pub struct ToolEnvironment<'call> {
    /// Stable host environment id used to route executor-scoped capabilities.
    pub environment_id: String,
    /// Effective working directory for this turn in the environment.
    pub cwd: AbsolutePathBuf,
    /// Filesystem implementation for this environment.
    pub file_system: Arc<dyn ExecutorFileSystem>,
    /// Sandbox context to use for filesystem operations.
    pub file_system_sandbox_context: FileSystemSandboxContext,
    // TODO(anp): Replace the marker with callback-scoped environment access.
    pub _lifetime: PhantomData<&'call ()>,
}

/// Turn-item emitter used when a caller does not expose visible item emission.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTurnItemEmitter;

impl TurnItemEmitter for NoopTurnItemEmitter {
    fn emit_started<'a>(&'a self, _item: ExtensionTurnItem) -> TurnItemEmissionFuture<'a> {
        Box::pin(std::future::ready(()))
    }

    fn emit_completed<'a>(&'a self, _item: ExtensionTurnItem) -> TurnItemEmissionFuture<'a> {
        Box::pin(std::future::ready(()))
    }
}

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

#[derive(Clone)]
pub struct ToolCall<'call> {
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: ToolName,
    pub model: String,
    pub codex_turn_metadata: Option<String>,
    pub truncation_policy: TruncationPolicy,
    pub source: ToolCallSource,
    pub conversation_history: ConversationHistory,
    pub turn_item_emitter: Arc<dyn TurnItemEmitter>,
    pub environments: Vec<ToolEnvironment<'call>>,
    pub payload: ToolPayload,
}

impl std::fmt::Debug for ToolCall<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolCall")
            .field("turn_id", &self.turn_id)
            .field("call_id", &self.call_id)
            .field("tool_name", &self.tool_name)
            .field("model", &self.model)
            .field(
                "has_codex_turn_metadata",
                &self.codex_turn_metadata.is_some(),
            )
            .field("truncation_policy", &self.truncation_policy)
            .field("source", &self.source)
            .field("conversation_history", &self.conversation_history)
            .field("turn_item_emitter", &"<host turn item emitter>")
            .field("environment_count", &self.environments.len())
            .field("payload", &self.payload)
            .finish()
    }
}

impl ToolCall<'_> {
    /// Returns the response-content budget, bounded by the tool's own size limit.
    ///
    /// Direct calls use the host's effective text-output allowance. Code Mode receives
    /// typed results without that truncation, so only the tool's limit applies.
    /// Callers must include serialization overhead when fitting a response to this budget.
    pub fn response_byte_budget(&self, max_response_bytes: usize) -> usize {
        match &self.source {
            ToolCallSource::Direct => max_response_bytes
                .min(with_serialization_allowance(self.truncation_policy).byte_budget()),
            ToolCallSource::CodeMode {
                cell_id: _,
                runtime_tool_call_id: _,
            } => max_response_bytes,
        }
    }

    pub fn function_arguments(&self) -> Result<&str, FunctionCallError> {
        match &self.payload {
            ToolPayload::Function { arguments } => Ok(arguments),
            _ => Err(FunctionCallError::Fatal(format!(
                "tool {} invoked with incompatible payload",
                self.tool_name
            ))),
        }
    }
}
