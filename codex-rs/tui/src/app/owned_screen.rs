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
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use super::owned_screen_resize::OwnedScreenLayout;
use super::*;
use crate::AltScreenBehavior;
use crate::key_hint::is_plain_text_key_event;
use crate::tui::MousePrimaryEvent;
use crate::tui::MousePrimaryEventKind;
use crate::tui::MouseScrollEvent;

const PANE_HEADER_HEIGHT: u16 = 1;

pub(super) struct OwnedScreen {
    viewport: ConversationViewport,
    replay_in_progress: bool,
    last_pane_area: Rect,
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
            last_pane_area: Rect::default(),
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
        // Composer history and cursor movement own arrows and Home/End. The transcript handles
        // paging keys and non-conflicting custom pager bindings while the composer is empty.
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat)
            || is_plain_text_key_event(key_event)
            || matches!(
                key_event.code,
                KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End
            )
        {
            return false;
        }
        self.viewport
            .handle_navigation_key(self.last_conversation_area, key_event)
    }

    fn set_keymap(&mut self, keymap: crate::keymap::PagerKeymap) {
        self.viewport.set_keymap(keymap);
    }

    fn handle_mouse_scroll(&mut self, event: MouseScrollEvent) -> bool {
        if self.viewport.selection_is_active() {
            return true;
        }
        if !self
            .last_conversation_area
            .contains(Position::new(event.column, event.row))
        {
            return false;
        }
        self.viewport.handle_mouse_scroll(event.direction);
        true
    }

    fn begin_selection(&mut self, position: Position) -> bool {
        self.viewport
            .begin_selection(self.last_conversation_area, position)
    }

    fn update_selection(&mut self, position: Position) -> bool {
        self.viewport
            .update_selection(self.last_conversation_area, position)
    }

    fn finish_selection(&mut self, position: Position) -> Option<String> {
        self.viewport
            .finish_selection(self.last_conversation_area, position)
    }

    fn cancel_selection(&mut self) {
        self.viewport.cancel_selection();
    }

    fn selection_is_active(&self) -> bool {
        self.viewport.selection_is_active()
    }

    fn clear_last_render_areas(&mut self) {
        self.last_pane_area = Rect::default();
        self.last_conversation_area = Rect::default();
    }
}

fn pane_body_area(area: Rect, show_header: bool) -> Rect {
    if !show_header {
        return area;
    }
    let header_height = PANE_HEADER_HEIGHT.min(area.height);
    Rect::new(
        area.x,
        area.y.saturating_add(header_height),
        area.width,
        area.height.saturating_sub(header_height),
    )
}

fn render_pane_header(slot: PaneSlot, focused: bool, area: Rect, buffer: &mut Buffer) {
    if area.height == 0 {
        return;
    }
    let label = match slot {
        PaneSlot::Parent => "Parent",
        PaneSlot::Side => "Side",
    };
    let line: Line<'static> = if focused {
        let parent: Span<'static> = conversation_panes::parent_pane_shortcut().into();
        let side: Span<'static> = conversation_panes::side_pane_shortcut().into();
        vec![
            " ".into(),
            label.cyan().bold(),
            "  ".into(),
            format!("{} / {} focus", parent.content, side.content).dim(),
        ]
        .into()
    } else {
        vec![" ".into(), label.dim()].into()
    };
    Paragraph::new(line).render(
        Rect::new(
            area.x,
            area.y,
            area.width,
            PANE_HEADER_HEIGHT.min(area.height),
        ),
        buffer,
    );
}

fn render_divider(area: Rect, active: bool, buffer: &mut Buffer) {
    if area.width == 0 {
        return;
    }
    let style = if active {
        Style::default().cyan().bold()
    } else {
        Style::default().dim()
    };
    for y in area.y..area.bottom() {
        buffer[(area.x, y)]
            .set_symbol(if active { "┃" } else { "│" })
            .set_style(style);
    }
}

fn render_pane(
    panes: &mut ConversationPanes,
    slot: PaneSlot,
    area: Rect,
    show_header: bool,
    focused: PaneSlot,
    buffer: &mut Buffer,
) -> Option<RenderedOwnedScreen> {
    render_pane_header(slot, slot == focused, area, buffer);
    let body_area = pane_body_area(area, show_header);
    let pane = panes.by_slot_mut(slot)?;
    pane.chat_widget.update_owned_screen_width(body_area.width);
    let screen = pane.owned_screen.as_mut()?;
    screen.last_pane_area = area;
    Some(screen.render(&pane.chat_widget, body_area, buffer))
}

fn render_layout(
    panes: &mut ConversationPanes,
    layout: OwnedScreenLayout,
    focused: PaneSlot,
    buffer: &mut Buffer,
) -> Option<RenderedOwnedScreen> {
    panes.record_owned_screen_layout(layout);
    let divider_active = panes.owned_screen_split_is_dragging();
    Clear.render(layout.area(), buffer);
    for slot in [PaneSlot::Parent, PaneSlot::Side] {
        let slot_is_visible = match layout {
            OwnedScreenLayout::Single { slot: visible, .. } => slot == visible,
            OwnedScreenLayout::Split { .. } => true,
        };
        if let Some(screen) = panes
            .by_slot_mut(slot)
            .and_then(|pane| pane.owned_screen.as_mut())
        {
            if !slot_is_visible {
                screen.cancel_selection();
            }
            screen.clear_last_render_areas();
        }
    }

    match layout {
        OwnedScreenLayout::Single {
            slot,
            area,
            show_header,
        } => render_pane(panes, slot, area, show_header, focused, buffer),
        OwnedScreenLayout::Split {
            parent,
            divider,
            side,
            ..
        } => {
            let parent_rendered = render_pane(
                panes,
                PaneSlot::Parent,
                parent,
                /*show_header*/ true,
                focused,
                buffer,
            );
            let side_rendered = render_pane(
                panes,
                PaneSlot::Side,
                side,
                /*show_header*/ true,
                focused,
                buffer,
            );
            render_divider(divider, divider_active, buffer);
            match focused {
                PaneSlot::Parent => parent_rendered,
                PaneSlot::Side => side_rendered,
            }
        }
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
        self.chat_widget
            .by_slot(PaneSlot::Parent)
            .is_some_and(|pane| pane.owned_screen.is_some())
    }

    pub(super) fn owned_screen_push_cell(&mut self, cell: Arc<dyn HistoryCell>) {
        if let Some(screen) = &mut self.chat_widget.owned_screen {
            screen.viewport.push_cell(cell);
        }
    }

    pub(super) fn begin_owned_screen_replay(&mut self) {
        if let Some(screen) = &mut self.chat_widget.owned_screen {
            screen.replay_in_progress = true;
        }
    }

    pub(super) fn finish_owned_screen_replay(&mut self) {
        if let Some(screen) = &mut self.chat_widget.owned_screen {
            screen.replay_in_progress = false;
        }
    }

    pub(super) fn owned_screen_replay_in_progress(&self) -> bool {
        self.chat_widget
            .owned_screen
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
            .chat_widget
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
        let selection_is_active = [PaneSlot::Parent, PaneSlot::Side].into_iter().any(|slot| {
            self.chat_widget
                .by_slot(slot)
                .and_then(|pane| pane.owned_screen.as_ref())
                .is_some_and(OwnedScreen::selection_is_active)
        });
        if self.chat_widget.owned_screen_split_is_dragging() || selection_is_active {
            return true;
        }
        for slot in [PaneSlot::Parent, PaneSlot::Side] {
            let handled = self.chat_widget.by_slot_mut(slot).is_some_and(|pane| {
                pane.chat_widget.no_modal_or_popup_active()
                    && pane
                        .owned_screen
                        .as_mut()
                        .is_some_and(|screen| screen.handle_mouse_scroll(event))
            });
            if handled {
                tui.frame_requester()
                    .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
                return true;
            }
        }
        false
    }

    pub(super) fn handle_owned_screen_mouse_primary(
        &mut self,
        tui: &mut tui::Tui,
        event: MousePrimaryEvent,
    ) -> bool {
        if self.overlay.is_some() || !self.chat_widget.no_modal_or_popup_active() {
            let split_canceled = self.chat_widget.cancel_owned_screen_split_drag();
            let selection_canceled = self.cancel_owned_screen_selection();
            if split_canceled || selection_canceled {
                tui.frame_requester().schedule_frame();
            }
            return false;
        }

        if self.chat_widget.handle_owned_screen_split_mouse(event) {
            self.cancel_owned_screen_selection();
            match event.kind {
                MousePrimaryEventKind::Drag => tui
                    .frame_requester()
                    .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL),
                MousePrimaryEventKind::Press | MousePrimaryEventKind::Release => {
                    tui.frame_requester().schedule_frame();
                }
            }
            return true;
        }
        let position = Position::new(event.column, event.row);
        if event.kind != MousePrimaryEventKind::Press {
            let active_slot = [PaneSlot::Parent, PaneSlot::Side].into_iter().find(|slot| {
                self.chat_widget
                    .by_slot(*slot)
                    .and_then(|pane| pane.owned_screen.as_ref())
                    .is_some_and(OwnedScreen::selection_is_active)
            });
            let Some(active_slot) = active_slot else {
                return false;
            };
            let mut copied = None;
            let handled = self
                .chat_widget
                .by_slot_mut(active_slot)
                .and_then(|pane| pane.owned_screen.as_mut())
                .is_some_and(|screen| match event.kind {
                    MousePrimaryEventKind::Drag => screen.update_selection(position),
                    MousePrimaryEventKind::Release => {
                        copied = screen.finish_selection(position);
                        true
                    }
                    MousePrimaryEventKind::Press => false,
                });
            if let Some(text) = copied
                && let Some(pane) = self.chat_widget.by_slot_mut(active_slot)
            {
                pane.chat_widget.copy_selected_text(&text);
                let lease = pane.chat_widget.take_clipboard_lease();
                self.chat_widget.retain_selection_clipboard_lease(lease);
            }
            if handled {
                match event.kind {
                    MousePrimaryEventKind::Drag => tui
                        .frame_requester()
                        .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL),
                    MousePrimaryEventKind::Press | MousePrimaryEventKind::Release => {
                        tui.frame_requester().schedule_frame();
                    }
                }
            }
            return handled;
        }

        let selection_canceled = self.cancel_owned_screen_selection();
        let target = [PaneSlot::Parent, PaneSlot::Side].into_iter().find(|slot| {
            self.chat_widget
                .by_slot(*slot)
                .and_then(|pane| pane.owned_screen.as_ref())
                .is_some_and(|screen| screen.last_pane_area.contains(position))
        });
        let Some(target) = target else {
            if selection_canceled {
                tui.frame_requester().schedule_frame();
            }
            return false;
        };
        let selection_started = self.chat_widget.by_slot_mut(target).is_some_and(|pane| {
            pane.chat_widget.no_modal_or_popup_active()
                && pane
                    .owned_screen
                    .as_mut()
                    .is_some_and(|screen| screen.begin_selection(position))
        });
        let focus_changed = self.chat_widget.focused_slot() != target;
        let backtrack_was_primed = self.backtrack.primed;
        if !self.focus_conversation_pane(target) {
            if selection_canceled {
                tui.frame_requester().schedule_frame();
            }
            return false;
        }
        if selection_started || selection_canceled || focus_changed || backtrack_was_primed {
            tui.frame_requester().schedule_frame();
        }
        true
    }

    pub(super) fn cancel_owned_screen_selection(&mut self) -> bool {
        let mut canceled = false;
        for slot in [PaneSlot::Parent, PaneSlot::Side] {
            if let Some(screen) = self
                .chat_widget
                .by_slot_mut(slot)
                .and_then(|pane| pane.owned_screen.as_mut())
                && screen.selection_is_active()
            {
                screen.cancel_selection();
                canceled = true;
            }
        }
        canceled
    }

    pub(crate) fn sync_owned_screen_cells(&mut self) {
        let cells = self.chat_widget.transcript_cells.clone();
        if let Some(screen) = &mut self.chat_widget.owned_screen {
            screen.viewport.replace_cells(cells);
        }
    }

    pub(super) fn sync_owned_screen_render_mode(&mut self) {
        self.chat_widget.for_each_installed_mut(|pane| {
            let render_mode = pane.chat_widget.history_render_mode();
            if let Some(screen) = &mut pane.owned_screen {
                screen.viewport.set_render_mode(render_mode);
            }
        });
    }

    pub(super) fn sync_owned_screen_keymap(&mut self) {
        let pager_keymap = self.keymap.pager.clone();
        self.chat_widget.for_each_installed_mut(|pane| {
            if let Some(screen) = &mut pane.owned_screen {
                screen.set_keymap(pager_keymap.clone());
            }
        });
    }

    pub(super) fn handle_owned_draw_pre_render(&mut self, tui: &mut tui::Tui) -> Result<bool> {
        if !self.has_owned_screen() {
            return Ok(false);
        }
        let size = tui.terminal.size()?;
        let size_changed = size != tui.terminal.last_known_screen_size;
        for slot in [PaneSlot::Parent, PaneSlot::Side] {
            if let Some(pane) = self.chat_widget.by_slot_mut(slot) {
                if size_changed {
                    pane.chat_widget.refresh_status_line();
                }
                pane.transcript_reflow.clear();
            }
        }
        tui.clear_pending_history_lines();
        Ok(true)
    }

    pub(super) fn render_owned_screen_frame(&mut self, tui: &mut tui::Tui) -> Result<Option<Rect>> {
        if !self.has_owned_screen() {
            return Ok(None);
        }
        if !self.chat_widget.no_modal_or_popup_active() {
            self.chat_widget.cancel_owned_screen_split_drag();
            self.cancel_owned_screen_selection();
        }
        let focused = self.chat_widget.focused_slot();
        let has_side = self.chat_widget.has_side();
        let split_preference = self.chat_widget.owned_screen_split_preference();
        let mut rendered_area = Rect::default();
        tui.draw(/*height*/ u16::MAX, |frame| {
            rendered_area = frame.area();
            let layout = OwnedScreenLayout::new(rendered_area, has_side, focused, split_preference);
            if let Some(rendered) =
                render_layout(&mut self.chat_widget, layout, focused, frame.buffer)
                && let Some((x, y)) = rendered.cursor
            {
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
