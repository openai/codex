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
        history_cell::FinalMessageSeparator::from_turn_once(
            turn,
            &mut self.turn_lifecycle.rendered_completion_turn_ids,
            self.bottom_pane.status_elapsed().filter(|_| live),
            live.then(Local::now),
        )
    }
}
