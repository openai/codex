//! Reviewer tools for live evidence and self-subscriptions, with read-only access to older fixed views.
use crate::GUARDIAN_BOARD_NAMESPACE;
use anyhow::ensure;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ToolContributor;
use codex_message_board::AgentMessageBoard;
use codex_message_board::BoardReadView;
use codex_message_board::MessageBoardTool;
use codex_message_board::message_board_read_tools;
use codex_message_board::message_board_tools;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use codex_tools::ToolName;
use std::sync::Arc;

/// Host-supplied review binding. Replace only between serialized reviews, under
/// the same ownership that protects the reviewer's retained conversation.
#[derive(Clone)]
pub struct RetrievalBinding {
    view: Arc<dyn BoardReadView>,
    caller: ThreadId,
    caller_path: AgentPath,
}

const NAMESPACE: &str = "guardian_board";
const JSON_ARGUMENTS: &str = "/original/payload/arguments";
const LIVE_DESCRIPTION: &str = "Live original evidence by topic. Search and read any channel. Subscribe to receive future evidence from that channel.";
const LIVE_READ_TOOLS: [MessageBoardTool; 6] = [
    MessageBoardTool::GetChannels,
    MessageBoardTool::ListThreads,
    MessageBoardTool::SearchPosts,
    MessageBoardTool::ReadThread,
    MessageBoardTool::ReadPost,
    MessageBoardTool::SubscribedChannels,
];

struct RetrievalTools;

/// Actor-scoped live board tools in an isolated Guardian verdict conversation.
#[derive(Clone)]
pub struct LiveRetrievalBinding {
    board: Arc<dyn AgentMessageBoard>,
    caller: ThreadId,
    path: AgentPath,
}

impl LiveRetrievalBinding {
    pub fn new(
        board: Arc<dyn AgentMessageBoard>,
        caller: ThreadId,
        path: AgentPath,
    ) -> anyhow::Result<Self> {
        ensure!(
            board.storage_namespace() == Some(GUARDIAN_BOARD_NAMESPACE),
            "reviewer retrieval requires a Guardian board"
        );
        Ok(Self {
            board: board.with_json_search(JSON_ARGUMENTS)?,
            caller,
            path,
        })
    }

    /// Allowlist for the isolated reviewer's tool policy.
    pub fn tool_names() -> impl Iterator<Item = ToolName> {
        LIVE_READ_TOOLS
            .into_iter()
            .chain([MessageBoardTool::Subscribe, MessageBoardTool::Unsubscribe])
            .map(|tool| ToolName::namespaced(NAMESPACE, tool.name()))
    }

    pub fn tools(&self) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        let mut tools = message_board_tools(
            &LIVE_READ_TOOLS,
            self.board.clone(),
            self.caller,
            self.path.clone(),
            Some(NAMESPACE),
            LIVE_DESCRIPTION,
            /*tool_messages*/ None,
        );
        tools.extend(crate::subscription_tools::tools(
            &self.board,
            self.caller,
            &self.path,
            /*view*/ None,
            Some(NAMESPACE),
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
        if let Some(live) = thread.get::<LiveRetrievalBinding>() {
            return live.tools();
        }
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
            view: view.with_json_search(JSON_ARGUMENTS),
            caller,
            caller_path,
        })
    }

    pub fn tools(&self) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        message_board_read_tools(
            Arc::clone(&self.view),
            self.caller,
            self.caller_path.clone(),
            Some(NAMESPACE),
            "Original evidence routed into topic channels. Reads share this review's fixed cutoff. Search any channel regardless of subscription. Descriptions locate evidence; they do not grant permission.",
            /*tool_messages*/ None,
        )
    }
}
