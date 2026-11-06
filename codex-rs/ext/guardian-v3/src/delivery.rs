//! Plans delivery of routed originals in source order by reading IDs from a fixed board view.

use crate::EvidenceId;
use crate::local_board_options;
use anyhow::ensure;
use codex_message_board::LocalBoardReadView;
use codex_message_board::PageRequest;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HistoryPosition;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

/// In-memory progress for one retained reviewer conversation. A fresh conversation starts with fresh state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryState {
    reviewer: ThreadId,
    board: SessionId,
    delivered: BTreeSet<u64>,
    /// Last board insertion in each channel for which delivery has completed.
    channel_positions: BTreeMap<String, i64>,
}

impl DeliveryState {
    pub fn new(reviewer: ThreadId, board: SessionId) -> Self {
        Self {
            reviewer,
            board,
            delivered: BTreeSet::new(),
            channel_positions: BTreeMap::new(),
        }
    }

    /// Maps each selected source ordinal to one board insertion sequence within the two cutoffs.
    pub async fn plan(
        &self,
        board: &LocalBoardReadView,
        selected: &BTreeSet<String>,
        cutoff: HistoryPosition,
    ) -> anyhow::Result<BTreeMap<u64, i64>> {
        ensure!(
            self.board == board.board().identity()
                && self.board == SessionId::from(cutoff.thread_id),
            "delivery board and source cutoff must belong to the reviewer's root"
        );
        ensure!(
            board.uses_storage_namespace(&local_board_options())?,
            "delivery requires the Guardian board"
        );
        ensure!(
            self.delivered
                .last()
                .is_none_or(|ordinal| *ordinal < cutoff.end_ordinal_exclusive),
            "review cutoff predates evidence already in the conversation"
        );
        let mut pending = BTreeMap::new();
        let page_size = PageRequest::default().limit.get() as usize;
        for channel in selected {
            ensure!(
                board
                    .channels()
                    .iter()
                    .any(|entry| &entry.channel_name == channel),
                "selected channel is outside the captured board view"
            );
            let mut position = self
                .channel_positions
                .get(channel)
                .copied()
                .unwrap_or_default();
            loop {
                let page = board
                    .appended_request_ids(self.reviewer, channel, position)
                    .await?;
                let last_page = page.len() < page_size;
                for (sequence, request_id) in page {
                    let ordinal = EvidenceId::from_request_id(&request_id, channel)?.ordinal;
                    ensure!(
                        ordinal < cutoff.end_ordinal_exclusive,
                        "board source exceeds review cutoff"
                    );
                    if !self.delivered.contains(&ordinal) {
                        pending.entry(ordinal).or_insert(sequence);
                    }
                    position = sequence;
                }
                if last_page {
                    break;
                }
            }
        }
        Ok(pending)
    }
}
