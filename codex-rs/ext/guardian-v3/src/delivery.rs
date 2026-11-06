//! Delivers housekeeper posts from the reviewer's selected channels in source order.
//! A plan fetches bodies one board page at a time; only IDs and positions survive the plan.

use crate::EvidenceId;
use crate::GUARDIAN_BOARD_NAMESPACE;
use crate::sampling::context_input;
use anyhow::ensure;
use codex_message_board::BoardReadView;
use codex_message_board::PageRequest;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::HistoryPosition;
use codex_protocol::protocol::TruncationPolicy;
use serde::Deserialize;
use serde::Serialize;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::future::Future;

/// In-memory progress for one retained reviewer conversation. A fresh conversation starts with fresh state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryState {
    reviewer: ThreadId,
    board: SessionId,
    delivered: BTreeSet<u64>,
    /// Last board insertion in each channel for which delivery has completed.
    channel_positions: BTreeMap<String, i64>,
}

/// Exclusive to one selection and captured view. Dropping a partial plan preserves
/// successful admissions. A subsequent plan resumes from the last completed channel ranges.
pub struct DeliveryPlan<'a> {
    state: &'a mut DeliveryState,
    board: &'a dyn BoardReadView,
    /// Source ordinal -> one immutable housekeeper post.
    pending: BTreeMap<u64, i64>,
    /// Texts for the leading pending ordinals, including any that did not fit in the last batch.
    prefetched: VecDeque<String>,
    channel_positions: BTreeMap<String, i64>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RoutedOriginal<'a> {
    id: EvidenceId,
    #[serde(borrow)]
    original: &'a RawValue,
}

#[derive(Serialize)]
struct DeliveryInput<'a> {
    sources: Vec<RoutedOriginal<'a>>,
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

    /// Collects the positions of undelivered originals within the board and source cutoffs.
    pub async fn plan<'a>(
        &'a mut self,
        board: &'a dyn BoardReadView,
        selected: &BTreeSet<String>,
        cutoff: HistoryPosition,
    ) -> anyhow::Result<DeliveryPlan<'a>> {
        ensure!(
            self.board == board.identity() && self.board == SessionId::from(cutoff.thread_id),
            "delivery board and source cutoff must belong to the reviewer's root"
        );
        ensure!(
            board.storage_namespace() == GUARDIAN_BOARD_NAMESPACE,
            "delivery requires the Guardian board"
        );
        ensure!(
            self.delivered
                .last()
                .is_none_or(|ordinal| *ordinal < cutoff.end_ordinal_exclusive),
            "review cutoff predates evidence already in the conversation"
        );
        let mut pending = BTreeMap::new();
        let mut channel_positions = BTreeMap::new();
        let page_size = PageRequest::default().limit.get() as usize;
        for channel in selected {
            ensure!(
                board
                    .channels()
                    .iter()
                    .any(|entry| &entry.channel_name == channel),
                "selected channel is outside the captured board view"
            );
            let previous = self
                .channel_positions
                .get(channel)
                .copied()
                .unwrap_or_default();
            let mut position = previous;
            loop {
                let page = board
                    .appended_request_ids(self.reviewer, channel, position)
                    .await?;
                let last_page = page.len() < page_size;
                for (post_position, request_id) in page {
                    let ordinal = EvidenceId::from_request_id(&request_id, channel)?.ordinal;
                    ensure!(
                        ordinal < cutoff.end_ordinal_exclusive,
                        "board source exceeds review cutoff"
                    );
                    if !self.delivered.contains(&ordinal) {
                        pending.entry(ordinal).or_insert(post_position);
                    }
                    position = post_position;
                }
                if last_page {
                    break;
                }
            }
            if position != previous {
                channel_positions.insert(channel.clone(), position);
            }
        }
        Ok(DeliveryPlan {
            state: self,
            board,
            pending,
            prefetched: VecDeque::new(),
            channel_positions,
        })
    }
}

impl DeliveryPlan<'_> {
    /// Admits one batch of selected board posts. The callback must record the items
    /// in the retained conversation before reporting success. Returns zero when done.
    pub async fn deliver<F, Fut>(
        &mut self,
        policy: TruncationPolicy,
        admit: F,
    ) -> anyhow::Result<usize>
    where
        F: FnOnce(Vec<ResponseItem>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        let mut input = DeliveryInput {
            sources: Vec::new(),
        };
        let mut bytes = serde_json::to_string(&input)?.len();
        let page_size = PageRequest::default().limit.get() as usize;
        let mut unread = self.pending.values().skip(self.prefetched.len());
        let mut count = 0;
        while count < self.pending.len() {
            if count == self.prefetched.len() {
                let positions: Vec<_> = unread.by_ref().take(page_size).copied().collect();
                self.prefetched.extend(
                    self.board
                        .read_post_texts(self.state.reviewer, &positions)
                        .await?,
                );
            }
            let additional = self.prefetched[count].len() + usize::from(count != 0);
            if additional > policy.byte_budget().saturating_sub(bytes) {
                ensure!(
                    count != 0,
                    "original evidence exceeds the model's tool-output budget"
                );
                break;
            }
            bytes += additional;
            count += 1;
        }
        for (ordinal, text) in self.pending.keys().zip(&self.prefetched).take(count) {
            let source: RoutedOriginal<'_> = serde_json::from_str(text)?;
            ensure!(
                source.id.ordinal == *ordinal,
                "selected board post has a different source ordinal"
            );
            input.sources.push(source);
        }
        if count != 0 {
            let item = context_input("guardian_original_evidence", &input, policy)?;
            admit(vec![item]).await?;
            self.prefetched.drain(..count);
            for _ in 0..count {
                if let Some((ordinal, _)) = self.pending.pop_first() {
                    self.state.delivered.insert(ordinal);
                }
            }
        }
        if self.pending.is_empty() {
            self.state
                .channel_positions
                .append(&mut self.channel_positions);
        }
        Ok(count)
    }
}
