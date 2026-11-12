//! Compose the owned transcript above the composer and route their selection gestures.
//! A docked side conversation paints the captured neighbor before the centered dialog.
//! Painting records the screen regions used by selection and pointer input.
//! Input that depends on those regions refreshes the layout before using them.
//! Reserve a cleared row below activity and previews, immediately above the composer.
//! Slash suggestions overlay already-painted rows so opening or closing them leaves transcript
//! geometry unchanged. Turn tips remain outside transcript history; their painted rows are selectable.
//! Plain Enter returns an empty composer to latest after transcript interactions and prompt editing.

use super::sidebar::PaneFocus;
use super::turn_tips::TipSurface;
use super::*;
use crate::bottom_pane::CommandPopupPlacement;
use crate::bottom_pane::ComposerRenderOptions;
use crate::custom_terminal::Frame;
use crate::empty_state_animation::ComposerState;
#[cfg(test)]
use crate::history_cell::HistoryRenderMode;
use crate::keymap::KeymapContext;
use crate::keymap::bindings_for_action;
use crate::keymap::configured_binding_for_action;
use crate::keymap::keymap_action_ids;
use crate::motion::MotionMode;
use crate::pager_overlay::TranscriptHistoryState;
use crate::sidebar_tab_strip::TabState;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::transcript_image::ImagePlacement;
use crate::transcript_view::TranscriptView;
use crate::transcript_view::ViewAction;
use crate::turn_tip::TurnTip;
use crossterm::event::MouseEventKind;
use ratatui::widgets::Widget;

/// Remember the last conversation bounds and neighboring content for input-triggered redraws.
#[derive(Default)]
pub(super) struct ConversationFrame {
    inset: Option<Rect>,
    pub(super) sidebar: Option<sidebar::SidebarLayout>,
    pub(super) chat_strip: Option<Rect>,
    pub(super) sidebar_input: Option<sidebar::SidebarInputState>,
    pub(super) close_buttons: [Rect; 2],
    /// A dialog can obscure neighboring content in the terminal's completed buffer.
    behind_dialog: Option<ratatui::buffer::Buffer>,
    caller_images: Vec<ImagePlacement>,
}

impl ConversationFrame {
    pub(super) fn has_caller_inset(&self) -> bool {
        self.inset.is_some() && self.sidebar.is_none() && self.chat_strip.is_none()
    }

    pub(super) fn side_pane(&self) -> Option<&side_pane::SidePaneLayout> {
        match &self.sidebar {
            Some(sidebar::SidebarLayout::Chat(panes)) => Some(panes),
            _ => None,
        }
    }

    pub(super) fn chooser_covers_transcript(&self) -> bool {
        matches!(self.sidebar, Some(sidebar::SidebarLayout::Chooser { main, .. }) if main.is_empty())
    }

    pub(super) fn chooser_area(&self) -> Option<Rect> {
        match self.sidebar {
            Some(sidebar::SidebarLayout::Chooser { sidebar, .. }) => Some(sidebar),
            _ => None,
        }
    }

    pub(super) fn sidebar_tab_strip(&self) -> Option<Rect> {
        self.chat_strip.or_else(|| {
            self.chooser_area()
                .map(|area| side_pane::tab_and_body(area)[0])
        })
    }

    pub(super) fn divider_area(&self) -> Option<Rect> {
        self.sidebar.as_ref()?.divider_area()
    }
}

pub(super) struct PreparedConversation {
    floating_shortcuts: bool,
    area: Rect,
    transcript_width: u16,
    composer: Option<ComposerState>,
    composer_hint: Option<HyperlinkLine>,
    latest_navigation: &'static str,
    motion: MotionMode,
    now: Instant,
    turn_tip: Option<(TipSurface, TurnTip)>,
}

/// The composer bounds and follow-up work from a successful draw.
/// Acknowledge a tip only if it was visible in the frame that was presented.
#[derive(Default)]
pub(super) struct PaintedConversation {
    image_range: std::ops::Range<usize>,
    bottom_area: Rect,
    shown_tip: Option<TipSurface>,
    needs_redraw: bool,
    /// Delays requested by copy feedback, the empty-state animation, and the transcript.
    delayed_frames: [Option<Duration>; 3],
}

impl PreparedConversation {
    /// Paint the conversation foreground before a centered dialog takes over the cursor.
    fn paint_foreground(
        &self,
        frame: &mut Frame<'_>,
        chat_widget: &ChatWidget,
        view: &TranscriptView,
        bottom: &dyn Renderable,
        bottom_area: Rect,
    ) {
        {
            let mut selection = chat_widget.rendered_selection.borrow_mut();
            selection.register_layout(chat_widget.take_warning_body());
            if let Some((_, tip)) = &self.turn_tip
                && let Some(area) = tip.rendered.get()
            {
                selection.register(area, /*invalidate*/ false);
            }
            selection.render(frame.buffer);
        }
        chat_widget.note_rendered_width(self.area.width);
        if let Some(position) = bottom.cursor_pos(bottom_area)
            && self.area.contains(position.into())
        {
            frame.set_cursor_style(bottom.cursor_style(bottom_area));
            frame.set_cursor_position(position);
        }
        if chat_widget.no_modal_or_popup_active() {
            frame.image_placements.extend(view.image_placements());
            if let Some(previews) = &chat_widget.composer_images {
                frame.image_placements.extend(previews.placements.take());
            }
        }
    }
}

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

    /// Publish a completed composer selection with the transcript/footer copy policy.
    fn handle_composer_selection_release(
        &mut self,
        tui: &mut tui::Tui,
        event: crossterm::event::MouseEvent,
    ) -> bool {
        let selection_copy = crate::tui::SelectionCopy::on_select(
            self.local_settings
                .copy_on_select(&codex_terminal_detection::terminal_info()),
            self.right_click_paste_environment.primary,
            // Editable selections remain visible so the next input can replace them.
            /*clear_selection*/
            false,
        );
        if !selection_copy.copies_clipboard() && !selection_copy.publishes_primary() {
            return false;
        }
        let Some((char_count, result)) = self.chat_widget.copy_composer_selection_on_release(
            event,
            selection_copy,
            |text, selection_copy| {
                tui.copy_selection(
                    text,
                    crate::clipboard_copy::CopyFormat::PlainText,
                    selection_copy,
                    text,
                )
            },
        ) else {
            return false;
        };
        if let Some(result) = result {
            self.transcript_view.show_copy_feedback(&result, char_count);
        }
        true
    }

    pub(super) fn sync_owned_transcript(&mut self, width: u16) -> bool {
        let chat_widget = &self.chat_widget;
        let transcript_width = chat_widget.history_wrap_width(width);
        let view = &mut self.transcript_view;
        view.mouse_scroll_speed = self.local_settings.tui.mouse_scroll_speed.unwrap_or(1.0);
        view.primary_selection = self.right_click_paste_environment.primary;
        view.copy_on_select = self
            .local_settings
            .copy_on_select(&codex_terminal_detection::terminal_info());
        view.copy_on_select_clear_selection =
            self.local_settings.tui.copy_on_select_clear_selection;
        view.set_keymap_bindings(&self.keymap);
        view.set_presentation(view.is_detailed(), chat_widget.history_render_mode());
        let active = chat_widget.active_activity_cell();
        let (live_summary, expanded) = view.sync_live_tool_summary(
            &self.transcript_cells,
            self.local_settings.tui.collapse_turn_details,
            active,
        );
        let active_key = chat_widget.active_cell_transcript_key();
        let detailed = view.is_detailed();
        let search_changed = view.sync_search_live_tail(transcript_width, active_key, |width| {
            chat_widget.active_cell_transcript_hyperlink_lines(width)
        });
        let changed = if detailed {
            view.sync_live_tail(transcript_width, active_key, |width| {
                chat_widget.active_cell_transcript_hyperlink_lines(width)
            })
        } else if let Some(row) = live_summary {
            view.sync_live_layout(transcript_width, active_key, |width| {
                Some(row.layout(width, expanded, chat_widget.active_auxiliary_lines(width)))
            })
        } else {
            view.sync_live_activity_tail(transcript_width, active_key, expanded, |width| {
                chat_widget.active_cell_owned_transcript_lines(width, expanded)
            })
        };
        changed || search_changed
    }

    pub(super) fn render_owned_transcript(
        &mut self,
        tui: &mut tui::Tui,
        screen_size: Size,
    ) -> Result<Rect> {
        self.transcript_view.image_protocol = self.prepare_frame_images();
        let frames = tui.frame_requester();
        if self.conversation_frame.sidebar.is_some() || self.conversation_frame.chat_strip.is_some()
        {
            // Recompute our own inset on every frame, including the first narrow frame.
            self.conversation_frame = ConversationFrame::default();
        }
        let caller_inset = self.conversation_frame.inset;
        let parent = self.sidebar_parent();
        if !self.conversation_focus_available(tui)
            && let Some(pane) = self.sidebars.get_mut(&parent)
        {
            pane.close = None;
        }
        let layout = sidebar::paint_layout(self, tui, screen_size.into());
        let displayed = self.current_displayed_thread_id();
        let chat = match displayed.and_then(|id| self.side_pair_key(id)) {
            Some(child) if Some(child) == displayed => TabState::Active,
            Some(_) => TabState::Inactive,
            None => TabState::Absent,
        };
        let (live, strip) = match &layout {
            Some(sidebar::SidebarLayout::Chat(panes)) => {
                (Some(panes.focused), Some(panes.tab_strip))
            }
            Some(sidebar::SidebarLayout::Chooser { main, .. }) => {
                ((!main.is_empty()).then_some(*main), None)
            }
            None if chat == TabState::Active
                && caller_inset.is_none()
                && self.side_retention_enabled(tui) =>
            {
                let [strip, body] = side_pane::tab_and_body(screen_size.into());
                (Some(body), Some(strip))
            }
            None => (None, None),
        };
        let mut prepared =
            self.prepare_conversation(screen_size.into(), caller_inset.or(live), &frames);
        self.conversation_frame.sidebar = layout;
        self.conversation_frame.chat_strip = strip;
        self.conversation_frame.sidebar_input =
            self.sidebar().map(sidebar::SidebarPane::input_state);
        let divider = self.conversation_frame.divider_area();
        if self.divider_drag.is_some_and(|drag| {
            divider.is_none()
                || !drag.matches(screen_size.into(), self)
                || !self.conversation_focus_available(tui)
        }) {
            self.divider_drag = None;
        }
        let highlight_divider =
            self.pane_resize.is_some() || tui.link_hover.shape == tui::PointerShape::Resize;
        let hovered_target = self.hovered_sidebar_target(tui.link_hover.position());
        // An input refresh opens a fresh terminal buffer. Carry the outer owner's
        // completed cells through it when only the conversation is being redrawn.
        let backdrop = caller_inset.map(|_| {
            self.conversation_frame
                .behind_dialog
                .take()
                .unwrap_or_else(|| tui.terminal.previous_buffer().clone())
        });
        let mut hud = self.workspace_hud(tui, screen_size.into());
        if let Some(hud) = &mut hud {
            hud.viewport = caller_inset;
        }
        prepared.floating_shortcuts = hud.is_some();
        let mut painted = PaintedConversation::default();
        tui.draw_with_shortcut_hud(screen_size.height, hud, |frame| {
            if let Some(backdrop) = backdrop {
                let viewport = frame.area();
                frame.image_placements.extend(
                    self.conversation_frame
                        .caller_images
                        .drain(..)
                        .filter(|placement| {
                            viewport.intersection(placement.area) == placement.area
                        }),
                );
                for (pos, cell) in backdrop.area.positions().zip(backdrop.content) {
                    if viewport.contains(pos) && !prepared.area.contains(pos) {
                        frame.buffer[pos] = cell;
                    }
                }
            }
            painted = self.paint_conversation(frame, &prepared);
            match &self.conversation_frame.sidebar {
                Some(sidebar::SidebarLayout::Chat(panes)) => {
                    if let Some(companion) = self.side_companions.get_mut(&panes.passive_thread) {
                        companion.view.image_protocol = self.transcript_view.image_protocol;
                        panes.paint_companion(
                            companion,
                            frame,
                            self.local_settings.tui.collapse_turn_details,
                            &frames,
                        );
                    }
                }
                Some(sidebar::SidebarLayout::Chooser { sidebar, focus, .. }) => {
                    let focused = *focus == PaneFocus::Sidebar;
                    let resize_hint = self.pane_resize.map(|_| side_pane::RESIZE_HINT);
                    if let Some(creation) = self.sidebar().and_then(|pane| pane.creation.as_ref()) {
                        use sidebar::CreationState;
                        ratatui::widgets::Clear.render(*sidebar, frame.buffer);
                        let mut body = sidebar.inner(ratatui::layout::Margin::new(1, 1));
                        if let Some(hint) = resize_hint {
                            let footer = Rect::new(
                                body.x,
                                body.bottom().saturating_sub(1),
                                body.width,
                                body.height.min(1),
                            );
                            crate::sidebar_chooser::resize_hint(hint, footer.width)
                                .render(footer, frame.buffer);
                            body.height = body.height.saturating_sub(1);
                        }
                        let text = match &creation.state {
                            CreationState::Waiting { started, .. } => {
                                let mode = MotionMode::from_animations_enabled(
                                    self.local_settings.tui.animations,
                                );
                                let glyph = crate::motion::loading_glyph(*started, mode, &frames);
                                format!("{glyph} Starting side chat…")
                            }
                            CreationState::Failed { message, .. } => {
                                if resize_hint.is_some() {
                                    message.clone()
                                } else {
                                    crate::sidebar_chooser::creation_failure_text(
                                        message,
                                        &self.keymap,
                                    )
                                }
                            }
                        };
                        ratatui::widgets::Paragraph::new(text)
                            .wrap(ratatui::widgets::Wrap { trim: false })
                            .render(body, frame.buffer);
                    } else if let Some(pane) = self.sidebar() {
                        let footer = if let Some(hint) = resize_hint {
                            let row = crate::sidebar_chooser::body_row(*sidebar, sidebar.y);
                            crate::sidebar_chooser::resize_hint(hint, row.width)
                        } else {
                            crate::sidebar_chooser::footer(
                                &self.keymap,
                                *sidebar,
                                pane.choice,
                                pane.chat_tabs(),
                                self.chat_widget.side_chat_creation_state(),
                            )
                            .0
                            .line
                        };
                        crate::sidebar_chooser::paint(
                            focused,
                            *sidebar,
                            frame.buffer,
                            footer,
                            pane.chat_tabs(),
                            pane.choice,
                            &crate::sidebar_chooser::SidebarStyle {
                                new_chat: self.chat_widget.side_chat_creation_state(),
                                use_theme_colors: self.local_settings.tui.status_line_use_colors,
                                motion: MotionMode::from_animations_enabled(
                                    self.local_settings.tui.animations,
                                ),
                                frames: &frames,
                            },
                        );
                        let chooser = if focused {
                            TabState::Active
                        } else {
                            TabState::Inactive
                        };
                        crate::sidebar_chooser::tab_strip(
                            *sidebar,
                            chooser,
                            TabState::Absent,
                            pane.chat_tabs(),
                            hovered_target,
                            frame.buffer,
                        );
                    }
                    frame
                        .image_placements
                        .retain(|image| !sidebar.intersects(image.area));
                    if focused {
                        frame.cursor_position = None;
                    }
                }
                None => {}
            }
            if let Some(strip) = strip
                && let Some(pane) = self.sidebar()
            {
                crate::sidebar_chooser::tab_strip(
                    strip,
                    TabState::Absent,
                    chat,
                    pane.chat_tabs(),
                    hovered_target,
                    frame.buffer,
                );
            }
            if let Some(divider) = divider {
                side_pane::paint_divider(frame, divider.x, self.divider_drag, highlight_divider);
            }
            self.conversation_frame.close_buttons = Default::default();
            if let Some(close) = self.sidebar().and_then(|pane| pane.close.as_ref()) {
                // A resize can collapse an unfocused sidebar while its confirmation is open.
                let viewport = frame.area();
                let column = self
                    .conversation_frame
                    .chooser_area()
                    .or(strip)
                    .unwrap_or(viewport);
                let area = Rect {
                    x: column.x,
                    width: column.width,
                    ..viewport
                };
                let buttons = close.paint(area, frame.buffer);
                frame
                    .image_placements
                    .retain(|image| !area.intersects(image.area));
                frame.cursor_position = None;
                self.conversation_frame.close_buttons = buttons;
            }
            self.paint_centered_dialog(frame);
        })?;
        self.finish_conversation(tui, painted)
    }

    pub(super) fn prepare_conversation(
        &mut self,
        viewport: Rect,
        conversation_inset: Option<Rect>,
        frames: &tui::FrameRequester,
    ) -> PreparedConversation {
        self.conversation_frame.inset = conversation_inset;
        self.conversation_frame.sidebar = None;
        self.conversation_frame.chat_strip = None;
        let area = self.conversation_area(viewport);
        if let Some((previous, reflowed)) = self.chat_widget.resize_owned_conversation(area.width) {
            self.transcript_view
                .rebind_observed_live_revision(previous, reflowed);
        }
        self.chat_widget.sync_warnings(&self.transcript_cells);
        let motion = MotionMode::from_animations_enabled(
            self.local_settings.tui.animations && self.local_settings.tui.effects.shimmer,
        );
        let composer = self.first_screen_composer();
        let latest_navigation = if self.enter_returns_to_latest() {
            "enter/esc latest"
        } else {
            "esc latest"
        };
        self.sync_owned_transcript(area.width);
        let transcript_width = self
            .chat_widget
            .history_wrap_width(area.width)
            .min(area.width);
        let composer_hint = if self
            .chat_widget
            .rendered_selection
            .borrow()
            .view
            .has_active_interaction()
        {
            Some(
                ratatui::text::Line::from("Selecting text · esc cancel")
                    .dim()
                    .into(),
            )
        } else {
            self.composer_hint(transcript_width)
        };
        let now = Instant::now();
        let turn_tip = self.turn_tip(transcript_width, now, frames);
        let view = &mut self.transcript_view;
        view.sync_history_tail(&self.transcript_cells);
        view.prepare_width(transcript_width);
        if view.advance_search(&self.transcript_cells) {
            frames.schedule_frame();
        }
        PreparedConversation {
            floating_shortcuts: false,
            area,
            transcript_width,
            composer,
            composer_hint,
            latest_navigation,
            motion,
            now,
            turn_tip,
        }
    }

    pub(super) fn paint_conversation(
        &mut self,
        frame: &mut Frame<'_>,
        prepared: &PreparedConversation,
    ) -> PaintedConversation {
        let PreparedConversation {
            area,
            transcript_width,
            composer,
            latest_navigation,
            motion,
            now,
            ..
        } = *prepared;
        let (working_tip, completion_tip) = match prepared.turn_tip.as_ref() {
            Some((TipSurface::Working, tip)) => (Some(tip), None),
            Some((TipSurface::Completion, tip)) => (None, Some(tip)),
            None => (None, None),
        };
        let mut composer_gap = (!self.chat_widget.has_active_view()
            && !self.chat_widget.is_external_writer_view())
        .then(crate::bottom_pane::ComposerGap::default);
        let prompt_footer = if self.pane_resize.is_some() {
            Some(crate::bottom_pane::TranscriptFooter {
                text: side_pane::RESIZE_HINT.into(),
                cursor_column: None,
                is_interactive: true,
            })
        } else {
            self.prompt_navigation_footer(area.width.saturating_sub(/*rhs*/ 2))
        };
        let chat_widget = &self.chat_widget;
        let view = &mut self.transcript_view;
        let active_key = chat_widget.active_cell_transcript_key();
        let footer_width = crate::bottom_pane::inset_footer_hint_area(area).width;
        // Measure the composer to find the space available for transcript rows.
        // Painting those rows tells us whether newer activity is still hidden,
        // which can change the footer hint and the composer's measured height.
        let footer = view.footer_with_navigation(footer_width, motion, latest_navigation);
        let max_composer_height = side_pane::composer_height_limit(area.height);
        let unfocused = matches!(
            self.conversation_frame.sidebar,
            Some(sidebar::SidebarLayout::Chooser {
                focus: PaneFocus::Sidebar,
                ..
            })
        );
        let options = |interacting| ComposerRenderOptions {
            unfocused,
            max_height: Some(max_composer_height),
            overlay_top: area.top(),
            floating_shortcuts: prepared.floating_shortcuts,
            command_popup_placement: if interacting || unfocused {
                CommandPopupPlacement::Hidden
            } else {
                CommandPopupPlacement::Overlay
            },
            working_tip,
            ..Default::default()
        };
        let bottom = chat_widget.bottom_pane_renderable(ComposerRenderOptions {
            footer: prompt_footer.as_ref().or(footer.as_ref()),
            composer_gap: composer_gap.as_ref(),
            ..options(view.has_active_interaction())
        });
        let agents_overview_present = chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some();
        let measure_bottom = |bottom: &dyn Renderable| {
            if agents_overview_present {
                area.height
            } else {
                bottom.desired_height(area.width).min(area.height)
            }
        };
        let bottom_height = measure_bottom(&bottom);
        drop(bottom);
        let rows_above_composer = area.height - bottom_height;
        let mut bottom_area = Rect::new(
            area.x,
            area.bottom().saturating_sub(bottom_height),
            area.width,
            bottom_height,
        );
        let mut transcript_area = Rect::new(
            area.x,
            area.y,
            transcript_width,
            rows_above_composer.saturating_sub(u16::from(composer_gap.is_none())),
        );
        chat_widget.rendered_selection.borrow_mut().areas.clear();
        ratatui::widgets::Clear.render(area, frame.buffer);
        let mut completion_tip_area = view.render_with_turn_tip_space(
            transcript_area,
            frame.buffer,
            &self.transcript_cells,
            completion_tip,
        );
        if let Some(gap) = composer_gap.as_mut() {
            gap.needs_separator = rows_above_composer > 1
                // Keep the tip under the pointer when selection hints or copy feedback appear.
                && prepared.turn_tip.is_none()
                && chat_widget.no_modal_or_popup_active()
                && view.composer_gap_has_content(transcript_width, prepared.composer_hint.as_ref(), now);
        }
        // Refresh the hint using the rows just painted.
        let footer = prompt_footer
            .or_else(|| view.footer_with_navigation(footer_width, motion, latest_navigation));
        let bottom = chat_widget.bottom_pane_renderable(ComposerRenderOptions {
            footer: footer.as_ref(),
            composer_gap: composer_gap.as_ref(),
            ..options(view.has_active_interaction())
        });
        let updated_bottom_height = measure_bottom(&bottom);
        let mut needs_redraw = updated_bottom_height != bottom_height;
        if needs_redraw && composer_gap.as_ref().is_some_and(|gap| gap.needs_separator) {
            bottom_area.height = updated_bottom_height;
            bottom_area.y = area.bottom().saturating_sub(bottom_area.height);
            // If the separator changes the composer height, repaint the transcript now
            // so its follow control lines up with the gap drawn below. Other height
            // changes request another frame after this draw.
            ratatui::widgets::Clear.render(bottom_area, frame.buffer);
            transcript_area.height = bottom_area.y - area.y;
            completion_tip_area = view.render_with_turn_tip_space(
                transcript_area,
                frame.buffer,
                &self.transcript_cells,
                completion_tip,
            );
            needs_redraw = false;
        }
        let blossom_tick = chat_widget
            .empty_state_animation
            .borrow_mut()
            .render_first_screen(
                area,
                Rect {
                    width: area.width,
                    ..view.remaining_area()
                },
                frame.buffer,
                composer,
                MotionMode::from_animations_enabled(self.local_settings.tui.animations),
            );
        bottom.render(bottom_area, frame.buffer);
        if let (Some(tip), Some(area)) = (completion_tip, completion_tip_area) {
            tip.render(area, frame.buffer);
        }
        let follow_area = if let Some(gap) = composer_gap.as_ref() {
            Some(Rect {
                width: transcript_width,
                ..gap.area.get()
            })
        } else {
            (bottom_area.y > area.y).then(|| {
                Rect::new(
                    area.x,
                    bottom_area.y - 1,
                    transcript_width,
                    /*height*/ 1,
                )
            })
        }
        .filter(|_| chat_widget.no_modal_or_popup_active());
        let feedback_tick = view.render_composer_gap(
            follow_area,
            prepared.composer_hint.as_ref(),
            frame.buffer,
            now,
        );
        let first_image = frame.image_placements.len();
        prepared.paint_foreground(frame, chat_widget, view, &bottom, bottom_area);
        let animating =
            view.is_following() && active_key.is_some_and(|key| key.animation_tick.is_some());
        let loading = view.is_loading_history() && motion == MotionMode::Animated;
        PaintedConversation {
            image_range: first_image..frame.image_placements.len(),
            bottom_area,
            shown_tip: prepared
                .turn_tip
                .as_ref()
                .and_then(|(surface, tip)| tip.rendered.get().map(|_| *surface)),
            needs_redraw,
            delayed_frames: [
                feedback_tick,
                blossom_tick,
                (animating || loading).then_some(Duration::from_millis(50)),
            ],
        }
    }

    /// Acknowledge only a presented frame. Keep neighboring images for the next input refresh.
    pub(super) fn finish_conversation(
        &mut self,
        tui: &mut tui::Tui,
        painted: PaintedConversation,
    ) -> Result<Rect> {
        if self.conversation_frame.behind_dialog.is_none() {
            self.conversation_frame.caller_images.clear();
            if self.conversation_frame.has_caller_inset() {
                self.conversation_frame.caller_images.extend(
                    tui.retained_images
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !painted.image_range.contains(index))
                        .map(|(_, placement)| placement.registration()),
                );
            }
        }
        if let Some(surface) = painted.shown_tip {
            self.turn_tips.acknowledge(surface);
        }
        let frames = tui.frame_requester();
        if painted.needs_redraw {
            frames.schedule_frame();
        }
        for delay in painted.delayed_frames.into_iter().flatten() {
            frames.schedule_frame_in(delay);
        }
        self.refresh_link_hover(tui)?;
        Ok(painted.bottom_area)
    }

    pub(super) fn paint_centered_dialog(&mut self, frame: &mut Frame<'_>) {
        self.conversation_frame.behind_dialog = None;
        let Some(dialog) = self.chat_widget.centered_dialog() else {
            return;
        };
        let area = frame.area();
        if self.conversation_frame.sidebar.is_some() {
            let covered = dialog.area(area);
            frame
                .image_placements
                .retain(|placement| !covered.intersects(placement.area));
        } else if self.conversation_frame.inset.is_some() {
            self.conversation_frame.behind_dialog = Some(frame.buffer.clone());
            // The conversation omits its own images while a dialog is open. Keep the
            // caller's registrations so dismissal can restore graphics behind the dialog.
            self.conversation_frame.caller_images = std::mem::take(&mut frame.image_placements);
            let covered = dialog.area(area);
            frame.image_placements.extend(
                self.conversation_frame
                    .caller_images
                    .iter()
                    .filter(|placement| !covered.intersects(placement.area))
                    .map(ImagePlacement::registration),
            );
        }
        dialog.render(area, frame.buffer);
        frame.cursor_position = dialog.cursor_pos(area).map(Into::into);
        frame.set_cursor_style(dialog.cursor_style(area));
    }

    pub(super) fn conversation_area(&self, viewport: Rect) -> Rect {
        self.conversation_frame
            .inset
            .map_or(viewport, |area| area.intersection(viewport))
    }

    pub(super) fn refresh_owned_transcript(&mut self, tui: &mut tui::Tui) -> Result<()> {
        let size = tui.prepare_draw_size()?;
        let rendered_area = self.render_owned_transcript(tui, size)?;
        self.draw_pet_images(tui, size, rendered_area)
    }

    /// Keep modal input ownership while allowing selection, copying, and disclosure in the visible transcript.
    pub(super) fn handle_owned_transcript_event(
        &mut self,
        tui: &mut tui::Tui,
        app_server: &mut AppServerSession,
        event: &TuiEvent,
    ) -> Result<bool> {
        let has_modal = self.chat_widget.has_active_modal();
        let is_vertical_wheel = matches!(event, TuiEvent::Mouse(mouse)
            if matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown));
        let modal_transcript_mouse = has_modal
            && matches!(event, TuiEvent::Mouse(_))
            && (self.chat_widget.centered_dialog().is_none() || is_vertical_wheel);
        let modal_transcript_draw = has_modal
            && self.chat_widget.centered_dialog().is_none()
            && matches!(event, TuiEvent::Draw);
        let modal_transcript_event = modal_transcript_mouse
            || modal_transcript_draw
            || (has_modal
                && matches!(event, TuiEvent::Key(key)
                    if crate::text_selection::is_copy_key(*key)
                        && self.transcript_view.owns_interaction_key(*key)));
        if !tui.is_owned_screen()
            || matches!(event, TuiEvent::FocusLost | TuiEvent::Resume)
            || self.overlay.is_some()
            || (!self.chat_widget.no_modal_or_popup_active() && !modal_transcript_event)
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
            self.chat_widget
                .empty_state_animation
                .borrow_mut()
                .cancel_replay();
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
                self.refresh_owned_transcript(tui)?;
            }
            if self
                .conversation_frame
                .chooser_area()
                .is_some_and(|area| area.contains((mouse.column, mouse.row).into()))
            {
                return Ok(true);
            }
            if (is_vertical_wheel || mouse.kind == MouseEventKind::Down(MouseButton::Right))
                && self.chat_widget.no_modal_or_popup_active()
                && let Some(panes) = self.conversation_frame.side_pane()
                && panes.passive.contains((mouse.column, mouse.row).into())
            {
                let id = panes.passive_thread;
                let handled = self.handle_passive_transcript_event(tui, id, event);
                if mouse.kind == MouseEventKind::ScrollUp
                    && app_server.has_older_history(id)
                    && self
                        .side_companions
                        .get_mut(&id)
                        .is_some_and(|pane| pane.view.needs_history(&pane.cells))
                    && self.request_older_history_page(app_server, id)
                    && let Some(pane) = self.side_companions.get_mut(&id)
                {
                    pane.view.history = TranscriptHistoryState::LoadingOlder;
                }
                if handled || is_vertical_wheel {
                    return Ok(true);
                }
            }
            if self.chat_widget.no_modal_or_popup_active()
                && self
                    .chat_widget
                    .empty_state_animation
                    .borrow_mut()
                    .handle_mouse(*mouse)
            {
                tui.frame_requester().schedule_frame();
                return Ok(true);
            }
            if composer_ready
                && self.handle_composer_copy_event(tui, event, |tui, text| {
                    tui.copy_transcript_selection(
                        text,
                        crate::clipboard_copy::CopyFormat::PlainText,
                    )
                })
            {
                return Ok(true);
            }
            if composer_ready && self.chat_widget.handle_composer_mouse(*mouse) {
                self.handle_composer_selection_release(tui, *mouse);
                if !is_vertical_wheel {
                    self.transcript_view.end_selection(&self.transcript_cells);
                    self.transcript_view.end_select_mode();
                    self.transcript_view.cancel_search();
                    self.transcript_view.clear_activity_focus();
                }
                tui.frame_requester().schedule_frame();
                return Ok(true);
            }
        }
        if !self.chat_widget.no_modal_or_popup_active() {
            self.chat_widget.end_composer_drag();
            if !modal_transcript_event {
                return Ok(false);
            }
        }
        // Raw pastes can arrive as thousands of Char/Enter/Tab events. Honor the composer's
        // paste batching instead of rendering the entire screen before each buffered key.
        // Keys owned by the transcript and non-text keys still need fresh geometry immediately.
        if let TuiEvent::Key(key) = event
            && key.kind != KeyEventKind::Release
            && (!self.chat_widget.is_in_paste_burst()
                || self.transcript_view.is_search_editing()
                || self.transcript_view.is_activity_focused()
                || self.transcript_view.owns_interaction_key(*key)
                || self.backtrack.overlay_preview_active
                || self.keymap.app.focus_activity.is_pressed(*key)
                || self.keymap.app.find_transcript.is_pressed(*key)
                || match key.code {
                    KeyCode::Char(_) => {
                        crate::key_hint::has_ctrl_or_alt(key.modifiers)
                            || key.modifiers.intersects(
                                KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META,
                            )
                    }
                    KeyCode::Enter => {
                        !matches!(key.modifiers, KeyModifiers::NONE | KeyModifiers::SHIFT)
                    }
                    KeyCode::Tab => key.modifiers != KeyModifiers::NONE,
                    _ => true,
                })
        {
            self.refresh_owned_transcript(tui)?;
        }
        if matches!(event, TuiEvent::Key(_))
            || matches!(event, TuiEvent::Mouse(mouse) if matches!(mouse.kind, crossterm::event::MouseEventKind::Down(_)))
        {
            self.chat_widget.end_composer_drag();
        }
        if let TuiEvent::Key(key) = event
            && self.chat_widget.no_modal_or_popup_active()
            && !self.transcript_view.is_select_mode()
            && self.keymap.app.select_transcript.is_pressed(*key)
        {
            self.transcript_view
                .begin_select_mode(&self.transcript_cells);
            tui.frame_requester().schedule_frame();
            return Ok(true);
        }
        // Read-only Escape belongs to session navigation, even when the transcript is scrolled.
        // Search and selection still consume Escape first to dismiss their interaction.
        if let TuiEvent::Key(key) = event
            && !self.transcript_view.has_active_interaction()
            && (self.chat_widget.is_external_writer_view() || self.chat_widget.has_provider_error())
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
        let empty_enter_returns_to_latest = matches!(
            event,
            TuiEvent::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                ..
            })
        ) && self.enter_returns_to_latest()
            && self.transcript_view.can_return_to_latest();
        if let TuiEvent::Key(key) = event
            && !self.transcript_view.has_active_interaction()
            && keymap_action_ids().any(|action| {
                action.context != KeymapContext::Pager
                    && !matches!(action.action, "find_transcript" | "focus_activity")
                    && self.active_keymap_contexts().contains_action(action)
                    && !(empty_enter_returns_to_latest
                        && action.context == KeymapContext::Composer
                        && action.action == "submit")
                    && ((key.modifiers.is_empty()
                        && matches!(key.code, KeyCode::PageUp | KeyCode::PageDown))
                        || crate::transcript_view::JumpTarget::from_key(*key).is_some()
                        || configured_binding_for_action(&self.local_settings.tui.keymap, action)
                            .is_some_and(std::option::Option::is_some))
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
        let was_select_mode = self.transcript_view.is_select_mode();
        let select_exit_key = Self::transcript_select_exit_key(event);
        let action = match event {
            TuiEvent::Key(key) if was_select_mode && !self.transcript_view.is_search_editing() => {
                if let Some(action) = self
                    .transcript_view
                    .handle_select_mode_key(*key, &self.transcript_cells)
                {
                    Some(action)
                } else {
                    if self.keymap.app.find_transcript.is_pressed(*key)
                        || self.keymap.pager.find.is_pressed(*key)
                    {
                        self.transcript_view.begin_search();
                    }
                    Some(ViewAction::Changed)
                }
            }
            TuiEvent::Key(key)
                if !self.transcript_view.is_search_editing()
                    && self.keymap.app.find_transcript.is_pressed(*key) =>
            {
                self.transcript_view.begin_search();
                Some(ViewAction::Changed)
            }
            TuiEvent::Key(key) => self
                .transcript_view
                .handle_key(*key, &self.transcript_cells),
            TuiEvent::Mouse(mouse) if modal_transcript_mouse => {
                self.transcript_view.handle_selection_mouse(
                    *mouse,
                    &self.transcript_cells,
                    self.chat_widget.current_patch_approval(),
                )
            }
            TuiEvent::Mouse(mouse) => self
                .transcript_view
                .handle_mouse(*mouse, &self.transcript_cells),
            TuiEvent::Paste(text) => {
                if self.transcript_view.is_select_mode()
                    && !self.transcript_view.is_search_editing()
                {
                    return Ok(true);
                }
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
            if empty_enter_returns_to_latest {
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
        if !matches!(action, ViewAction::ExitSelectMode) {
            if let Some((characters, result)) = self.transcript_view.copy_action(
                tui,
                &self.transcript_cells,
                &action,
                crate::clipboard_copy::CopyFormat::Markdown,
            ) {
                self.transcript_view.show_copy_feedback(&result, characters);
                if resume_following {
                    match result {
                        Ok(crate::clipboard_copy::CopyStatus::Confirmed) => {
                            self.transcript_view.jump_to_latest();
                        }
                        Ok(crate::clipboard_copy::CopyStatus::Pending(id)) => {
                            if let Some(key) = select_exit_key {
                                self.backtrack.pending_select_exit_key = Some((id, key));
                            }
                        }
                        Ok(
                            crate::clipboard_copy::CopyStatus::Unconfirmed
                            | crate::clipboard_copy::CopyStatus::Busy,
                        )
                        | Err(_) => {}
                    }
                }
            } else if let ViewAction::OpenLink(url) = action {
                self.open_url_in_browser(url);
            }
        }
        if was_select_mode && !self.transcript_view.is_select_mode() {
            self.backtrack.pending_select_exit_key = None;
            self.backtrack.select_exit_key = select_exit_key;
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
pub(super) mod tests;

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
