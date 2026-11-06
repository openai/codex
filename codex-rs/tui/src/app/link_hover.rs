//! Match hover feedback to transcript links and the pane divider. Keep passive link drags with their original view.
//! Recheck after painting so scrolling and live output cannot leave a hand over stale content.

use codex_protocol::ThreadId;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton::Left;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind::Drag;
use crossterm::event::MouseEventKind::Moved;
use crossterm::event::MouseEventKind::ScrollDown;
use crossterm::event::MouseEventKind::ScrollUp;
use crossterm::event::MouseEventKind::Up;

use super::App;
use crate::transcript_view::ViewAction;
use crate::tui::PointerShape;
use crate::tui::Tui;
use crate::tui::TuiEvent;

impl App {
    pub(super) fn transcript_link_at(&self, mouse: MouseEvent) -> Option<String> {
        if !mouse.modifiers.is_empty()
            && !mouse
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER)
        {
            return None;
        }
        let view = if let Some(panes) = self.conversation_frame.side_pane()
            && panes.passive.contains((mouse.column, mouse.row).into())
        {
            let passive = &self.side_companions.get(&panes.passive_thread)?.view;
            if passive.is_select_mode() {
                return None;
            }
            passive
        } else {
            &self.transcript_view
        };
        view.link_at(mouse.column, mouse.row)
    }

    pub(super) fn handle_passive_transcript_event(
        &mut self,
        tui: &mut Tui,
        id: ThreadId,
        event: &TuiEvent,
    ) -> bool {
        let Some(companion) = self.side_companions.get_mut(&id) else {
            return false;
        };
        let view = &mut companion.view;
        view.mouse_scroll_speed = self.transcript_view.mouse_scroll_speed;
        view.copy_on_select = self.transcript_view.copy_on_select;
        view.copy_on_select_clear_selection = self.transcript_view.copy_on_select_clear_selection;
        view.primary_selection = self.transcript_view.primary_selection;
        let action = match event {
            TuiEvent::Mouse(mouse) => view.handle_mouse(*mouse, &companion.cells),
            TuiEvent::Key(key) if crate::text_selection::is_copy_key(*key) => view
                .handle_selection_key(*key, &companion.cells)
                .filter(|action| matches!(action, ViewAction::Copy(_))),
            _ => None,
        };
        let Some(action) = action else {
            return false;
        };
        if let Some((count, result)) = view.copy_action(
            tui,
            &companion.cells,
            &action,
            crate::clipboard_copy::CopyFormat::Markdown,
        ) {
            self.transcript_view.show_copy_feedback(&result, count);
        } else if let ViewAction::OpenLink(url) = action {
            self.open_url_in_browser(url);
        }
        tui.frame_requester().schedule_frame();
        true
    }

    pub(super) fn continue_passive_link_drag(
        &mut self,
        tui: &mut Tui,
        event: &TuiEvent,
    ) -> color_eyre::eyre::Result<bool> {
        let Some((id, area)) = self.passive_link_drag else {
            return Ok(false);
        };
        if self.conversation_focus_available(tui) {
            match event {
                TuiEvent::Draw => {
                    if let Some(companion) = self.side_companions.get_mut(&id)
                        && companion.view.tick_selection(&companion.cells)
                    {
                        tui.frame_requester()
                            .schedule_frame_in(crate::tui::TARGET_FRAME_INTERVAL);
                    }
                    return Ok(false);
                }
                TuiEvent::Mouse(mouse) if mouse.kind == Moved => return Ok(false),
                TuiEvent::Mouse(mouse)
                    if matches!(mouse.kind, Drag(Left) | Up(Left) | ScrollUp | ScrollDown) =>
                {
                    self.refresh_owned_transcript(tui)?;
                    if self
                        .conversation_frame
                        .side_pane()
                        .is_some_and(|panes| panes.passive_thread == id && panes.passive == area)
                    {
                        self.handle_passive_transcript_event(tui, id, event);
                        if mouse.kind == Up(Left) {
                            self.passive_link_drag = None;
                        }
                        return Ok(true);
                    }
                }
                _ => {}
            }
        }
        self.passive_link_drag = None;
        if let Some(companion) = self.side_companions.get_mut(&id) {
            companion.view.end_drag();
        } else if self.current_displayed_thread_id() == Some(id) {
            self.transcript_view.end_drag();
        }
        Ok(false)
    }

    pub(super) fn refresh_link_hover(&self, tui: &mut Tui) -> std::io::Result<()> {
        let shape = if tui
            .link_hover
            .mouse
            .is_some_and(|mouse| self.conversation_frame.hud_contains(mouse))
            || !tui.is_owned_screen()
            || self.overlay.is_some()
        {
            PointerShape::Default
        } else if self.conversation_focus_available(tui)
            && (self.divider_drag.is_some()
                || tui.link_hover.mouse.is_some_and(|mouse| {
                    self.conversation_frame
                        .divider_area()
                        .is_some_and(|area| area.contains((mouse.column, mouse.row).into()))
                        && self.divider_is_settled(tui, tui.terminal.last_known_screen_size.into())
                }))
        {
            PointerShape::Resize
        } else if self.chat_widget.no_modal_or_popup_active()
            && tui
                .link_hover
                .mouse
                .is_some_and(|mouse| self.transcript_link_at(mouse).is_some())
        {
            PointerShape::Link
        } else {
            PointerShape::Default
        };
        if (shape == PointerShape::Resize) != (tui.link_hover.shape == PointerShape::Resize) {
            tui.frame_requester().schedule_frame();
        }
        tui.link_hover.shape = shape;
        tui.set_pointer_shape(shape)
    }
}
