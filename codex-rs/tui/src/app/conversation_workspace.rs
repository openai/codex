//! The docked Command Center requires the owned transcript renderer.
//! Its view stays in the workspace when focus moves to the conversation.

use super::App;
use super::agents_overview_view::AgentsOverviewView;
use super::sidebar::PaneFocus;
use crate::app_event::AppEvent;
use crate::app_event::ExitMode;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::key_hint::KeyBindingListExt;
use crate::render::renderable::Renderable;
use crate::tui::Tui;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;

use ratatui::style::Style;

const CENTER_WIDTH: u16 = 36;
// One column for the divider and one for padding before the conversation.
const GUTTER_WIDTH: u16 = 2;
const MIN_CHAT_WIDTH: u16 = 48;
const MIN_SPLIT_WIDTH: u16 = CENTER_WIDTH + GUTTER_WIDTH + MIN_CHAT_WIDTH;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pane {
    Center,
    Conversation,
}

#[derive(Default)]
pub(super) struct Workspace {
    pub(super) view: Option<AgentsOverviewView>,
    /// None while the dock is closed.
    focus: Option<Pane>,
}

impl Workspace {
    pub(super) fn is_open(&self) -> bool {
        self.focus.is_some()
    }

    pub(super) fn focus_center(&mut self) {
        self.focus = Some(Pane::Center);
    }

    pub(super) fn replace_view(&mut self, mut view: AgentsOverviewView) {
        if self.focus == Some(Pane::Conversation)
            && let Some(previous) = &self.view
        {
            view.follow_changed_conversation(previous);
        }
        self.view = Some(view);
    }

    pub(super) fn selected_index(&self) -> Option<usize> {
        self.view.as_ref()?.selected_index()
    }
}

impl App {
    pub(super) fn workspace_center_owns_input(&self) -> bool {
        self.agents_overview.workspace.focus == Some(Pane::Center)
            && self.overlay.is_none()
            && !self.chat_widget.has_active_view()
    }

    pub(super) fn workspace_chat_area(&self, viewport: Rect) -> Rect {
        if self.agents_overview.workspace.is_open() {
            self.workspace_areas(viewport).1.unwrap_or_default()
        } else {
            viewport
        }
    }

    pub(super) fn workspace_areas(&self, viewport: Rect) -> (Option<Rect>, Option<Rect>) {
        if viewport.width >= MIN_SPLIT_WIDTH {
            let [center, _, conversation] = Layout::horizontal([
                Constraint::Length(CENTER_WIDTH),
                Constraint::Length(GUTTER_WIDTH),
                Constraint::Min(MIN_CHAT_WIDTH),
            ])
            .areas(viewport);
            (Some(center), Some(conversation))
        } else if self.workspace_center_owns_input() {
            (Some(viewport), None)
        } else {
            (None, Some(viewport))
        }
    }

    pub(super) fn focus_workspace_composer(&mut self, tui: &mut Tui) {
        if self.agents_overview.workspace.is_open() {
            self.agents_overview.workspace.focus = Some(Pane::Conversation);
        }
        if self.panel_focus() == Some(PaneFocus::Sidebar) {
            let pane = self.sidebar_mut();
            if let Some(content) = pane.visible_content() {
                pane.show(content, PaneFocus::Main);
            }
        }
        self.cancel_transcript_browsing(tui);
        self.transcript_view.end_select_mode();
        self.transcript_view.end_selection(&self.transcript_cells);
        self.transcript_view.cancel_search();
        self.transcript_view.clear_activity_focus();
        self.cancel_pending_key_chord();
    }

    pub(super) fn handle_workspace_key(&mut self, tui: &mut Tui, mut key: KeyEvent) {
        if self.should_recover_vim_insert_escape(tui, key)
            && let Some(view) = &mut self.agents_overview.workspace.view
        {
            view.handle_key_event(KeyCode::Esc.into());
            key.modifiers.remove(KeyModifiers::ALT);
        }
        let Some(key) = self.route_key_chord_event(tui, key) else {
            return;
        };
        if self.handle_voice_toggle_key(tui, key) {
            return;
        }
        let Some(view) = &mut self.agents_overview.workspace.view else {
            return;
        };
        if !view.owns_nested_input() && self.keymap.app.select_transcript.is_pressed(key) {
            self.focus_workspace_composer(tui);
            self.transcript_view
                .begin_select_mode(&self.transcript_cells);
        } else {
            if crate::key_hint::ctrl(KeyCode::Char('c')).is_press(key) {
                if view.on_ctrl_c() == CancellationEvent::NotHandled {
                    self.app_event_tx
                        .send(AppEvent::Exit(ExitMode::ShutdownFirst));
                }
            } else {
                view.handle_key_event(key);
            }
            if view.is_complete() {
                self.agents_overview.workspace.focus = None;
                self.focus_workspace_composer(tui);
                self.conversation_frame = Default::default();
            }
        }
        tui.frame_requester().schedule_frame();
    }

    pub(super) fn handle_workspace_mouse(&mut self, tui: &mut Tui, mouse: MouseEvent) -> bool {
        if !self.agents_overview.workspace.is_open()
            || self.overlay.is_some()
            || self.chat_widget.has_active_view()
        {
            return false;
        }
        let viewport: Rect = tui.terminal.last_known_screen_size.into();
        let (center, conversation) = self.workspace_areas(viewport);
        let center_was_focused = self.agents_overview.workspace.focus == Some(Pane::Center);
        // The press chose the owner. Let its selection finish even outside its pane.
        if matches!(mouse.kind, MouseEventKind::Drag(_) | MouseEventKind::Up(_)) {
            return center_was_focused;
        }

        let point = (mouse.column, mouse.row).into();
        let pressed = matches!(mouse.kind, MouseEventKind::Down(_));
        if center.is_some_and(|rect| rect.contains(point)) {
            if pressed {
                self.agents_overview.workspace.focus_center();
                self.cancel_pending_key_chord();
                tui.frame_requester().schedule_frame();
            }
            return true;
        }
        if conversation.is_some_and(|rect| rect.contains(point)) {
            if center_was_focused && pressed {
                if !self
                    .agents_overview
                    .workspace
                    .view
                    .as_ref()
                    .is_some_and(|view| view.can_return_to_chat)
                {
                    return true;
                }
                self.focus_workspace_composer(tui);
            }
            return false;
        }
        true
    }

    pub(super) fn paint_command_center(
        &mut self,
        frame: &mut crate::custom_terminal::Frame<'_>,
        center: Rect,
        conversation: Option<Rect>,
    ) {
        let center_focused = self.workspace_center_owns_input();
        frame
            .image_placements
            .retain(|image| !center.intersects(image.area));
        if center_focused {
            frame.cursor_position = None;
        }
        let area = center;
        if let Some(view) = &mut self.agents_overview.workspace.view {
            view.dock_focused = conversation.map(|_| center_focused);
            view.render(area, frame.buffer);
            if center_focused {
                frame.cursor_position = view.cursor_pos(area).map(Into::into);
                frame.set_cursor_style(view.cursor_style(area));
            }
        }
        if conversation.is_some() {
            for y in center.y..center.bottom() {
                frame.buffer[(center.right(), y)]
                    .set_symbol("│")
                    .set_style(Style::default().dim());
            }
        }
    }
}
