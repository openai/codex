//! Save a browsing origin independently of the compact/detailed position pair.
//! Retained cell identities and source offsets survive pagination and rewrapping.

use super::*;

pub(crate) struct TranscriptBookmark {
    position: Position,
    saved_position: Option<Position>,
    suppressed_prompt_header: Option<prompt_header::SuppressedHeader>,
    detailed: bool,
    mode: HistoryRenderMode,
    snapshot: Option<ViewSnapshot>,
    last_tail: Option<EntryKey>,
    unseen_activity: bool,
}

impl TranscriptView {
    pub(crate) fn bookmark(&mut self, cells: &[Arc<dyn HistoryCell>]) -> TranscriptBookmark {
        TranscriptBookmark {
            position: self.position,
            saved_position: self.saved_position,
            suppressed_prompt_header: self.suppressed_prompt_header,
            detailed: self.detailed,
            mode: self.mode,
            snapshot: (!self.is_following()).then(|| self.capture_snapshot(cells)),
            last_tail: self.last_tail,
            unseen_activity: self.unseen_activity,
        }
    }

    pub(crate) fn restore_bookmark(&mut self, bookmark: TranscriptBookmark) {
        self.jump_to_latest();
        self.set_presentation(bookmark.detailed, bookmark.mode);
        self.position = bookmark.position;
        self.saved_position = bookmark.saved_position;
        self.suppressed_prompt_header = bookmark.suppressed_prompt_header;
        self.held_reading = bookmark.snapshot;
        self.rewrap_snapshot(self.area.width);
        self.highlight = None;
        self.unseen_activity = bookmark.unseen_activity || self.last_tail != bookmark.last_tail;
    }
}

#[cfg(test)]
#[path = "bookmark_tests.rs"]
mod tests;
