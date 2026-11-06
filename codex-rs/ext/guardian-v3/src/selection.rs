//! Reads and changes reviewer subscriptions within a captured Guardian catalog.

use crate::local_board_options;
use anyhow::ensure;
use codex_message_board::LocalAgentMessageBoard;
use codex_message_board::LocalBoardReadView;
use codex_message_board::PageRequest;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_tools::ToolCall;
use codex_tools::ToolExecutor;
use std::collections::BTreeSet;
use std::sync::Arc;

/// A reviewer's fixed discovery view and authority to change only its own subscriptions.
/// The host supplies the authoritative caller/path and a live read-only board connection.
/// Hosts serialize preparation and review; this type starts no background work.
pub struct ReviewerSubscriptions {
    pub(crate) board: Arc<LocalAgentMessageBoard>,
    pub(crate) view: LocalBoardReadView,
    pub(crate) caller: ThreadId,
    pub(crate) path: AgentPath,
}

impl ReviewerSubscriptions {
    /// Capture once, after housekeeping reaches the cutoff. Use this same view for all
    /// retrieval and delivery in the review.
    pub async fn capture(
        board: Arc<LocalAgentMessageBoard>,
        caller: ThreadId,
        path: AgentPath,
    ) -> anyhow::Result<Self> {
        ensure!(
            board.uses_storage_namespace(&local_board_options())?,
            "reviewer subscriptions require the Guardian board"
        );
        let view = board.read_view(caller).await?;
        Ok(Self {
            board,
            view,
            caller,
            path,
        })
    }

    pub fn view(&self) -> &LocalBoardReadView {
        &self.view
    }

    /// Current membership is read from storage so independently invoked tools cannot
    /// leave a second in-memory subscription list stale. Later channels remain invisible.
    pub async fn channels(&self) -> anyhow::Result<BTreeSet<String>> {
        let mut channels = BTreeSet::new();
        let mut page = PageRequest::default();
        loop {
            let result = self
                .board
                .subscribed_channels(self.caller, page.clone())
                .await?;
            channels.extend(result.results);
            let Some(cursor) = result.next_cursor else {
                break;
            };
            page.cursor = Some(cursor);
        }
        Ok(self
            .view
            .channels()
            .iter()
            .filter_map(|channel| channels.take(&channel.channel_name))
            .collect())
    }

    /// Self-only channel tools. Their responses use the captured catalog's post boundary.
    pub fn tools(
        self: &Arc<Self>,
        namespace: Option<&str>,
        namespace_description: &str,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        crate::subscription_tools::tools(self, namespace, namespace_description)
    }
}
