//! Reviewer tools for searching live evidence and managing the acting agent’s subscriptions.
use crate::GUARDIAN_BOARD_NAMESPACE;
use crate::LiveDelivery;
use crate::inbox::ActorInbox;
use anyhow::ensure;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolContributor;
use codex_message_board::AgentMessageBoard;
use codex_message_board::MessageBoardTool;
use codex_message_board::message_board_tools;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use codex_tools::ToolName;
use std::sync::Arc;

const NAMESPACE: &str = "guardian_board";
const JSON_ARGUMENTS: &str = "/original/payload/arguments";
const LIVE_DESCRIPTION: &str = "Live original evidence by topic. Search and read any channel. Subscribe to receive future evidence from that channel.";
const LIVE_READ_TOOLS: [MessageBoardTool; 5] = [
    MessageBoardTool::GetChannels,
    MessageBoardTool::ListThreads,
    MessageBoardTool::SearchPosts,
    MessageBoardTool::ReadThread,
    MessageBoardTool::ReadPost,
];

struct RetrievalTools;

/// Actor-scoped live board tools in an isolated Guardian verdict conversation.
#[derive(Clone)]
pub struct LiveRetrievalBinding {
    board: Arc<dyn AgentMessageBoard>,
    pub(super) inbox: Arc<ActorInbox>,
    path: AgentPath,
}

impl LiveRetrievalBinding {
    /// Register a fresh reviewer inbox. Clone this binding to reuse its subscriptions.
    pub fn new(
        board: Arc<dyn AgentMessageBoard>,
        caller: ThreadId,
        path: AgentPath,
        delivery: &Arc<LiveDelivery>,
    ) -> anyhow::Result<Self> {
        ensure!(
            board.storage_namespace() == Some(GUARDIAN_BOARD_NAMESPACE),
            "reviewer retrieval requires a Guardian board"
        );
        ensure!(
            board.identity() == delivery.root.into(),
            "reviewer retrieval requires the inbox's board"
        );
        Ok(Self {
            board: board.with_json_search(JSON_ARGUMENTS)?,
            inbox: delivery.register(caller),
            path,
        })
    }

    /// Allowlist for the isolated reviewer's tool policy.
    pub fn tool_names() -> impl Iterator<Item = ToolName> {
        LIVE_READ_TOOLS
            .into_iter()
            .map(MessageBoardTool::name)
            .chain(crate::subscription_tools::names())
            .map(|name| ToolName::namespaced(NAMESPACE, name))
    }

    pub fn tools(&self) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        let mut tools = message_board_tools(
            &LIVE_READ_TOOLS,
            self.board.clone(),
            self.inbox.actor,
            self.path.clone(),
            Some(NAMESPACE),
            LIVE_DESCRIPTION,
            /*tool_messages*/ None,
        );
        tools.extend(crate::subscription_tools::tools(
            &self.inbox,
            &self.path,
            NAMESPACE,
            LIVE_DESCRIPTION,
        ));
        tools
    }
}

impl ToolContributor for RetrievalTools {
    fn tools(
        &self,
        _session: &ExtensionData,
        thread: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        thread
            .get::<LiveRetrievalBinding>()
            .map_or_else(Vec::new, |binding| binding.tools())
    }
}

/// Installs capabilities only. The host must install this in the isolated reviewer
/// registry and provide a binding; installation does not open storage or start work.
pub fn install_retrieval_tools<C: Sync>(registry: &mut ExtensionRegistryBuilder<C>) {
    registry.tool_contributor(Arc::new(RetrievalTools));
}
