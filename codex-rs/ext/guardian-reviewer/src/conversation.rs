//! Tracks reviewed transcript progress separately from committed conversation history.
//! Hosts serialize reviews and checkpoint commits, and supply opaque completed history.

use codex_guardian_context::TranscriptCursor;

/// A completed history and the transcript progress that produced it.
#[derive(Clone)]
pub struct ConversationCheckpoint<H> {
    history: H,
    cursor: Option<TranscriptCursor>,
    completed_review_count: usize,
}

impl<H> ConversationCheckpoint<H> {
    pub fn history(&self) -> &H {
        &self.history
    }
}

/// Shared bookkeeping for a host-owned Guardian conversation.
pub struct ConversationState<H> {
    cursor: Option<TranscriptCursor>,
    completed_review_count: usize,
    checkpoint: Option<ConversationCheckpoint<H>>,
}

impl<H> Default for ConversationState<H> {
    fn default() -> Self {
        Self {
            cursor: None,
            completed_review_count: 0,
            checkpoint: None,
        }
    }
}

impl<H> ConversationState<H> {
    /// Starts an independent conversation from exactly the committed history and cursor.
    pub fn fork(checkpoint: ConversationCheckpoint<H>) -> (Self, H) {
        (
            Self {
                cursor: checkpoint.cursor,
                completed_review_count: checkpoint.completed_review_count,
                ..Self::default()
            },
            checkpoint.history,
        )
    }

    pub fn cursor(&self) -> Option<TranscriptCursor> {
        self.cursor
    }

    pub fn completed_review_count(&self) -> usize {
        self.completed_review_count
    }

    /// Rebuilds the active transcript after compaction. Forks can still use the last
    /// committed history; it owns its own cursor and is independent of this reset.
    pub fn reset_transcript(&mut self) {
        self.cursor = None;
    }

    /// Advances live progress once the host's review completes.
    pub fn complete_review(&mut self, cursor: TranscriptCursor) {
        self.cursor = Some(cursor);
        self.completed_review_count = self.completed_review_count.saturating_add(1);
    }

    /// Pairs host-supplied history with the current progress for subsequent forks.
    pub fn commit_snapshot(&mut self, history: H) {
        self.checkpoint = Some(ConversationCheckpoint {
            history,
            cursor: self.cursor,
            completed_review_count: self.completed_review_count,
        });
    }

    pub fn snapshot(&self) -> Option<&ConversationCheckpoint<H>> {
        self.checkpoint.as_ref()
    }
}

#[cfg(test)]
#[path = "conversation_tests.rs"]
mod tests;
