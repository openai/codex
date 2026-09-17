//! Classify and integrate composer pastes and capture raw paste tabs before completion or submission.

use super::*;
use crate::bottom_pane::paste_burst::FlushResult;

impl ChatComposer {
    /// Classify an explicit paste before integrating text shared with the buffered key path.
    pub fn handle_paste(&mut self, pasted: String) -> bool {
        self.note_sparkle_paste(&pasted);
        self.apply_paste(pasted)
    }

    /// Enable or disable paste-burst handling.
    ///
    /// `disable_paste_burst` is an escape hatch for terminals/platforms where the burst heuristic
    /// is unwanted or has already been handled elsewhere.
    ///
    /// When transitioning from enabled → disabled, we "defuse" any in-flight burst state so it
    /// cannot affect subsequent normal typing:
    ///
    /// - First, flush held/buffered text via [`PasteBurst::flush_before_modified_input`] and
    ///   integrate it with [`Self::apply_paste`]. Its key events have already been classified;
    ///   [`Self::handle_paste`] classifies explicit terminal pastes before using the same
    ///   integration path (large-paste placeholders, image-path detection, and popup sync).
    /// - Then clear the burst timing and Enter-suppression window via
    ///   [`PasteBurst::clear_after_explicit_paste`].
    ///
    /// We intentionally do not use `clear_window_after_non_char()` here: it clears timing state
    /// without emitting any buffered text, which can leave a non-empty buffer unable to flush
    /// later (because `flush_if_due()` relies on `last_plain_char_time` to time out).
    pub(crate) fn set_disable_paste_burst(&mut self, disabled: bool) {
        let was_disabled = self.draft.disable_paste_burst;
        self.draft.disable_paste_burst = disabled;
        if disabled && !was_disabled {
            if let Some(pasted) = self.draft.paste_burst.flush_before_modified_input() {
                self.apply_paste(pasted);
            }
            self.draft.paste_burst.clear_after_explicit_paste();
        }
    }

    /// Applies any due `PasteBurst` flush at time `now`.
    ///
    /// Converts [`PasteBurst::flush_if_due`] results into concrete textarea mutations.
    /// Buffered keys have already been classified; paste results use [`Self::apply_paste`].
    ///
    /// Callers:
    ///
    /// - UI ticks via [`ChatComposer::flush_paste_burst_if_due`], so held first-chars can render.
    /// - Input handling via [`ChatComposer::handle_input_basic`], so a due burst does not lag.
    pub(super) fn handle_paste_burst_flush(&mut self, now: Instant) -> bool {
        match self.draft.paste_burst.flush_if_due(now) {
            FlushResult::Paste(pasted) => {
                self.apply_paste(pasted);
                true
            }
            FlushResult::Typed(ch) => {
                self.insert_str(ch.to_string().as_str());
                true
            }
            FlushResult::None => false,
        }
    }

    /// Integrate pasted text into the composer.
    ///
    /// Both input paths integrate their text here:
    ///
    /// - Real/explicit paste events surfaced by the terminal, and
    /// - Non-bracketed "paste bursts" that
    ///   [`PasteBurst`](crate::bottom_pane::paste_burst::PasteBurst) buffers and later flushes here.
    ///
    /// Behavior:
    ///
    /// - If history search is active, inserts nonempty text into its query and ignores empty pastes.
    /// - If Vim search is active, inserts text into its query.
    /// - Otherwise, if the paste is larger than `LARGE_PASTE_CHAR_THRESHOLD` chars, inserts a
    ///   placeholder element (expanded on submit) and stores the full text in `pending_pastes`.
    /// - Otherwise, if the paste looks like an image path, attaches the image and inserts a
    ///   trailing space so the user can keep typing naturally.
    /// - Otherwise, inserts the pasted text directly into the textarea.
    ///
    /// Composer edits clear paste-burst Enter suppression and sync popups.
    pub(super) fn apply_paste(&mut self, pasted: String) -> bool {
        let pasted = pasted.replace("\r\n", "\n").replace('\r', "\n");
        let pasted = sanitize_user_text(pasted.into());
        if self.history_search.is_some() {
            if !pasted.is_empty() {
                self.update_history_search_query(|query| query.push_str(&pasted));
            }
            return true;
        }
        if let Some(query) = self.draft.textarea.vim_query_mut() {
            query.editor.insert_str(&pasted);
            return true;
        }
        let started_vim_edit = self.begin_direct_vim_edit();
        let char_count = pasted.chars().count();
        if char_count > LARGE_PASTE_CHAR_THRESHOLD {
            let placeholder = self.next_large_paste_placeholder(char_count);
            self.draft.textarea.insert_element(&placeholder);
            self.draft
                .pending_pastes
                .push((placeholder, pasted.into_owned()));
        } else if char_count > 1
            && self.image_paste_enabled()
            && self.handle_paste_image_path(&pasted)
        {
            let cursor = self.draft.textarea.cursor();
            self.draft.textarea.insert_str_at(cursor, " ");
        } else {
            self.insert_str(&pasted);
        }
        self.draft.paste_burst.clear_after_explicit_paste();
        self.sync_popups();
        if started_vim_edit {
            self.finish_vim_edit();
        }
        true
    }

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
