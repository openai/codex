//! Owns reviewer transcript progress, fork history, and checkpoint recovery bookkeeping.
//! The host captures history and validates authorization before publishing a checkpoint.
//! Evidence advances only after the matching review turn records its transcript input.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use codex_analytics::GuardianReviewSessionKind;
use codex_history::InitialHistory;
use codex_history::RolloutItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use tokio::sync::Mutex;

use crate::ConversationCheckpoint;
use crate::ConversationState;
use crate::GUARDIAN_TRANSCRIPT_START;

/// Opaque conversation progress retained while ThreadManager starts a reviewer.
pub struct GuardianReviewState {
    pub conversation: ConversationState<GuardianReviewHistory>,
    // Only the first attempt may use the checkpoint loaded without prior reviewer history.
    pub fresh_parent_checkpoint: bool,
    // The cursor describes evidence admitted before any later reviewer compaction.
    pub transcript_history_version: u64,
    pub transcript_source: Option<GuardianTranscriptSource>,
    pub last_admitted_node_repl_response_sequence: u64,
    pub pending_node_repl_evidence_admission: Option<PendingNodeReplEvidenceAdmission>,
}

// These histories have independent version counters; a cursor must identify its source.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GuardianTranscriptSource {
    Retained,
    LoadedParentCheckpoint,
}

pub struct PendingNodeReplEvidenceAdmission {
    pub turn_id: String,
    pub response_sequence: u64,
}

pub type GuardianReviewForkSnapshot = ConversationCheckpoint<GuardianReviewHistory>;

/// Host-owned history and admitted evidence used to seed a private reviewer fork.
#[derive(Clone)]
pub struct GuardianReviewHistory {
    pub initial_history: InitialHistory,
    transcript_source: Option<GuardianTranscriptSource>,
    last_admitted_node_repl_response_sequence: u64,
}

/// Controls whether selection may reuse a session or must start from the parent checkpoint.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReviewerSelection {
    ReuseIfAvailable,
    FreshParentCheckpoint,
}

/// Requests one bounded restart from the captured parent checkpoint when
/// reviewer compaction invalidates the pending input. The flag belongs to this attempt.
pub struct CheckpointRecovery {
    pub requested: Arc<AtomicBool>,
    pub history_version: u64,
    pub fresh_parent_checkpoint: bool,
}

impl GuardianReviewState {
    /// Restores fork progress or starts from the captured parent checkpoint.
    pub fn for_session(
        kind: GuardianReviewSessionKind,
        snapshot: Option<GuardianReviewForkSnapshot>,
        reviewer_selection: ReviewerSelection,
        parent_compaction: Option<&ResponseItem>,
    ) -> (Self, Option<InitialHistory>) {
        let mut conversation = snapshot
            .filter(|_| reviewer_selection == ReviewerSelection::ReuseIfAvailable)
            .map(ConversationState::from)
            .unwrap_or_default();
        if parent_compaction.is_some() && conversation.cursor().is_none() {
            // A compacted fork has no valid transcript progress. Prefer the
            // captured parent checkpoint to its lossy reviewer summary.
            conversation = ConversationState::default();
        }
        let history = match kind {
            GuardianReviewSessionKind::EphemeralForked => conversation.take_history(),
            // Replacement trunks keep the checkpoint if their first review is invalidated.
            GuardianReviewSessionKind::TrunkNew | GuardianReviewSessionKind::TrunkReused => {
                conversation
                    .snapshot()
                    .map(|checkpoint| checkpoint.history().clone())
            }
        };
        let state = Self {
            conversation,
            fresh_parent_checkpoint: history.is_none() && parent_compaction.is_some(),
            transcript_history_version: 0,
            transcript_source: history
                .as_ref()
                .and_then(|history| history.transcript_source),
            last_admitted_node_repl_response_sequence: history.as_ref().map_or(0, |history| {
                history.last_admitted_node_repl_response_sequence
            }),
            pending_node_repl_evidence_admission: None,
        };
        let initial_history = history.map(|history| history.initial_history).or_else(|| {
            parent_compaction
                .cloned()
                .map(|item| InitialHistory::Forked(vec![RolloutItem::ResponseItem(item.into())]))
        });
        (state, initial_history)
    }

    /// Publishes captured history after the host's final authorization validation.
    pub fn commit_snapshot(
        &mut self,
        items: Vec<RolloutItem>,
        history_version: u64,
        checkpoint_recovery: Option<&CheckpointRecovery>,
    ) {
        // Forks must not inherit a cursor for evidence the completed turn compacted away.
        if self.transcript_history_version != history_version && checkpoint_recovery.is_some() {
            self.conversation.reset_transcript();
            self.transcript_source = None;
            self.last_admitted_node_repl_response_sequence = 0;
        }
        let last_admitted_node_repl_response_sequence =
            self.last_admitted_node_repl_response_sequence;
        let transcript_source = self.transcript_source;
        self.conversation.commit_snapshot(GuardianReviewHistory {
            initial_history: InitialHistory::Forked(items),
            transcript_source,
            last_admitted_node_repl_response_sequence,
        });
    }
}

pub async fn admit_node_repl_evidence(state: &Mutex<GuardianReviewState>, event: &Event) {
    // Annotated review inputs are recorded as response items without UI turn
    // items. Both input paths emit this event after history admission.
    let EventMsg::RawResponseItem(recorded) = &event.msg else {
        return;
    };
    let ResponseItem::Message { role, content, .. } = &recorded.item else {
        return;
    };
    if role != "user"
        || !content.iter().any(|item| {
            matches!(item, ContentItem::InputText { text }
                if text == GUARDIAN_TRANSCRIPT_START || text == ">>> TRANSCRIPT DELTA START\n")
        })
    {
        return;
    }

    let mut state = state.lock().await;
    let Some(pending) = state.pending_node_repl_evidence_admission.as_ref() else {
        return;
    };
    if event.id == pending.turn_id {
        state.last_admitted_node_repl_response_sequence = state
            .last_admitted_node_repl_response_sequence
            .max(pending.response_sequence);
        state.pending_node_repl_evidence_admission = None;
    }
}
