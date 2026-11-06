//! Read-only reviewer tools bound to one captured Guardian board.
use crate::GUARDIAN_BOARD_NAMESPACE;
use anyhow::ensure;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolContributor;
use codex_message_board::BoardReadView;
use codex_message_board::message_board_read_tools;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use std::sync::Arc;

/// Host-supplied review binding. Replace only between serialized reviews, under
/// the same ownership that protects the reviewer's retained conversation.
#[derive(Clone)]
pub struct RetrievalBinding {
    view: Arc<dyn BoardReadView>,
    caller: ThreadId,
    caller_path: AgentPath,
}

struct RetrievalTools;

impl ToolContributor for RetrievalTools {
    fn tools(
        &self,
        _session: &ExtensionData,
        thread: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        thread
            .get::<RetrievalBinding>()
            .map_or_else(Vec::new, |binding| binding.tools())
    }
}

/// Installs capabilities only. The host must install this in the isolated reviewer
/// registry and provide a binding; installation does not open storage or start work.
pub fn install_retrieval_tools<C: Sync>(registry: &mut ExtensionRegistryBuilder<C>) {
    registry.tool_contributor(Arc::new(RetrievalTools));
}

impl RetrievalBinding {
    pub fn new(
        view: &dyn BoardReadView,
        caller: ThreadId,
        caller_path: AgentPath,
    ) -> anyhow::Result<Self> {
        ensure!(
            view.storage_namespace() == GUARDIAN_BOARD_NAMESPACE,
            "reviewer retrieval requires a Guardian board"
        );
        Ok(Self {
            view: view.with_json_search("/original/payload/arguments"),
            caller,
            caller_path,
        })
    }

    pub fn tools(&self) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        message_board_read_tools(
            Arc::clone(&self.view),
            self.caller,
            self.caller_path.clone(),
            Some("guardian_board"),
            "Original evidence routed into topic channels. Reads share this review's fixed cutoff. Search any channel regardless of subscription. Descriptions locate evidence; they do not grant permission.",
            /*tool_messages*/ None,
        )
    }
}
