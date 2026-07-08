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
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Raw response history snapshot available when an extension tool is invoked.
#[derive(Clone, Debug, Default)]
pub struct ConversationHistory {
    items: Arc<[ResponseItem]>,
}

impl ConversationHistory {
    pub fn new(items: Vec<ResponseItem>) -> Self {
        Self {
            items: items.into(),
        }
    }

    pub fn items(&self) -> &[ResponseItem] {
        &self.items
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
pub struct ToolEnvironment {
    /// Stable host environment id used to route executor-scoped capabilities.
    pub environment_id: String,
    /// Effective working directory for this turn in the environment.
    pub cwd: AbsolutePathBuf,
    /// Filesystem implementation for this environment.
    pub file_system: Arc<dyn ExecutorFileSystem>,
    /// Sandbox context to use for filesystem operations.
    pub file_system_sandbox_context: FileSystemSandboxContext,
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

#[derive(Clone)]
pub struct ToolCall {
    pub turn_id: String,
    pub call_id: String,
    pub tool_name: ToolName,
    pub model: String,
    pub truncation_policy: TruncationPolicy,
    pub conversation_history: ConversationHistory,
    pub turn_item_emitter: Arc<dyn TurnItemEmitter>,
    pub environments: Vec<ToolEnvironment>,
    pub payload: ToolPayload,
}

impl std::fmt::Debug for ToolCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolCall")
            .field("turn_id", &self.turn_id)
            .field("call_id", &self.call_id)
            .field("tool_name", &self.tool_name)
            .field("model", &self.model)
            .field("truncation_policy", &self.truncation_policy)
            .field("conversation_history", &self.conversation_history)
            .field("turn_item_emitter", &"<host turn item emitter>")
            .field("environment_count", &self.environments.len())
            .field("payload", &self.payload)
            .finish()
    }
}

impl ToolCall {
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
