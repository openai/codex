//! Exposes a narrow history capability backed by the parent's live MCP runtime.
//! Guardian owns tool exposure and output limits, never a second Apps connection.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::Weak;

use codex_core::CodexThread;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStartPhase;

pub(super) struct ConversationHistoryTools {
    pub parent: Weak<CodexThread>,
}

struct HistoryTools(Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>>);

impl TurnLifecycleContributor for ConversationHistoryTools {
    fn turn_start_phase(&self, _thread_store: &ExtensionData) -> TurnStartPhase {
        TurnStartPhase::RegularTaskStart
    }

    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let tools = match self.parent.upgrade() {
                Some(parent) => {
                    let max_output_tokens = parent
                        .config()
                        .await
                        .guardian_conversation_history_max_output_tokens
                        .map_or(/*default*/ 4_000, NonZeroUsize::get);
                    codex_core::guardian_review::conversation_history_tools(
                        &parent,
                        max_output_tokens,
                    )
                    .await
                    .unwrap_or_default()
                }
                None => Vec::new(),
            };
            input.thread_store.insert(HistoryTools(tools));
        })
    }
}

impl ToolContributor for ConversationHistoryTools {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        thread_store
            .get::<HistoryTools>()
            .map(|tools| tools.0.clone())
            .unwrap_or_default()
    }
}
