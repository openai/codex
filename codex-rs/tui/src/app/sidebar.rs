//! Each parent owns one sidebar. Its content survives hiding so the same surface
//! can paint the closing frames and reverse direction if reopened mid-slide.

use super::App;
use super::pane_motion::PaneMotion;
use super::side_pane;
use super::side_pane::SidePaneLayout;
use crate::sidebar_chooser::ChatTabs;
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

pub(super) enum CreationState {
    Waiting {
        request: uuid::Uuid,
        started: std::time::Instant,
    },
    Failed {
        message: String,
        cleanup: Option<ThreadId>,
    },
}

pub(super) struct SideCreation {
    pub(super) owner: crate::backend_id::BackendId,
    pub(super) state: CreationState,
    pub(super) message: Option<crate::chatwidget::UserMessage>,
}

impl SideCreation {
    pub(super) fn request(&self) -> Option<uuid::Uuid> {
        match self.state {
            CreationState::Waiting { request, .. } => Some(request),
            CreationState::Failed { .. } => None,
        }
    }
}

#[derive(Default)]
pub(super) struct SidebarPane {
    pub(super) creation: Option<SideCreation>,
    content: SidebarContent,
    tabs: Vec<ThreadId>,
    // Visit order is independent of the order in which tabs appear.
    visits: Vec<ThreadId>,
    pub(super) choice: usize,
    tabs_revision: usize,
    focus: Option<PaneFocus>,
    motion: PaneMotion,
}

impl SidebarPane {
    pub(super) fn show(&mut self, content: SidebarContent, focus: PaneFocus) {
        match content {
            SidebarContent::Chat(id) => {
                if !self.tabs.contains(&id) {
                    self.tabs.push(id);
                    self.tabs_revision += 1;
                }
                self.visits.retain(|visited| *visited != id);
                self.visits.push(id);
            }
            SidebarContent::Chooser if self.content != content || self.focus.is_none() => {
                self.choice = self
                    .selected_chat()
                    .and_then(|id| self.chat_tabs().choice_index(id))
                    .unwrap_or(0);
            }
            SidebarContent::Chooser => {}
        }
        self.content = content;
        self.focus = Some(focus);
    }

    pub(super) fn chat_tabs(&self) -> ChatTabs<'_> {
        ChatTabs {
            ids: &self.tabs,
            selected: self.selected_chat(),
        }
    }

    pub(super) fn selected_chat(&self) -> Option<ThreadId> {
        self.visits.last().copied()
    }

    pub(super) fn input_state(&self) -> (usize, Option<ThreadId>, usize) {
        (self.tabs_revision, self.selected_chat(), self.choice)
    }

    pub(super) fn chat_ready(&mut self, child: ThreadId) {
        self.creation = None;
        if !self.tabs.contains(&child) {
            self.tabs.push(child);
            self.tabs_revision += 1;
            // A completed background fork must not replace the user's selection.
            self.visits.insert(0, child);
        }
        if self.content == SidebarContent::Chooser
            && let Some(selected) = self.selected_chat()
        {
            self.content = SidebarContent::Chat(selected);
        }
    }

    pub(super) fn hide(&mut self) {
        self.focus = None;
    }

    pub(super) fn remove_chat(&mut self, id: ThreadId) {
        let Some(index) = self.tabs.iter().position(|tab| *tab == id) else {
            return;
        };
        self.tabs.remove(index);
        self.tabs_revision += 1;
        self.visits.retain(|visited| *visited != id);
        if index + 1 < self.choice {
            self.choice -= 1;
        }
        self.choice = self.choice.min(self.tabs.len());
        if self.content == SidebarContent::Chat(id) {
            self.content = match self.selected_chat() {
                Some(child) => SidebarContent::Chat(child),
                None => {
                    self.hide();
                    self.motion = PaneMotion::default();
                    SidebarContent::Chooser
                }
            };
        }
    }

    pub(super) fn visible_content(&self) -> Option<SidebarContent> {
        self.focus.map(|_| self.content)
    }
}

impl App {
    fn side_creation_pending(&self, owner: crate::backend_id::BackendId) -> bool {
        self.sidebars.values().any(|pane| {
            pane.creation
                .as_ref()
                .is_some_and(|creation| creation.owner == owner && creation.request().is_some())
        })
    }

    /// Before the fork returns its ID, ownership routing would drop the child's MCP statuses.
    /// Keep these events with their backend, then route them normally after registration.
    pub(super) fn defer_side_startup_notification(
        &self,
        event: &crate::app_server_session::AppServerSessionEvent,
        displayed: crate::backend_id::BackendId,
    ) -> bool {
        use codex_app_server_client::AppServerEvent;
        use codex_app_server_protocol::ServerNotification;
        let AppServerEvent::ServerNotification(notification) = &event.event else {
            return false;
        };
        let id = match notification.as_ref() {
            ServerNotification::ThreadStarted(started) if started.thread.ephemeral => {
                Some(started.thread.id.as_str())
            }
            ServerNotification::McpServerStatusUpdated(status) => status.thread_id.as_deref(),
            _ => None,
        };
        id.and_then(|id| ThreadId::from_string(id).ok())
            .is_some_and(|id| {
                (event.backend_id != displayed || !self.owns_thread_for_routing(id))
                    && self.side_creation_pending(event.backend_id)
            })
    }

    pub(super) fn sidebar_parent(&self) -> Option<ThreadId> {
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

    pub(super) fn sidebar_has_focus(&self) -> bool {
        self.active_side_parent_thread_id().is_some()
            || self.chooser_focus() == Some(PaneFocus::Sidebar)
    }
}

#[derive(PartialEq)]
pub(super) enum SidebarLayout {
    Chooser {
        main: Rect,
        sidebar: Rect,
        focus: PaneFocus,
    },
    Chat(SidePaneLayout),
}

impl SidebarLayout {
    pub(super) fn divider_area(&self) -> Option<Rect> {
        let (x, pane) = match self {
            Self::Chooser { main, .. } => (main.right(), *main),
            Self::Chat(panes) => (panes.divider, panes.focused.union(panes.tab_strip)),
        };
        (!pane.is_empty()).then(|| Rect::new(x, pane.y, side_pane::GAP, pane.height))
    }
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
    content_layout(app, content, focus, [main, sidebar])
}

pub(super) fn content_layout(
    app: &App,
    content: SidebarContent,
    focus: PaneFocus,
    [main, sidebar]: [Rect; 2],
) -> Option<SidebarLayout> {
    match content {
        SidebarContent::Chooser => (app.sidebar_chooser_available()
            && (focus == PaneFocus::Main || app.chat_widget.completion_can_yield_focus()))
        .then_some(SidebarLayout::Chooser {
            main,
            sidebar,
            focus,
        }),
        SidebarContent::Chat(child) => {
            side_pane::pair_layout(app, child, [main, sidebar]).map(SidebarLayout::Chat)
        }
    }
}
