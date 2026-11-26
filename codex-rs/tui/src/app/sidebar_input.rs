//! Route sidebar pointer events and dispatch input to the visible content owner.
//! Pointer activation is valid only when the painted target still matches current state.

use super::sidebar::PaneFocus;
use super::sidebar::SidebarContent;
use super::*;
use crate::sidebar_chooser;
use crate::sidebar_chooser::ChooserAction;
use crate::sidebar_tab_strip::SidebarTab;
use crate::sidebar_tab_strip::TabTarget;

#[path = "sidebar_input_chooser.rs"]
mod chooser;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarTarget {
    Tab(TabTarget<SidebarTab>),
    Row(usize),
    Footer(ChooserAction),
    Restore,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SidebarPress {
    parent: Option<ThreadId>,
    state: sidebar::SidebarInputState,
    target: SidebarTarget,
    area: Rect,
    button: MouseButton,
}

enum PointerResult {
    Consumed,
    Unhandled,
    Continue,
    Chooser(ChooserAction),
    Action(SidebarAction),
}

enum SidebarAction {
    StepTab(isize),
    ActivateTab(SidebarTab),
    CloseTab(SidebarTab),
    OpenChooser,
    Hide,
    Restore,
}

impl App {
    pub(super) async fn handle_sidebar_input(
        &mut self,
        tui: &mut tui::Tui,
        server: &mut AppServerSession,
        event: &TuiEvent,
    ) -> bool {
        if !self.conversation_focus_available(tui) || !self.sidebar_chooser_available() {
            self.sidebar_press = None;
            return false;
        }
        let pointer_action = if let TuiEvent::Mouse(mouse) = event {
            match self.handle_sidebar_pointer(tui, *mouse) {
                PointerResult::Consumed => return true,
                PointerResult::Unhandled => return false,
                PointerResult::Continue => None,
                PointerResult::Action(action) => {
                    let result = match action {
                        SidebarAction::StepTab(step) => {
                            self.step_sidebar_tab(tui, server, step).await
                        }
                        SidebarAction::ActivateTab(id) => {
                            self.activate_sidebar_tab(tui, server, id).await
                        }
                        SidebarAction::CloseTab(id) => {
                            self.close_sidebar_tab(tui, server, id).await
                        }
                        SidebarAction::OpenChooser => self.open_sidebar_tabs(tui, server).await,
                        SidebarAction::Hide => self.hide_sidebar(tui, server).await,
                        SidebarAction::Restore => self.restore_sidebar(tui, server).await,
                    };
                    if let Err(err) = result {
                        self.chat_widget.add_error_message(err.to_string());
                    }
                    return true;
                }
                PointerResult::Chooser(action) => {
                    self.sidebar_mut()
                        .show(SidebarContent::Chooser, PaneFocus::Sidebar);
                    Some(action)
                }
            }
        } else {
            None
        };
        match self
            .sidebar()
            .and_then(sidebar::SidebarPane::visible_content)
        {
            Some(SidebarContent::Document(_)) => {
                self.handle_document_input(tui, server, event).await
            }
            Some(SidebarContent::Subagents) => {
                self.handle_docked_agent_input(tui, server, event).await
            }
            _ if self.chooser_focus().is_some() => {
                self.handle_chooser_input(tui, server, event, pointer_action)
                    .await
            }
            _ => false,
        }
    }

    fn handle_sidebar_pointer(
        &mut self,
        tui: &mut tui::Tui,
        mouse: crossterm::event::MouseEvent,
    ) -> PointerResult {
        let point = (mouse.column, mouse.row).into();
        let frame = &self.conversation_frame;
        let over_strip = frame
            .sidebar_tab_strip()
            .is_some_and(|area| area.contains(point));
        let over_restore = frame
            .restore_header
            .is_some_and(|area| area.contains(point));
        let over_chooser = self.chooser_focus().is_some()
            && frame.panel_area().is_some_and(|area| area.contains(point));
        if tui.shortcut_hud_contains(mouse) {
            self.sidebar_press = None;
            return PointerResult::Unhandled;
        }
        if over_strip
            && matches!(
                mouse.kind,
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            )
        {
            let step = if mouse.kind == MouseEventKind::ScrollUp {
                -1
            } else {
                1
            };
            if frame.sidebar_input == self.sidebar().map(sidebar::SidebarPane::input_state)
                && frame
                    .sidebar_targets
                    .iter()
                    .any(|(target, _)| matches!(target, SidebarTarget::Tab(_)))
            {
                return PointerResult::Action(SidebarAction::StepTab(step));
            }
            return PointerResult::Consumed;
        }
        if let MouseEventKind::Down(button @ (MouseButton::Left | MouseButton::Middle))
        | MouseEventKind::Up(button @ (MouseButton::Left | MouseButton::Middle)) = mouse.kind
        {
            let down = matches!(mouse.kind, MouseEventKind::Down(_));
            let pressed = self.sidebar_press.take();
            if over_strip || over_chooser || over_restore || pressed.is_some() {
                let current = self.sidebar().map(sidebar::SidebarPane::input_state);
                let hit = frame
                    .sidebar_targets
                    .iter()
                    .find(|(_, area)| area.contains(point));
                let candidate = if frame.sidebar_input == current && self.pane_resize.is_none() {
                    hit.zip(current)
                        .map(|(&(target, area), state)| SidebarPress {
                            parent: self.sidebar_parent(),
                            state,
                            target,
                            area,
                            button,
                        })
                } else {
                    None
                };
                tui.frame_requester().schedule_frame();
                if down {
                    self.sidebar_press = candidate;
                    if button == MouseButton::Left
                        && mouse.modifiers.is_empty()
                        && self.chooser_focus() == Some(PaneFocus::Main)
                        && self.sidebar().is_some_and(|pane| pane.showing_file_path)
                        && frame.sidebar_input == current
                        && self.pane_resize.is_none()
                        && frame.panel_area().is_some_and(|area| {
                            sidebar_chooser::file_path_areas(area)[0].contains(point)
                        })
                    {
                        self.sidebar_mut()
                            .show(SidebarContent::Chooser, PaneFocus::Sidebar);
                    }
                    return PointerResult::Consumed;
                }
                let Some(press) = candidate.filter(|candidate| Some(*candidate) == pressed) else {
                    return PointerResult::Consumed;
                };
                match press.target {
                    SidebarTarget::Tab(target) => {
                        let action = match (button, target) {
                            (MouseButton::Left, TabTarget::Close(id))
                            | (MouseButton::Middle, TabTarget::Tab(id) | TabTarget::Close(id)) => {
                                SidebarAction::CloseTab(id)
                            }
                            (MouseButton::Left, TabTarget::Add) => SidebarAction::OpenChooser,
                            (
                                MouseButton::Left,
                                TabTarget::Tab(id) | TabTarget::Overflow { tab: id, .. },
                            ) => SidebarAction::ActivateTab(id),
                            (MouseButton::Left, TabTarget::Hide) => SidebarAction::Hide,
                            _ => return PointerResult::Consumed,
                        };
                        return PointerResult::Action(action);
                    }
                    SidebarTarget::Restore if button == MouseButton::Left => {
                        return PointerResult::Action(SidebarAction::Restore);
                    }
                    SidebarTarget::Row(index) if button == MouseButton::Left => {
                        return PointerResult::Chooser(ChooserAction::Open(index));
                    }
                    SidebarTarget::Footer(action) if button == MouseButton::Left => {
                        return PointerResult::Chooser(action);
                    }
                    _ => return PointerResult::Consumed,
                }
            } else if down && button == MouseButton::Left && self.chooser_focus().is_some() {
                self.sidebar_mut()
                    .show(SidebarContent::Chooser, PaneFocus::Main);
            }
        }
        PointerResult::Continue
    }
}
