//! Frame-level layout and interaction state for application-owned alternate-screen chrome.
//!
//! The frame wraps the conversation-pane layout. Left and right rails therefore remain global while
//! `/side` continues to divide only the center conversation region.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use ratatui::layout::Position;
use ratatui::layout::Rect;

use crate::app_event::OwnedScreenPanel;
use crate::app_event::OwnedScreenPanelPreference;
use crate::tui::MousePrimaryEvent;
use crate::tui::MousePrimaryEventKind;
use crate::tui::MouseScrollDirection;
use crate::tui::MouseScrollEvent;

#[path = "owned_screen_frame_layout.rs"]
mod layout;
#[path = "owned_screen_frame_render.rs"]
mod render;

const PREFERRED_CENTER_WIDTH: u16 = 80;
const PANEL_DIVIDER_WIDTH: u16 = 1;
const SIDEBAR_DEFAULT_WIDTH: u16 = 28;
const SIDEBAR_MIN_WIDTH: u16 = 24;
const SIDEBAR_MAX_WIDTH: u16 = 48;
const SUMMARY_DEFAULT_WIDTH: u16 = 34;
const SUMMARY_MIN_WIDTH: u16 = 30;
const SUMMARY_MAX_WIDTH: u16 = 48;
const PANEL_SCROLL_ROWS: u16 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnedScreenPanelPresentation {
    Docked,
    Overlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OwnedScreenPanelLayout {
    pub(super) area: Rect,
    pub(super) presentation: OwnedScreenPanelPresentation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OwnedScreenFrameLayout {
    pub(super) area: Rect,
    pub(super) center: Rect,
    pub(super) sidebar: Option<OwnedScreenPanelLayout>,
    pub(super) sidebar_divider: Option<Rect>,
    pub(super) summary: Option<OwnedScreenPanelLayout>,
    pub(super) summary_divider: Option<Rect>,
}

impl OwnedScreenFrameLayout {
    fn panel(self, panel: OwnedScreenPanel) -> Option<OwnedScreenPanelLayout> {
        match panel {
            OwnedScreenPanel::Sidebar => self.sidebar,
            OwnedScreenPanel::Summary => self.summary,
        }
    }

    fn divider(self, panel: OwnedScreenPanel) -> Option<Rect> {
        match panel {
            OwnedScreenPanel::Sidebar => self.sidebar_divider,
            OwnedScreenPanel::Summary => self.summary_divider,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OwnedScreenFrameFocus {
    Conversation,
    Sidebar,
    Summary,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum OwnedScreenRightRailContent {
    #[default]
    Summary,
    Browser,
}

impl OwnedScreenFrameFocus {
    fn panel(self) -> Option<OwnedScreenPanel> {
        match self {
            Self::Conversation => None,
            Self::Sidebar => Some(OwnedScreenPanel::Sidebar),
            Self::Summary => Some(OwnedScreenPanel::Summary),
        }
    }
}

impl From<OwnedScreenPanel> for OwnedScreenFrameFocus {
    fn from(panel: OwnedScreenPanel) -> Self {
        match panel {
            OwnedScreenPanel::Sidebar => Self::Sidebar,
            OwnedScreenPanel::Summary => Self::Summary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PanelState {
    preference: OwnedScreenPanelPreference,
    width: u16,
    scroll: u16,
    content_height: u16,
    viewport_height: u16,
}

impl PanelState {
    fn new(width: u16) -> Self {
        Self {
            preference: OwnedScreenPanelPreference::Auto,
            width,
            scroll: 0,
            content_height: 0,
            viewport_height: 0,
        }
    }

    fn max_scroll(self) -> u16 {
        self.content_height.saturating_sub(self.viewport_height)
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.min(self.max_scroll());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameInteraction {
    Idle,
    Resize {
        panel: OwnedScreenPanel,
        origin_column: u16,
        has_resized: bool,
    },
    PanelPointer(OwnedScreenPanel),
}

#[derive(Debug)]
pub(super) struct OwnedScreenFrameState {
    sidebar: PanelState,
    summary: PanelState,
    focus: OwnedScreenFrameFocus,
    right_rail_content: OwnedScreenRightRailContent,
    last_activated_panel: Option<OwnedScreenPanel>,
    interaction: FrameInteraction,
    layout: Option<OwnedScreenFrameLayout>,
}

impl Default for OwnedScreenFrameState {
    fn default() -> Self {
        Self {
            sidebar: PanelState::new(SIDEBAR_DEFAULT_WIDTH),
            summary: PanelState::new(SUMMARY_DEFAULT_WIDTH),
            focus: OwnedScreenFrameFocus::Conversation,
            right_rail_content: OwnedScreenRightRailContent::Summary,
            last_activated_panel: None,
            interaction: FrameInteraction::Idle,
            layout: None,
        }
    }
}

impl OwnedScreenFrameState {
    pub(super) fn focus(&self) -> OwnedScreenFrameFocus {
        self.focus
    }

    pub(super) fn focus_conversation(&mut self) {
        self.focus = OwnedScreenFrameFocus::Conversation;
    }

    pub(super) fn right_rail_content(&self) -> OwnedScreenRightRailContent {
        self.right_rail_content
    }

    pub(super) fn set_right_rail_content(&mut self, content: OwnedScreenRightRailContent) {
        self.right_rail_content = content;
    }

    pub(super) fn select_right_rail_content(&mut self, content: OwnedScreenRightRailContent) {
        self.set_right_rail_content(content);
        self.set_preference(OwnedScreenPanel::Summary, OwnedScreenPanelPreference::Shown);
        self.focus = OwnedScreenFrameFocus::Summary;
    }

    pub(super) fn preference(&self, panel: OwnedScreenPanel) -> OwnedScreenPanelPreference {
        self.panel_state(panel).preference
    }

    pub(super) fn set_preference(
        &mut self,
        panel: OwnedScreenPanel,
        preference: OwnedScreenPanelPreference,
    ) {
        self.panel_state_mut(panel).preference = preference;
        self.interaction = FrameInteraction::Idle;
        match preference {
            OwnedScreenPanelPreference::Shown => self.last_activated_panel = Some(panel),
            OwnedScreenPanelPreference::Auto | OwnedScreenPanelPreference::Hidden => {
                if self.last_activated_panel == Some(panel) {
                    self.last_activated_panel = None;
                }
                if self.focus.panel() == Some(panel) {
                    self.focus = OwnedScreenFrameFocus::Conversation;
                }
            }
        }
        self.layout = None;
    }

    pub(super) fn toggle(&mut self, panel: OwnedScreenPanel) {
        let preference = match self.preference(panel) {
            OwnedScreenPanelPreference::Shown => OwnedScreenPanelPreference::Hidden,
            OwnedScreenPanelPreference::Hidden => OwnedScreenPanelPreference::Shown,
            OwnedScreenPanelPreference::Auto => {
                if self.layout.and_then(|layout| layout.panel(panel)).is_some() {
                    OwnedScreenPanelPreference::Hidden
                } else {
                    OwnedScreenPanelPreference::Shown
                }
            }
        };
        self.set_preference(panel, preference);
    }

    pub(super) fn clear_layout(&mut self) {
        self.layout = None;
        self.interaction = FrameInteraction::Idle;
    }

    pub(super) fn is_resizing(&self) -> bool {
        matches!(self.interaction, FrameInteraction::Resize { .. })
    }

    pub(super) fn is_interacting(&self) -> bool {
        self.interaction != FrameInteraction::Idle
    }

    pub(super) fn traps_background_input(&self) -> bool {
        self.layout.and_then(active_overlay).is_some()
    }

    pub(super) fn cancel_interaction(&mut self) -> bool {
        let handled = self.interaction != FrameInteraction::Idle;
        self.interaction = FrameInteraction::Idle;
        handled
    }

    pub(super) fn handle_mouse_primary(&mut self, event: MousePrimaryEvent) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        let position = Position::new(event.column, event.row);
        match event.kind {
            MousePrimaryEventKind::Press => {
                self.interaction = FrameInteraction::Idle;
                if let Some(panel) = active_overlay(layout) {
                    let Some(overlay) = layout.panel(panel) else {
                        return false;
                    };
                    if overlay.area.contains(position) {
                        self.focus = panel.into();
                        self.interaction = FrameInteraction::PanelPointer(panel);
                    } else {
                        self.set_preference(panel, OwnedScreenPanelPreference::Hidden);
                    }
                    return true;
                }
                for panel in [OwnedScreenPanel::Sidebar, OwnedScreenPanel::Summary] {
                    if layout
                        .divider(panel)
                        .is_some_and(|area| area.contains(position))
                    {
                        self.interaction = FrameInteraction::Resize {
                            panel,
                            origin_column: event.column,
                            has_resized: false,
                        };
                        return true;
                    }
                    if layout
                        .panel(panel)
                        .is_some_and(|panel_layout| panel_layout.area.contains(position))
                    {
                        self.focus = panel.into();
                        self.interaction = FrameInteraction::PanelPointer(panel);
                        return true;
                    }
                }
                self.focus = OwnedScreenFrameFocus::Conversation;
                false
            }
            MousePrimaryEventKind::Drag => match self.interaction {
                FrameInteraction::Resize {
                    panel,
                    origin_column,
                    has_resized,
                } => {
                    let has_resized = has_resized || event.column != origin_column;
                    if has_resized {
                        self.mark_panel_resized(panel);
                        self.resize_panel(panel, event.column, layout.area);
                    }
                    self.interaction = FrameInteraction::Resize {
                        panel,
                        origin_column,
                        has_resized,
                    };
                    true
                }
                FrameInteraction::PanelPointer(_) => true,
                FrameInteraction::Idle => false,
            },
            MousePrimaryEventKind::Release => {
                let handled = self.interaction != FrameInteraction::Idle;
                if let FrameInteraction::Resize {
                    panel,
                    origin_column,
                    has_resized,
                } = self.interaction
                    && (has_resized || event.column != origin_column)
                {
                    self.mark_panel_resized(panel);
                    self.resize_panel(panel, event.column, layout.area);
                }
                self.interaction = FrameInteraction::Idle;
                handled
            }
        }
    }

    pub(super) fn handle_mouse_scroll(&mut self, event: MouseScrollEvent) -> bool {
        let Some(layout) = self.layout else {
            return false;
        };
        let position = Position::new(event.column, event.row);
        if let Some(panel) = active_overlay(layout) {
            if layout
                .panel(panel)
                .is_some_and(|panel_layout| panel_layout.area.contains(position))
            {
                self.scroll(panel, event.direction, PANEL_SCROLL_ROWS);
            }
            return true;
        }
        let Some(panel) = [OwnedScreenPanel::Sidebar, OwnedScreenPanel::Summary]
            .into_iter()
            .find(|panel| {
                layout
                    .panel(*panel)
                    .is_some_and(|panel_layout| panel_layout.area.contains(position))
            })
        else {
            return false;
        };
        self.scroll(panel, event.direction, PANEL_SCROLL_ROWS);
        true
    }

    pub(super) fn handle_navigation_key(&mut self, key_event: KeyEvent) -> bool {
        if !matches!(key_event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return false;
        }
        if key_event.code == KeyCode::Esc
            && let Some(panel) = self.layout.and_then(active_overlay)
        {
            self.set_preference(panel, OwnedScreenPanelPreference::Hidden);
            self.focus = OwnedScreenFrameFocus::Conversation;
            return true;
        }
        if matches!(key_event.code, KeyCode::Tab | KeyCode::BackTab) {
            let Some(layout) = self.layout else {
                return false;
            };
            if let Some(panel) = active_overlay(layout) {
                self.focus = panel.into();
                return true;
            }
            if key_event.code == KeyCode::BackTab
                && self.focus == OwnedScreenFrameFocus::Conversation
            {
                return false;
            }
            let mut focus_order = vec![OwnedScreenFrameFocus::Conversation];
            if layout.sidebar.is_some() {
                focus_order.push(OwnedScreenFrameFocus::Sidebar);
            }
            if layout.summary.is_some() {
                focus_order.push(OwnedScreenFrameFocus::Summary);
            }
            if focus_order.len() == 1 {
                return false;
            }
            let current = focus_order
                .iter()
                .position(|focus| *focus == self.focus)
                .unwrap_or_default();
            let next = if key_event.code == KeyCode::BackTab {
                current.checked_sub(1).unwrap_or(focus_order.len() - 1)
            } else {
                (current + 1) % focus_order.len()
            };
            self.focus = focus_order[next];
            return true;
        }
        let Some(panel) = self.focus.panel() else {
            return false;
        };
        let viewport_height = self.panel_state(panel).viewport_height.max(/*other*/ 1);
        match key_event.code {
            KeyCode::Up => self.scroll(panel, MouseScrollDirection::Up, /*rows*/ 1),
            KeyCode::Down => self.scroll(panel, MouseScrollDirection::Down, /*rows*/ 1),
            KeyCode::PageUp => self.scroll(panel, MouseScrollDirection::Up, viewport_height),
            KeyCode::PageDown => self.scroll(panel, MouseScrollDirection::Down, viewport_height),
            KeyCode::Home => self.panel_state_mut(panel).scroll = 0,
            KeyCode::End => {
                let max_scroll = self.panel_state(panel).max_scroll();
                self.panel_state_mut(panel).scroll = max_scroll;
            }
            KeyCode::Esc => {
                self.focus = OwnedScreenFrameFocus::Conversation;
            }
            KeyCode::Enter => {}
            _ => {
                if self.layout.and_then(active_overlay) == Some(panel) {
                    return true;
                }
                self.focus = OwnedScreenFrameFocus::Conversation;
                return false;
            }
        }
        true
    }

    fn panel_state(&self, panel: OwnedScreenPanel) -> &PanelState {
        match panel {
            OwnedScreenPanel::Sidebar => &self.sidebar,
            OwnedScreenPanel::Summary => &self.summary,
        }
    }

    fn panel_state_mut(&mut self, panel: OwnedScreenPanel) -> &mut PanelState {
        match panel {
            OwnedScreenPanel::Sidebar => &mut self.sidebar,
            OwnedScreenPanel::Summary => &mut self.summary,
        }
    }

    fn scroll(&mut self, panel: OwnedScreenPanel, direction: MouseScrollDirection, rows: u16) {
        let state = self.panel_state_mut(panel);
        state.scroll = match direction {
            MouseScrollDirection::Up => state.scroll.saturating_sub(rows),
            MouseScrollDirection::Down => state.scroll.saturating_add(rows).min(state.max_scroll()),
        };
    }

    fn resize_panel(&mut self, panel: OwnedScreenPanel, column: u16, area: Rect) {
        let width = match panel {
            OwnedScreenPanel::Sidebar => column
                .saturating_sub(area.x)
                .clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH),
            OwnedScreenPanel::Summary => area
                .right()
                .saturating_sub(column.saturating_add(PANEL_DIVIDER_WIDTH))
                .clamp(SUMMARY_MIN_WIDTH, SUMMARY_MAX_WIDTH),
        };
        self.panel_state_mut(panel).width = width;
    }

    fn mark_panel_resized(&mut self, panel: OwnedScreenPanel) {
        self.panel_state_mut(panel).preference = OwnedScreenPanelPreference::Shown;
        self.last_activated_panel = Some(panel);
    }
}

fn active_overlay(layout: OwnedScreenFrameLayout) -> Option<OwnedScreenPanel> {
    [OwnedScreenPanel::Sidebar, OwnedScreenPanel::Summary]
        .into_iter()
        .find(|panel| {
            layout
                .panel(*panel)
                .is_some_and(|layout| layout.presentation == OwnedScreenPanelPresentation::Overlay)
        })
}

#[cfg(test)]
#[path = "owned_screen_frame_tests.rs"]
mod tests;
