//! Capture raw paste tabs before completion and submission shortcuts see them.

use super::*;

impl ChatComposer {
    pub(super) fn handle_paste_tab(&mut self, key: KeyEvent, now: Instant) -> bool {
        if key.code != KeyCode::Tab
            || !key.modifiers.is_empty()
            || self.draft.disable_paste_burst
            || !self.draft.textarea.allows_paste_burst()
        {
            return false;
        }

        self.handle_paste_burst_flush(now);
        if self
            .draft
            .paste_burst
            .append_control_char_if_active('\t', now)
        {
            return true;
        }

        // Short non-ASCII prefixes are inserted directly, without a held first character.
        if self
            .draft
            .paste_burst
            .direct_insert_newline_should_insert(now)
        {
            self.draft
                .paste_burst
                .begin_with_retro_grabbed(String::new(), now);
            return self
                .draft
                .paste_burst
                .append_control_char_if_active('\t', now);
        }

        false
    }
}
