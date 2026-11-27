//! Completion metadata for live and restored turns. Saved timestamps are authoritative;
//! only a live completion may fall back to the local clock. Labels are deduplicated per thread.

use super::*;

impl ChatWidget {
    pub(crate) fn completion_cell(
        &mut self,
        turn: &Turn,
        replay_kind: Option<ReplayKind>,
    ) -> Option<history_cell::FinalMessageSeparator> {
        let live = replay_kind.is_none();
        let elapsed = if live {
            self.turn_lifecycle
                .elapsed_at_idle
                .take_if(|(id, _)| *id == turn.id)
                .map(|(_, elapsed)| elapsed)
                .or_else(|| self.bottom_pane.status_elapsed())
        } else {
            None
        };
        history_cell::FinalMessageSeparator::from_turn_once(
            turn,
            &mut self.turn_lifecycle.rendered_completion_turn_ids,
            elapsed,
            live.then(Local::now),
        )
        .filter(|cell| self.local_settings.transcript_mode.is_owned() || cell.has_timing())
    }
}
