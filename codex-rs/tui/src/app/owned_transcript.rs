//! Compose the owned transcript above the existing composer and route transcript-only gestures.
//! Reserve a cleared row between transcript content and the composer. Slash suggestions overlay
//! already-painted rows so opening or closing them leaves transcript geometry unchanged.
//! Fresh-thread decoration is painted separately in unused cells and never enters history.
//! Plain Enter returns an empty composer to latest after transcript interactions and prompt editing.

use super::*;
use crate::history_cell::HistoryRenderMode;
use crate::keymap::KeymapContext;
use crate::keymap::bindings_for_action;
use crate::keymap::keymap_action_ids;
use crate::motion::MotionMode;
use crate::pager_overlay::TranscriptHistoryState;
use crate::transcript_view::ViewAction;
use ratatui::widgets::Widget;

impl App {
    fn sync_owned_transcript(&mut self, width: u16) -> bool {
        let chat_widget = &self.chat_widget;
        let transcript_width = chat_widget.history_wrap_width(width);
        let view = &mut self.transcript_view;
        view.set_presentation(view.is_detailed(), chat_widget.history_render_mode());
        let active_key = chat_widget.active_cell_transcript_key();
        let detailed = view.is_detailed();
        view.sync_live_tail(transcript_width, active_key, |width| {
            chat_widget.active_cell_hyperlink_lines_with(width, |cell, width| {
                if detailed {
                    cell.transcript_hyperlink_lines(width)
                } else if chat_widget.history_render_mode() == HistoryRenderMode::Raw {
                    cell.display_hyperlink_lines_for_mode(width, HistoryRenderMode::Raw)
                } else {
                    cell.compact_hyperlink_lines(width)
                }
            })
        })
    }

    pub(super) fn render_owned_transcript(
        &mut self,
        tui: &mut tui::Tui,
        screen_size: Size,
    ) -> Result<Rect> {
        let motion = MotionMode::from_animations_enabled(self.local_settings.tui.animations);
        let latest_navigation = if self.enter_returns_to_latest() {
            "enter/esc latest"
        } else {
            "esc latest"
        };
        self.sync_owned_transcript(screen_size.width);
        let chat_widget = &self.chat_widget;
        let transcript_width = chat_widget.history_wrap_width(screen_size.width);
        let view = &mut self.transcript_view;
        let active_key = chat_widget.active_cell_transcript_key();
        view.sync_history_tail(&self.transcript_cells);
        let footer_area = crate::bottom_pane::inset_footer_hint_area(Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            screen_size.width,
            /*height*/ 1,
        ));
        let footer = view.footer_with_navigation(footer_area.width, motion, latest_navigation);
        let bottom = chat_widget.bottom_pane_renderable(footer.as_ref());
        let dashboard_visible = chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some();
        let bottom_height = if dashboard_visible {
            screen_size.height
        } else {
            bottom
                .desired_height(screen_size.width)
                .min(screen_size.height)
        };
        let available = screen_size.height.saturating_sub(bottom_height);
        let bottom_area = Rect::new(
            /*x*/ 0,
            screen_size.height.saturating_sub(bottom_height),
            screen_size.width,
            bottom_height,
        );
        let mut rendered_cursor = None;
        let mut footer_height_changed = false;
        let mut feedback_tick = None;
        tui.draw(screen_size.height, |frame| {
            ratatui::widgets::Clear.render(
                Rect::new(/*x*/ 0, /*y*/ 0, screen_size.width, available),
                frame.buffer,
            );
            view.render(
                Rect::new(
                    /*x*/ 0,
                    /*y*/ 0,
                    transcript_width,
                    available.saturating_sub(/*rhs*/ 1),
                ),
                frame.buffer,
                &self.transcript_cells,
            );
            let follow_area =
                (available > 0 && chat_widget.no_modal_or_popup_active()).then(|| {
                    Rect::new(
                        /*x*/ 0,
                        available - 1,
                        transcript_width,
                        /*height*/ 1,
                    )
                });
            feedback_tick = view.render_composer_gap(follow_area, /*hint*/ None, frame.buffer);
            // Rendering resolves whether new activity is still hidden. Paint that result in
            // this frame so a revision change cannot flash a stale activity hint.
            let footer = view.footer_with_navigation(footer_area.width, motion, latest_navigation);

            let bottom = chat_widget.bottom_pane_renderable(footer.as_ref());
            footer_height_changed = !dashboard_visible
                && bottom
                    .desired_height(screen_size.width)
                    .min(screen_size.height)
                    != bottom_height;
            bottom.render(bottom_area, frame.buffer);
            chat_widget.note_rendered_width(screen_size.width);
            rendered_cursor = bottom.cursor_pos(bottom_area);
            if let Some(position) = rendered_cursor {
                frame.set_cursor_style(bottom.cursor_style(bottom_area));
                frame.set_cursor_position(position);
            }
        })?;
        if footer_height_changed {
            tui.frame_requester().schedule_frame();
        }
        if let Some(delay) = feedback_tick {
            tui.frame_requester().schedule_frame_in(delay);
        }

        let animating =
            view.is_following() && active_key.is_some_and(|key| key.animation_tick.is_some());
        let loading = view.is_loading_history() && motion == MotionMode::Animated;
        if animating || loading {
            tui.frame_requester()
                .schedule_frame_in(Duration::from_millis(/*millis*/ 50));
        }
        Ok(bottom_area)
    }

    /// Consume transcript gestures only when a modal or another overlay does not own input.
    pub(super) fn handle_owned_transcript_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        event: &TuiEvent,
    ) -> Result<bool> {
        if !tui.is_owned_screen()
            || matches!(event, TuiEvent::FocusLost | TuiEvent::Resume)
            || self.overlay.is_some()
            || !self.chat_widget.no_modal_or_popup_active()
        {
            self.transcript_view.end_drag();
        }
        if !tui.is_owned_screen() || self.overlay.is_some() {
            return Ok(false);
        }
        if matches!(event, TuiEvent::FocusLost) {
            // Show the static, faded decoration immediately when the terminal loses focus.
            tui.frame_requester().schedule_frame();
        }
        if self.transcript_view.history == TranscriptHistoryState::Idle {
            self.transcript_view.history = if self.scrollback_has_older_history {
                TranscriptHistoryState::Partial
            } else {
                TranscriptHistoryState::Complete
            };
        }
        if matches!(event, TuiEvent::Draw) {
            if self.transcript_view.tick_selection(&self.transcript_cells) {
                tui.frame_requester()
                    .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
            }
            return Ok(false);
        }
        if !self.chat_widget.no_modal_or_popup_active() {
            return Ok(false);
        }
        let input = matches!(event, TuiEvent::Key(key) if key.kind != KeyEventKind::Release)
            || matches!(event, TuiEvent::Mouse(mouse) if mouse.kind != crossterm::event::MouseEventKind::Moved);
        if input {
            let size = tui.prepare_draw_size()?;
            self.render_owned_transcript(tui, size)?;
        }
        if !self.transcript_view.has_active_interaction()
            && self.chat_widget.shortcut_overlay_visible()
            && matches!(event, TuiEvent::Key(key) if key.code == KeyCode::Esc)
        {
            return Ok(false);
        }
        if let TuiEvent::Key(key) = event
            && !self.transcript_view.has_active_interaction()
            && self.chat_widget.should_handle_vim_insert_escape(*key)
        {
            return Ok(false);
        }
        if let TuiEvent::Key(key) = event
            && !self.transcript_view.has_active_interaction()
            && !self.backtrack.overlay_preview_active
            && self.transcript_view.is_detailed()
            && self.keymap.pager.close_transcript.is_pressed(*key)
        {
            self.close_transcript_overlay(tui);
            return Ok(true);
        }
        if let TuiEvent::Key(key) = event
            && ((key.modifiers.is_empty()
                && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown))
                || crate::transcript_view::JumpTarget::from_key(*key).is_some())
            && !self.transcript_view.has_active_interaction()
            && keymap_action_ids().any(|action| {
                action.context != KeymapContext::Pager
                    && self.active_keymap_contexts().contains_action(action)
                    && bindings_for_action(
                        &self.keymap,
                        action.context.config_name(),
                        action.action,
                    )
                    .is_some_and(|bindings| bindings.is_pressed(*key))
            })
        {
            return Ok(false);
        }
        let action = match event {
            TuiEvent::Key(key) => self
                .transcript_view
                .handle_key(*key, &self.transcript_cells),
            TuiEvent::Mouse(mouse) => self
                .transcript_view
                .handle_mouse(*mouse, &self.transcript_cells),
            TuiEvent::Paste(_) => {
                self.transcript_view.end_selection(&self.transcript_cells);
                None
            }
            TuiEvent::Draw
            | TuiEvent::Resume
            | TuiEvent::Resize(_)
            | TuiEvent::FocusGained
            | TuiEvent::FocusLost => None,
        };
        let Some(action) = action else {
            if matches!(
                event,
                TuiEvent::Key(KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                })
            ) && self.enter_returns_to_latest()
                && self.transcript_view.can_return_to_latest()
            {
                self.transcript_view.jump_to_latest();
                tui.frame_requester().schedule_frame();
                return Ok(true);
            }
            return Ok(false);
        };
        let resume_following = matches!(action, ViewAction::CopyAndFollow(_));
        match action {
            ViewAction::Changed => {}
            ViewAction::Copy(text) | ViewAction::CopyAndFollow(text) => {
                let result = self.transcript_view.copy_selected_text_with(
                    &self.transcript_cells,
                    &text,
                    |text| tui.copy_transcript_selection(text),
                );
                self.transcript_view
                    .show_copy_feedback(&result, text.chars().count());
                if resume_following
                    && matches!(result, Ok(crate::clipboard_copy::CopyStatus::Confirmed))
                {
                    if self.backtrack.overlay_preview_active {
                        self.close_transcript_overlay(tui);
                    }
                    self.transcript_view.jump_to_latest();
                }
            }
            ViewAction::OpenLink(url) => self.open_url_in_browser(url),
        }
        self.request_owned_history(tui, app_server);
        tui.frame_requester().schedule_frame();
        Ok(true)
    }

    /// Keep the navigation hint and input guard aligned, including pending paste and attachments.
    fn enter_returns_to_latest(&self) -> bool {
        self.chat_widget.composer_is_empty()
            && self.chat_widget.no_modal_or_popup_active()
            && !self.backtrack.primed
            && !self.backtrack.overlay_preview_active
    }

    pub(super) fn request_owned_history(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
    ) {
        if !self.scrollback_has_older_history {
            return;
        }
        let view = &mut self.transcript_view;
        if !view.needs_history(&self.transcript_cells)
            && view.history != TranscriptHistoryState::LoadingBeginning
        {
            return;
        }
        if let Some(thread_id) = self.chat_widget.thread_id()
            && app_server.has_older_history(thread_id)
            && self.request_older_history_page(app_server, thread_id)
        {
            if self.transcript_view.history != TranscriptHistoryState::LoadingBeginning {
                self.transcript_view.history = TranscriptHistoryState::LoadingOlder;
            }
            tui.frame_requester().schedule_frame();
        }
    }
}

#[cfg(test)]
#[path = "owned_transcript_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "owned_transcript_follow_tests.rs"]
mod follow_tests;

#[cfg(test)]
#[path = "owned_transcript_input_tests.rs"]
mod input_tests;
