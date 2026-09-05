//! Route question actions through the shared composer, preserving drafts until delivery accepts them.
//! Inline Other reserves Up/Down for choices; j/k retain their Vim behavior.

use super::*;
use ratatui::layout::Rect;

impl AsyncQuestions {
    pub(crate) fn handles_key_as_editing(&self, key: KeyEvent) -> bool {
        self.focus_is_notes() && self.composer.handles_key_as_editing(key)
    }

    pub(super) fn other_selected(&self) -> bool {
        self.has_options() && self.selected_option_index() == Some(self.options().len())
    }

    pub(super) fn other_label(&self) -> String {
        self.current_answer()
            .filter(|_| !self.other_selected())
            .map(|answer| answer.draft.text.trim())
            .filter(|text| !text.is_empty())
            .map(|text| &text[..text.floor_char_boundary(512)])
            .map(|text| crate::text_formatting::truncate_text(text, /*max_graphemes*/ 128))
            .unwrap_or_else(|| self.other_placeholder().to_string())
    }

    pub(super) fn focus_other(&mut self) {
        self.select_option(self.options().len());
    }

    pub(super) fn select_option(&mut self, index: usize) {
        self.save_current_draft();
        self.state.pending[self.state.current_idx]
            .options_state
            .selected_idx = Some(index);
        self.sync_composer_placeholder();
    }

    pub(super) fn edit(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Repeat && self.keymap.composer.queue.is_pressed(key) {
            return;
        }

        if self.keymap.composer.submit.is_pressed(key) && !self.composer.is_in_paste_burst() {
            self.composer.flush_pending_input();
        }
        let before = self.composer.snapshot_draft();
        let (result, _) = self.composer.handle_key_event(key);
        let queued = matches!(result, InputResult::Queued { .. });
        if matches!(
            result,
            InputResult::Submitted { .. } | InputResult::Queued { .. }
        ) {
            self.composer.restore_draft(before);
            self.go_next_or_submit();
            if queued && let Some(QuestionSubmission::Submit(text)) = self.submission.take() {
                self.submission = Some(QuestionSubmission::Queue(text));
            }
        }
    }

    pub(super) fn other_prefix_width(&self, width: u16) -> u16 {
        let full = format!("› {}. ", self.options_len()).width() as u16;
        if full + 2 <= width {
            full
        } else {
            width.saturating_sub(2).min(2)
        }
    }

    pub(super) fn other_input_area(&self, options_area: Rect) -> Rect {
        let prefix_width = self.other_prefix_width(options_area.width);
        let width = options_area.width.saturating_sub(prefix_width);
        let height = self
            .composer
            .inline_input_height(width)
            .clamp(1, 8)
            .min(options_area.height);
        Rect::new(
            options_area.x.saturating_add(prefix_width),
            options_area.bottom().saturating_sub(height),
            width,
            height,
        )
    }

    pub(super) fn render_inline_options(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        use ratatui::text::Line;
        use ratatui::widgets::Widget;

        let row_width = area.width.saturating_add(1);
        let input = self.other_input_area(area);
        let named_area = Rect::new(area.x, area.y, area.width, input.y.saturating_sub(area.y));
        let mut rows = self.option_rows();
        rows.pop();
        let mut state = ScrollState::default();
        while state.scroll_top + 1 < rows.len()
            && measure_rows_height(&rows, &state, rows.len(), row_width) > named_area.height
        {
            state.scroll_top += 1;
        }
        if measure_rows_height(&rows, &state, rows.len(), row_width) <= named_area.height {
            self.visible_options
                .set((state.scroll_top, rows.len() - state.scroll_top));
        }
        crate::bottom_pane::request_user_input::render::render_rows_bottom_aligned(
            named_area,
            buf,
            &rows,
            &state,
            rows.len(),
            "",
        );
        let margin = Rect::new(
            area.x,
            input.y,
            input.x.saturating_sub(area.x),
            input.height.min(1),
        );
        let prefix = format!("› {}. ", self.options_len());
        Line::from(if usize::from(margin.width) < prefix.width() {
            "› ".to_string()
        } else {
            prefix
        })
        .style(crate::style::accent_style())
        .render(margin, buf);
        self.composer.render_inline_input(input, buf);
    }
    pub(crate) fn set_keymap(&mut self, keymap: &RuntimeKeymap) {
        self.keymap = keymap.clone();
        self.composer.set_keymap_bindings(keymap);
    }

    pub(crate) fn set_vim_enabled(&mut self, enabled: bool) {
        self.composer.set_vim_enabled(enabled);
    }
}

impl BottomPaneView for AsyncQuestions {
    fn keymap_contexts(&self) -> crate::keymap::KeymapContextSet {
        if self.focus_is_notes() {
            self.composer.keymap_contexts().with(KeymapContext::Chat)
        } else {
            crate::keymap::KeymapContextSet::new(KeymapContext::List).with(KeymapContext::Chat)
        }
    }

    fn prefer_esc_to_handle_key_event(&self) -> bool {
        true
    }

    fn will_interrupt_turn_on_key_event(&self, key: KeyEvent) -> bool {
        !self.handles_key_as_editing(key) && self.keymap.chat.interrupt_turn.is_pressed(key)
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Repeat && self.other_selector == Some(key.code) {
            return;
        }
        self.other_selector = None;
        if key.kind == KeyEventKind::Release || self.is_complete() {
            return;
        }
        self.snooze_auto_resolution();
        if self.handles_key_as_editing(key) {
            self.edit(key);
            return;
        }
        if self.keymap.chat.interrupt_turn.is_pressed(key) {
            self.app_event_tx.interrupt();
            return;
        }
        if key.kind == KeyEventKind::Press && self.keymap.chat.skip_question.is_pressed(key) {
            if self.delivery_enabled {
                self.accept_answer();
            }
            return;
        }
        if self.keymap.chat.edit_queued_message.is_pressed(key) {
            self.navigate(/*forward*/ true);
            return;
        }
        if self.keymap.chat.prompt_stack_back.is_pressed(key) {
            self.navigate(/*forward*/ false);
            return;
        }
        if self.focus_is_notes() && self.keymap.composer.submit.is_pressed(key) {
            if key.kind != KeyEventKind::Press {
                return;
            }
            let empty = self.composer.current_text_with_pending().trim().is_empty();
            if empty && !self.composer.is_in_paste_burst() {
                if !self.other_selected() {
                    self.go_next_or_submit();
                }
            } else {
                self.edit(key);
            }
            return;
        }
        if self.has_options() {
            // Default j/k start Other; explicit printable list remaps keep their meaning.
            let printable = matches!(key.code, KeyCode::Char(_))
                && !crate::key_hint::has_ctrl_or_alt(key.modifiers);
            let remapped = self.keymap.list.action_for(key).is_some_and(|action| {
                self.keymap.list.bindings_for(action)
                    != RuntimeKeymap::defaults().list.bindings_for(action)
            });
            if (!printable || remapped)
                && (!self.focus_is_notes() || matches!(key.code, KeyCode::Up | KeyCode::Down))
            {
                let count = self.options_len();
                let visible = self.visible_options.get().1.max(1);
                let mut state = self.state.pending[self.state.current_idx].options_state;
                match self.keymap.list.action_for(key) {
                    Some(ListAction::MoveUp) => state.move_up_wrap(count),
                    Some(ListAction::MoveDown) => state.move_down_wrap(count),
                    Some(ListAction::PageUp) => state.page_up_clamped(count, visible),
                    Some(ListAction::PageDown) => state.page_down_clamped(count, visible),
                    Some(ListAction::JumpTop) => state.jump_top(count, visible),
                    Some(ListAction::JumpBottom) => state.jump_bottom(count, visible),
                    Some(ListAction::Cancel) => {
                        self.set_expanded(/*expanded*/ false);
                        return;
                    }
                    Some(ListAction::MoveLeft | ListAction::MoveRight) => return,
                    Some(ListAction::Accept) | None => {}
                }
                if !matches!(
                    self.keymap.list.action_for(key),
                    Some(ListAction::Accept) | None
                ) {
                    self.select_option(state.selected_idx.unwrap_or(0));
                    self.state.pending[self.state.current_idx].options_state = state;
                    return;
                }
            }
        }
        if self.focus_is_notes() {
            self.edit(key);
            return;
        }
        if self.keymap.list.action_for(key) == Some(ListAction::Accept) {
            if key.kind != KeyEventKind::Press {
                return;
            }
            self.go_next_or_submit();
        } else if let KeyCode::Char(ch) = key.code
            && !crate::key_hint::has_ctrl_or_alt(key.modifiers)
        {
            if let Some(index) = self.option_index_for_digit(ch) {
                if key.kind != KeyEventKind::Press {
                    return;
                }
                self.select_option(index);
                if self.other_selected() {
                    self.other_selector = Some(key.code);
                } else {
                    self.go_next_or_submit();
                }
            } else {
                self.focus_other();
                self.composer.resume_text_entry();
                self.edit(key);
            }
        }
    }

    fn terminal_title_requires_action(&self) -> bool {
        !self.is_complete()
    }
    fn is_complete(&self) -> bool {
        self.unanswered_count() == 0
    }
    fn on_ctrl_c(&mut self) -> CancellationEvent {
        if self.composer.cancel_vim_search() || self.composer.cancel_history_search() {
            return CancellationEvent::Handled;
        }
        if self.focus_is_notes() && self.composer.clear_for_ctrl_c().is_some() {
            self.save_current_draft();
            self.sync_composer_placeholder();
        } else {
            return CancellationEvent::NotHandled;
        }
        CancellationEvent::Handled
    }
    fn handle_paste(&mut self, text: String) -> bool {
        if self.is_complete() || text.is_empty() {
            return false;
        }
        self.snooze_auto_resolution();
        if !self.focus_is_notes() {
            self.focus_other();
        }
        self.composer.flush_pending_input();
        self.composer.handle_paste(text)
    }
    fn flush_paste_burst_if_due(&mut self) -> bool {
        self.composer.flush_paste_burst_if_due()
    }
    fn is_in_paste_burst(&self) -> bool {
        self.composer.is_in_paste_burst()
    }
    fn next_frame_delay(&self) -> Option<Duration> {
        self.timer_remaining(Instant::now())
            .map(|remaining| {
                if remaining > Duration::from_secs(20) {
                    remaining - Duration::from_secs(20)
                } else {
                    remaining.min(Duration::from_secs(1))
                }
            })
            .into_iter()
            .chain(self.composer.footer_flash_delay())
            .min()
    }
}
