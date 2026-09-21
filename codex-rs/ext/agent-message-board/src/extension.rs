//! Installs caller-bound tools using persistent host identities.
//!
//! Backend selection is a host concern. Startup failures leave tools unavailable
//! and emit a warning; a disabled factory does not open storage.

use crate::AgentMessageBoard;
use crate::message_board_tools;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolContributor;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use futures::future::BoxFuture;
use std::sync::Arc;

type BoardFactory<C> = dyn Fn(&C, SessionId, ThreadId) -> BoxFuture<'static, Result<Option<Arc<dyn AgentMessageBoard>>>>
    + Send
    + Sync;

struct BoardExtension<C> {
    open: Box<BoardFactory<C>>,
    events: Arc<dyn ExtensionEventSink>,
}
struct Binding {
    board: Arc<dyn AgentMessageBoard>,
    caller: ThreadId,
    path: AgentPath,
}

impl<C: Sync> ThreadLifecycleContributor<C> for BoardExtension<C> {
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let result = async {
                let tree =
                    SessionId::from_string(input.session_store.level_id()).map_err(|_| {
                        CodexErr::InvalidRequest("invalid board session identity".into())
                    })?;
                let caller =
                    ThreadId::from_string(input.thread_store.level_id()).map_err(|_| {
                        CodexErr::InvalidRequest("invalid board caller identity".into())
                    })?;
                if let Some(board) = (self.open)(input.config, tree, caller).await? {
                    if board.identity() != tree {
                        return Err(CodexErr::InvalidRequest(
                            "message-board factory returned another tree".into(),
                        ));
                    }
                    input.thread_store.insert(Binding {
                        board,
                        caller,
                        path: input
                            .session_source
                            .get_agent_path()
                            .unwrap_or_else(AgentPath::root),
                    });
                }
                Ok::<(), CodexErr>(())
            }
            .await;
            if result.is_err() {
                self.events.emit_warning(ExtensionWarning {
                    thread_id: input.thread_store.level_id().into(), turn_id: None,
                    message: "Agent message-board initialization failed; its tools are unavailable for this runtime.".into(),
                });
            }
        })
    }
}

impl<C: Sync> ToolContributor for BoardExtension<C> {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        thread_store
            .get::<Binding>()
            .map_or_else(Vec::new, |binding| {
                message_board_tools(binding.board.clone(), binding.caller, binding.path.clone())
            })
    }
}

/// Installs message-board lifecycle and tool contributions. The factory selects
/// a local or remote backend, or returns None when disabled. Configuration is
/// read at runtime startup, including resume; no board is created by installation.
pub fn install<C: Sync + 'static>(
    registry: &mut ExtensionRegistryBuilder<C>,
    open: impl Fn(
        &C,
        SessionId,
        ThreadId,
    ) -> BoxFuture<'static, Result<Option<Arc<dyn AgentMessageBoard>>>>
    + Send
    + Sync
    + 'static,
) {
    let extension = Arc::new(BoardExtension {
        open: Box::new(open),
        events: registry.event_sink(),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.tool_contributor(extension);
}
