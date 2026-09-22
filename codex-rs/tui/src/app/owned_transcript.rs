//! Compose the owned transcript above the composer and route their selection gestures.
//! Reserve a cleared row below activity and previews, immediately above the composer.
//! Slash suggestions overlay already-painted rows so opening or closing them leaves transcript
//! geometry unchanged.
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
    /// Copy draft selections before key shortcuts, or after mouse layout has been refreshed.
    pub(super) fn handle_composer_copy_event(
        &mut self,
        tui: &mut tui::Tui,
        event: &TuiEvent,
        copy: impl FnOnce(&mut tui::Tui, &str) -> Result<crate::clipboard_copy::CopyStatus, String>,
    ) -> bool {
        if tui.is_owned_screen()
            && self.overlay.is_none()
            && !self.transcript_view.has_active_interaction()
            && let Some((char_count, result)) = self
                .chat_widget
                .copy_composer_selection(event, |text| copy(tui, text))
        {
            self.cancel_pending_key_chord();
            self.transcript_view.show_copy_feedback(&result, char_count);
            tui.frame_requester().schedule_frame();
            return true;
        }
        false
    }

    fn sync_owned_transcript(&mut self, width: u16) -> bool {
        let chat_widget = &self.chat_widget;
        let transcript_width = chat_widget.history_wrap_width(width);
        let view = &mut self.transcript_view;
        view.set_keymap_bindings(&self.keymap);
        view.set_presentation(view.is_detailed(), chat_widget.history_render_mode());
        let active_key = chat_widget.active_cell_transcript_key();
        let detailed = view.is_detailed();
        let active_ids = chat_widget.active_activity_ids();
        let expanded = view.sync_live_activity(&self.transcript_cells, active_ids)
            && chat_widget.history_render_mode() == HistoryRenderMode::Rich;
        if detailed {
            view.sync_live_tail(transcript_width, active_key, |width| {
                chat_widget.active_cell_transcript_hyperlink_lines(width)
            })
        } else {
            view.sync_live_activity_tail(transcript_width, active_key, expanded, |width| {
                chat_widget.active_cell_owned_transcript_lines(width, expanded)
            })
        }
    }

    pub(super) fn render_owned_transcript(
        &mut self,
        tui: &mut tui::Tui,
        screen_size: Size,
    ) -> Result<Rect> {
        self.chat_widget.sync_warnings(&self.transcript_cells);
        let motion = MotionMode::from_animations_enabled(
            self.local_settings.tui.animations && self.local_settings.tui.effects.shimmer,
        );
        let focused = tui.is_terminal_focused();
        self.empty_state_presentation(
            MotionMode::from_animations_enabled(
                self.local_settings.tui.animations && self.local_settings.tui.effects.welcome,
            ),
            focused,
        );
        let latest_navigation = if self.enter_returns_to_latest() {
            "enter/esc latest"
        } else {
            "esc latest"
        };
        self.sync_owned_transcript(screen_size.width);
        let transcript_width = self.chat_widget.history_wrap_width(screen_size.width);
        let composer_hint = self.composer_hint(transcript_width);
        let composer_gap = (!self.chat_widget.has_active_view()
            && !self.chat_widget.is_external_writer_view())
        .then(crate::bottom_pane::ComposerGap::default);
        let mut prompt_footer =
            self.prompt_navigation_footer(screen_size.width.saturating_sub(/*rhs*/ 2));
        let chat_widget = &self.chat_widget;
        let view = &mut self.transcript_view;
        let active_key = chat_widget.active_cell_transcript_key();
        view.sync_history_tail(&self.transcript_cells);
        view.prepare_width(transcript_width);
        if view.advance_search(&self.transcript_cells) {
            tui.frame_requester().schedule_frame();
        }
        let footer_area = crate::bottom_pane::inset_footer_hint_area(Rect::new(
            /*x*/ 0,
            /*y*/ 0,
            screen_size.width,
            /*height*/ 1,
        ));
        let footer = view.footer_with_navigation(footer_area.width, motion, latest_navigation);
        let bottom = chat_widget.bottom_pane_renderable(
            prompt_footer.as_ref().or(footer.as_ref()),
            if view.has_active_interaction() {
                crate::bottom_pane::CommandPopupPlacement::Hidden
            } else {
                crate::bottom_pane::CommandPopupPlacement::Overlay
            },
            composer_gap.as_ref(),
        );
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
        drop(bottom);
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
                    available.saturating_sub(u16::from(composer_gap.is_none())),
                ),
                frame.buffer,
                &self.transcript_cells,
            );
            // Rendering resolves whether new activity is still hidden. Paint that result in
            // this frame so a revision change cannot flash a stale activity hint.
            let mut footer =
                view.footer_with_navigation(footer_area.width, motion, latest_navigation);
            if let Some(footer) = prompt_footer
                .as_mut()
                .or(footer.as_mut())
                .filter(|footer| footer.is_interactive)
                && let Some(items) = self
                    .key_chord_matcher
                    .pending_hint_items(&self.keymap.chords, footer_area.width)
                && let Some(progress) = footer.text.lines.last_mut()
            {
                *progress = crate::bottom_pane::footer_hint_items_line(&items);
            }
            let bottom = chat_widget.bottom_pane_renderable(
                prompt_footer.as_ref().or(footer.as_ref()),
                if view.has_active_interaction() {
                    crate::bottom_pane::CommandPopupPlacement::Hidden
                } else {
                    crate::bottom_pane::CommandPopupPlacement::Overlay
                },
                composer_gap.as_ref(),
            );
            footer_height_changed = !dashboard_visible
                && bottom
                    .desired_height(screen_size.width)
                    .min(screen_size.height)
                    != bottom_height;
            bottom.render(bottom_area, frame.buffer);
            let follow_area = if let Some(gap) = composer_gap.as_ref() {
                Some(Rect {
                    width: transcript_width,
                    ..gap.area.get()
                })
            } else {
                (available > 0).then(|| {
                    Rect::new(
                        /*x*/ 0,
                        available - 1,
                        transcript_width,
                        /*height*/ 1,
                    )
                })
            }
            .filter(|_| chat_widget.no_modal_or_popup_active());
            feedback_tick =
                view.render_composer_gap(follow_area, composer_hint.as_ref(), frame.buffer);
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
        if !tui.is_owned_screen()
            || matches!(event, TuiEvent::FocusLost | TuiEvent::Resume)
            || self.overlay.is_some()
        {
            self.chat_widget.end_composer_drag();
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
        if let TuiEvent::Mouse(mouse) = event {
            let composer_ready = self.chat_widget.prepare_composer_mouse(*mouse);
            if mouse.kind != crossterm::event::MouseEventKind::Moved {
                let size = tui.prepare_draw_size()?;
                self.render_owned_transcript(tui, size)?;
            }
            if composer_ready
                && self.handle_composer_copy_event(tui, event, tui::Tui::copy_transcript_selection)
            {
                return Ok(true);
            }
            if composer_ready && self.chat_widget.handle_composer_mouse(*mouse) {
                self.transcript_view.end_selection(&self.transcript_cells);
                self.transcript_view.cancel_search();
                self.transcript_view.clear_activity_focus();
                tui.frame_requester().schedule_frame();
                return Ok(true);
            }
        }
        if !self.chat_widget.no_modal_or_popup_active() {
            self.chat_widget.end_composer_drag();
            return Ok(false);
        }
        if matches!(event, TuiEvent::Key(key) if key.kind != KeyEventKind::Release) {
            let size = tui.prepare_draw_size()?;
            self.render_owned_transcript(tui, size)?;
        }
        if matches!(event, TuiEvent::Key(_))
            || matches!(event, TuiEvent::Mouse(mouse) if matches!(mouse.kind, crossterm::event::MouseEventKind::Down(_)))
        {
            self.chat_widget.end_composer_drag();
        }
        // Read-only Escape belongs to session navigation, even when the transcript is scrolled.
        // Search and selection still consume Escape first to dismiss their interaction.
        if let TuiEvent::Key(key) = event
            && !self.transcript_view.has_active_interaction()
            && self.chat_widget.is_external_writer_view()
            && crate::key_hint::plain(KeyCode::Esc).is_press(*key)
        {
            return Ok(false);
        }
        // Visible shortcut help owns Escape before returning a paused viewport to latest.
        // A transcript search or selection still owns Escape while it replaces the help footer.
        if let TuiEvent::Key(key) = event
            && !self.transcript_view.has_active_interaction()
            && self.chat_widget.shortcut_overlay_visible()
            && crate::key_hint::plain(KeyCode::Esc).is_press(*key)
        {
            return Ok(false);
        }
        if let TuiEvent::Key(key) = event
            && !self.transcript_view.has_active_interaction()
            && self.chat_widget.should_handle_vim_insert_escape(*key)
        {
            return Ok(false);
        }
        // Prompt preview owns its navigation before ordinary Escape-to-latest scrolling.
        // Selection and search retain their existing priority and keep the prompt untouched.
        if self.backtrack.overlay_preview_active
            && !self.transcript_view.has_active_interaction()
            && self.handle_owned_backtrack_event(tui, event)?
        {
            self.request_owned_history(tui, app_server);
            return Ok(true);
        }
        // Capture the origin before the first Escape returns a paused viewport to latest.
        if !self.backtrack.overlay_preview_active
            && !self.transcript_view.has_active_interaction()
            && !self.reconnect.offline
            && let TuiEvent::Key(key) = event
            && key.code == KeyCode::Esc
            && key.modifiers.is_empty()
            && key.kind == KeyEventKind::Press
            && self.should_handle_backtrack_esc(*key)
        {
            if !self.handle_owned_backtrack_event(tui, event)? {
                self.handle_backtrack_esc_key(tui);
                if !self.backtrack.overlay_preview_active {
                    self.transcript_view.jump_to_latest();
                }
            }
            tui.frame_requester().schedule_frame();
            return Ok(true);
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
                    && !matches!(action.action, "find_transcript" | "focus_activity")
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
            TuiEvent::Key(key)
                if !self.transcript_view.is_search_active()
                    && self.keymap.app.find_transcript.is_pressed(*key) =>
            {
                self.transcript_view.begin_search();
                Some(ViewAction::Changed)
            }
            TuiEvent::Key(key) => self
                .transcript_view
                .handle_key(*key, &self.transcript_cells),
            TuiEvent::Mouse(mouse) => self
                .transcript_view
                .handle_mouse(*mouse, &self.transcript_cells),
            TuiEvent::Paste(text) => {
                self.transcript_view.end_selection(&self.transcript_cells);
                if self.transcript_view.paste_search(text) {
                    Some(ViewAction::Changed)
                } else {
                    self.transcript_view.clear_activity_focus();
                    None
                }
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
            if self.reconnect.offline {
                if let TuiEvent::Key(key) = event
                    && self.transcript_view.is_detailed()
                    && self.keymap.pager.close_transcript.is_pressed(*key)
                {
                    self.close_transcript_overlay(tui);
                    return Ok(true);
                }
                return Ok(false);
            }
            return self.handle_owned_backtrack_event(tui, event);
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
        let browsing_needs_history = self.browsing_needs_history();
        let view = &mut self.transcript_view;
        if !browsing_needs_history
            && !view.needs_history(&self.transcript_cells)
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
#[path = "empty_state_animation_tests.rs"]
mod empty_state_animation_tests;

#[cfg(test)]
#[path = "owned_transcript_input_tests.rs"]
mod input_tests;

#[cfg(test)]
#[path = "warning_notice_tests.rs"]
mod warning_notice_tests;

#[cfg(test)]
#[path = "owned_transcript_follow_tests.rs"]
mod follow_tests;
