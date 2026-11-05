//! Each parent owns one sidebar. Its content survives hiding so the same surface
//! can paint the closing frames and reverse direction if reopened mid-slide.

use super::App;
use super::pane_motion::PaneMotion;
use super::side_pane;
use super::side_pane::SidePaneLayout;
use crate::tui::Tui;
use codex_protocol::ThreadId;
use ratatui::layout::Rect;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum SidebarContent {
    #[default]
    Chooser,
    Chat(ThreadId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PaneFocus {
    Main,
    Sidebar,
}

#[derive(Default)]
pub(super) struct SidebarPane {
    content: SidebarContent,
    focus: Option<PaneFocus>,
    motion: PaneMotion,
}

impl SidebarPane {
    pub(super) fn show(&mut self, content: SidebarContent, focus: PaneFocus) {
        self.content = content;
        self.focus = Some(focus);
    }

    pub(super) fn hide(&mut self) {
        self.focus = None;
    }

    pub(super) fn contains_chat(&self, id: ThreadId) -> bool {
        self.content == SidebarContent::Chat(id)
    }

    pub(super) fn visible_content(&self) -> Option<SidebarContent> {
        self.focus.map(|_| self.content)
    }
}

impl App {
    fn sidebar_parent(&self) -> Option<ThreadId> {
        self.active_side_parent_thread_id()
            .or_else(|| self.current_displayed_thread_id())
    }

    pub(super) fn sidebar(&self) -> Option<&SidebarPane> {
        self.sidebars.get(&self.sidebar_parent())
    }

    pub(super) fn sidebar_mut(&mut self) -> &mut SidebarPane {
        self.sidebars.entry(self.sidebar_parent()).or_default()
    }

    pub(super) fn chooser_focus(&self) -> Option<PaneFocus> {
        let pane = self.sidebar()?;
        pane.focus
            .filter(|_| pane.content == SidebarContent::Chooser)
    }
}

pub(super) enum SidebarLayout {
    Chooser {
        main: Rect,
        sidebar: Rect,
        focus: PaneFocus,
    },
    Chat(SidePaneLayout),
}

/// Step motion once per frame. Input and saved width calculations use the settled split.
pub(super) fn paint_layout(app: &mut App, tui: &Tui, area: Rect) -> Option<SidebarLayout> {
    let zoomed =
        app.zoomed_thread.is_some() && app.zoomed_thread == app.current_displayed_thread_id();
    let allowed =
        !zoomed && app.side_retention_enabled(tui) && !app.conversation_frame.has_caller_inset();
    let animate = allowed
        && area.width >= 100
        && app.local_settings.tui.animations
        && app.pane_resize.is_none()
        && app.divider_drag.is_none();
    let parent = app.sidebar_parent();
    let pane = app.sidebars.get_mut(&parent)?;
    let open = pane.focus.is_some();
    let fraction = pane
        .motion
        .advance(open && allowed, animate.then(|| tui.frame_requester()));
    if !allowed || (!open && fraction == 0.0) {
        return None;
    }
    let content = pane.content;
    let focus = pane.focus.unwrap_or(PaneFocus::Main);
    let [main, sidebar] = if area.width < 100 {
        if focus == PaneFocus::Main {
            return None;
        }
        [Rect::default(), area]
    } else {
        side_pane::sidebar_areas(app, tui, area, fraction)?
    };
    match content {
        SidebarContent::Chooser if app.sidebar_chooser_available() => {
            Some(SidebarLayout::Chooser {
                main,
                sidebar,
                focus,
            })
        }
        SidebarContent::Chooser => None,
        SidebarContent::Chat(child) => {
            side_pane::pair_layout(app, child, [main, sidebar]).map(SidebarLayout::Chat)
        }
    }
}
