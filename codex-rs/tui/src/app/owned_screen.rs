//! Application-owned alternate-screen rendering.
//!
//! The owned mode keeps committed conversation cells in a retained viewport and reserves the
//! bottom of every frame for the composer. Inline mode continues to use terminal scrollback.

use crossterm::cursor::SetCursorStyle;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::widgets::Clear;
use ratatui::widgets::Widget;

use super::*;
use crate::AltScreenBehavior;
use crate::tui::MouseScrollEvent;

pub(super) struct OwnedScreen {
    viewport: ConversationViewport,
    replay_in_progress: bool,
    last_conversation_area: Rect,
}

struct RenderedOwnedScreen {
    cursor: Option<(u16, u16)>,
    cursor_style: SetCursorStyle,
}

impl OwnedScreen {
    fn new(chat_widget: &ChatWidget, keymap: crate::keymap::PagerKeymap) -> Self {
        Self {
            viewport: ConversationViewport::new(
                Vec::new(),
                chat_widget.history_render_mode(),
                keymap,
            ),
            replay_in_progress: false,
            last_conversation_area: Rect::default(),
        }
    }

    fn render(
        &mut self,
        chat_widget: &ChatWidget,
        area: Rect,
        buffer: &mut Buffer,
    ) -> RenderedOwnedScreen {
        Clear.render(area, buffer);

        let bottom_pane = chat_widget.bottom_pane_renderable();
        let bottom_height = bottom_pane.desired_height(area.width).min(area.height);
        let conversation_height = area.height.saturating_sub(bottom_height);
        let conversation_area = Rect::new(
            area.x,
            area.y,
            chat_widget.history_wrap_width(area.width),
            conversation_height,
        );
        let bottom_area = Rect::new(
            area.x,
            area.y.saturating_add(conversation_height),
            area.width,
            bottom_height,
        );
        self.last_conversation_area = conversation_area;

        self.viewport
            .set_render_mode(chat_widget.history_render_mode());
        let active_key = chat_widget.active_cell_render_key();
        self.viewport
            .sync_live_tail(conversation_area.width, active_key, |width| {
                chat_widget.active_cell_display_hyperlink_lines(width)
            });
        self.viewport.render(conversation_area, buffer);
        bottom_pane.render(bottom_area, buffer);

        RenderedOwnedScreen {
            cursor: bottom_pane.cursor_pos(bottom_area),
            cursor_style: bottom_pane.cursor_style(bottom_area),
        }
    }

    fn handle_navigation_key(&mut self, key_event: KeyEvent) -> bool {
        // Alternate-scroll wheel events are indistinguishable from physical arrow keys. Keep
        // arrows, Home/End, and printable pager bindings available to the composer until the TUI
        // has direct mouse events or an explicit viewport-focus mode.
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            || !matches!(key_event.code, KeyCode::PageUp | KeyCode::PageDown)
        {
            return false;
        }
        self.viewport
            .handle_navigation_key(self.last_conversation_area, key_event)
    }

    fn handle_mouse_scroll(&mut self, event: MouseScrollEvent) -> bool {
        if !self
            .last_conversation_area
            .contains(Position::new(event.column, event.row))
        {
            return false;
        }
        self.viewport.handle_mouse_scroll(event.direction);
        true
    }
}

impl App {
    pub(super) fn owned_screen_for_behavior(
        alt_screen_behavior: AltScreenBehavior,
        chat_widget: &ChatWidget,
        keymap: crate::keymap::PagerKeymap,
    ) -> Option<OwnedScreen> {
        match alt_screen_behavior {
            AltScreenBehavior::Disabled | AltScreenBehavior::OverlayOnly => None,
            AltScreenBehavior::Owned => Some(OwnedScreen::new(chat_widget, keymap)),
        }
    }

    pub(super) fn has_owned_screen(&self) -> bool {
        self.owned_screen.is_some()
    }

    pub(super) fn owned_screen_push_cell(&mut self, cell: Arc<dyn HistoryCell>) {
        if let Some(screen) = &mut self.owned_screen {
            screen.viewport.push_cell(cell);
        }
    }

    pub(super) fn begin_owned_screen_replay(&mut self) {
        if let Some(screen) = &mut self.owned_screen {
            screen.replay_in_progress = true;
        }
    }

    pub(super) fn finish_owned_screen_replay(&mut self) {
        if let Some(screen) = &mut self.owned_screen {
            screen.replay_in_progress = false;
        }
    }

    pub(super) fn owned_screen_replay_in_progress(&self) -> bool {
        self.owned_screen
            .as_ref()
            .is_some_and(|screen| screen.replay_in_progress)
    }

    pub(super) fn handle_owned_screen_navigation_key(
        &mut self,
        tui: &mut tui::Tui,
        key_event: KeyEvent,
    ) -> bool {
        if !self.chat_widget.composer_is_empty() || !self.chat_widget.no_modal_or_popup_active() {
            return false;
        }
        let handled = self
            .owned_screen
            .as_mut()
            .is_some_and(|screen| screen.handle_navigation_key(key_event));
        if handled {
            tui.frame_requester()
                .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
        }
        handled
    }

    pub(super) fn handle_owned_screen_mouse_scroll(
        &mut self,
        tui: &mut tui::Tui,
        event: MouseScrollEvent,
    ) -> bool {
        if !self.chat_widget.no_modal_or_popup_active() {
            return false;
        }
        let handled = self
            .owned_screen
            .as_mut()
            .is_some_and(|screen| screen.handle_mouse_scroll(event));
        if handled {
            tui.frame_requester()
                .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
        }
        handled
    }

    pub(crate) fn sync_owned_screen_cells(&mut self) {
        if let Some(screen) = &mut self.owned_screen {
            screen.viewport.replace_cells(self.transcript_cells.clone());
        }
    }

    pub(super) fn sync_owned_screen_render_mode(&mut self) {
        if let Some(screen) = &mut self.owned_screen {
            screen
                .viewport
                .set_render_mode(self.chat_widget.history_render_mode());
        }
    }

    pub(super) fn handle_owned_draw_pre_render(&mut self, tui: &mut tui::Tui) -> Result<bool> {
        if self.owned_screen.is_none() {
            return Ok(false);
        }
        let size = tui.terminal.size()?;
        if size.width != tui.terminal.last_known_screen_size.width {
            self.chat_widget.on_terminal_resize(size.width);
        }
        if size != tui.terminal.last_known_screen_size {
            self.refresh_status_line();
        }
        self.transcript_reflow.clear();
        tui.clear_pending_history_lines();
        Ok(true)
    }

    pub(super) fn render_owned_screen_frame(&mut self, tui: &mut tui::Tui) -> Result<Option<Rect>> {
        let Some(screen) = &mut self.owned_screen else {
            return Ok(None);
        };
        self.chat_widget
            .update_owned_screen_width(tui.terminal.size()?.width);
        let chat_widget = &self.chat_widget;
        let mut rendered_area = Rect::default();
        tui.draw(/*height*/ u16::MAX, |frame| {
            rendered_area = frame.area();
            let rendered = screen.render(chat_widget, rendered_area, frame.buffer);
            if let Some((x, y)) = rendered.cursor {
                frame.set_cursor_style(rendered.cursor_style);
                frame.set_cursor_position((x, y));
            }
        })?;
        Ok(Some(rendered_area))
    }
}

#[cfg(test)]
#[path = "owned_screen_tests.rs"]
mod tests;
