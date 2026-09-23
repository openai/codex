//! Embed the composer in question rows, resetting history navigation when drafts change.
//! Recovered appends cancel pending Vim commands before moving the cursor and stay out of dot repeat.
//! Normal-mode recovery is one undoable edit; active insert/replace sessions keep their grouping.
//! Paste payloads stay intact and sparkle stays dismissed.

use super::super::textarea::VimPersistentState;
use super::*;

impl ComposerDraft {
    pub(in crate::bottom_pane) fn text_with_pending(&self) -> String {
        ChatComposer::expand_pending_pastes(
            &self.text,
            self.text_elements.clone(),
            &self.pending_pastes,
        )
        .0
    }
}

impl ChatComposer {
    pub(in crate::bottom_pane) fn append_recovered_drafts(&mut self, drafts: &str) {
        self.flush_pending_input();
        self.dismiss_sparkle();
        if self.draft.textarea.is_vim_operator_pending() {
            self.draft.textarea.enter_vim_normal_mode();
        }
        self.finish_vim_edit();
        let mut vim_state = VimPersistentState::default();
        self.draft
            .textarea
            .swap_vim_persistent_state(&mut vim_state);
        let started_vim_edit = self.begin_direct_vim_edit();
        self.move_cursor_to_end();
        if !self.current_text().is_empty() {
            self.insert_str("\n");
        }
        let char_count = drafts.chars().count();
        if char_count > LARGE_PASTE_CHAR_THRESHOLD {
            let placeholder = self.next_large_paste_placeholder(char_count);
            self.draft.textarea.insert_element(&placeholder);
            self.draft.pending_pastes.push((placeholder, drafts.into()));
            self.sync_popups();
        } else {
            self.insert_str(drafts);
        }
        self.draft
            .textarea
            .swap_vim_persistent_state(&mut vim_state);
        if started_vim_edit {
            self.finish_vim_edit();
        }
    }

    pub(in crate::bottom_pane) fn restore_inline_draft(&mut self, draft: ComposerDraft) {
        self.history.reset_navigation();
        self.restore_draft(draft);
    }

    pub(crate) fn inline_flash(&self) -> Option<Line<'static>> {
        self.footer
            .flash
            .as_ref()
            .filter(|_| self.footer.flash_visible())
            .map(|flash| flash.line.clone())
    }

    pub(crate) fn reset_vim_mode(&mut self) {
        self.vim_history = VimHistory::default();
        self.draft.textarea.enter_vim_insert_mode();
    }

    /// Refresh at traversal start without resetting recalled entries or pending lookups.
    pub(crate) fn copy_history_for_key(&mut self, composer: &Self, key: KeyEvent) {
        let recall = self.is_empty()
            && !self.history.is_navigating()
            && if self.draft.textarea.is_vim_normal_mode() {
                self.vim_normal_keymap.move_up.is_pressed(key)
                    || self.vim_normal_keymap.move_down.is_pressed(key)
            } else {
                self.editor_keymap.move_up.is_pressed(key)
                    || self.editor_keymap.move_down.is_pressed(key)
            };
        let search =
            self.history_search_previous_keys.is_pressed(key) && !self.handles_key_as_editing(key);
        if self.history_search.is_none() && (recall || search) {
            self.history = composer.history.clone();
            self.history.reset_navigation();
        }
    }

    /// Give modal editing precedence over a containing view's submit/navigation keys.
    /// Up/Down remain available to inline choices; j/k keep their Vim editing behavior.
    pub(crate) fn handles_key_as_editing(&self, key: KeyEvent) -> bool {
        self.history_search.is_some()
            || self.should_handle_vim_insert_escape(key)
            || self.draft.textarea.is_vim_operator_pending()
            || self.draft.textarea.wants_vim_search_key(key)
            || (self.draft.textarea.is_vim_normal_mode()
                && !matches!(key.code, KeyCode::Up | KeyCode::Down)
                && !self.submit_keys.is_pressed(key)
                && !self.queue_keys.is_pressed(key)
                && (self.vim_normal_keymap.move_up.is_pressed(key)
                    || self.vim_normal_keymap.move_down.is_pressed(key)))
    }

    /// Integrate already-classified buffered typing before another answer takes over this editor.
    pub(crate) fn flush_pending_input(&mut self) {
        if let Some(text) = self.draft.paste_burst.flush_before_modified_input() {
            self.apply_paste(text);
        }
        self.draft.paste_burst.clear_after_explicit_paste();
    }

    pub(crate) fn inline_input_height(&self, width: u16) -> u16 {
        self.draft.textarea.desired_height(width.max(1))
            + u16::from(self.history_search.is_some() || self.draft.textarea.vim_query().is_some())
            + u16::from(self.footer.flash_visible())
    }

    pub(crate) fn inline_cursor_pos(&self, mut area: Rect) -> Option<(u16, u16)> {
        area.height = area
            .height
            .saturating_sub(u16::from(self.footer.flash_visible()));
        let query_area = Rect::new(
            area.x,
            area.bottom().saturating_sub(1),
            area.width,
            area.height.min(1),
        );
        if let Some(query) = self.draft.textarea.vim_query() {
            return query.cursor_pos(query_area);
        }
        if self.history_search.is_some() {
            return self.history_search_query_cursor_pos(query_area);
        }
        self.draft
            .textarea
            .cursor_pos_with_state(area, *self.draft.textarea_state.borrow())
    }

    pub(crate) fn render_inline_input(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        ratatui::widgets::Clear.render(area, buf);
        Block::default()
            .style(Style::reset().patch(user_message_style()))
            .render(area, buf);
        let mut area = area;
        if self.footer.flash_visible()
            && let Some(flash) = &self.footer.flash
        {
            area.height = area.height.saturating_sub(1);
            flash.line.clone().render(
                Rect::new(area.x, area.bottom(), area.width, /*height*/ 1),
                buf,
            );
        }
        if let Some(query) = self.draft.textarea.vim_query() {
            area.height = area.height.saturating_sub(1);
            query.render(
                Rect::new(area.x, area.bottom(), area.width, /*height*/ 1),
                buf,
            );
        } else if let Some(line) = self.history_search_footer_line() {
            area.height = area.height.saturating_sub(1);
            line.render(
                Rect::new(area.x, area.bottom(), area.width, /*height*/ 1),
                buf,
            );
        }
        let mut state = self.draft.textarea_state.borrow_mut();
        let highlights: Vec<_> = self
            .history_search_highlight_ranges()
            .into_iter()
            .chain(self.draft.textarea.vim_search_highlights())
            .map(|range| (range, Style::new().reversed().bold()))
            .collect();
        self.draft.textarea.render_ref_styled_with_highlights(
            area,
            buf,
            &mut state,
            Style::default(),
            &highlights,
        );
        if self.draft.textarea.is_empty() {
            Line::from(self.placeholder_text.as_str().dim()).render(area, buf);
        }
    }
}
