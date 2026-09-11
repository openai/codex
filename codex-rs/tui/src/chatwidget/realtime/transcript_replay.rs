//! Keep retained voice captions before the subsequent agent turn during history replay.
//!
//! Anchors are assigned only to completed captions when a new live turn starts. Partial
//! captions remain eligible for late completion, and captions with no later turn stay last.

use super::ChatWidget;
use super::MAX_REPLAY_TRANSCRIPT_CELLS;
use super::RealtimeTranscriptRecord;
use std::collections::VecDeque;

impl ChatWidget {
    pub(crate) fn anchor_realtime_transcripts_before_turn(&mut self, turn_id: &str) {
        for record in &mut self.realtime_conversation.accepted_transcripts {
            if record.complete && record.before_turn_id.is_none() {
                record.before_turn_id = Some(turn_id.to_string());
            }
        }
    }

    pub(crate) fn queue_realtime_transcripts_for_replay(
        &mut self,
        records: VecDeque<RealtimeTranscriptRecord>,
    ) {
        self.realtime_conversation.replay_transcripts = Some(records);
    }

    pub(crate) fn restore_realtime_transcripts_before_turn(&mut self, turn_id: &str) {
        let mut remaining = VecDeque::new();
        let Some(records) = self.realtime_conversation.replay_transcripts.take() else {
            return;
        };
        for record in records {
            if record.before_turn_id.as_deref() == Some(turn_id) {
                // Insert after the queued consolidation, before replaying the next turn.
                let cell = self.realtime_transcript_history_cell(&record.role, &record.text);
                self.add_boxed_history(cell);
                if self.realtime_conversation.accepted_transcripts.len()
                    >= MAX_REPLAY_TRANSCRIPT_CELLS
                {
                    self.realtime_conversation.accepted_transcripts.pop_front();
                }
                self.realtime_conversation
                    .accepted_transcripts
                    .push_back(record);
            } else {
                remaining.push_back(record);
            }
        }
        self.realtime_conversation.replay_transcripts = Some(remaining);
    }

    pub(crate) fn finish_realtime_transcript_replay(&mut self) {
        if let Some(records) = self.realtime_conversation.replay_transcripts.take() {
            self.restore_realtime_transcript_cells(records);
        }
    }
}
